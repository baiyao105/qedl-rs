<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl

Qualcomm EDL 高层 SDK 门面。

</div>

---

## 📖 About

提供 `QedlClient` 单一入口，封装底层 crate（transport、sahara、firehose、storage、image、job），提供 Builder 模式 API。

## ✨ Features

- Builder 模式配置
- 完整初始化序列抽象（connect → handshake → configure → load GPT）
- 分区操作（dump / write / erase / flash / verify）
- 设备查询（info / GPT / peek / poke / reboot）
- Feature gate 可选模块（sahara、sparse、gui）

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出, feature gates
├── client.rs       # QedlClient, QedlClientBuilder, QedlClientTrait
└── error.rs        # QedlFacadeError
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `QedlClient` | 高层客户端 |
| `QedlClientBuilder` | Builder 配置 |
| `QedlClientTrait` | 客户端接口 trait |

### Feature Flags

| Feature | 说明 |
|---------|------|
| `full` | 启用所有功能（默认） |
| `sahara` | Sahara 握手 |
| `sparse` | sparse 镜像 + flash/verify |
| `gui` | GUI 事件类型 |
| `minimal` | 无可选功能 |

## ⚡ Workflow

```
QedlClient::builder()
    .port("COM3")
    .loader("prog_firehose.mbn")
    .build()
    │
    ▼
client.init()
    │
    ├─ connect() → 串口打开
    ├─ handshake() → Sahara loader 上传
    ├─ init_firehose() → Firehose 配置
    └─ load_gpt() → 分区加载
    │
    ▼
client.dump() / .write() / .erase() / .flash()
    │
    ▼
client.reboot()
```

## 🔗 Dependencies

依赖： qedl-core、qedl-transport、qedl-firehose、qedl-storage、qedl-job、qedl-sahara（可选）、qedl-image（可选）。
被使用：qedl-cli。

## 💡 Design Notes

- Builder 模式 + `QedlClientTrait` 支持 mock 测试
- Feature gate 允许不含 Sahara/sparse 的最小构建
- 所有子 crate 类型统一 re-export
