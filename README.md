<!-- markdownlint-disable MD033 MD036 MD041 -->

<h1 align="center">
  qedl-rs
</h1>

<p align="center">
  纯 Rust 实现的 Qualcomm Sahara + Firehose 协议工具
</p>

<div align="center">

[![星标](https://img.shields.io/github/stars/baiyao105/qedl-rs?style=for-the-badge&color=orange&label=%E6%98%9F%E6%A0%87)](https://github.com/baiyao105/qedl-rs) [![当前版本](https://img.shields.io/github/v/release/baiyao105/qedl-rs?style=for-the-badge&color=purple&label=%E5%BD%93%E5%89%8D%E7%89%88%E6%9C%AC)](https://github.com/baiyao105/qedl-rs/releases/latest) [![License MIT](https://img.shields.io/badge/license-MIT-blue.svg?label=%E5%BC%80%E6%BA%90%E8%AE%B8%E5%8F%AF%E8%AF%81&style=for-the-badge)](https://github.com/baiyao105/qedl-rs?tab=MIT-1-ov-file)
[![Crates.io](https://img.shields.io/crates/v/qedl)](https://crates.io/crates/qedl)

</div>

> [!WARNING]
> 本项目还在开发中，不建议在生产环境中使用。

---

## ✨ Features

- **Sahara 握手**：Loader 上传 + PblHack 恢复，支持跳过（设备已在 Firehose 模式时）
- **Firehose XML 引擎**：通过 XML 命令读写扇区、擦除分区
- **GPT 解析**：主备 GPT 表 + 多 LUN（UFS）支持
- **rawprogram.xml**：解析并执行 QFIL 格式的刷机布局
- **Sparse 镜像**：自动检测并展开 Android sparse 镜像
- **分区操作**：按名称 dump、flash、erase
- **SHA256 校验**：设备端 SHA256 摘要验证
- **跨平台**：Windows (COM)、Linux (ttyUSB)、macOS

## 👀 Preview

```
$ qedl devices
Devices (1)
Qualcomm HS-USB QDLoader 9008
├── VID:PID        05C6:9008
├── Location       1-1
└── Interfaces
     └── COM3  EDL   Qualcomm HS-USB QDLoader 9008  05C6:9008

$ qedl gpt
    32 partitions:
      boot           LBA      131072 -     2097151   1.0 GiB  LUN 0
      system         LBA     2097152 -    25165823  11.0 GiB  LUN 0
      userdata       LBA    25165824 -   121775103  46.0 GiB  LUN 0

$ qedl dump boot boot.img
  [OK] dumped boot (4.0 MiB) to "boot.img" @ 2.3 MB/s
```

## 🚀 Quick Start

```bash
# 安装
cargo install qedl-cli

# 列出设备
qedl devices

# 导出分区
qedl dump boot boot.img

# 刷入分区
qedl write boot boot.img

# 从 rawprogram.xml 刷机
qedl flash rawprogram0.xml --image-dir ./images

# 重启设备
qedl reboot
```

## 📦 Installation

### 从 crates.io 安装

```bash
cargo install qedl-cli
```

### 前置条件

- Rust 2024 edition
- Windows: 需要 Qualcomm USB 驱动

## 📝 Usage

### SDK 用法

```rust
use qedl::QedlClient;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = QedlClient::builder()
        .port("COM3")
        .loader("prog_firehose.mbn")
        .build();

    client.init().await?;

    // 列出分区
    for p in client.partitions() {
        println!("{}: LUN {} LBA {}-{}", p.name, p.physical_partition, p.first_lba, p.last_lba);
    }

    // Dump 分区
    client.dump("boot", "boot.img").await?;

    // 写入分区
    client.write("boot", Path::new("boot.img")).await?;

    // 擦除分区
    client.erase("userdata", true, qedl::EraseMethod::WriteZero).await?;

    // 重启
    client.reboot().await?;
    Ok(())
}
```

### CLI 用法

| 命令 | 说明 |
|------|------|
| `qedl devices` | 列出 EDL/DIAG 设备 |
| `qedl info` | 设备信息 |
| `qedl gpt` | 打印 GPT 分区表 |
| `qedl dump <分区> <文件>` | 导出分区（`-r` 断点续传） |
| `qedl read <分区> <文件>` | dump 的别名 |
| `qedl dump-partitions` | 导出所有分区（`-o` 指定目录） |
| `qedl write <分区> <文件>` | 写入分区 |
| `qedl erase <分区>` | 擦除分区（`--native-erase` 使用原生命令） |
| `qedl flash <rawprogram> [patch]` | 从 XML 刷机（`--image-dir` 指定目录） |
| `qedl verify <分区> <文件>` | SHA256 校验 |
| `qedl peek <地址> <大小>` | 读取物理内存（`-o` 输出到文件） |
| `qedl poke <地址> <数据>` | 写入物理内存 |
| `qedl xml <XML>` | 发送原始 XML（`-f` 从文件读取） |
| `qedl genxml <输出>` | 从 GPT 生成 rawprogram.xml |
| `qedl reboot` | 重启设备 |

## ⚙️ Configuration

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `--port` | 自动检测 | 串口名称（COM3, /dev/ttyUSB0） |
| `--serial` | — | 按序列号筛选设备 |
| `--loader` | — | Sahara loader 文件路径 |
| `--timeout` | 45000 | 串口超时（毫秒） |
| `--dry-run` | false | 仅解析，跳过执行 |
| `-v` / `-vv` | info | debug / trace 日志 |
| `--wait-device` | — | 等待设备出现（可选超时秒数） |
| `--no-switch-edl` | false | 禁止自动 DIAG→EDL 切换 |
| `--force-mode` | — | 强制指定模式（edl/diag） |

## 📁 Project Structure

```text
qedl-rs/
├── qedl-core/       # 核心类型、错误定义、协议常量
├── qedl-transport/  # USB/串口传输抽象
├── qedl-sahara/     # Sahara 握手协议
├── qedl-firehose/   # Firehose XML 命令引擎
├── qedl-storage/    # GPT 解析 + 分区映射
├── qedl-image/      # rawprogram/patch XML + sparse 展开
├── qedl-job/        # 任务编排（flash/dump/erase）
├── qedl/            # 统一 SDK 门面
├── qedl-cli/        # CLI 二进制
├── docs/            # 架构文档
└── README.md
```

## 💻 Development
[贡献指南](https://github.com/baiyao105/qedl-rs?tab=contributing-ov-file)

## 🤝 Contributing

[![Ask zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=for-the-badge&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/baiyao105/qedl-rs) [![Ask DeepWiki](https://img.shields.io/badge/Ask_DeepWiki-blue.svg?style=for-the-badge&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACwAAAAyCAYAAAAnWDnqAAAAAXNSR0IArs4c6QAAA05JREFUaEPtmUtyEzEQhtWTQyQLHNak2AB7ZnyXZMEjXMGeK/AIi+QuHrMnbChYY7MIh8g01fJoopFb0uhhEqqcbWTp06/uv1saEDv4O3n3dV60RfP947Mm9/SQc0ICFQgzfc4CYZoTPAswgSJCCUJUnAAoRHOAUOcATwbmVLWdGoH//PB8mnKqScAhsD0kYP3j/Yt5LPQe2KvcXmGvRHcDnpxfL2zOYJ1mFwrryWTz0advv1Ut4CJgf5uhDuDj5eUcAUoahrdY/56ebRWeraTjMt/00Sh3UDtjgHtQNHwcRGOC98BJEAEymycmYcWwOprTgcB6VZ5JK5TAJ+fXGLBm3FDAmn6oPPjR4rKCAoJCal2eAiQp2x0vxTPB3ALO2CRkwmDy5WohzBDwSEFKRwPbknEggCPB/imwrycgxX2NzoMCHhPkDwqYMr9tRcP5qNrMZHkVnOjRMWwLCcr8ohBVb1OMjxLwGCvjTikrsBOiA6fNyCrm8V1rP93iVPpwaE+gO0SsWmPiXB+jikdf6SizrT5qKasx5j8ABbHpFTx+vFXp9EnYQmLx02h1QTTrl6eDqxLnGjporxl3NL3agEvXdT0WmEost648sQOYAeJS9Q7bfUVoMGnjo4AZdUMQku50McDcMWcBPvr0SzbTAFDfvJqwLzgxwATnCgnp4wDl6Aa+Ax283gghmj+vj7feE2KBBRMW3FzOpLOADl0Isb5587h/U4gGvkt5v60Z1VLG8BhYjbzRwyQZemwAd6cCR5/XFWLYZRIMpX39AR0tjaGGiGzLVyhse5C9RKC6ai42ppWPKiBagOvaYk8lO7DajerabOZP46Lby5wKjw1HCRx7p9sVMOWGzb/vA1hwiWc6jm3MvQDTogQkiqIhJV0nBQBTU+3okKCFDy9WwferkHjtxib7t3xIUQtHxnIwtx4mpg26/HfwVNVDb4oI9RHmx5WGelRVlrtiw43zboCLaxv46AZeB3IlTkwouebTr1y2NjSpHz68WNFjHvupy3q8TFn3Hos2IAk4Ju5dCo8B3wP7VPr/FGaKiG+T+v+TQqIrOqMTL1VdWV1DdmcbO8KXBz6esmYWYKPwDL5b5FA1a0hwapHiom0r/cKaoqr+27/XcrS5UwSMbQAAAABJRU5ErkJggg==)](https://deepwiki.com/baiyao105/qedl-rs)

感谢所有为 qedl-rs 做出贡献的人！

[![Contributors](https://contrib.nn.ci/api?repo=baiyao105/qedl-rs)](https://github.com/baiyao105/qedl-rs/graphs/contributors)
![Alt](https://repobeats.axiom.co/api/embed/171ce892b4d905c10fe3798defa216d44b21eb67.svg "Repobeats analytics image")

## 🙏 Acknowledgments

- [serialport-rs](https://github.com/serialport/serialport-rs) — Rust 串口库
- [rusb](https://github.com/a1ien/rusb) — Rust USB 库
