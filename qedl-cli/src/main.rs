mod args;
mod devices;
mod output;

use args::{Cli, Commands, ForceMode};
use clap::Parser;
use output::Spinner;
use output::*;
use owo_colors::OwoColorize;
use qedl::EraseMethod;
use qedl::transport::DeviceEnumerator;
use std::process;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    let filter = match cli.global.verbose {
        0 => EnvFilter::new("info"),
        1 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    let indicatif_layer = IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_timer(fmt::time::Uptime::default())
                .with_writer(indicatif_layer.get_stderr_writer()),
        )
        .with(indicatif_layer)
        .try_init()?;

    tracing::debug!("qedl-cli starting...");

    // Handle `devices` subcommand early — it doesn't need a device connection
    if let Commands::Devices { watch, json } = &cli.command {
        return devices::run_devices(*watch, *json);
    }

    if let Some(wait_secs) = cli.global.wait_device {
        let timeout = if wait_secs == 0 { None } else { Some(wait_secs) };
        let spinner = Spinner::new("Waiting for device...");
        match DeviceEnumerator::wait_for_device(cli.global.port.as_deref(), cli.global.serial.as_deref(), timeout, 1000)
        {
            Ok(_device) => { /* spinner will be dropped, logs already printed */ }
            Err(e) => {
                drop(spinner);
                let msg = format!("{}", e);
                error(&msg);
                process::exit(if msg.contains("not found") { 2 } else { 1 });
            }
        }
    }

    let mut builder = qedl::QedlClient::builder()
        .timeout(std::time::Duration::from_millis(cli.global.timeout))
        .dry_run(cli.global.dry_run)
        .verbose(cli.global.verbose > 0)
        .auto_edl_switch(!cli.global.no_switch_edl)
        .spinner_factory(|msg| Box::new(Spinner::new(msg)))
        .progress_factory(|| Box::new(output::IndicatifProgress::new()));

    if let Some(ref force) = cli.global.force_mode {
        builder = builder.force_mode(match force {
            ForceMode::Edl => qedl::ModeOverride::Edl,
            ForceMode::Diag => qedl::ModeOverride::Diag,
        });
    }

    if let Some(ref port) = cli.global.port {
        builder = builder.port(port.as_str());
    }
    if let Some(ref serial) = cli.global.serial {
        builder = builder.serial(serial.as_str());
    }
    if let Some(ref loader) = cli.global.loader {
        builder = builder.loader(loader.as_path());
    }

    let mut client = builder.build();

    if matches!(cli.global.force_mode, Some(ForceMode::Diag)) {
        let spinner = Spinner::new("Switching DIAG to EDL...");
        client.connect()?;
        drop(spinner);
        success("Device switched to EDL (9008) mode");
        return Ok(());
    }

    run(cli.command, &mut client).await
}

