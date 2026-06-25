#[allow(dead_code)]
pub fn format_device_info(
    storage_type: &str,
    sector_size: u32,
    total_sectors: u64,
    partition_count: usize,
    manufacturer: Option<&str>,
) -> String {
    let capacity = qedl::image::humanize_size(total_sectors * sector_size as u64);

    let mut lines = vec![
        format!("Storage:         {}", storage_type),
        format!("Sector Size:     {} bytes", sector_size),
        format!("Total Sectors:   {}", total_sectors),
        format!("Capacity:        {}", capacity),
        format!("Partition Count: {}", partition_count),
    ];

    if let Some(mfg) = manufacturer {
        lines.push(format!("Manufacturer:    {}", mfg));
    }

    lines.join("\n")
}

#[allow(dead_code)]
pub fn format_gpt_table(entries: &[qedl::storage::GptEntry], sector_size: u32) -> String {
    let mut lines = vec![
        format!(
            "{:<24} {:>12} {:>12} {:>12} {:>6}",
            "Name", "Start LBA", "End LBA", "Size", "LUN"
        ),
        "-".repeat(70),
    ];

    for entry in entries {
        let name = entry.name.trim().trim_end_matches('\0');
        lines.push(format!(
            "{:<24} {:>12} {:>12} {:>12} {:>6}",
            name,
            entry.first_lba,
            entry.last_lba,
            qedl::image::humanize_size(entry.size_bytes(sector_size)),
            entry.physical_partition,
        ));
    }

    lines.join("\n")
}
