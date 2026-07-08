<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-sahara

Qualcomm Sahara 握手协议实现。

</div>

---

## 📖 About

实现 Sahara 二进制协议，在设备初始握手阶段传输 Firehose loader 并将设备切换到 Firehose 模式。

## ✨ Features

- 读取并解析 Sahara Hello 包
- 协议版本协商
- 分块传输 Firehose loader（ReadData/ReadData64）
- PblHack 恢复（设备未发送 Hello 时）
- 查询设备信息（MSM HW ID、序列号）
- 检测设备是否已在 Firehose 模式
- Done 请求完成握手

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出
├── session.rs      # SaharaSession — 握手状态机
├── protocol.rs     # SaharaHello, 常量
└── error.rs        # SaharaError
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `SaharaSession<T>` | 握手状态机，泛型于 Transport |
| `SaharaHello` | 设备发送的 Hello 包 |
| `SaharaHelloResponse` | 响应包 |
| `SaharaDeviceInfo` | MSM HW ID 和序列号 |

## ⚡ Workflow

```
SaharaSession::handshake()
    │
    ├─ read_hello()
    │   └─ 超时 → pbl_hack() → read_hello()
    │
    ├─ send_hello_response()
    │
    ├─ [可选] exec_cmd(MSM_HW_ID_READ)
    ├─ [可选] exec_cmd(SERIAL_NUM_READ)
    │
    ├─ upload_loader()
    │   └─ loop: read ReadData → send_loader_chunk()
    │
    ├─ send_done()
    │
    └─ 设备进入 Firehose 模式
```

## 🔗 Dependencies

依赖：qedl-core（SaharaCommand, EventSink）、qedl-transport（Transport）。
被使用：qedl-job、qedl（可选 feature）。

## 💡 Design Notes

- PblHack：设备未发送 Hello 时，直接发 HelloResponse，等 CMD_READY，再 ModeSwitch
- Firehose 检测：发送 NOP XML 检查设备是否已在 Firehose 模式
- 设备信息查询失败为非致命错误
- 状态机：WaitingHello → HelloReceived → Transferring → Done
