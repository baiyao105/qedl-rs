mod args;

use args::{Cli, Commands};
use clap::Parser;
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

    if let Some(wait_secs) = cli.global.wait_device {
        let timeout = if wait_secs == 0 { None } else { Some(wait_secs) };
        match DeviceEnumerator::wait_for_device(cli.global.port.as_deref(), cli.global.serial.as_deref(), timeout, 1000)
        {
            Ok(device) => tracing::info!("Device found after waiting: {}", device),
            Err(e) => {
                let msg = format!("{}", e);
                eprintln!("Error: {}", msg);
                process::exit(if msg.contains("not found") { 2 } else { 1 });
            }
        }
    }

    let mut builder = qedl::QedlClient::builder()
        .timeout(std::time::Duration::from_millis(cli.global.timeout))
        .dry_run(cli.global.dry_run)
        .verbose(cli.global.verbose > 0)
        .auto_edl_switch(!cli.global.no_switch_edl);

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

    run(cli.command, &mut client).await
}

async fn run(command: Commands, client: &mut qedl::QedlClient) -> color_eyre::Result<()> {
    match command {
        Commands::List => {
            let devices = DeviceEnumerator::list()?;
            if devices.is_empty() {
                println!("No 9008/DIAG devices found.");
            } else {
                println!("Found {} device(s):", devices.len());
                for dev in &devices {
                    println!("  {}", dev);
                }
            }
            Ok(())
        }
        Commands::Info => {
            client.init().await?;
            let result = client.info().await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Gpt => {
            client.init().await?;
            let result = client.gpt().await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Reboot => {
            client.init_firehose_only().await?;
            client.reboot().await?;
            println!("Device rebooting...");
            Ok(())
        }
        Commands::Xml { xml, file } => {
            client.init_firehose_only().await?;
            let xml_content = resolve_xml_input(xml, file)?;
            client.raw_xml(&xml_content).await?;
            println!("XML command sent successfully.");
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
            client.init().await?;
            let result = if resume {
                client.dump_resume(&partition, &file).await?
            } else {
                client.dump(&partition, &file).await?
            };
            println!("{}", result.message);
            Ok(())
        }
        Commands::Write { partition, file } => {
            client.init().await?;
            let result = client.write(&partition, &file).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Erase { partition } => {
            client.init().await?;
            let result = client.erase(&partition).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Flash {
            rawprogram,
            patch,
            image_dir,
        } => {
            client.init().await?;
            let image_dir = image_dir.unwrap_or_else(|| ".".into());
            let result = client.flash(&rawprogram, patch.as_deref(), &image_dir).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Verify { partition, file } => {
            client.init().await?;
            let result = client.verify(&partition, &file).await?;
            println!("{}", result.message);
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
        (None, None) => Err(color_eyre::eyre::eyre!("Specify XML with --xml or --file")),
    }
}
