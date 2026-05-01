---
description: "Use when adding user-facing strings, error messages, or text constants. Covers centralized string management, template patterns, Chinese UI text conventions, and constants organization."
applyTo:
  - src/**/*.rs
---

# 字符串与文案管理规范

## 集中管理原则

所有用户可见的文本常量（错误消息、字段名、标签）**必须**集中定义在对应模块的 `strings.rs` 文件中，而非散落在业务逻辑代码中。

示例：`src/tools/get_operator/strings.rs`

## 常量分类与组织

在 `strings.rs` 中按用途分组，使用分隔注释：

```rust
//! `get_operator` 工具的所有文本常量定义。
//!
//! 将字符串字面量集中管理，使 `tool.rs` 中的业务逻辑与文本内容解耦。

// ── 合法 category 值 ─────────────────────────────────────────────────────────

/// 工具接受的所有合法 `category` 标识符（经 `to_uppercase()` 后匹配）。
pub const VALID_CATEGORIES: &[&str] = &[
    "BASIC", "COMBAT", "BUILD", "LORE", "GALLERY", "VOICE", "ALL",
];

// ── Wiki 字段键名 ─────────────────────────────────────────────────────────────

/// wikitext 中存储干员中文名的字段键。
pub const FIELD_CHAR_NAME: &str = "干员名";

// ── 错误信息模板 ──────────────────────────────────────────────────────────────

/// 无效 `category` 参数的错误提示模板（`{category}` 替换为用户传入的值）。
pub const ERR_INVALID_CATEGORY: &str = concat!(
    "❌ 错误：无效的 `category` 参数「{category}」。\n",
    "请使用以下任一标准值（大小写不敏感）：\n",
    "BASIC / COMBAT / BUILD / LORE / GALLERY / VOICE / ALL"
);
```

## 命名约定

| 前缀 | 用途 | 示例 |
|------|------|------|
| `FIELD_` | Wiki 字段键名 | `FIELD_CHAR_NAME`, `FIELD_FOREIGN_NAME` |
| `ERR_` | 错误信息模板 | `ERR_INVALID_CATEGORY`, `ERR_OPERATOR_NOT_FOUND` |
| `VALID_` | 合法值列表 | `VALID_CATEGORIES` |
| `_PAGE_SUFFIX` | URL/页面后缀 | `VOICE_PAGE_SUFFIX` |
| `_API` | API URL 常量 | `OPENSEARCH_API`, `FULLTEXT_SEARCH_API` |
| `_CATEGORY` | 分类标签 | `OPERATOR_CATEGORY` |
| `_BASE` | URL 前缀 | `PRTS_PAGE_BASE` |

## 模板字符串模式

使用 `.replace("{placeholder}", value)` 模式进行参数替换：

```rust
// 定义模板
pub const ERR_INVALID_CATEGORY: &str = "❌ 错误：无效的 `category` 参数「{category}」。";

// 使用模板
CallToolResult::with_error(CallToolError::from_message(
    ERR_INVALID_CATEGORY.replace("{category}", &self.category),
))
```

对于长文本模板，使用 `concat!()` 宏拼接多行：

```rust
pub const ERR_OPERATOR_NOT_FOUND: &str = concat!(
    "🔍 未找到干员「{name}」的 Wiki 页面，请检查名称是否正确。\n",
    "示例正确名称：「能天使」、「陈」、「Mon3tr」。"
);
```

## 中文文案规则

- 所有用户可见文本使用**中文**
- **推荐**在错误消息中使用 emoji 前缀增强可读性（`❌` = 参数错误，`🔍` = 未找到），但不强制
- 代码标识符两侧使用反引号（如 `` `category` ``）
- 干员名示例使用「」书名号包裹
- 工具 description 中的 `<when_to_use>` 等标签使用英文 XML 标签，内容使用中文
