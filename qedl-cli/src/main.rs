mod args;
mod output;

use args::{Cli, Commands};
use clap::Parser;
use qedl::job::{DumpJob, EraseJob, ExecutorConfig, FlashJob, JobContext, JobExecutor, VerifyJob, WriteJob};
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

    let config = ExecutorConfig {
        port: cli.global.port.clone(),
        serial: cli.global.serial.clone(),
        loader: cli.global.loader.clone(),
        timeout: std::time::Duration::from_millis(cli.global.timeout),
        dry_run: cli.global.dry_run,
        verbose: cli.global.verbose > 0,
        max_retries: 3,
        event_sink: None,
    };

    if let Some(wait_secs) = cli.global.wait_device {
        let timeout = if wait_secs == 0 { None } else { Some(wait_secs) };
        match DeviceEnumerator::wait_for_device(cli.global.port.as_deref(), cli.global.serial.as_deref(), timeout, 1000)
        {
            Ok(device) => {
                tracing::info!("Device found after waiting: {}", device);
                if cli.global.port.is_none() {
                    // 如果没指定 --port, 用发现的设备端口
                    // 这里不做修改, 因为后续 connect 会用同样的逻辑查找
                }
            }
            Err(e) => {
                let msg = format!("{}", e);
                eprintln!("Error: {}", msg);
                process::exit(if msg.contains("not found") { 2 } else { 1 });
            }
        }
    }

    run(cli, config).await?;

    Ok(())
}

async fn run(cli: Cli, config: ExecutorConfig) -> color_eyre::Result<()> {
    match cli.command {
        Commands::List => {
            let devices = DeviceEnumerator::list()?;
            if devices.is_empty() {
                println!("No 9008/90B8 devices found.");
            } else {
                println!("Found {} device(s):", devices.len());
                for dev in &devices {
                    println!("  {}", dev);
                }
            }
            Ok(())
        }
        Commands::Info => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let partitions: Vec<_> = executor.partitions().all_entries();
            let ss = executor.firehose().sector_size();
            println!(
                "{}",
                output::format_device_info(
                    &executor.firehose().memory_name,
                    ss,
                    executor.firehose().total_sectors,
                    partitions.len(),
                    None,
                )
            );
            Ok(())
        }
        Commands::Gpt => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let entries: Vec<qedl::storage::GptEntry> =
                executor.partitions().all_entries().into_iter().cloned().collect();
            let ss = executor.firehose().sector_size();
            println!("{}", output::format_gpt_table(&entries, ss));
            Ok(())
        }
        Commands::Reboot => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            executor.reboot().await?;
            println!("Device rebooting...");
            Ok(())
        }
        Commands::Xml { xml, file } => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let xml_content = resolve_xml_input(xml, file)?;
            executor.raw_xml(&xml_content).await?;
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
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let job = DumpJob {
                partition_name: partition,
                output_path: file,
                show_progress: true,
                resume,
            };
            let result = executor.execute(&job).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Write { partition, file } => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let job = WriteJob {
                partition_name: partition,
                image_path: file,
            };
            let result = executor.execute(&job).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Erase { partition } => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let job = EraseJob {
                partition_name: partition,
            };
            let result = executor.execute(&job).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Flash {
            rawprogram,
            patch,
            image_dir,
        } => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let job = FlashJob {
                rawprogram,
                patch,
                image_dir: image_dir.unwrap_or_else(|| ".".into()),
            };
            let result = executor.execute(&job).await?;
            println!("{}", result.message);
            Ok(())
        }
        Commands::Verify { partition, file } => {
            let mut executor = JobExecutor::new(config);
            executor.init().await?;
            let job = VerifyJob {
                partition_name: partition,
                image_path: file,
                show_progress: true,
            };
            let result = executor.execute(&job).await?;
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
