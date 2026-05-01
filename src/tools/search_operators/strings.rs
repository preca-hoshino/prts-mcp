//! `search_operators` 工具的文本常量定义。
//!
//! 将字符串字面量集中管理，使 `tool.rs` 中的业务逻辑与文本内容解耦。

// ── 错误 / 警告消息模板 ──────────────────────────────────────────────────────

/// 搜索关键词为空的错误提示。
pub const ERR_QUERY_EMPTY: &str = "❌ 搜索关键词不能为空。";

/// 搜索无匹配结果的提示模板（`{query}` 替换为用户输入的关键词）。
pub const ERR_SEARCH_NO_MATCH: &str = concat!(
    "未找到与「{query}」相关的干员。\n\n",
    "建议检查关键词拼写，或尝试更短的关键词。"
);

/// 分类验证无匹配的提示模板（`{query}` 替换为用户输入的关键词）。
pub const ERR_CATEGORY_NO_MATCH: &str = concat!(
    "搜索「{query}」未找到符合条件的干员页面。\n\n",
    "建议：尝试更精确的干员名称，或使用中文关键词。"
);

/// 过滤后交集为空的警告模板。
/// `{query}` = 搜索关键词，`{count}` = 未过滤前命中数，`{filter}` = 筛选条件描述。
pub const WARN_FILTER_NO_MATCH: &str = concat!(
    "⚠️ 搜索「{query}」找到 {count} 名干员，但无人满足筛选条件（{filter}）。\n\n",
    "建议放宽或移除筛选参数后重试。"
);

// ── 输出格式化模板 ──────────────────────────────────────────────────────────

/// 过滤模式下的标题模板：`{query}` = 搜索关键词，`{count}` = 命中数。
pub const FMT_TITLE_FILTERED: &str = "## 搜索「{query}」并筛选后，共找到 {count} 名干员\n";

/// 纯搜索模式下的标题模板：`{query}` = 搜索关键词，`{count}` = 命中数。
pub const FMT_TITLE_SEARCH: &str = "## 搜索「{query}」共找到 {count} 名干员\n";

/// 结果可能超限的提示模板：`{limit}` = 上限值。
pub const FMT_LIMIT_HINT: &str =
    "> 结果已达上限 {limit} 条，可能存在更多匹配干员。建议使用更精确的关键词缩小范围。";

/// 底部提示：引导用户使用 `get_operator` 获取详细信息。
pub const FMT_GET_OPERATOR_HINT: &str =
    "> 提示：使用 `get_operator` 工具并传入干员中文名，可获取详细的技能、属性、档案等数据。";

/// 过滤条件中「无筛选」的占位文本。
pub const FILTER_HINT_NONE: &str = "无";

/// 过滤条件中单个条件的格式化模板：`{k}` = 条件名，`{v}` = 条件值。
pub const FMT_FILTER_ITEM: &str = "{k}={v}";

/// 过滤条件中获取途径含模糊匹配的格式化模板：`{v}` = 条件值。
pub const FMT_FILTER_OBTAIN: &str = "获取途径含「{v}」";
