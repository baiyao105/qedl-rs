<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-core

核心类型、错误定义和协议常量。

</div>

---

## 📖 About

提供所有 crate 共享的基础类型，确保错误处理、事件上报和协议定义在整个协议栈中保持一致。

## ✨ Features

- 统一的 `QedlError` 错误类型和 `ErrorCode` 机器可读错误码
- 设备状态机、模式枚举、能力描述
- 会话状态管理（Firehose 配置后的设备信息）
- 事件系统（`EventSink` trait），支持 GUI 集成
- `ProgressReporter` trait，进度条集成
- 协议常量（Sahara 命令、Firehose 尺寸）

## 📁 Structure

```text
src/
├── lib.rs              # 公开导出
├── error.rs            # QedlError, ErrorCode
├── event.rs            # Event, EventSink, SaharaEvent, FirehoseEvent
├── session.rs          # Session（配置后的设备状态）
├── state.rs            # DeviceState 枚举
├── types.rs            # DeviceInfo, DeviceMode, PartitionInfo
├── util.rs             # hex_dump, humanize_size
└── protocol/
    ├── mod.rs          # 模块导出
    ├── firehose.rs     # Firehose XML 响应解析
    ├── firehose_types.rs   # FirehoseInfo, FirehoseFunction
    └── sahara.rs       # SaharaCommand, SaharaMode, 常量
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `QedlError` | 统一错误类型 |
| `ErrorCode` | 机器可读错误码 |
| `DeviceMode` | USB 模式（Edl/Diag/Modem/Nmea/Adb/Unknown） |
| `DeviceInfo` | 设备端口、序列号、PID/VID |
| `Session` | Firehose 配置后的设备状态 |
| `PartitionInfo` | 分区名、LBA 范围、物理分区号 |
| `Event` / `EventSink` | 事件系统 |
| `SaharaCommand` | Sahara 协议命令枚举 |
| `FirehoseInfo` | Firehose 能力信息 |

## 🔗 Dependencies

被所有其他 crate 依赖：qedl-transport、qedl-sahara、qedl-firehose、qedl-storage、qedl-image、qedl-job、qedl。

## 💡 Design Notes

- `ErrorCode` 使用字符串标签，便于机器识别
- `EventSink` 支持 Tauri/egui/Iced 等 GUI 集成
- 协议常量使用 `const`，零成本抽象

## 🔧 Extension Points

- 新错误码：添加 `ErrorCode` 变体
- 新事件：添加 `Event`、`SaharaEvent`、`FirehoseEvent` 变体
- 新协议常量：添加到 `protocol/` 目录
