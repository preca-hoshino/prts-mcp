#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! Wikitext 文本清洗与动态变量提取工具函数。

use regex::Regex;
use std::sync::LazyLock;

// ─── 预编译正则集 ──────────────────────────────────────────────

static REX_CMT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").expect("regex CMT"));

static REX_COLOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{color\|#?[a-zA-Z0-9]*\|(.*?)\}\}").expect("regex COLOR"));

/// `{{*|text|+text}}` 或 `{{*|text}}` → 取第一参数，若第二参数为 `+{text}` 则取第二参数
static REX_STAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\*\|(.*?)(?:\|(.*?))?\}\}").expect("regex STAR"));

static REX_VAR_LITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\{\{变动数值lite\|.*?\|.*?\|(.*?)\}\}").expect("regex VAR_LITE")
});

/// `[[link|display]]` → display；`[[link]]` → link
static REX_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[(?:[^\]|]*\|)?([^\]]+)\]\]").expect("regex LINK"));

static REX_DRNAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{DrName([^}]*)\}\}").expect("regex DRNAME"));

static REX_DRNAME_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"前缀=([^|}]+)").expect("regex DRNAME_PREFIX"));

static REX_TERM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{术语\|.*?\|(.*?)\}\}").expect("regex TERM"));

static REX_ABNORMAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{异常效果\|(.*?)\}\}").expect("regex ABNORMAL"));

static REX_POPUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\{\{popup\|内容=(.*?)\}\}").expect("regex POPUP"));

static REX_MAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{材料消耗\|(.*?)\|(.*?)\}\}").expect("regex MAT"));

static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("regex BR"));

/// `{{变动数值(lite?)|...|...|val}}` → `val`（用于 `extract_vars` 中的占位符提取）
static REX_VAR_COLOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{color\|#?[a-zA-Z0-9]*\|([^{}]+)\}\}").expect("regex VAR_COLOR")
});

static REX_VAR_LITE2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\{\{变动数值lite\|.*?\|.*?\|([^{}]+)\}\}").expect("regex VAR_LITE2")
});

// ─── 公开 API ──────────────────────────────────────────────────

/// 清洗 wikitext，移除或转换各种 wiki 模板语法为纯文本。
///
/// 对应 JS `utils.js:cleanWikitext()`。
#[must_use]
pub fn clean_wikitext(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let s = text.to_string();

    // 1. 移除 HTML 注释
    let s = REX_CMT.replace_all(&s, "").into_owned();
    // 2. {{color|...|text}} → text
    let s = REX_COLOR.replace_all(&s, "$1").into_owned();
    // 3. {{*|p1|p2?}} — 若 p2 == "+p1" 则取 p2，否则取 p1
    let s = REX_STAR
        .replace_all(&s, |caps: &regex::Captures<'_>| {
            let p1 = caps.get(1).map_or("", |m| m.as_str());
            let p2 = caps.get(2).map_or("", |m| m.as_str());
            if p2 == format!("+{p1}").as_str() {
                p2.to_string()
            } else {
                p1.to_string()
            }
        })
        .into_owned();
    // 4. {{变动数值lite|...|...|val}} → val
    let s = REX_VAR_LITE.replace_all(&s, "$1").into_owned();
    // 5. [[link|display]] → display（或 [[link]] → link）
    let s = REX_LINK.replace_all(&s, "$1").into_owned();
    // 6. {{DrName|前缀=X}} → X{@Doctor}；无前缀 → {@Doctor}
    let s = REX_DRNAME
        .replace_all(&s, |caps: &regex::Captures<'_>| {
            let attrs = caps.get(1).map_or("", |m| m.as_str());
            if let Some(pm) = REX_DRNAME_PREFIX.captures(attrs) {
                let prefix = pm.get(1).map_or("", |m| m.as_str());
                format!("{prefix}{{@Doctor}}")
            } else {
                "{@Doctor}".to_string()
            }
        })
        .into_owned();
    // 7. {{术语|...|text}} → text
    let s = REX_TERM.replace_all(&s, "$1").into_owned();
    // 8. {{异常效果|text}} → [text]
    let s = REX_ABNORMAL.replace_all(&s, "[$1]").into_owned();
    // 9. {{popup|内容=text}} → （text）
    let s = REX_POPUP.replace_all(&s, "（$1）").into_owned();
    // 10. {{材料消耗|mat|qty}} → mat*qty
    let s = REX_MAT.replace_all(&s, "$1*$2").into_owned();
    // 11. <br> → \n（保留换行语义）
    let s = REX_BR.replace_all(&s, "\n").into_owned();

    s.trim().to_string()
}

/// 提取后的结构体：包含带占位符的模板字符串和各占位符对应的变量值列表。
///
/// 对应 JS `utils.js:extractVars()` 的返回结构 `{ clean, vars }`。
#[derive(Debug)]
pub struct ExtractedVars {
    /// 将变动数值替换为 `{var1}`、`{var2}` 等占位符后清洗过的文本。
    pub clean: String,
    /// 各占位符对应的值（按顺序）。
    pub vars: Vec<String>,
}

/// 从技能描述中提取动态变量，生成模板化的描述字符串和变量值列表。
///
/// 对应 JS `utils.js:extractVars()`。
#[must_use]
pub fn extract_vars(text: &str) -> ExtractedVars {
    let mut vars: Vec<String> = Vec::new();

    // 将颜色标注数值替换为占位符
    let s = REX_VAR_COLOR
        .replace_all(text, |caps: &regex::Captures<'_>| {
            vars.push(caps[1].to_string());
            format!("{{var{}}}", vars.len())
        })
        .into_owned();

    // 将动态数值 lite 替换为占位符
    let s = REX_VAR_LITE2
        .replace_all(&s, |caps: &regex::Captures<'_>| {
            vars.push(caps[1].to_string());
            format!("{{var{}}}", vars.len())
        })
        .into_owned();

    ExtractedVars {
        clean: clean_wikitext(&s),
        vars,
    }
}
