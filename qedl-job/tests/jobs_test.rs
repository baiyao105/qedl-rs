use qedl_job::jobs::*;
use qedl_job::testutil::MockJobContext;
use qedl_core::PartitionInfo;
use std::path::PathBuf;

fn system_partition() -> PartitionInfo {
    PartitionInfo {
        name: "system".to_string(),
        first_lba: 1024,
        last_lba: 4095,
        physical_partition: 0,
    }
}

// ── InfoJob ──

#[tokio::test]
async fn test_info_job() {
    let mut ctx = MockJobContext::simple();
    let job = InfoJob;
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("eMMC"));
    assert!(result.message.contains("Partitions:"));
}

#[tokio::test]
async fn test_info_job_with_storage_logs() {
    let mut ctx = MockJobContext::simple();
    ctx.storage_info_response = Some(Ok(vec![
        "Total User Area: 14.00 GB".to_string(),
        "Boot Area: 4.00 MB".to_string(),
    ]));
    let job = InfoJob;
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.message.contains("Total User Area"));
}

#[tokio::test]
async fn test_info_job_name() {
    assert_eq!(InfoJob.name(), "info");
}

// ── GptJob ──

#[tokio::test]
async fn test_gpt_job() {
    let mut ctx = MockJobContext::simple();
    ctx.partitions
        .insert("system".to_string(), system_partition());
    let job = GptJob;
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("2 partitions"));
    assert!(result.message.contains("boot"));
    assert!(result.message.contains("system"));
}

#[tokio::test]
async fn test_gpt_job_empty() {
    let mut ctx = MockJobContext::simple();
    ctx.partitions.clear();
    let job = GptJob;
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("0 partitions"));
}

#[tokio::test]
async fn test_gpt_job_name() {
    assert_eq!(GptJob.name(), "gpt");
}

// ── DumpJob ──

#[tokio::test]
async fn test_dump_job() {
    let mut ctx = MockJobContext::simple();
    // Provide data: 1024 sectors * 512 bytes = 512KB
    let data = vec![0xABu8; 1024 * 512];
    ctx.push_read(bytes::Bytes::from(data));

    let tmp = std::env::temp_dir().join("qedl_test_dump.bin");
    let _ = std::fs::remove_file(&tmp);

    let job = DumpJob {
        partition_name: "boot".to_string(),
        output_path: tmp.clone(),
        show_progress: false,
        resume: false,
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("dumped"));
    assert!(result.steps_completed == 1);

    let metadata = std::fs::metadata(&tmp).unwrap();
    assert_eq!(metadata.len(), 1024 * 512);
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_dump_job_partition_not_found() {
    let mut ctx = MockJobContext::simple();
    let job = DumpJob {
        partition_name: "nonexistent".to_string(),
        output_path: PathBuf::from("/tmp/test"),
        show_progress: false,
        resume: false,
    };
    let result = job.execute(&mut ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_dump_job_resume_full() {
    let mut ctx = MockJobContext::simple();
    let tmp = std::env::temp_dir().join("qedl_test_dump_resume.bin");
    // Write a file that is >= total partition size
    let total_bytes = 1024 * 512u64;
    let content = vec![0xABu8; total_bytes as usize];
    std::fs::write(&tmp, &content).unwrap();

    let job = DumpJob {
        partition_name: "boot".to_string(),
        output_path: tmp.clone(),
        show_progress: false,
        resume: true,
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("already dumped"));
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_dump_job_name() {
    assert_eq!(
        DumpJob {
            partition_name: "x".to_string(),
            output_path: PathBuf::from("/tmp/x"),
            show_progress: false,
            resume: false,
        }
        .name(),
        "dump"
    );
}

// ── EraseJob ──

#[tokio::test]
async fn test_erase_job() {
    let mut ctx = MockJobContext::simple();
    let job = EraseJob {
        partition_name: "boot".to_string(),
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("erased"));
    assert!(!ctx.write_log.is_empty());
}

#[tokio::test]
async fn test_erase_job_partition_not_found() {
    let mut ctx = MockJobContext::simple();
    let job = EraseJob {
        partition_name: "nonexistent".to_string(),
    };
    let result = job.execute(&mut ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_erase_job_name() {
    assert_eq!(EraseJob { partition_name: "x".to_string() }.name(), "erase");
}

// ── RebootJob ──

#[tokio::test]
async fn test_reboot_job() {
    let mut ctx = MockJobContext::simple();
    let job = RebootJob;
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(ctx.reboot_called);
}

#[tokio::test]
async fn test_reboot_job_name() {
    assert_eq!(RebootJob.name(), "reboot");
}

// ── XmlJob ──

#[tokio::test]
async fn test_xml_job_from_string() {
    let mut ctx = MockJobContext::simple();
    ctx.push_xml_ack();

    let job = XmlJob {
        xml: Some(r#"<custom command="test" />"#.to_string()),
        file: None,
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("successfully"));
}

#[tokio::test]
async fn test_xml_job_nak() {
    let mut ctx = MockJobContext::simple();
    ctx.push_xml_nak("invalid command");

    let job = XmlJob {
        xml: Some(r#"<bad />"#.to_string()),
        file: None,
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(!result.success);
    assert!(result.message.contains("invalid command"));
}

#[tokio::test]
async fn test_xml_job_no_input() {
    let mut ctx = MockJobContext::simple();
    let job = XmlJob {
        xml: None,
        file: None,
    };
    let result = job.execute(&mut ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_xml_job_from_file() {
    let mut ctx = MockJobContext::simple();
    ctx.push_xml_ack();

    let tmp = std::env::temp_dir().join("qedl_test_xml.xml");
    std::fs::write(&tmp, r#"<custom />"#).unwrap();

    let job = XmlJob {
        xml: None,
        file: Some(tmp.clone()),
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_xml_job_name() {
    assert_eq!(XmlJob { xml: None, file: None }.name(), "xml");
}

// ── WriteJob ──

#[tokio::test]
async fn test_write_job() {
    let mut ctx = MockJobContext::simple();

    // Create a small non-sparse image file (512 bytes = 1 sector)
    let tmp = std::env::temp_dir().join("qedl_test_write.img");
    let content = vec![0x55u8; 512];
    std::fs::write(&tmp, &content).unwrap();

    let job = WriteJob {
        partition_name: "boot".to_string(),
        image_path: tmp.clone(),
    };
    let result = job.execute(&mut ctx).await.unwrap();
    assert!(result.success);
    assert!(result.message.contains("wrote"));
    assert_eq!(ctx.write_log.len(), 1);
    assert_eq!(ctx.write_log[0].1, 0); // start_sector
    assert_eq!(ctx.write_log[0].2, 1); // num_sectors
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_write_job_partition_not_found() {
    let mut ctx = MockJobContext::simple();
    let tmp = std::env::temp_dir().join("qedl_test_write2.img");
    std::fs::write(&tmp, [0x55u8; 512]).unwrap();

    let job = WriteJob {
        partition_name: "nonexistent".to_string(),
        image_path: tmp.clone(),
    };
    let result = job.execute(&mut ctx).await;
    assert!(result.is_err());
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_write_job_name() {
    assert_eq!(
        WriteJob {
            partition_name: "x".to_string(),
            image_path: PathBuf::from("/tmp/x"),
        }
        .name(),
        "write"
    );
}
