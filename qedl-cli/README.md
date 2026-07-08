<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-cli

Qualcomm 9008 EDL 模式命令行工具。

</div>

---

## 📖 About

`qedl` 命令行二进制，支持设备列举、分区操作、固件刷入、内存读写、原始 XML 命令等。

## ✨ Features

- clap 参数解析（子命令 + 全局选项）
- 设备列举（EDL + DIAG 模式）
- 设备插拔实时监控（watch 模式）
- 彩色输出 + 进度条（indicatif）
- 断点续传 dump
- SHA256 校验
- JSON 输出（devices 命令）

## 📁 Structure

```text
src/
├── main.rs         # 入口, 命令分发
├── args.rs         # CLI 参数定义
├── devices.rs      # 设备列举和 watch
└── output.rs       # 彩色输出, spinner, 进度条, hex dump
build.rs            # 构建时版本号
```

## 📚 Commands

| 命令 | 说明 |
|------|------|
| `qedl devices` | 列出 EDL/DIAG 设备 |
| `qedl info` | 设备存储信息 |
| `qedl gpt` | GPT 分区表 |
| `qedl dump <part> <file>` | 导出分区 |
| `qedl write <part> <file>` | 写入分区 |
| `qedl erase <part>` | 擦除分区 |
| `qedl flash <rawprogram> [patch]` | 从 XML 刷机 |
| `qedl verify <part> <file>` | SHA256 校验 |
| `qedl peek <addr> <size>` | 读物理内存 |
| `qedl poke <addr> <data>` | 写物理内存 |
| `qedl xml <xml>` | 原始 XML |
| `qedl genxml <output>` | 从 GPT 生成 rawprogram.xml |
| `qedl reboot` | 重启 |

## ⚡ Workflow

```
main()
    │
    ├─ 解析 CLI 参数
    ├─ 初始化 tracing
    │
    ├─ [devices] → devices::run_devices()
    ├─ [--wait-device] → wait_for_device()
    │
    ├─ 构建 QedlClient
    │
    └─ run(command, client)
        ├─ [info/gpt/dump/write/erase/flash/verify] → client.init() + 操作
        └─ [peek/poke/xml/reboot] → client.init_firehose_only() + 操作
```

## 🔗 Dependencies

依赖：qedl（full feature）、clap、tracing、indicatif、color-eyre、owo-colors、rusb、serde_json。

## 💡 Design Notes

- `color-eyre` 错误报告 + tracing-indicatif spinner 协调
- 进度条用 `indicatif` 显示字节级进度
- 设备列举支持 EDL（9008）和 DIAG 模式
- watch 模式轮询设备变化
- build.rs 嵌入 Cargo.toml 版本 + git commit
