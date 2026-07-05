use qedl::{EraseMethod, QedlClient};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("qedl=debug").init();

    let mut client = QedlClient::builder().port("COM3").build();

    println!("Connecting to device...");
    client.init().await?;

    println!("\nPartitions:");
    for part in client.partitions() {
        println!(
            "  {} (LUN {}, sectors {}-{})",
            part.name, part.physical_partition, part.first_lba, part.last_lba
        );
    }

    println!("\nFlashing firmware...");
    let result = client
        .flash(
            Path::new("rawprogram.xml"),
            Some(Path::new("patch.xml")),
            Path::new("./images"),
            EraseMethod::WriteZero,
        )
        .await?;
    println!("Flash complete: {}", result.message);

    println!("\nRebooting device...");
    client.reboot().await?;

    Ok(())
}
