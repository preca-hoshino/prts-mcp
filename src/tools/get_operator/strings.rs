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

/// wikitext 中存储干员外文名的字段键。
pub const FIELD_FOREIGN_NAME: &str = "干员外文名";

/// wikitext 中注入到 combat 块的全局字段键列表（职业/分支/特性）。
pub const COMBAT_GLOBAL_FIELDS: &[&str] = &["职业", "分支", "特性"];

/// 语音记录子页面的路径模板（`{}` 替换为干员名）。
pub const VOICE_PAGE_SUFFIX: &str = "/语音记录";

// ── 错误信息模板 ──────────────────────────────────────────────────────────────

/// 无效 `category` 参数的错误提示模板（`{category}` 替换为用户传入的值）。
pub const ERR_INVALID_CATEGORY: &str = concat!(
    "❌ 错误：无效的 `category` 参数「{category}」。\n",
    "请使用以下任一标准值（大小写不敏感）：\n",
    "BASIC / COMBAT / BUILD / LORE / GALLERY / VOICE / ALL"
);

/// 干员 Wiki 页面未找到的错误提示模板（`{name}` 替换为用户传入的干员名）。
pub const ERR_OPERATOR_NOT_FOUND: &str = concat!(
    "🔍 未找到干员「{name}」的 Wiki 页面，请检查名称是否正确。\n",
    "示例正确名称：「能天使」、「陈」、「Mon3tr」。"
);
