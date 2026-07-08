<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-firehose

Qualcomm Firehose XML 命令引擎。

</div>

---

## 📖 About

实现 Firehose XML 协议，提供扇区级读写擦除、内存 peek/poke、存储信息查询和 SHA256 校验。

## ✨ Features

- 配置 Firehose 协议参数（扇区大小、内存类型、最大载荷）
- 执行 XML 命令并解析 ACK/NAK 响应
- 扇区读取（leftover buffer 处理混合 XML+raw 响应）
- 扇区编程（写入）
- 扇区擦除
- 存储信息查询
- SHA256 摘要计算
- 物理内存 peek/poke
- 设备重启
- Sahara 握手后的初始化消息排空

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出 + FirehoseProtocol trait
├── client.rs       # FirehoseClient — 主实现
├── command.rs      # FirehoseCommand 枚举（XML 序列化）
├── response.rs     # FirehoseResponse 解析
└── error.rs        # FirehoseError
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `FirehoseClient` | Firehose 协议主实现 |
| `FirehoseCommand` | 所有 XML 命令的枚举 |
| `FirehoseResponse` | 解析后的响应 |
| `FirehoseProtocol` | 异步 trait，抽象 Firehose 操作 |

## ⚡ Workflow

```
FirehoseClient::configure()
    │
    ├─ 发送 <configure> XML
    ├─ 解析响应（sector_size, max_payload 等）
    │
    ▼
read_sectors() / program_sectors() / erase_sectors()
    │
    ├─ 发送命令 XML
    ├─ [读] 接收 raw 数据 + 完成响应
    ├─ [写] ACK 后发送数据
    │
    ▼
ACK/NAK 响应解析
```

## 🔗 Dependencies

依赖：qedl-core（EventSink）、qedl-transport（Transport）、quick-xml。
被使用：qedl-job、qedl。

## 💡 Design Notes

- leftover buffer 处理 MSM8937 等设备的混合 XML+raw 数据
- `trace-xml` feature 启用完整 XML 日志
- `FirehoseProtocol` trait 支持 mock 测试
- 扇区大小和最大载荷在 configure 阶段协商
- 读操作后必须消费完成响应，防止后续命令数据错乱

## 🔧 Extension Points

- 新命令：添加 `FirehoseCommand` 变体
- 替代实现：实现 `FirehoseProtocol` trait
