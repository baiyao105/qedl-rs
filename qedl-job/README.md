<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-job

任务编排：flash / dump / erase / verify / info。

</div>

---

## 📖 About

提供高层任务执行框架，协调 Firehose I/O、分区查找、进度上报和错误处理。每个操作封装为 `Job` trait 实现。

## ✨ Features

- `Job` trait 抽象可执行操作
- `JobContext` trait 抽象设备 I/O（可测试）
- 完整设备生命周期管理（connect → handshake → configure → load GPT）
- 分块读写 + 进度上报
- dump 断点续传
- sparse 镜像自动展开
- SHA256 设备端校验
- rawprogram.xml + patch.xml 刷机流程

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出
├── executor.rs     # JobExecutor, ExecutorConfig
├── context.rs      # JobContext trait
├── jobs.rs         # DumpJob, WriteJob, EraseJob, FlashJob 等
├── reader.rs       # ChunkedReader（流式文件读取）
├── error.rs        # JobError
└── testutil.rs     # 测试工具
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `JobExecutor` | 设备生命周期管理 |
| `ExecutorConfig` | 配置（端口、loader、超时等） |
| `JobContext` | I/O 抽象 trait |
| `Job` | 可执行操作 trait |
| `DumpJob` | 导出分区（支持续传） |
| `WriteJob` | 写入镜像到分区 |
| `EraseJob` | 擦除分区 |
| `FlashJob` | 从 rawprogram.xml 刷机 |
| `VerifyJob` | SHA256 校验 |
| `InfoJob` / `GptJob` | 信息查询 |

## ⚡ Workflow

```
JobExecutor::init()
    │
    ├─ connect()          → DeviceEnumerator
    ├─ handshake()        → SaharaSession（可选）
    ├─ init_firehose()    → FirehoseClient::configure()
    ├─ load_gpt()         → PartitionMap
    │
    ▼
JobExecutor::execute(job)
    │
    ├─ job.validate(ctx)
    └─ job.execute(ctx)
        │
        ▼
    JobContext: read_sectors / write_sectors / find_partition / reboot
```

## 🔗 Dependencies

依赖：qedl-core、qedl-transport、qedl-sahara（可选）、qedl-firehose、qedl-storage、qedl-image（可选）。
被使用：qedl。

## 💡 Design Notes

- `JobContext` 解耦 job 和具体 transport/firehose 类型
- 默认 write-zero 擦除（更安全），native erase 为可选
- 静态 512KB 零缓冲区减少 write-zero 擦除的分配
- dump 续传：检查已有文件大小，从断点恢复
- `sahara` 和 `sparse` feature 为可选，支持最小化构建
