<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-transport

USB/串口传输抽象层。

</div>

---

## 📖 About

提供可插拔的 I/O 层，通过串口与 Qualcomm 设备通信。处理设备发现、模式检测（EDL vs DIAG）和 DIAG→EDL 切换协议。

## ✨ Features

- 枚举 Qualcomm USB 设备
- 从 USB 接口描述符检测设备模式
- 串口连接管理（115200, 8N2, 无流控）
- DIAG→EDL 模式切换（HDLC 帧 + CRC16）
- `Transport` async trait（read/write/flush）
- MockTransport 用于测试

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出
├── port.rs         # Transport trait 定义
├── serial.rs       # SerialTransport 实现
├── device.rs       # DeviceEnumerator, 模式检测, DIAG→EDL
├── error.rs        # TransportError
└── mock.rs         # MockTransport
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `Transport` | 异步传输 trait |
| `SerialTransport` | 串口实现 |
| `DeviceEnumerator` | 设备发现和模式切换 |
| `DeviceInfo` | 设备信息 |
| `MockTransport` | 测试用内存传输 |

## ⚡ Workflow

```
DeviceEnumerator::auto_select()
    │
    ▼
serialport::available_ports()
    │
    ▼
query_per_interface_modes()  ← rusb USB 描述符
    │
    ▼
DeviceInfo { port, serial, mode }
    │
    ▼
SerialTransport::open()
    │
    ▼
Transport trait (read/write/flush)
```

## 🔗 Dependencies

依赖：qedl-core（类型、DeviceMode）。
被使用：qedl-sahara、qedl-firehose、qedl-job、qedl。

## 💡 Design Notes

- 串口：115200 波特率，8 数据位，2 停止位，无校验，无流控
- 模式检测优先读 USB 接口描述符，PID 启发式作为后备
- DIAG→EDL 切换尝试 115200 和 921600 两种波特率
- `trace-transport` feature 启用原始 TX/RX 日志

## 🔧 Extension Points

- 新传输后端：实现 `Transport` trait
- 自定义设备选择：实现 `DeviceEnumeratorTrait`
