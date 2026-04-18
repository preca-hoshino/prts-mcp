#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
#![allow(clippy::too_many_lines)]
//! 基建后勤与材料消耗解析器（BUILD 域）。
//!
//! 对应 JS `temp/src/parsers/build.js`。
//! 输出内容：精英化材料、技能升级材料、专精训练表、模组材料消耗。

use crate::utils::wikitext::clean_wikitext;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static REX_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|(.*?)=(.*?)$").expect("regex PARAM"));

static REX_ELITE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^精[12]$").expect("regex ELITE"));

static REX_GEN_LVL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)$").expect("regex GEN_LVL"));

static REX_SPEC_LVL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([一二三四])(\d+)$").expect("regex SPEC_LVL"));

static REX_MOD_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"===(.*?)=== *\n").expect("regex MOD_TITLE"));

static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("regex BR"));

/// 数字转中文数字序号（1→"一"，2→"二" 等）。
fn idx_to_chinese(n: usize) -> &'static str {
    match n {
        1 => "一",
        2 => "二",
        3 => "三",
        4 => "四",
        _ => "?",
    }
}

/// 清洗符合格式的材料行：移除 `<br>` 并将空白转为逗号分隔。
fn clean_material(raw: &str) -> String {
    REX_BR
        .replace_all(&clean_wikitext(raw), " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(", ")
}

/// 解析 BUILD 域，生成材料消耗 Markdown 行列表。
///
/// 对应 JS `parseBuild(lines)`。
/// 本实现直接接受完整 wikitext 行（已由 caller 展开），与 JS 保持一致。
///
/// - `lines` — 来自整页 wikitext 的行切片。
#[must_use]
pub fn parse_build(lines: &[String]) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let full_text = lines.join("\n");

    let mut elite_block: Vec<String> = Vec::new();
    // BTreeMap 保证等级排序：key = 等级数字字符串
    let mut gen_skills_map: BTreeMap<u32, String> = BTreeMap::new();
    // outer key = 专精等级(1/2/3)，inner key = 技能序号(1/2/3...)
    let mut spec_skills_map: BTreeMap<u32, BTreeMap<usize, String>> = BTreeMap::new();
    let mut modules_block: Vec<String> = Vec::new();

    // ── 解析精英化与技能材料（按行扫描）────────────────────────
    for line in lines {
        let Some(cap) = REX_PARAM.captures(line) else {
            continue;
        };
        let k = cap[1].trim().to_string();
        let v = clean_material(cap[2].trim());

        if v.is_empty() {
            continue;
        }

        if REX_ELITE.is_match(&k) {
            elite_block.push(format!("- **{k}**: {v}"));
        } else if let Some(gc) = REX_GEN_LVL.captures(&k) {
            let lvl: u32 = gc[1].parse().unwrap_or(0);
            gen_skills_map.insert(lvl, v);
        } else if let Some(sc) = REX_SPEC_LVL.captures(&k) {
            let chinese = &sc[1];
            let raw_lvl: u32 = sc[2].parse().unwrap_or(7);
            let spec_lvl = raw_lvl.saturating_sub(7); // 8→1, 9→2, 10→3
            let skill_idx = match chinese {
                "一" => 1usize,
                "二" => 2,
                "三" => 3,
                "四" => 4,
                _ => continue,
            };
            spec_skills_map
                .entry(spec_lvl)
                .or_default()
                .insert(skill_idx, v);
        }
    }

    // ── 解析模组材料消耗（正则扫描整页）────────────────────────
    if let Some(mod_start) = full_text.find("==模组==") {
        let mod_section = &full_text[mod_start..];
        for chunk in split_module_chunks(mod_section) {
            let title = REX_MOD_TITLE
                .captures(&chunk)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let mut params: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (pk, pv) in parse_mod_kv(&chunk) {
                params.entry(pk).or_insert(pv);
            }

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

            let mut mod_costs: Vec<String> = Vec::new();
            for (suffix, label) in &[("", "1级"), ("2", "2级"), ("3", "3级")] {
                let key = format!("材料消耗{suffix}");
                if let Some(raw) = params.get(&key) {
                    mod_costs.push(format!("- **{label}**: {}", clean_material(raw)));
                }
            }

            if !mod_costs.is_empty() {
                modules_block.push(format!("\n#### {name}"));
                modules_block.extend(mod_costs);
            }
        }
    }

    // ── 输出组装 ─────────────────────────────────────────────
    if !elite_block.is_empty() {
        result.push("### 精英化材料".to_string());
        result.extend(elite_block);
        result.push(String::new());
    }

    let has_gen = !gen_skills_map.is_empty();
    let has_spec = !spec_skills_map.is_empty();

    if has_gen || has_spec {
        result.push("### 技能升级材料".to_string());

        if has_gen {
            result.push("#### 技能升级".to_string());
            result.push("| 等级提升 | 消耗材料 |".to_string());
            result.push("| :---: | :--- |".to_string());
            for (lvl, mat) in &gen_skills_map {
                let from = lvl.saturating_sub(1);
                result.push(format!("| {from} → {lvl} | {mat} |"));
            }
            result.push(String::new());
        }

        if has_spec {
            result.push("#### 专精训练".to_string());

            let max_skill = spec_skills_map
                .values()
                .flat_map(|inner| inner.keys())
                .copied()
                .max()
                .unwrap_or(1);

            let mut header = vec!["专精等级".to_string()];
            let mut divider = vec![":---:".to_string()];
            for i in 1..=max_skill {
                header.push(format!("第{}技能", idx_to_chinese(i)));
                divider.push(":---".to_string());
            }
            result.push(format!("| {} |", header.join(" | ")));
            result.push(format!("| {} |", divider.join(" | ")));

            for (spec_lvl, skill_map) in &spec_skills_map {
                let mut row = vec![format!("等级{spec_lvl}")];
                for i in 1..=max_skill {
                    row.push(
                        skill_map
                            .get(&i)
                            .cloned()
                            .unwrap_or_else(|| "-".to_string()),
                    );
                }
                result.push(format!("| {} |", row.join(" | ")));
            }
            result.push(String::new());
        }
    }

    if !modules_block.is_empty() {
        result.push("### 模组材料".to_string());
        result.extend(modules_block);
        result.push(String::new());
    }

    result
}

/// 将 wikitext 按模组块分割，返回各模组块的字符串列表。
fn split_module_chunks(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut start: Option<usize> = None;
    let bytes = text.as_bytes();
    let mut i = 0;

    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
            i += 2;
            continue;
        }
        if bytes[i] == b'}' && bytes[i + 1] == b'}' && depth > 0 {
            depth -= 1;
            if depth == 0
                && let Some(s) = start.take()
            {
                chunks.push(text[s..i + 2].to_string());
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    chunks
}

/// 逐行解析模组块中的 `|key=value` 参数。
fn parse_mod_kv(text: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "{{" || trimmed == "}}" {
            if let Some(k) = current_key.take() {
                result.push((k, current_val.trim().to_string()));
                current_val.clear();
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix('|') {
            if let Some(eq) = rest.find('=') {
                if let Some(k) = current_key.take() {
                    result.push((k, current_val.trim().to_string()));
                    current_val.clear();
                }
                let k = rest[..eq].trim().to_string();
                let v = rest[eq + 1..].to_string();
                if !k.is_empty() {
                    current_key = Some(k);
                    current_val = v;
                }
            }
        } else if current_key.is_some() {
            if !current_val.is_empty() {
                current_val.push('\n');
            }
            current_val.push_str(trimmed);
        }
    }
    if let Some(k) = current_key {
        result.push((k, current_val.trim().to_string()));
    }
    result
}
