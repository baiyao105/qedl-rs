use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub const VERSION: &str = env!("VERSION");
pub const FULL_VERSION: &str = env!("FULL_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "qedl",
    about = "A Qualcomm 9008 EDL Tool",
    long_about = ABOUT,
    version = VERSION,
    long_version = FULL_VERSION,
)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub global: GlobalArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct GlobalArgs {
    /// 指定串口（例如 COM3 或 /dev/ttyUSB0）
    #[arg(long, global = true)]
    pub port: Option<String>,

    #[arg(long, global = true)]
    pub serial: Option<String>,

    /// Sahara loader 文件路径（可选：未指定时假设设备已在 Firehose 模式）
    #[arg(long, global = true, value_name = "FILE")]
    pub loader: Option<PathBuf>,

    /// 串口超时时间（毫秒）
    #[arg(long, global = true, default_value = "45000", value_name = "MS")]
    pub timeout: u64,

    #[arg(long, global = true)]
    pub dry_run: bool,

    /// 日志级别（-v、-vv）
    #[arg(long, short, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[arg(
        long,
        global = true,
        num_args = 0..=1,
        default_missing_value = "0",
        value_name = "SECS"
    )]
    pub wait_device: Option<u64>,

    /// 禁止自动从 DIAG 模式切换到 EDL 模式 (9008)
    #[arg(long, global = true)]
    pub no_switch_edl: bool,

    /// 强制指定设备模式（覆盖自动检测）
    #[arg(long, global = true, value_enum)]
    pub force_mode: Option<ForceMode>,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ForceMode {
    /// EDL 模式，跳过 DIAG→EDL 切换
    Edl,
    /// DIAG 模式，发送切换命令后退出
    Diag,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 列出设备（按 EDL/DIAG 分组）
    Devices {
        /// 实时监控设备插拔（可选刷新间隔秒数，默认 1s）
        #[arg(long, num_args = 0..=1, default_missing_value = "1", value_name = "SECS")]
        watch: Option<u64>,

        /// JSON 格式输出
        #[arg(long)]
        json: bool,
    },

    /// 显示设备信息
    Info,

    /// 显示 GPT 分区表
    Gpt,

    /// 导出分区
    Dump {
        partition: String,
        file: PathBuf,
        /// 如果文件已存在，尝试从断点处继续下载
        #[arg(long, short)]
        resume: bool,
    },

    /// Dump 的别名
    #[command(name = "read")]
    Read {
        partition: String,
        file: PathBuf,
        /// 如果文件已存在，尝试从断点处继续下载
        #[arg(long, short)]
        resume: bool,
    },

    /// 刷写分区
    Write { partition: String, file: PathBuf },

    /// 擦除分区
    Erase {
        partition: String,
        /// 使用原生 Firehose erase 命令（更快，但部分设备可能有 bug）
        #[arg(long)]
        native_erase: bool,
    },

    /// 根据 rawprogram.xml 刷写
    Flash {
        rawprogram: PathBuf,
        patch: Option<PathBuf>,
        /// 镜像文件所在目录（默认当前目录）
        #[arg(long, value_name = "DIR")]
        image_dir: Option<PathBuf>,
        /// 使用原生 Firehose erase 命令（更快，但部分设备可能有 bug）
        #[arg(long)]
        native_erase: bool,
    },

    /// 校验分区
    Verify { partition: String, file: PathBuf },

    /// 读取内存 (peek)
    Peek {
        /// 物理地址 (十六进制，如 0x08071320)
        address: String,
        /// 读取字节数
        size: u32,
        /// 输出到文件
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// 写入内存 (poke)
    Poke {
        /// 物理地址 (十六进制，如 0x08071320)
        address: String,
        /// 十六进制数据 (如 "0xAA 0xBB 0xCC" 或 "AABBCC")
        data: String,
    },

    /// 重启设备
    Reboot,

    /// 发送自定义 XML
    Xml {
        /// XML 字符串
        xml: Option<String>,

        /// 从文件读取 XML
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
    },

    /// 根据 GPT 生成 rawprogram.xml
    #[command(name = "genxml")]
    GenXml {
        /// 输出文件路径 (例如 rawprogram.xml)
        output: PathBuf,
    },
}

const ABOUT: &str = r#"qedl - A Qualcomm 9008 EDL Tool

Repository:
  https://github.com/baiyao105/qedl-rs
"#;
