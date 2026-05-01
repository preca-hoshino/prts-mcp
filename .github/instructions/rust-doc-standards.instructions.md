---
description: "Use when adding or updating documentation comments in Rust source files. Covers module docs, function docs, Errors sections, comment language conventions, and JS reference patterns."
applyTo:
  - src/**/*.rs
---

# Rust 文档注释规范

## 模块级文档 `//!`

每个 `.rs` 文件必须以 `//!` 模块文档开头，说明该模块的职责：

```rust
//! Wikitext 行语义分类器。
//!
//! 将整页 wikitext 按内容类型路由到 5 个语义桶，对应 JS `index.js:categorizeLines()`。
```

```rust
//! `get_operator` 工具的所有文本常量定义。
//!
//! 将字符串字面量集中管理，使 `tool.rs` 中的业务逻辑与文本内容解耦。
```

## 函数级文档 `///`

所有 `pub fn` 必须有 `///` 文档注释：

```rust
/// 按语义将 wikitext 的各行路由到对应的语义桶。
///
/// 对应 JS `index.js:categorizeLines()`。
#[must_use]
pub fn categorize_lines(wikitext: &str) -> WikiBlocks {
    // ...
}
```

### `# Errors` 节

对于返回 `Result` 的函数，必须包含 `# Errors` 节说明所有错误情况：

```rust
/// 执行 Tool 逻辑：拉取 wikitext → 分块 → 路由到对应解析器 → 返回 Markdown。
///
/// # Errors
/// 若网络请求失败，返回协议级 `CallToolError`。
/// 若干员不存在或 `category` 非法，返回带 `isError: true` 的工具执行错误。
pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
```

## 字段文档 `///`

结构体字段必须有文档注释：

```rust
pub struct WikiBlocks {
    /// 基础干员信息行（CharinfoV2 / 人员档案set 等）。
    pub basic: Vec<String>,
    /// 战斗数据行（属性、天赋、技能、模组、召唤物）。
    pub combat: Vec<String>,
}
```

## 注释语言约定

| 内容类型 | 语言 | 说明 |
|---------|------|------|
| 模块/函数文档说明 | 中文 | 业务逻辑说明 |
| `# Errors` 节 | 中文 | 与整体文档保持一致 |
| 技术术语（Trait、Struct、Regex） | 英文 | 保留 Rust 标准术语 |
| JS 对应关系标注 | 中英混合 | 格式：`对应 JS \`file.js:funcName()\`` |
| 行内注释 | 中文 | 复杂逻辑的解释 |

## 内部文件的 lint 豁免

内部解析器文件（`basic.rs`、`combat.rs` 等）由于包含辅助逻辑和 `.expect()` 构建正则，文件开头允许：

```rust
#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
```

这个 `#![allow]` 列表是**该模块允许的默认豁免集合**。如需新增其他 `allow` 宏，必须在 PR 中说明原因并经评审同意。
