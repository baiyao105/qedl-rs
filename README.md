# qedl-rs

**Qualcomm Emergency Download (EDL) SDK and CLI — written in Rust**

A pure-Rust implementation of the Qualcomm Sahara + Firehose protocol stack for 9008 mode flashing.

## Features

- **Sahara handshake** — Loader upload with PblHack recovery
- **Firehose XML engine** — Read / program / erase sectors via XML commands
- **GPT parsing** — Primary + Backup GPT with multi-LUN (UFS) support
- **rawprogram.xml** — Parse and execute QFIL-format flash layouts
- **Sparse image** — Auto-detect and expand Android sparse images
- **Partition ops** — Dump, flash, erase by partition name
- **Cross-platform** — Windows (COM), Linux (ttyUSB), macOS

## Quick Start

```rust
use qedl::QedlClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = QedlClient::builder()
        .port("COM3")
        .loader("prog_firehose.mbn")
        .build();

    client.init().await?;

    // List partitions
    for p in client.partitions() {
        println!("{}: LUN {} LBA {}-{}", p.name, p.physical_partition, p.first_lba, p.last_lba);
    }

    // Dump a partition
    client.dump("boot", "boot.img").await?;

    // Reboot
    client.reboot().await?;
    Ok(())
}
```

## CLI Usage

```bash
qedl list                          # List 9008 devices
qedl info                          # Device info
qedl gpt                           # Print GPT partition table
qedl dump boot boot.img            # Dump partition
qedl write boot boot.img           # Write partition
qedl erase userdata                # Erase partition
qedl flash rawprogram0.xml         # Flash from XML
qedl reboot                        # Reboot device
```

## Architecture

```
qedl-transport  → USB/serial I/O abstraction
qedl-sahara     → Sahara handshake protocol
qedl-firehose   → Firehose XML command engine
qedl-storage    → GPT parsing + partition mapping
qedl-image      → rawprogram/patch XML + sparse expansion
qedl-job        → Job orchestration (flash/dump/erase)
qedl            → Unified SDK facade
qedl-cli        → CLI binary
```

## License

MIT
