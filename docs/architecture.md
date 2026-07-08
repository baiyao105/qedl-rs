<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# Architecture

qedl-rs 架构设计与外部调用指南。

</div>

---

## 📖 概览

qedl-rs 是一个分层架构的 Qualcomm EDL 协议栈，从底层 I/O 到高层 SDK 逐层抽象：

```
┌─────────────────────────────────────────┐
│              qedl-cli (CLI)             │
├─────────────────────────────────────────┤
│             qedl (SDK 门面)             │
├─────────────────────────────────────────┤
│           qedl-job (任务编排)            │
├──────────┬──────────┬───────────────────┤
│ qedl-    │ qedl-    │ qedl-             │
│ sahara   │ firehose │ storage / image   │
├──────────┴──────────┴───────────────────┤
│         qedl-transport (I/O)            │
├─────────────────────────────────────────┤
│           qedl-core (类型)              │
└─────────────────────────────────────────┘
```

## 📦 Crate 依赖关系

```
qedl-core ← qedl-transport ← qedl-sahara
                    ↑              ↑
              qedl-firehose       │
                    ↑              │
              qedl-storage        │
              qedl-image          │
                    ↑              │
                qedl-job ─────────┘
                    ↑
                  qedl
                    ↑
                qedl-cli
```

## 🔄 协议流程

### 完整初始化流程

```
USB 设备 (VID=0x05C6)
    │
    ├─ PID=0x9008 → EDL 模式
    │   └─ 直接进入 Sahara 握手
    │
    ├─ PID=0x90B8 → DIAG 模式
    │   └─ DIAG 子系统命令切换到 EDL
    │       └─ 等待重新枚举为 0x9008
    │
    ▼
Sahara 握手
    │
    ├─ 读 Hello（1s 超时）
    │   ├─ 超时 → PblHack 恢复
    │   └─ 已在 Firehose → 跳过
    │
    ├─ 发 HelloResponse（版本协商）
    │
    ├─ [可选] 查询 MSM HW ID、序列号
    │
    ├─ 上传 Firehose loader（分块传输）
    │
    ├─ 发 Done
    │
    ▼
Firehose 配置
    │
    ├─ 发 <configure> XML
    ├─ 解析：sector_size, max_payload, memory_name
    │
    ▼
GPT 加载
    │
    ├─ 读 LBA 1（primary GPT header）
    ├─ 读分区条目
    ├─ [UFS] 扫描 LUN 0-3
    │
    ▼
设备就绪，可以执行操作
```

### 操作流程

```
读扇区：
    send <read> XML → 接收 raw data → 解析完成响应

写扇区：
    send <program> XML → 等 ACK → 发送 raw data → 等完成响应

擦除：
    send <erase> XML → 等 ACK
    或 write-zero：循环 write_sectors(零数据)

peek/poke：
    send <peek>/<poke> XML → 解析响应
```

## 🔌 外部调用指南

### SDK 集成

```rust
use qedl::QedlClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = QedlClient::builder()
        .port("COM3")
        .loader("prog_firehose.mbn")
        .timeout(std::time::Duration::from_secs(30))
        .build();

    // 完整初始化
    client.init().await?;

    // 或分步初始化
    // client.connect()?;
    // client.handshake().await?;
    // client.init_firehose().await?;
    // client.load_gpt().await?;

    // 执行操作
    let partitions = client.partitions();
    client.dump("boot", "boot.img").await?;
    client.write("boot", std::path::Path::new("boot.img")).await?;
    client.erase("userdata", true, qedl::EraseMethod::WriteZero).await?;

    // 重启
    client.reboot().await?;
    Ok(())
}
```

### GUI 集成（Tauri 示例）

```rust
use qedl::{QedlClient, EventSink, Event};
use std::sync::Arc;

struct TauriEventSink {
    // tauri::AppHandle
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: Event) {
        match event {
            Event::Progress { current, total, message } => {
                // 发送到前端
            }
            Event::Sahara(sahara_event) => {
                // 处理 Sahara 事件
            }
            Event::Firehose(fh_event) => {
                // 处理 Firehose 事件
            }
            _ => {}
        }
    }
}

let client = QedlClient::builder()
    .port("COM3")
    .event_sink(Arc::new(TauriEventSink { /* ... */ }))
    .build();
```

### Mock 测试

```rust
use qedl::Transport;
use qedl_transport::MockTransport;

#[tokio::test]
async fn test_dump() {
    let mock = MockTransport::new();
    // 设置 mock 响应...
    // 测试 job 执行
}
```

## 🏗️ 设计要点

### 错误处理

- 统一 `QedlError` 类型，`ErrorCode` 枚举用于机器识别
- 每个 crate 有自己的错误类型，在 `qedl` 层统一转换
- `color-eyre` 提供带上下文的错误报告

### 事件系统

- `EventSink` trait 支持 GUI 集成
- 事件类型：State、Progress、Error、Sahara、Firehose、Job
- 默认 `NoopEventSink` 不影响性能

### Feature Gates

| Feature | 包含内容 |
|---------|----------|
| `full` | 所有功能 |
| `sahara` | qedl-sahara + Sahara 握手 |
| `sparse` | qedl-image + sparse 镜像 + flash/verify |
| `gui` | GUI 事件类型 |
| `minimal` | 仅核心功能 |

### 传输抽象

- `Transport` trait：async read/write/flush + timeout
- `SerialTransport`：串口实现
- `MockTransport`：内存测试实现
- 可扩展：网络传输、USB-IP 等

### 异步架构

- 全栈 async/await（tokio runtime）
- `async-trait` 用于 trait 方法
- 非阻塞 I/O + 超时控制

## 📊 性能特征

- 串口通信：115200 波特率，实际吞吐约 10-15 KB/s
- 分块传输：max_payload 大小在 configure 阶段协商
- 静态零缓冲区：避免 write-zero 擦除的重复分配
- leftover buffer：处理混合 XML+raw 响应，避免数据丢失
