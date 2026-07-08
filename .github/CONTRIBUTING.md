<!-- markdownlint-disable MD033 MD036 MD041 -->

<div align="right">

简体中文

</div>

# 向 `qedl-rs` 贡献

## 反馈

### 反馈 Bug

如果您在使用 qedl-rs 时遇到问题，可在 Issues 中提交 Bug 反馈。提交前请先完成以下检查：

- 确认问题在 [最新 Release 版本](https://github.com/baiyao105/qedl-rs/releases/latest) 和 [主分支最新提交](https://github.com/baiyao105/qedl-rs/commits/master) 中未被修复；
- 确认没有相同或相似的 Issue 已存在（可通过关键词搜索验证）。

Bug 反馈需包含的信息：

- 操作系统及版本（如 Windows 11 23H2、Ubuntu 22.04、macOS Sonoma 14.0）；
- Rust 版本（`rustc --version` 输出）；
- qedl-rs 版本（`cargo install --list` 或 git commit hash）；
- Qualcomm 设备型号及 PID；
- 问题复现步骤（清晰描述操作流程）；
- 实际结果与预期结果；
- 相关截图或日志（可用 `RUST_LOG=trace` 获取详细日志）。

若反馈重复，将被标记为 "duplicate" 并关闭，您可通过 Issue 关联找到原始讨论。

### 提交新功能请求

若您有新功能想法，可在 Issues 中提交功能请求。请确保：

- 功能未在最新版本或提交中实现；
- 无相同或相似的 Issue 存在；
- 功能符合项目核心目标（Qualcomm EDL 设备通信与刷机），且具有广泛适用性。

功能请求建议包含：

- 功能背景（解决什么问题）；
- 具体实现思路（可选）；
- 适用场景及用户群体。

不符合上述要求的请求可能被关闭。

## 贡献代码

### 贡献准则

您贡献的代码需满足：

- **稳定性**：兼容 Windows 7+、Linux（主流发行版）、macOS 10.13+，避免引入平台特异性代码（若无法避免，需通过条件编译 `#[cfg]` 兼容）；
- **通用适用性**：面向多数用户需求，专用性功能建议通过 feature gate 可选引入；
- **异步安全**：使用 async/await，避免阻塞 tokio 运行时；
- **错误处理**：使用项目统一的 `QedlError` 类型，避免 `unwrap()`/`expect()`。

### 提交规范

请遵循 [约定式提交](https://www.conventionalcommits.org/zh-hans) 规范：

```
feat: 添加 XXX 功能
fix: 修复 XXX 问题
docs: 更新文档
refactor: 重构 XXX
test: 添加测试
chore: 构建/工具变更
```

### 分支命名

建议格式：`feat/功能名` 或 `fix/bug描述`

示例：
- `feat/ufs-multi-lun`
- `fix/sahara-timeout`
- `docs/update-readme`

### 发起拉取请求（PR）

1. **环境准备**

   ```bash
   # 安装 Rust 工具链
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # 克隆仓库
   git clone https://github.com/baiyao105/qedl-rs.git
   cd qedl-rs

   # 构建并运行测试
   cargo build
   cargo nextest run
   ```

2. **分支准备**

   ```bash
   git checkout -b feat/my-feature master
   git fetch origin
   git rebase origin/master
   ```

3. **提交前检查**

   ```bash
   # 格式化代码
   cargo fmt

   # Clippy 检查
   cargo clippy -- -D warnings

   # 运行所有测试
   cargo nextest run

   # 构建 release 验证
   cargo build --release
   ```

   检查清单：
   - [ ] `cargo fmt` 无格式变更
   - [ ] `cargo clippy` 无警告
   - [ ] `cargo nextest run` 全部通过
   - [ ] 至少在一个操作系统上验证功能
   - [ ] 新增依赖已添加至对应 `Cargo.toml`
   - [ ] 文档已更新（如适用）

4. **PR 描述**

   - **标题**：简要说明修改（建议与提交信息一致）；
   - **内容**：
     - 修改目的及实现思路；
     - 已测试的操作系统（如 "测试通过：Windows 11、Ubuntu 22.04"）；
     - 关联的 Issue 编号（如 `Closes #123`、`Fixes #456`）。

5. **合并方式**

   - 优先使用 **Rebase** 操作，避免 Merge 提交；
   - 使用描述性标题，避免直接使用 Issue 编号。

### 代码风格

- 遵循 `rustfmt` 默认格式（`cargo fmt`）；
- 使用 `clippy` 检查常见错误；
- 公开 API 需添加文档注释（`///`）；
- 模块级注释使用 `//!`；
- 避免不必要的 `pub` 导出。

### 测试规范

- 单元测试放在对应模块的 `tests/` 目录；
- 使用 `MockTransport` 进行传输层测试；
- 测试文件命名：`test_功能名.rs` 或 `功能名_test.rs`；
- 使用 `pretty_assertions` 增强断言输出；
- 使用 `tempfile` 处理临时文件。

## 致谢

感谢所有为 qedl-rs 做出贡献的人！

<!-- PLACEHOLDER_CONTRIBUTORS_LIST -->
