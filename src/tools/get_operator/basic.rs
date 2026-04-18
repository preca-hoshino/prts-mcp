#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
//! 基础干员信息解析器（BASIC 域）。
//!
//! 对应 JS `temp/src/parsers/basic.js`。
//! 输出内容：干员简介、干员信息表、画师/CV、获得方式。

use crate::utils::wikitext::clean_wikitext;
use regex::Regex;
use std::sync::LazyLock;

static REX_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|(.*?)=(.*?)$").expect("regex PARAM"));

/// 将稀有度数字转换为星星字符串。
fn to_stars(n: &str) -> String {
    let count = n.trim().parse::<usize>().unwrap_or(0);
    "★".repeat(count)
}

/// 将 `"2025-04-07 16:00"` 格式的日期转为 `"2025年4月7日 16:00"` 格式。
fn format_date(s: &str) -> String {
    static REX_DATE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\d{4})-(\d{2})-(\d{2})\s*(.*)").expect("regex DATE"));
    if let Some(cap) = REX_DATE.captures(s) {
        let year = &cap[1];
        let month: u32 = cap[2].parse().unwrap_or(0);
        let day: u32 = cap[3].parse().unwrap_or(0);
        let rest = cap[4].trim();
        let base = format!("{year}年{month}月{day}日");
        if rest.is_empty() {
            base
        } else {
            format!("{base} {rest}")
        }
    } else {
        s.to_string()
    }
}

/// 解析 BASIC 域行，生成基础信息 Markdown 行列表。
///
/// 对应 JS `parseBasic(lines, mainText)`。
///
/// - `lines` — 由 `categorize_lines()` 分出的 `basic` 桶行。
/// - `main_text` — 完整 wikitext，用于提取特性字段。
#[must_use]
pub fn parse_basic(lines: &[String], main_text: &str) -> Vec<String> {
    // 从原始 wikitext 提取特性（虽属 combat，但 basic 视图也需要展示）
    let trait_text = extract_field(main_text, "特性")
        .map(clean_wikitext)
        .unwrap_or_default();

    // 构建参数字典
    let mut params: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut in_module_section = false;

    for line in lines {
        if line.contains("<section begin=专属模组") {
            in_module_section = true;
            continue;
        }
        if line.contains("<section end=专属模组") {
            in_module_section = false;
            continue;
        }
        if in_module_section {
            continue;
        }

        if let Some(cap) = REX_PARAM.captures(line) {
            let k = cap[1].trim().to_string();
            let v = clean_wikitext(cap[2].trim());
            if !v.is_empty()
                && !k.starts_with("干员名j")
                && !k.contains("坐标")
                && !k.contains("缩放")
                && !k.contains("触发类型")
            {
                params.insert(k, v);
            }
        }
    }

    let mut result: Vec<String> = Vec::new();

    // ── 简介 ──────────────────────────────────────────────────
    let brief = params.get("干员简介").cloned().unwrap_or_default();
    let brief_extra = params.get("干员简介补充").cloned().unwrap_or_default();
    let info_code = params.get("情报编号").cloned().unwrap_or_default();

    if !brief.is_empty() {
        result.push(brief.clone());
    }
    if !brief_extra.is_empty() {
        result.push(format!("> {brief_extra}"));
    }
    if !brief.is_empty() || !brief_extra.is_empty() {
        result.push(String::new());
    }

    // ── 干员信息 ──────────────────────────────────────────────
    result.push("### 干员信息".to_string());
    result.push(String::new());

    // 画师 + 配音 blockquote
    let illustrator = params.get("画师").cloned().unwrap_or_default();
    let cv_cn = params.get("中文配音").cloned().unwrap_or_default();
    let cv_jp = params.get("日文配音").cloned().unwrap_or_default();
    let cv_en = params.get("英文配音").cloned().unwrap_or_default();
    let cv_kr = params.get("韩文配音").cloned().unwrap_or_default();

    let mut cv_parts: Vec<String> = Vec::new();
    if !cv_cn.is_empty() {
        cv_parts.push(format!("{cv_cn} **(中)**"));
    }
    if !cv_jp.is_empty() {
        cv_parts.push(format!("{cv_jp} **(日)**"));
    }
    if !cv_en.is_empty() {
        cv_parts.push(format!("{cv_en} **(英)**"));
    }
    if !cv_kr.is_empty() {
        cv_parts.push(format!("{cv_kr} **(韩)**"));
    }

    if !illustrator.is_empty() {
        result.push(format!("> ***画师**: {illustrator}*"));
    }
    if !cv_parts.is_empty() {
        result.push(format!("> \n> ***配音**: {}*", cv_parts.join(" | ")));
    }
    result.push(String::new());

    if !info_code.is_empty() {
        result.push(format!("编号：{info_code}"));
    }
    result.push(String::new());

    let stars = to_stars(params.get("稀有度").map_or("0", String::as_str));
    if !stars.is_empty() {
        result.push(format!("星级: {stars}"));
    }
    result.push(String::new());

    let faction = params
        .get("所属国家")
        .or_else(|| params.get("所属"))
        .cloned()
        .unwrap_or_default();
    if !faction.is_empty() {
        result.push(format!("势力: {faction}"));
    }
    result.push(String::new());

    if let Some(pos) = params.get("位置") {
        result.push(format!("位置: {pos}"));
    }
    result.push(String::new());

    if let Some(tags) = params.get("标签") {
        let tag_str = tags.split_whitespace().collect::<Vec<_>>().join(",");
        result.push(format!("词缀: {tag_str}"));
    }
    result.push(String::new());

    let job = params.get("职业").cloned().unwrap_or_default();
    let branch = params.get("分支").cloned().unwrap_or_default();
    let job_line = if branch.is_empty() {
        job.clone()
    } else {
        format!("{job}-{branch}")
    };
    if !job_line.is_empty() {
        result.push(format!("职业: {job_line}"));
    }
    if !trait_text.is_empty() {
        result.push(format!("> {trait_text}"));
    }
    result.push(String::new());

    // ── 获得方式 ──────────────────────────────────────────────
    result.push("### 获得方式".to_string());
    result.push(String::new());

    if let Some(way) = params.get("获得方式") {
        result.push(format!("获得方式：{way}"));
    }
    result.push(String::new());

    if let Some(date) = params.get("上线时间") {
        result.push(format!("上线时间：{}", format_date(date)));
    }
    result.push(String::new());

    result
}

/// 从完整 wikitext 中提取形如 `|key = value` 的单行字段值。
fn extract_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("|{key}");
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('|')
            && let Some(eq) = trimmed.find('=')
        {
            let k = trimmed[1..eq].trim();
            if k == key {
                return Some(trimmed[eq + 1..].trim());
            }
        }
        let _ = &pattern;
        None
    })
}
