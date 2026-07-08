<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="center">

# qedl-image

rawprogram/patch XML 解析 + Android sparse 镜像展开。

</div>

---

## 📖 About

解析 Qualcomm 刷机布局文件（rawprogram.xml, patch.xml）并处理 Android sparse 镜像展开。

## ✨ Features

- 解析 `rawprogram.xml`（program/erase 条目）
- 解析 `patch.xml`（字节级扇区补丁）
- 展开 Android sparse 镜像（magic 0xED26FF3A）
- MD5/SHA256/CRC32 校验
- 校验引用的镜像文件是否存在

## 📁 Structure

```text
src/
├── lib.rs              # 公开导出
├── rawprogram.rs       # TaskList, TaskEntry
├── patch.rs            # PatchSet, PatchEntry
├── sparse.rs           # SparseExpander, ChunkHeader
├── checksum.rs         # 校验和计算
└── error.rs            # ImageError
```

## 📚 API

| 类型 | 说明 |
|------|------|
| `TaskList` | 解析后的 rawprogram.xml |
| `TaskEntry` | 单个刷机任务（扇区范围、文件名、sparse 标志） |
| `PatchSet` | 解析后的 patch.xml |
| `SparseExpander` | 流式 sparse 镜像展开 |

## ⚡ Workflow

```
rawprogram.xml
    │
    ▼
TaskList::from_file() → TaskEntry × N
    │
    ▼
TaskEntry { start_sector, num_sectors, filename, sparse }

sparse 镜像 (magic 0xED26FF3A)
    │
    ▼
SparseExpander::for_each_chunk() → callback
    │
    ▼
expand_to_vec() → raw bytes
```

## 🔗 Dependencies

依赖：qedl-core（util）、quick-xml、sha2、md5、crc32fast。
被使用：qedl-job、qedl（可选 feature）。

## 💡 Design Notes

- sparse 展开通过 `for_each_chunk()` 回调实现流式处理
- patch 值支持变量替换（NUM_DISK_SECTORS, LAST_PARTITION_END）
- TaskList 在执行前校验文件是否存在
- 校验和支持内存和文件流式计算