async fn run(command: Commands, client: &mut qedl::QedlClient) -> color_eyre::Result<()> {
    match command {
        Commands::Devices { .. } => unreachable!(),
        Commands::Info => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let result = client.info().await?;
            header("Storage Info");
            for line in result.message.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    kv(key.trim(), value.trim());
                } else {
                    info(line);
                }
            }
            Ok(())
        }
        Commands::Gpt => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let result = client.gpt().await?;
            header("Partition Table");
            for line in result.message.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // First line is the count header
                if line.ends_with(':') {
                    println!("    {}", line.bold());
                } else {
                    // Parse and format partition rows
                    // Format: "  name                  LBA first - last  size  LUN x"
                    println!("    {}", line);
                }
            }
            Ok(())
        }
        Commands::Reboot => {
            let spinner = Spinner::new("Connecting to device...");
            client.init_firehose_only().await?;
            drop(spinner);
            client.reboot().await?;
            success("Device rebooting...");
            Ok(())
        }
        Commands::Xml { xml, file } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init_firehose_only().await?;
            drop(spinner);
            let xml_content = resolve_xml_input(xml, file)?;
            let result = client.raw_xml(&xml_content).await?;
            xml_response(result.success, &result.message);
            if !result.success {
                process::exit(1);
            }
            Ok(())
        }
        Commands::GenXml { output } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);

            let sector_size = client.session().map(|s| s.sector_size()).unwrap_or(4096);
            let programs: Vec<String> = client.partitions().iter().map(|p| {
                let n = p.name.trim().trim_end_matches('\0');
                let sectors = p.last_lba - p.first_lba + 1;
                format!(
                    r#"  <program SECTOR_SIZE_IN_BYTES="{}" file_sector_offset="0" filename="{n}.img" label="{n}" num_partition_sectors="{sectors}" physical_partition_number="{}" size_in_KB="{}" sparse="false" start_byte="0" start_sector="{}" />"#,
                    sector_size, p.physical_partition, (sectors * sector_size as u64) / 1024, p.first_lba
                )
            }).collect();

            let xml = std::iter::once(r#"<?xml version="1.0" encoding="UTF-8"?>"#)
                .chain(std::iter::once("<data>"))
                .chain(programs.iter().map(|s| s.as_str()))
                .chain(std::iter::once("</data>"))
                .collect::<Vec<_>>()
                .join("\n");

            std::fs::write(&output, xml)?;
            success(&format!("Generated {}", output.display()));
            Ok(())
        }
        Commands::Dump {
            partition,
            file,
            resume,
        }
        | Commands::Read {
            partition,
            file,
            resume,
        } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let result = if resume {
                client.dump_resume(&partition, &file).await?
            } else {
                client.dump(&partition, &file).await?
            };
            success(&result.message);
            Ok(())
        }
        Commands::Write { partition, file } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let result = client.write(&partition, &file).await?;
            success(&result.message);
            Ok(())
        }
        Commands::Erase {
            partition,
            native_erase,
        } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let erase_method = if native_erase {
                EraseMethod::Native
            } else {
                EraseMethod::WriteZero
            };
            let result = client.erase(&partition, true, erase_method).await?;
            success(&result.message);
            Ok(())
        }
        Commands::Flash {
            rawprogram,
            patch,
            image_dir,
            native_erase,
        } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let image_dir = image_dir.unwrap_or_else(|| ".".into());
            let erase_method = if native_erase {
                EraseMethod::Native
            } else {
                EraseMethod::WriteZero
            };
            let result = client
                .flash(&rawprogram, patch.as_deref(), &image_dir, erase_method)
                .await?;
            success(&result.message);
            Ok(())
        }
        Commands::Verify { partition, file } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init().await?;
            drop(spinner);
            let result = client.verify(&partition, &file).await?;
            if result.success {
                success(&result.message);
                Ok(())
            } else {
                error(&result.message);
                process::exit(1);
            }
        }
        Commands::Peek { address, size, output } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init_firehose_only().await?;
            drop(spinner);
            let addr = parse_hex_or_decimal(&address)?;
            let data = client.peek(addr, size).await?;
            if let Some(path) = output {
                std::fs::write(&path, &data)?;
                success(&format!("Wrote {} bytes to {}", data.len(), path.display()));
            } else {
                // Display hex dump
                header(&format!("Peek: 0x{:X} ({} bytes)", addr, data.len()));
                hex_dump_display(&data);
            }
            Ok(())
        }
        Commands::Poke { address, data } => {
            let spinner = Spinner::new("Connecting to device...");
            client.init_firehose_only().await?;
            drop(spinner);
            let addr = parse_hex_or_decimal(&address)?;
            let bytes = parse_hex_data(&data)?;
            client.poke(addr, &bytes).await?;
            success(&format!("Poke: wrote {} bytes to 0x{:X}", bytes.len(), addr));
            Ok(())
        }
    }
}

fn resolve_xml_input(xml: Option<String>, file: Option<std::path::PathBuf>) -> color_eyre::Result<String> {
    match (xml, file) {
        (Some(x), _) => Ok(x),
        (_, Some(path)) => {
            let content = std::fs::read_to_string(&path)?;
            Ok(content)
        }
        (None, None) => Err(color_eyre::eyre::eyre!("Specify XML either via --file <path> or xml <xml-text>")),
    }
}
