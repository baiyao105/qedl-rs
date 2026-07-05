use crate::context::JobContext;
use crate::error::Result;
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use qedl_core::util::humanize_size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EraseMethod {
    /// Write zero bytes to erase (default, safer - some Firehose erase commands have bugs).
    #[default]
    WriteZero,
    /// Use native Firehose erase command (faster but may have bugs on some devices).
    Native,
}

#[derive(Debug)]
pub struct JobResult {
    pub success: bool,
    pub message: String,
    pub steps_completed: usize,
}

#[async_trait]
pub trait Job: Send + Sync {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult>;

    fn name(&self) -> &str;
}

pub struct DumpJob {
    pub partition_name: String,
    pub output_path: PathBuf,
    pub show_progress: bool,
    pub resume: bool,
}

#[async_trait]
impl Job for DumpJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let entry =
            ctx.find_partition(&self.partition_name)
                .ok_or_else(|| qedl_storage::StorageError::PartitionNotFound {
                    name: self.partition_name.clone(),
                })?;

        let physical_partition = entry.physical_partition;
        let first_lba = entry.first_lba;
        let last_lba = entry.last_lba;

        let sector_size = ctx.sector_size() as u64;
        let total_sectors = last_lba - first_lba + 1;
        let total_bytes = total_sectors * sector_size;

        let mut start_sector = first_lba;
        let mut dumped_bytes: u64 = 0;

        if self.resume && self.output_path.exists() {
            let metadata = std::fs::metadata(&self.output_path)?;
            let file_size = metadata.len();
            if file_size > 0 && file_size < total_bytes {
                let dumped_sectors = file_size / sector_size;
                dumped_bytes = dumped_sectors * sector_size;
                start_sector = first_lba + dumped_sectors;
                tracing::info!(
                    partition = %self.partition_name,
                    offset = %humanize_size(dumped_bytes),
                    "Resuming dump from breakpoint"
                );
            } else if file_size >= total_bytes {
                tracing::info!(
                    partition = %self.partition_name,
                    "Partition already fully dumped, skipping"
                );
                return Ok(JobResult {
                    success: true,
                    message: format!("partition {} already dumped", self.partition_name),
                    steps_completed: 0,
                });
            }
        }

        tracing::info!(
            partition = %self.partition_name,
            size = %humanize_size(total_bytes),
            path = ?self.output_path,
            "Dumping partition"
        );

        let max_payload = ctx.max_payload_size() as u64;
        let sectors_per_chunk = (max_payload / sector_size).max(1);

        std::fs::create_dir_all(self.output_path.parent().unwrap_or(&self.output_path))?;

        let mut output_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(!self.resume)
            .append(self.resume)
            .open(&self.output_path)?;

        let mut sector = start_sector;
        let mut remaining = total_sectors - (start_sector - first_lba);

        let start_time = Instant::now();

        let pb = if self.show_progress {
            let style = ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .expect("valid progress template")
                .progress_chars("#>-");
            let pb = ProgressBar::new(total_bytes).with_style(style);
            pb.set_position(dumped_bytes);
            Some(pb)
        } else {
            None
        };

        while remaining > 0 {
            let chunk = remaining.min(sectors_per_chunk);
            let data = ctx.read_sectors(physical_partition, sector, chunk).await?;
            output_file.write_all(&data)?;

            sector += chunk;
            remaining -= chunk;

            let bytes_read = chunk * sector_size;
            if let Some(ref pb) = pb {
                pb.inc(bytes_read);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message("done");
        }

        let total_elapsed = start_time.elapsed().as_secs_f64();
        let avg_rate = if total_elapsed > 0.0 {
            total_bytes as f64 / total_elapsed / 1024.0 / 1024.0
        } else {
            0.0
        };

        Ok(JobResult {
            success: true,
            message: format!(
                "dumped {} ({}) to {:?} @ {:.1} MB/s",
                self.partition_name,
                humanize_size(total_bytes),
                self.output_path,
                avg_rate
            ),
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "dump"
    }
}

pub struct WriteJob {
    pub partition_name: String,
    pub image_path: PathBuf,
}

#[async_trait]
impl Job for WriteJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let entry =
            ctx.find_partition(&self.partition_name)
                .ok_or_else(|| qedl_storage::StorageError::PartitionNotFound {
                    name: self.partition_name.clone(),
                })?;

        let physical_partition = entry.physical_partition;
        let first_lba = entry.first_lba;

        let sector_size = ctx.sector_size() as u64;
        let max_size = entry.size_bytes(ctx.sector_size());
        let max_payload = ctx.max_payload_size() as u64;
        let chunk_size = (max_payload as usize).max(sector_size as usize);

        let is_sparse = {
            let mut file = std::fs::File::open(&self.image_path)?;
            let mut magic = [0u8; 4];
            std::io::Read::read(&mut file, &mut magic)?;
            magic == [0xED, 0x26, 0xFF, 0x36] // SPARSE_HEADER_MAGIC
        };

        if is_sparse {
            #[cfg(feature = "sparse")]
            {
                tracing::info!("Detected sparse image, expanding...");
                let image_data = std::fs::read(&self.image_path)?;
                let data = qedl_image::sparse::expand_to_vec(&image_data)?;

                if data.len() as u64 > max_size {
                    return Err(crate::error::JobError::PreconditionFailed {
                        reason: format!(
                            "image too large: {} > partition {}",
                            humanize_size(data.len() as u64),
                            humanize_size(max_size)
                        ),
                    });
                }

                let num_sectors = (data.len() as u64).div_ceil(sector_size);
                let total_bytes = num_sectors * sector_size;
                let sectors_per_chunk = (max_payload / sector_size).max(1);

                let mut written: u64 = 0;
                let mut remaining = num_sectors;

                let style = ProgressStyle::default_bar()
                    .template(
                        "[{elapsed_precise}] [{bar:40.green/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
                    )
                    .expect("valid progress template")
                    .progress_chars("#>-");
                let pb = ProgressBar::new(total_bytes).with_style(style);

                let mut chunk_buffer = Vec::with_capacity(sectors_per_chunk as usize * sector_size as usize);

                while remaining > 0 {
                    let chunk = remaining.min(sectors_per_chunk);
                    let chunk_bytes = (chunk * sector_size) as usize;
                    let start = written as usize * sector_size as usize;
                    let end = (start + chunk_bytes).min(data.len());

                    chunk_buffer.clear();
                    chunk_buffer.extend_from_slice(&data[start..end]);
                    chunk_buffer.resize(chunk_bytes, 0);

                    ctx.write_sectors(physical_partition, first_lba + written, chunk, &chunk_buffer)
                        .await?;

                    written += chunk;
                    remaining -= chunk;

                    pb.inc(chunk_bytes as u64);
                }

                pb.finish_with_message("done");
            }
            #[cfg(not(feature = "sparse"))]
            {
                return Err(crate::error::JobError::PreconditionFailed {
                    reason: "Sparse image support requires the 'sparse' feature".to_string(),
                });
            }
        } else {
            let mut reader = crate::reader::ChunkedReader::new(&self.image_path, chunk_size)?;
            let total_bytes = reader.total_size();

            if total_bytes > max_size {
                return Err(crate::error::JobError::PreconditionFailed {
                    reason: format!(
                        "image too large: {} > partition {}",
                        humanize_size(total_bytes),
                        humanize_size(max_size)
                    ),
                });
            }

            tracing::info!(
                path = ?self.image_path,
                partition = %self.partition_name,
                size = %humanize_size(total_bytes),
                "Writing image to partition (streaming)"
            );

            let num_sectors = total_bytes.div_ceil(sector_size);
            let sectors_per_chunk = (max_payload / sector_size).max(1);

            let mut written: u64 = 0;
            let mut remaining = num_sectors;

            let style = ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.green/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .expect("valid progress template")
                .progress_chars("#>-");
            let pb = ProgressBar::new(total_bytes).with_style(style);

            let mut chunk_buffer = vec![0u8; chunk_size];

            while remaining > 0 {
                let chunk = remaining.min(sectors_per_chunk);
                let chunk_bytes = (chunk * sector_size) as usize;

                let mut data_read = 0;
                while data_read < chunk_bytes {
                    let n = reader.read_chunk(&mut chunk_buffer[data_read..chunk_bytes])?;
                    if n == 0 {
                        break;
                    }
                    data_read += n;
                }

                // Pad to sector boundary if needed
                if data_read < chunk_bytes {
                    chunk_buffer[data_read..chunk_bytes].fill(0);
                }

                ctx.write_sectors(
                    physical_partition,
                    first_lba + written,
                    chunk,
                    &chunk_buffer[..chunk_bytes],
                )
                .await?;

                written += chunk;
                remaining -= chunk;

                pb.inc(chunk_bytes as u64);
            }

            pb.finish_with_message("done");
        }

        Ok(JobResult {
            success: true,
            message: format!("wrote {:?} to {}", self.image_path, self.partition_name),
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "write"
    }
}

pub struct EraseJob {
    pub partition_name: String,
    pub show_progress: bool,
    pub erase_method: EraseMethod,
}

#[async_trait]
impl Job for EraseJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let entry =
            ctx.find_partition(&self.partition_name)
                .ok_or_else(|| qedl_storage::StorageError::PartitionNotFound {
                    name: self.partition_name.clone(),
                })?;

        let physical_partition = entry.physical_partition;
        let first_lba = entry.first_lba;
        let last_lba = entry.last_lba;

        let sector_size = ctx.sector_size() as u64;
        let total_sectors = last_lba - first_lba + 1;
        let total_bytes = total_sectors * sector_size;

        tracing::info!(
            partition = %self.partition_name,
            sectors = total_sectors,
            size = %humanize_size(total_bytes),
            method = ?self.erase_method,
            "Erasing partition"
        );

        let start_time = Instant::now();

        match self.erase_method {
            EraseMethod::Native => {
                ctx.erase_sectors_native(physical_partition, first_lba, total_sectors)
                    .await?;
            }
            EraseMethod::WriteZero => {
                let pb = if self.show_progress {
                    let style = ProgressStyle::default_bar()
                        .template(
                            "[{elapsed_precise}] [{bar:40.red/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
                        )
                        .expect("valid progress template")
                        .progress_chars("#>-");
                    let pb = ProgressBar::new(total_bytes).with_style(style);
                    Some(pb)
                } else {
                    None
                };

                ctx.erase_sectors(physical_partition, first_lba, total_sectors).await?;

                if let Some(pb) = pb {
                    pb.set_position(total_bytes);
                    pb.finish_with_message("done");
                }
            }
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        let avg_rate = if elapsed > 0.0 {
            total_bytes as f64 / elapsed / 1024.0 / 1024.0
        } else {
            0.0
        };

        Ok(JobResult {
            success: true,
            message: format!(
                "erased {} ({}) via {} @ {:.1} MB/s",
                self.partition_name,
                humanize_size(total_bytes),
                match self.erase_method {
                    EraseMethod::WriteZero => "write-zero",
                    EraseMethod::Native => "native-erase",
                },
                avg_rate
            ),
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "erase"
    }
}

#[cfg(feature = "sparse")]
pub struct FlashJob {
    pub rawprogram: PathBuf,
    pub patch: Option<PathBuf>,
    pub image_dir: PathBuf,
    pub erase_method: EraseMethod,
}

#[cfg(feature = "sparse")]
#[async_trait]
impl Job for FlashJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let task_list = qedl_image::TaskList::from_file(&self.rawprogram)?;
        task_list.validate_files(&self.image_dir)?;

        tracing::info!(
            tasks = task_list.len(),
            rawprogram = ?self.rawprogram,
            "Flashing firmware"
        );

        let sector_size = ctx.sector_size() as u64;
        let max_payload = ctx.max_payload_size() as u64;
        let sectors_per_chunk = (max_payload / sector_size).max(1);
        let start_time = Instant::now();

        let style = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.yellow/blue}] {pos}/{len} tasks ({eta})")
            .expect("valid progress template")
            .progress_chars("#>-");
        let pb = ProgressBar::new(task_list.len() as u64).with_style(style);

        for (i, task) in task_list.entries.iter().enumerate() {
            pb.set_message(format!("Task {}/{}: {:?}", i + 1, task_list.len(), task.task_type));
            tracing::debug!(task_num = i + 1, total = task_list.len(), task_type = ?task.task_type, "Executing flash task");

            match task.task_type {
                qedl_image::TaskType::Erase => match self.erase_method {
                    EraseMethod::Native => {
                        ctx.erase_sectors_native(task.physical_partition, task.start_sector, task.num_sectors)
                            .await?;
                    }
                    EraseMethod::WriteZero => {
                        ctx.erase_sectors(task.physical_partition, task.start_sector, task.num_sectors)
                            .await?;
                    }
                },
                qedl_image::TaskType::Program => {
                    let Some(ref filename) = task.filename else {
                        tracing::warn!("Skipping program task with no filename");
                        continue;
                    };

                    let image_path = self.image_dir.join(filename);

                    if task.sparse {
                        let image_data = std::fs::read(&image_path)?;
                        let data = qedl_image::sparse::expand_to_vec(&image_data)?;

                        let num_sectors = (data.len() as u64).div_ceil(sector_size);
                        let mut written: u64 = 0;
                        let mut remaining = num_sectors;
                        let mut chunk_buffer = Vec::with_capacity(sectors_per_chunk as usize * sector_size as usize);

                        while remaining > 0 {
                            let chunk = remaining.min(sectors_per_chunk);
                            let chunk_bytes = (chunk * sector_size) as usize;
                            let start = written as usize * sector_size as usize;
                            let end = (start + chunk_bytes).min(data.len());

                            chunk_buffer.clear();
                            chunk_buffer.extend_from_slice(&data[start..end]);
                            chunk_buffer.resize(chunk_bytes, 0);

                            ctx.write_sectors(
                                task.physical_partition,
                                task.start_sector + written,
                                chunk,
                                &chunk_buffer,
                            )
                            .await?;

                            written += chunk;
                            remaining -= chunk;
                        }
                    } else {
                        let chunk_size = (max_payload as usize).max(sector_size as usize);
                        let mut reader = crate::reader::ChunkedReader::new(&image_path, chunk_size)?;
                        let total_bytes = reader.total_size();
                        let num_sectors = total_bytes.div_ceil(sector_size);
                        let mut written: u64 = 0;
                        let mut remaining = num_sectors;
                        let mut chunk_buffer = vec![0u8; chunk_size];

                        while remaining > 0 {
                            let chunk = remaining.min(sectors_per_chunk);
                            let chunk_bytes = (chunk * sector_size) as usize;

                            let mut data_read = 0;
                            while data_read < chunk_bytes {
                                let n = reader.read_chunk(&mut chunk_buffer[data_read..chunk_bytes])?;
                                if n == 0 {
                                    break;
                                }
                                data_read += n;
                            }

                            if data_read < chunk_bytes {
                                chunk_buffer[data_read..chunk_bytes].fill(0);
                            }

                            ctx.write_sectors(
                                task.physical_partition,
                                task.start_sector + written,
                                chunk,
                                &chunk_buffer[..chunk_bytes],
                            )
                            .await?;

                            written += chunk;
                            remaining -= chunk;
                        }
                    }
                }
            }
            pb.inc(1);
        }

        pb.finish_with_message("All tasks done");

        if let Some(ref patch_path) = self.patch {
            tracing::info!(path = ?patch_path, "Applying patches");
            let patch_set = qedl_image::PatchSet::from_file(patch_path)?;

            let mut vars = std::collections::HashMap::new();
            let (disk_sectors, last_end) = {
                let all_parts = ctx.all_partitions();
                if let Some(last_entry) = all_parts.last() {
                    (last_entry.last_lba + 1, last_entry.last_lba * sector_size + sector_size)
                } else {
                    (0, 0)
                }
            };
            vars.insert("NUM_DISK_SECTORS".to_string(), disk_sectors.to_string());
            vars.insert("LAST_PARTITION_END".to_string(), last_end.to_string());

            for patch_entry in &patch_set.entries {
                let patch_data = match patch_entry.resolve_value(&vars) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!(error = %e, "Patch entry skipped");
                        continue;
                    }
                };
                let sector = patch_entry.start_sector;
                let existing = match ctx.read_sectors(patch_entry.physical_partition, sector, 1).await {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!(error = %e, sector = sector, "Failed to read sector for patch");
                        continue;
                    }
                };

                let offset = patch_entry.byte_offset as usize;
                let mut patched = existing.to_vec();
                let end = (offset + patch_data.len()).min(patched.len());
                patched[offset..end].copy_from_slice(&patch_data[..end - offset]);

                if let Err(e) = ctx
                    .write_sectors(patch_entry.physical_partition, sector, 1, &patched)
                    .await
                {
                    tracing::warn!(error = %e, sector = sector, "Failed to apply patch");
                }
            }
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        tracing::info!(elapsed = elapsed, "Flash completed");

        Ok(JobResult {
            success: true,
            message: format!("flashed {} tasks ({:.1}s)", task_list.len(), elapsed),
            steps_completed: task_list.len(),
        })
    }

    fn name(&self) -> &str {
        "flash"
    }
}

