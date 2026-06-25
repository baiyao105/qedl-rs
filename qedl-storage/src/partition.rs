//! 分区映射
//!
//! 管理多 LUN 的 GPT 分区表，提供统一的分区查找接口。

use crate::gpt::{GptEntry, GptTable};
use std::collections::HashMap;

/// 统一分区映射
///
/// 支持 UFS 多 LUN 场景，每个 LUN 独立读取 GPT，合并为统一 map。
/// 使用 HashMap 优化查找性能。
pub struct PartitionMap {
    /// 每个 LUN 的 GPT 表
    tables: Vec<GptTable>,
    /// 分区名到 (table_idx, entry_idx) 的索引，用于 O(1) 查找
    name_index: HashMap<String, (usize, usize)>,
}

impl PartitionMap {
    pub fn new() -> Self {
        Self {
            tables: Vec::new(),
            name_index: HashMap::new(),
        }
    }

    /// 添加一个 LUN 的 GPT 表
    pub fn add_table(&mut self, table: GptTable) {
        let table_idx = self.tables.len();
        for (entry_idx, entry) in table.entries.iter().enumerate() {
            let name = entry.name.trim().trim_matches('\0').trim().to_string();
            if !name.is_empty() {
                self.name_index.entry(name).or_insert((table_idx, entry_idx));
            }
        }
        self.tables.push(table);
    }

    /// 按名称查找分区（跨所有 LUN）
    pub fn find_partition(&self, name: &str) -> Option<&GptEntry> {
        // Use index for O(1) lookup
        if let Some(&(table_idx, entry_idx)) = self.name_index.get(name) {
            return self.tables.get(table_idx).and_then(|t| t.entries.get(entry_idx));
        }

        // Fallback to linear search if not in index (e.g., partial name match)
        for table in &self.tables {
            if let Some(entry) = table.find_partition(name) {
                return Some(entry);
            }
        }
        None
    }

    /// 列出所有分区（跨所有 LUN）
    pub fn all_entries(&self) -> Vec<&GptEntry> {
        self.tables.iter().flat_map(|t| t.entries.iter()).collect()
    }

    /// 按 LUN 获取分区列表
    pub fn entries_for_lun(&self, lun: u8) -> Vec<&GptEntry> {
        self.tables
            .iter()
            .filter(|t| t.physical_partition == lun)
            .flat_map(|t| t.entries.iter())
            .collect()
    }

    /// 获取所有 LUN 编号
    pub fn luns(&self) -> Vec<u8> {
        self.tables.iter().map(|t| t.physical_partition).collect()
    }

    /// 分区总数
    pub fn total_partitions(&self) -> usize {
        self.tables.iter().map(|t| t.entries.len()).sum()
    }
}

impl Default for PartitionMap {
    fn default() -> Self {
        Self::new()
    }
}
