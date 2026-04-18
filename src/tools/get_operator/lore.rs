#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! 档案文本解析器（LORE 域）。
//!
//! 对应 JS `temp/src/parsers/lore.js`。
//! 输出内容：客观履历、临床诊断、干员档案 1-4、晋升记录、模组故事。

use crate::utils::wikitext::clean_wikitext;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static REX_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|(.*?)=(.*)$").expect("regex PARAM"));

static REX_ARCHIVE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^档案(\d+)$").expect("regex ARCHIVE_KEY"));

static REX_ARCHIVE_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^档案(\d+)文本$").expect("regex ARCHIVE_TEXT"));

static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("regex BR"));

static REX_MOD_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)==模组==(.+?)(?:==[^=]|$)").expect("regex MOD_SECTION"));

static REX_MOD_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"===(.*?)=== *\n").expect("regex MOD_TITLE"));

/// 干员档案条目。
#[derive(Default)]
struct LoreEntry {
    /// 档案标题。
    title: String,
    /// 档案正文。
    text: String,
}

/// 解析 LORE 域行，生成档案文本 Markdown 行列表。
///
/// 对应 JS `parseLore(lines, mainText)`。
///
/// - `lines` — 由 `categorize_lines()` 分出的 `lore` 桶行。
/// - `main_text` — 完整 wikitext，用于提取模组故事。
#[must_use]
pub fn parse_lore(lines: &[String], main_text: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut current_title = String::new();
    let mut lore_map: BTreeMap<u32, LoreEntry> = BTreeMap::new();
    let mut current_key: Option<u32> = None;

    for line in lines {
        if line.contains("{{人员档案") || line.contains("{{干员/临床诊断") {
            current_title.clear();
            current_key = None;
            continue;
        }

        if let Some(cap) = REX_PARAM.captures(line) {
            let k = cap[1].trim().to_string();
            let v = cap[2].trim().to_string();
            current_key = None;

            if k == "标题" {
                current_title = clean_wikitext(&v);
            } else if k == "文本" {
                let text = REX_BR.replace_all(&clean_wikitext(&v), "\n").into_owned();
                if current_title.is_empty() {
                    result.push(format!("\n```text\n{text}\n```"));
                } else {
                    result.push(format!("\n### {current_title}\n```text\n{text}\n```"));
                    current_title.clear();
                }
            } else if let Some(cap) = REX_ARCHIVE_KEY.captures(&k) {
                let idx: u32 = cap[1].parse().unwrap_or(0);
                lore_map.entry(idx).or_default().title = clean_wikitext(&v);
            } else if let Some(cap) = REX_ARCHIVE_TEXT.captures(&k) {
                let idx: u32 = cap[1].parse().unwrap_or(0);
                let text = REX_BR.replace_all(&clean_wikitext(&v), "\n").into_owned();
                lore_map.entry(idx).or_default().text = text;
                current_key = Some(idx);
            }
        } else if line.trim().starts_with("###") {
            let h = line.trim();
            if h != "### 干员档案" {
                result.push(format!("\n{h}\n"));
            }
            current_key = None;
        } else if let Some(idx) = current_key {
            // 多行档案文本续行
            if !line.contains("}}") {
                let appended = REX_BR.replace_all(&clean_wikitext(line), "\n").into_owned();
                if let Some(entry) = lore_map.get_mut(&idx) {
                    entry.text.push('\n');
                    entry.text.push_str(&appended);
                }
            }
        } else {
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && !trimmed.contains("{{")
                && !trimmed.contains("}}")
                && !trimmed.contains('|')
            {
                let clean = REX_BR
                    .replace_all(&clean_wikitext(trimmed), "\n")
                    .into_owned();
                if !clean.is_empty() {
                    result.push(clean);
                }
            }
        }
    }

    // 追加 lore_map 中的档案条目（按编号排序）
    for entry in lore_map.values() {
        if !entry.title.is_empty() && !entry.text.is_empty() {
            result.push(format!(
                "\n### {}\n```text\n{}\n```\n",
                entry.title, entry.text
            ));
        }
    }

    // 提取模组故事
    let module_stories = parse_module_stories(main_text);
    if !module_stories.is_empty() {
        result.push("\n### 模组故事".to_string());
        result.extend(module_stories);
    }

    result
}

/// 从完整 wikitext 中解析模组故事段落。
fn parse_module_stories(main_text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // 找到 ==模组== 区段
    let Some(mod_start) = main_text.find("==模组==") else {
        return out;
    };

    let mod_section = if let Some(cap) = REX_MOD_SECTION.captures(&main_text[mod_start..]) {
        cap.get(1).map_or("", |m| m.as_str()).to_string()
    } else {
        main_text[mod_start..].to_string()
    };

    // 按 {{模组 分块
    for chunk in mod_section.split("{{模组") {
        if chunk.trim().is_empty() {
            continue;
        }
        let full_chunk = format!("{{{{模组{chunk}");

        let title = REX_MOD_TITLE
            .captures(&full_chunk)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let mut params: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (k, v) in parse_kv_block(&full_chunk) {
            params.entry(k).or_insert(v);
        }

        // 跳过基础证章
        if params.contains_key("基础证章") {
            continue;
        }

        let name = params
            .get("名称")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if title.is_empty() {
                    "未命名模组".to_string()
                } else {
                    title.clone()
                }
            });

        let mod_type = params.get("类型").cloned().unwrap_or_default();
        let display_title = if mod_type.is_empty() {
            name.clone()
        } else {
            let suffix = if mod_type.contains('-') {
                mod_type.split('-').next_back().unwrap_or(&mod_type)
            } else {
                mod_type.as_str()
            };
            format!("{suffix}模：{name}")
        };

        if let Some(story) = params.get("基础信息") {
            let text = REX_BR
                .replace_all(&clean_wikitext(story), "\n")
                .into_owned();
            out.push(format!("\n#### {display_title}\n```text\n{text}\n```\n"));
        }
    }

    out
}

/// 逐行解析 `|key=value` 格式的参数块，跳过 `}}` 终止行。
/// 返回 (key, value) 对的列表。
fn parse_kv_block(text: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in text.lines() {
        if line.trim() == "}}" {
            if let Some(k) = current_key.take() {
                result.push((k, std::mem::take(&mut current_val).trim().to_string()));
            }
            break;
        }
        if let Some(rest) = line.strip_prefix('|') {
            if let Some(eq) = rest.find('=') {
                // 新的 key=value 开始
                if let Some(k) = current_key.take() {
                    result.push((k, std::mem::take(&mut current_val).trim().to_string()));
                }
                let k = rest[..eq].trim().to_string();
                let v = rest[eq + 1..].trim().to_string();
                if !k.is_empty() {
                    current_key = Some(k);
                    current_val = v;
                }
            }
        } else if current_key.is_some() {
            // 多行 value 续行
            if !current_val.is_empty() {
                current_val.push('\n');
            }
            current_val.push_str(line.trim());
        }
    }
    if let Some(k) = current_key {
        result.push((k, current_val.trim().to_string()));
    }
    result
}
