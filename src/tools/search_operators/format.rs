#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `search_operators` 的输出格式化。
//!
//! 将干员名列表组装为 Markdown 输出文本。

use super::api::PRTS_PAGE_BASE;
use super::strings::{FMT_GET_OPERATOR_HINT, FMT_LIMIT_HINT, FMT_TITLE_FILTERED, FMT_TITLE_SEARCH};

/// 将干员名列表格式化为 Markdown 输出。
///
/// `filtered` 为 `true` 时使用过滤模式标题，否则使用纯搜索模式标题。
pub(crate) fn format_result(
    query: &str,
    operators: &[String],
    limit: usize,
    filtered: bool,
) -> String {
    let count = operators.len();
    let mut lines: Vec<String> = Vec::new();

    if filtered {
        lines.push(
            FMT_TITLE_FILTERED
                .replace("{query}", query)
                .replace("{count}", &count.to_string()),
        );
    } else {
        lines.push(
            FMT_TITLE_SEARCH
                .replace("{query}", query)
                .replace("{count}", &count.to_string()),
        );
    }

    for name in operators {
        let url = format!("{PRTS_PAGE_BASE}{}", urlencoding::encode(name));
        lines.push(format!("- **{name}** — [PRTS Wiki 页面]({url})"));
    }

    if count >= limit {
        lines.push(String::new());
        lines.push(FMT_LIMIT_HINT.replace("{limit}", &limit.to_string()));
    }

    lines.push(String::new());
    lines.push(FMT_GET_OPERATOR_HINT.to_string());

    lines.join("\n")
}
