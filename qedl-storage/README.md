<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-storage

GPT 分区表解析 + 多 LUN 分区映射。

</div>

---

## 📖 About

解析 GPT 数据并提供统一的分区查找接口，支持 eMMC 和 UFS 多 LUN 场景。

## ✨ Features

- 解析 GPT header（LBA 1 primary / backup）
- CRC32 校验
- UTF-16LE 分区名解码
- 多 LUN GPT（UFS: LUN 0-3）
- O(1) 分区名查找（HashMap 索引）

## 📁 Structure

```text
src/
├── lib.rs          # 公开导出
├── gpt.rs          # GptHeader, GptEntry, GptTable
├── partition.rs    # PartitionMap — 多 LUN 统一映射
└── error.rs        # StorageError
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `GptTable` | 解析后的 GPT 表 |
| `GptEntry` | 单个分区条目 |
| `PartitionMap` | 多 LUN 分区映射（O(1) 查找） |

## ⚡ Workflow

```
读取 GPT header (LBA 1)
    │
    ▼
GptHeader::from_bytes() → CRC32 校验
    │
    ▼
读取分区条目 → GptEntry × N
    │
    ▼
PartitionMap::add_table()
    │
    ▼
find_partition("boot") → GptEntry
```

## 🔗 Dependencies

依赖：qedl-core（PartitionInfo）。
被使用：qedl-job、qedl。

## 💡 Design Notes

- 分区名从 UTF-16LE 解码（GPT 标准）
- 空条目（零 type GUID）在解析时过滤
- 支持 backup GPT：primary 损坏时回退到 backup LBA
- PartitionMap 用 HashMap 索引实现 O(1) 查找，线性扫描作为 fallback