pub struct InfoJob;

#[async_trait]
impl Job for InfoJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let extra_logs = match ctx.refresh_storage_info().await {
            Ok(logs) => logs,
            Err(e) => {
                tracing::warn!(error = %e, "getstorageinfo failed during info command");
                vec![]
            }
        };

        let storage_type = ctx.storage_name().to_string();
        let sector_size = ctx.sector_size();
        let total_sectors = ctx.total_sectors();

        let mut msg = format!(
            "Storage:       {}\n\
             Sector Size:   {} bytes\n\
             Total Sectors: {}\n\
             Capacity:      {}\n\
             Partitions:    {}",
            storage_type,
            sector_size,
            total_sectors,
            humanize_size(total_sectors * sector_size as u64),
            ctx.all_partitions().len(),
        );

        // Add Sahara device info if available
        if let Some(session) = ctx.session() {
            if let Some(ref msm_id) = session.msm_hw_id {
                let hex_str: Vec<String> = msm_id.iter().map(|b| format!("{:02X}", b)).collect();
                msg.push_str(&format!("\nMSM HW ID:     {}", hex_str.join("")));
                // Try to extract SOC_HW_VERSION (first 4 bytes LE)
                if msm_id.len() >= 4 {
                    let soc_hw_ver = u32::from_le_bytes([msm_id[0], msm_id[1], msm_id[2], msm_id[3]]);
                    msg.push_str(&format!("\nSOC HW Ver:    0x{:08X}", soc_hw_ver));
                }
            }
            if let Some(serial) = session.serial_num {
                msg.push_str(&format!("\nSerial:        0x{:016X}", serial));
            }
            if let Some(ref target) = session.firehose.target_name {
                msg.push_str(&format!("\nTarget:        {}", target));
            }
            if let Some(ref version) = session.firehose.version {
                msg.push_str(&format!("\nFH Version:    {}", version));
            }
        }

        // Parse and display storage info from getstorageinfo response
        for log_entry in &extra_logs {
            if log_entry.contains("storage_info") {
                // Try to parse JSON storage info
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(log_entry)
                    && let Some(info) = json_val.get("storage_info")
                    && let Some(obj) = info.as_object()
                {
                    msg.push_str("\n\nStorage Info:");
                    // Display fields in a readable order
                    let fields = [
                        ("mem_type", "Memory Type"),
                        ("prod_name", "Product Name"),
                        ("total_blocks", "Total Blocks"),
                        ("block_size", "Block Size"),
                        ("page_size", "Page Size"),
                        ("num_physical", "Physical Units"),
                        ("num_lun", "LUNs"),
                    ];
                    for (key, label) in &fields {
                        if let Some(val) = obj.get(*key) {
                            let val_str = match val {
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            msg.push_str(&format!("\n  {:<16} {}", format!("{}:", label), val_str));
                        }
                    }
                    // Display any remaining fields
                    for (key, val) in obj {
                        if !fields.iter().any(|(k, _)| k == key) {
                            let val_str = match val {
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            msg.push_str(&format!("\n  {:<16} {}", format!("{}:", key), val_str));
                        }
                    }
                } else {
                    // Not JSON, just display as-is
                    msg.push_str(&format!("\n  {}", log_entry));
                }
            } else if !log_entry.starts_with("Error") {
                msg.push_str(&format!("\n  {}", log_entry));
            }
        }

        Ok(JobResult {
            success: true,
            message: msg,
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "info"
    }
}

pub struct GptJob;

#[async_trait]
impl Job for GptJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let entries = ctx.all_partitions();
        let sector_size = ctx.sector_size();

        let mut msg = format!("{} partitions:\n", entries.len());
        for entry in &entries {
            msg.push_str(&format!(
                "  {:24} LBA {:>10} - {:>10}  {}  LUN {}\n",
                entry.name.trim().trim_end_matches('\0'),
                entry.first_lba,
                entry.last_lba,
                humanize_size(entry.size_bytes(sector_size)),
                entry.physical_partition,
            ));
        }

        Ok(JobResult {
            success: true,
            message: msg,
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "gpt"
    }
}

pub struct RebootJob;

#[async_trait]
impl Job for RebootJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        tracing::info!("Rebooting device");
        ctx.reboot().await?;
        Ok(JobResult {
            success: true,
            message: "rebooting device".to_string(),
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "reboot"
    }
}

#[cfg(feature = "sparse")]
pub struct VerifyJob {
    pub partition_name: String,
    pub image_path: PathBuf,
    pub show_progress: bool,
}

#[cfg(feature = "sparse")]
#[async_trait]
impl Job for VerifyJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        use qedl_image::checksum;

        let entry =
            ctx.find_partition(&self.partition_name)
                .ok_or_else(|| crate::error::JobError::PreconditionFailed {
                    reason: format!("partition '{}' not found", self.partition_name),
                })?;

        let physical_partition = entry.physical_partition;
        let first_lba = entry.first_lba;

        let sector_size = ctx.sector_size() as u64;

        // Read local file, expanding sparse images if needed
        let raw_file = std::fs::read(&self.image_path)?;
        let is_sparse = raw_file.len() >= 4 && u32::from_le_bytes(raw_file[..4].try_into().unwrap()) == 0x52415350;
        let (image_data, image_label) = if is_sparse {
            tracing::info!("Local image is sparse, expanding for verification");
            let expanded = qedl_image::sparse::expand_to_vec(&raw_file)?;
            (expanded, "expanded".to_string())
        } else {
            (raw_file, "raw".to_string())
        };

        let image_len = image_data.len() as u64;
        let sectors_to_read = image_len.div_ceil(sector_size);

        tracing::info!(
            "Verifying {} image ({} bytes, {} sectors) against partition '{}'",
            image_label,
            image_len,
            sectors_to_read,
            self.partition_name
        );

        // Compute local SHA256
        let local_sha256 = checksum::compute_sha256(&image_data);
        tracing::info!("Local image SHA256: {}", local_sha256);

        let start_time = Instant::now();

        // Get SHA256 digest from device
        tracing::info!("Requesting SHA256 digest from device...");
        let device_sha256 = ctx
            .get_sha256_digest(physical_partition, first_lba, sectors_to_read)
            .await?;

        tracing::info!("Device SHA256:     {}", device_sha256);

        if local_sha256 == device_sha256 {
            tracing::info!("Verification SUCCESS: SHA256 matches!");
            Ok(JobResult {
                success: true,
                message: format!(
                    "Verification passed! SHA256: {} ({} bytes, took {:.2}s)",
                    device_sha256,
                    image_len,
                    start_time.elapsed().as_secs_f64()
                ),
                steps_completed: 1,
            })
        } else {
            tracing::error!("Verification FAILED! SHA256 mismatch");
            Err(crate::error::JobError::PreconditionFailed {
                reason: format!("SHA256 mismatch: local = {}, device = {}", local_sha256, device_sha256),
            })
        }
    }

    fn name(&self) -> &str {
        "verify"
    }
}

pub struct XmlJob {
    pub xml: Option<String>,
    pub file: Option<PathBuf>,
}

#[async_trait]
impl Job for XmlJob {
    async fn execute(&self, ctx: &mut dyn JobContext) -> Result<JobResult> {
        let xml_content = if let Some(ref xml_str) = self.xml {
            xml_str.clone()
        } else if let Some(ref file_path) = self.file {
            let data = std::fs::read(file_path)?;
            String::from_utf8(data).map_err(|e| crate::error::JobError::PreconditionFailed {
                reason: format!("File is not valid UTF-8: {}", e),
            })?
        } else {
            return Err(crate::error::JobError::PreconditionFailed {
                reason: "Either --xml or --file must be specified".to_string(),
            });
        };

        tracing::info!(length = xml_content.len(), "Sending custom XML command");
        tracing::debug!(xml = %xml_content, "XML content");

        let resp = ctx.raw_xml(&xml_content).await?;

        let mut msg = "XML command sent successfully".to_string();
        if let Some(ref error) = resp.error {
            msg.push_str(&format!("\nError: {}", error));
        }

        Ok(JobResult {
            success: resp.is_ack,
            message: msg,
            steps_completed: 1,
        })
    }

    fn name(&self) -> &str {
        "xml"
    }
}
