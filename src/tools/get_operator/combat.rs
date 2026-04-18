#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::if_not_else)]
//! 战斗数据解析器（COMBAT 域）。
//!
//! 对应 JS `temp/src/parsers/combat.js`。
//! 输出内容：属性面板、攻击范围、天赋、潜能、技能列表、召唤物、模组。

use crate::utils::range_grid::get_range_grid;
use crate::utils::wikitext::{ExtractedVars, clean_wikitext, extract_vars};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

static REX_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^### (.*?)$").expect("regex HEADER"));

static REX_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|(.*?)=(.*?)$").expect("regex PARAM"));

static REX_STAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \*\*(.*?)_(生命上限|攻击|防御|法术抗性)\*\*: (.*)$").expect("regex STAT")
});

static REX_BASE_PROP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \*\*(再部署|部署费用|阻挡数|攻击速度|所属势力|隐藏势力|势力|职业|分支|特性)\*\*: (.*)$")
        .expect("regex BASE_PROP")
});

static REX_TALENT_MAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*天赋\*\*: (.*)$").expect("regex TALENT_MAIN"));

static REX_TALENT_SUB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^- \*\*(潜能增强_)?天赋(\d+)(.*)\*\*: (.*)$").expect("regex TALENT_SUB")
});

static REX_POT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*潜能(\d+)\*\*: (.*)$").expect("regex POT"));

static REX_BOLD_KV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*([^*]+)\*\*: (.*)$").expect("regex BOLD_KV"));

static REX_SKILL_LEVEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^技能(.*?)(描述|初始|消耗|持续)$").expect("regex SKILL_LEVEL"));

static REX_SKILL_TYPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^技能类型").expect("regex SKILL_TYPE"));

static REX_SKILL_NAME_FOREIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^技能名(jp|en)$").expect("regex SKILL_NAME_FOREIGN"));

static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("regex BR"));

static REX_MOD_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"===(.*?)=== *\n").expect("regex MOD_TITLE"));

static REX_NUM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((?:[+\-])?\d+(?:\.\d+)?%?(?:（[+\-]?\d+(?:\.\d+)?%?）)?)").expect("regex NUM")
});

static REX_CHINESE_NUM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"第(.)天赋").expect("regex CHINESE_NUM"));

// ─── 内部数据结构 ──────────────────────────────────────────────

/// 技能数据。
struct SkillData {
    /// 技能名。
    name: String,
    /// 技能类型列表（触发方式、恢复方式等）。
    types: Vec<String>,
    /// 基础属性行（攻击范围、充能时长等）。
    base_props: Vec<String>,
    /// 备注行。
    notes: Vec<String>,
    /// 分等级数据：key = 等级标识（"1"~"7"、"专精1"~"专精3"）。
    levels: BTreeMap<String, HashMap<String, String>>,
}

impl SkillData {
    /// 创建新的技能数据对象。
    fn new(name: String) -> Self {
        Self {
            name,
            types: Vec::new(),
            base_props: Vec::new(),
            notes: Vec::new(),
            levels: BTreeMap::new(),
        }
    }
}

/// 天赋阶段数据。
#[derive(Default)]
struct TalentStage {
    /// 解锁条件。
    condition: String,
    /// 天赋效果描述。
    effect: String,
}

/// 天赋数据。
struct TalentData {
    /// 天赋序号（中文序数）。
    index: String,
    /// 天赋名称。
    name: String,
    /// 各阶段数据：key = 阶段编号。
    stages: BTreeMap<String, TalentStage>,
    /// 备注行。
    notes: Vec<String>,
}

impl TalentData {
    /// 创建新的天赋数据对象。
    fn new(index: String) -> Self {
        Self {
            index,
            name: String::new(),
            stages: BTreeMap::new(),
            notes: Vec::new(),
        }
    }
}

// ─── 公开 API ──────────────────────────────────────────────────

/// 解析 COMBAT 域行，生成战斗数据 Markdown 行列表。
///
/// 对应 JS `parseCombat(lines, modulesRaw)`。
///
/// - `lines` — 由 `categorize_lines()` 分出的 `combat` 桶行。
/// - `modules_raw` — 完整 wikitext（可选），用于解析模组数据。
pub async fn parse_combat(lines: &[String], modules_raw: Option<&str>) -> Vec<String> {
    let mut buckets: HashMap<&str, Vec<String>> = HashMap::from([
        ("attributes", Vec::new()),
        ("range", Vec::new()),
        ("talents", Vec::new()),
        ("potentials", Vec::new()),
        ("summons", Vec::new()),
    ]);
    let mut skills_data: Vec<SkillData> = Vec::new();
    let mut current_sub = "attributes";
    let mut current_skill: Option<usize> = None; // index into skills_data

    for line in lines {
        // 标题行切换桶
        if let Some(cap) = REX_HEADER.captures(line) {
            let h = &cap[1];
            current_sub = if h.contains("属性") || h.contains("面板") {
                "attributes"
            } else if h.contains("范围") {
                "range"
            } else if h.contains("天赋") || h.contains("特性") {
                "talents"
            } else if h.contains("潜能") {
                "potentials"
            } else if h.contains("模块") || h.contains("模组") {
                "modules"
            } else if h.contains("召唤物") {
                "summons"
            } else if h.contains("技能") {
                "skills"
            } else {
                current_sub
            };
            continue;
        }

        if let Some(cap) = REX_PARAM.captures(line) {
            let k = cap[1].trim().to_string();
            let v_raw = cap[2].trim().to_string();
            let v = clean_wikitext(&v_raw);

            if v_raw.is_empty() && v.is_empty() {
                continue;
            }

            // 备注行
            if k == "备注" {
                let note = format!("> {}", v.replace('\n', "\n> "));
                if current_sub == "skills" {
                    if let Some(idx) = current_skill {
                        skills_data[idx].notes.push(note);
                    }
                } else if let Some(bucket) = buckets.get_mut(current_sub) {
                    bucket.push(note);
                } else if let Some(b) = buckets.get_mut("attributes") {
                    b.push(note);
                }
                continue;
            }

            // 技能名 → 新建技能
            if k == "技能名" {
                skills_data.push(SkillData::new(v.clone()));
                current_skill = Some(skills_data.len() - 1);
                current_sub = "skills";
                continue;
            }

            // 技能等级字段
            if current_sub == "skills"
                && let Some(idx) = current_skill
            {
                if let Some(lc) = REX_SKILL_LEVEL.captures(&k) {
                    let level_str = lc[1].to_string();
                    let field = lc[2].to_string();
                    let level_key = if level_str.is_empty() {
                        "1".to_string()
                    } else {
                        level_str
                    };
                    if level_key.parse::<u32>().is_ok() || level_key.starts_with("专精") {
                        skills_data[idx]
                            .levels
                            .entry(level_key)
                            .or_default()
                            .insert(field, v_raw);
                    } else {
                        skills_data[idx]
                            .base_props
                            .push(format!("- **{k}**: {}", v.replace('\n', " ")));
                    }
                    continue;
                }

                if REX_SKILL_TYPE.is_match(&k) {
                    skills_data[idx].types.push(v.replace('\n', " "));
                    continue;
                }
                if REX_SKILL_NAME_FOREIGN.is_match(&k) {
                    continue; // 忽略外文名
                }
                if k.starts_with("技能") {
                    if k.ends_with("范围") {
                        let grid = get_range_grid(&v).await;
                        if grid != v {
                            skills_data[idx]
                                .base_props
                                .push(format!("- **{k}**:\n```text\n{grid}\n```"));
                        } else {
                            skills_data[idx]
                                .base_props
                                .push(format!("- **{k}**: {}", v.replace('\n', " ")));
                        }
                    } else {
                        skills_data[idx]
                            .base_props
                            .push(format!("- **{k}**: {}", v.replace('\n', " ")));
                    }
                    continue;
                }
            }

            // 非技能字段 → 路由到各桶
            if k.contains("坐标")
                || k.contains("缩放")
                || k.contains("触发类型")
                || k.starts_with("干员名j")
            {
                continue;
            }

            let line_output = if k.ends_with("范围") {
                let grid = get_range_grid(&v).await;
                if grid != v {
                    format!("- **{k}**: \n```text\n{grid}\n```")
                } else {
                    format!("- **{k}**: {}", v.replace('\n', " "))
                }
            } else {
                format!("- **{k}**: {}", v.replace('\n', " "))
            };

            // 按 Key 语义自动路由
            if k.contains("天赋") {
                if let Some(b) = buckets.get_mut("talents") {
                    b.push(line_output);
                }
                current_sub = "talents";
            } else if k.contains("范围") {
                if let Some(b) = buckets.get_mut("range") {
                    b.push(line_output);
                }
                current_sub = "range";
            } else if k.contains("潜能") {
                if let Some(b) = buckets.get_mut("potentials") {
                    b.push(line_output);
                }
                current_sub = "potentials";
            } else if k == "特性" && current_sub == "skills" {
                // 跳过
            } else if k.contains("属性")
                || k.contains("模组")
                || k.contains("攻击")
                || k.contains("生命")
                || k.contains("阻挡")
                || k.contains("费用")
                || k.contains("再部署")
                || k.contains("职业")
                || k.contains("分支")
                || k.contains("特性")
            {
                if let Some(b) = buckets.get_mut("attributes") {
                    b.push(line_output);
                }
                current_sub = "attributes";
            } else if buckets.contains_key(current_sub) && current_sub != "skills" {
                if let Some(b) = buckets.get_mut(current_sub) {
                    b.push(line_output);
                }
            } else {
                if let Some(b) = buckets.get_mut("attributes") {
                    b.push(line_output);
                }
                current_sub = "attributes";
            }
        } else {
            // 非 key=val 的纯文本行
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.contains("{{")
                || trimmed.contains("}}")
                || trimmed.contains('|')
                || trimmed.contains("###")
            {
                continue;
            }
            let ln = REX_BR
                .replace_all(&clean_wikitext(trimmed), " ")
                .into_owned();
            if ln.is_empty() {
                continue;
            }
            if current_sub != "skills" {
                if let Some(b) = buckets.get_mut(current_sub) {
                    b.push(ln);
                }
            } else if let Some(idx) = current_skill {
                skills_data[idx]
                    .base_props
                    .push(format!("- **备注**: {ln}"));
            }
        }
    }

    // ── 组装输出 ──────────────────────────────────────────────
    let mut output: Vec<String> = Vec::new();

    // 属性
    if let Some(attr) = buckets.get("attributes")
        && !attr.is_empty()
    {
        output.extend(render_attributes(attr));
    }

    if let Some(rng) = buckets.get("range")
        && !rng.is_empty()
    {
        output.push("### 攻击范围".to_string());
        output.extend(rng.iter().cloned());
        output.push(String::new());
    }

    if let Some(talents) = buckets.get("talents")
        && !talents.is_empty()
    {
        output.extend(render_talents(talents));
    }

    if let Some(pots) = buckets.get("potentials")
        && !pots.is_empty()
    {
        output.extend(render_potentials(pots));
    }

    // 技能
    if !skills_data.is_empty() {
        output.extend(render_skills(&skills_data));
    }

    if let Some(summons) = buckets.get("summons")
        && !summons.is_empty()
    {
        output.push("### 召唤物".to_string());
        output.extend(summons.iter().cloned());
        output.push(String::new());
    }

    // 模组
    if let Some(raw) = modules_raw {
        let mod_blocks = parse_modules_raw(raw);
        if !mod_blocks.is_empty() {
            output.push("### 模组".to_string());
            output.extend(mod_blocks);
            output.push(String::new());
        }
    }

    output
}

// ─── 私有渲染函数 ──────────────────────────────────────────────

/// 渲染属性面板区块。
fn render_attributes(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec!["### 属性".to_string()];
    let core_stats = ["生命上限", "攻击", "防御", "法术抗性"];
    let mut phases: Vec<String> = Vec::new();
    let mut stat_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut others: Vec<String> = Vec::new();

    for line in lines {
        if let Some(cap) = REX_STAT.captures(line) {
            let phase = cap[1].to_string();
            let stat = cap[2].to_string();
            let val = cap[3].to_string();
            if phase == "信赖加成" {
                others.push(line.clone());
                continue;
            }
            if !phases.contains(&phase) {
                phases.push(phase.clone());
            }
            stat_map.entry(stat).or_default().insert(phase, val);
        } else {
            others.push(line.clone());
        }
    }

    let mut base_obj: HashMap<String, String> = HashMap::new();
    let mut main_factions: Vec<String> = Vec::new();
    let mut hidden_factions: Vec<String> = Vec::new();
    let mut final_others: Vec<String> = Vec::new();

    for line in &others {
        if let Some(cap) = REX_BASE_PROP.captures(line) {
            let key = cap[1].to_string();
            let val = cap[2].to_string();
            if key.contains("势力") {
                if !val.is_empty() && val != "无" && val != "——" {
                    if key == "隐藏势力" {
                        hidden_factions.push(val);
                    } else {
                        main_factions.push(val);
                    }
                }
            } else {
                base_obj.entry(key).or_insert(val);
            }
        } else {
            final_others.push(line.clone());
        }
    }

    if let Some(job) = base_obj.get("职业") {
        let branch = base_obj
            .get("分支")
            .map(|b| format!(" - {b}"))
            .unwrap_or_default();
        out.push(format!("- **职业**: {job}{branch}"));
    }
    if let Some(trait_desc) = base_obj.get("特性") {
        out.push(format!("- **分支描述**: {trait_desc}"));
    }
    out.push(String::new());

    if !base_obj.is_empty() {
        out.push(format!(
            "| 再部署时间 | {} | 初始部署费用 | {} |",
            base_obj.get("再部署").map_or("-", String::as_str),
            base_obj.get("部署费用").map_or("-", String::as_str),
        ));
        out.push("| :---: | :---: | :---: | :---: |".to_string());
        out.push(format!(
            "| 阻挡数 | {} | 攻击间隔 | {} |",
            base_obj.get("阻挡数").map_or("-", String::as_str),
            base_obj.get("攻击速度").map_or("-", String::as_str),
        ));
        out.push(String::new());
    }

    if !phases.is_empty() {
        let header: Vec<String> = std::iter::once("状态 \\ 属性".to_string())
            .chain(phases.iter().map(|p| p.replace('_', " ")))
            .collect();
        out.push(format!("| {} |", header.join(" | ")));
        out.push(format!(
            "| {} |",
            header
                .iter()
                .map(|_| ":---:")
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        for stat in &core_stats {
            let Some(row_map) = stat_map.get(*stat) else {
                continue;
            };
            let mut row = vec![(*stat).to_string()];
            for phase in &phases {
                row.push(
                    row_map
                        .get(phase)
                        .cloned()
                        .unwrap_or_else(|| "-".to_string()),
                );
            }
            out.push(format!("| {} |", row.join(" | ")));
        }
        out.push(String::new());
    }

    let main_str = if main_factions.is_empty() {
        "无".to_string()
    } else {
        main_factions.join(", ")
    };
    let hidden_str = if hidden_factions.is_empty() {
        "无".to_string()
    } else {
        hidden_factions.join(", ")
    };
    out.push(format!("- **所属势力**: {main_str}"));
    out.push(format!("- **隐藏势力**: {hidden_str}"));
    out.push(String::new());

    out
}

/// 渲染天赋区块。
fn render_talents(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec!["### 天赋".to_string()];
    let mut talent_blocks: Vec<TalentData> = Vec::new();
    let mut misc_lines: Vec<String> = Vec::new();
    let mut current_talent: Option<usize> = None;

    for line in lines {
        if line.starts_with('>') {
            if let Some(idx) = current_talent {
                talent_blocks[idx].notes.push(line.clone());
            } else {
                misc_lines.push(line.clone());
            }
            continue;
        }

        if let Some(cap) = REX_TALENT_MAIN.captures(line) {
            let td = TalentData::new(cap[1].to_string());
            talent_blocks.push(td);
            current_talent = Some(talent_blocks.len() - 1);
            continue;
        }

        if let Some(cap) = REX_TALENT_SUB.captures(line) {
            let is_pot = cap.get(1).is_some();
            let sid = cap[2].to_string();
            let subk = cap[3].to_string();
            let val = cap[4].to_string();

            if let Some(idx) = current_talent {
                let td = &mut talent_blocks[idx];
                if subk.is_empty() {
                    td.name = val;
                } else {
                    let stage = td.stages.entry(sid).or_default();
                    let field_name = if is_pot {
                        format!("潜能增强{subk}")
                    } else {
                        subk.clone()
                    };
                    match field_name.as_str() {
                        "条件" => stage.condition = val,
                        "效果" => stage.effect = val,
                        _ => {} // 其他字段忽略
                    }
                }
            }
            continue;
        }

        if let Some(cap) = REX_BOLD_KV.captures(line) {
            let _ = &cap; // 非天赋相关的通用行
            misc_lines.push(line.clone());
            current_talent = None;
        } else {
            misc_lines.push(line.clone());
        }
    }

    out.extend(misc_lines);

    for td in &talent_blocks {
        out.push(format!("#### {}", td.name));

        // 天赋序号（中文数字→阿拉伯）
        let t_num = REX_CHINESE_NUM
            .captures(&td.index)
            .and_then(|c| c.get(1))
            .map_or(1, |m| match m.as_str() {
                "二" => 2u8,
                "三" => 3,
                "四" => 4,
                "五" => 5,
                "六" => 6,
                _ => 1,
            });
        out.push(format!("- **天赋序号**: {t_num}"));

        // 构建效果字符串列表和条件列表
        let mut all_strs: Vec<String> = Vec::new();
        let mut row_defs: Vec<String> = Vec::new();

        for stage in td.stages.values() {
            if !stage.effect.is_empty() && !stage.condition.contains("模组") {
                all_strs.push(stage.effect.clone());
                let cond = if stage.condition == "——" {
                    "基础".to_string()
                } else {
                    stage.condition.clone()
                };
                row_defs.push(cond);
            }
        }

        render_talent_description(&mut out, &all_strs, &row_defs);

        if !td.notes.is_empty() {
            out.extend(td.notes.iter().cloned());
            out.push(String::new());
        }
    }

    out.push(String::new());
    out
}

/// 渲染天赋描述（含动态变量模板化）。
fn render_talent_description(out: &mut Vec<String>, all_strs: &[String], row_defs: &[String]) {
    if all_strs.is_empty() {
        return;
    }

    // 尝试提取动态变量
    let tokenized: Vec<Vec<String>> = all_strs
        .iter()
        .map(|s| {
            REX_NUM
                .split(s)
                .map(String::from)
                .chain(REX_NUM.find_iter(s).map(|m| m.as_str().to_string()))
                .collect::<Vec<_>>()
        })
        .collect();

    let same_len = !tokenized.is_empty() && tokenized.iter().all(|t| t.len() == tokenized[0].len());

    if same_len && tokenized[0].len() > 1 {
        let mut template_tokens: Vec<String> = Vec::new();
        let mut vars_table: Vec<Vec<String>> = vec![Vec::new(); all_strs.len()];
        let mut is_extractable = false;

        for i in 0..tokenized[0].len() {
            let first = &tokenized[0][i];
            let is_var = (1..tokenized.len()).any(|j| tokenized[j][i] != *first);
            if is_var {
                is_extractable = true;
                let var_idx = vars_table[0].len() + 1;
                template_tokens.push(format!("{{var{var_idx}}}"));
                for (j, row) in vars_table.iter_mut().enumerate() {
                    if j < tokenized.len() {
                        row.push(tokenized[j][i].clone());
                    }
                }
            } else {
                template_tokens.push(first.clone());
            }
        }

        if is_extractable {
            let template_str = template_tokens.join("");
            out.push(format!("- **天赋描述**: {template_str}"));
            out.push(String::new());

            let var_count = vars_table[0].len();
            let mut header = vec!["条件".to_string()];
            header.extend((1..=var_count).map(|i| format!("var{i}")));
            out.push(format!("| {} |", header.join(" | ")));
            out.push(format!(
                "| {} |",
                header
                    .iter()
                    .map(|_| ":---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));

            // 合并相同 vars 的行
            let mut unique: Vec<(Vec<String>, Vec<String>)> = Vec::new();
            for (i, vars) in vars_table.iter().enumerate() {
                if let Some(entry) = unique.iter_mut().find(|(v, _)| v == vars) {
                    entry.1.push(row_defs[i].clone());
                } else {
                    unique.push((vars.clone(), vec![row_defs[i].clone()]));
                }
            }
            for (vars, conds) in &unique {
                let mut row = vec![conds.join(" / ")];
                row.extend(vars.iter().cloned());
                out.push(format!("| {} |", row.join(" | ")));
            }
            out.push(String::new());
            return;
        }
    }

    // 回退：精确字符串去重
    let header = ["条件", "效果"];
    let mut unique: Vec<(String, Vec<String>)> = Vec::new();
    for (i, s) in all_strs.iter().enumerate() {
        if let Some(entry) = unique.iter_mut().find(|(v, _)| v == s) {
            entry.1.push(row_defs[i].clone());
        } else {
            unique.push((s.clone(), vec![row_defs[i].clone()]));
        }
    }

    if unique.len() == 1 {
        out.push(format!("- **天赋描述**: {}", unique[0].0));
        out.push(String::new());
    } else {
        out.push(format!("| {} |", header.join(" | ")));
        out.push(format!(
            "| {} |",
            header
                .iter()
                .map(|_| ":---")
                .collect::<Vec<_>>()
                .join(" | ")
        ));
        for (text, conds) in &unique {
            out.push(format!("| {} | {} |", conds.join(" / "), text));
        }
        out.push(String::new());
    }
}

/// 渲染潜能区块。
fn render_potentials(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec!["### 潜能".to_string()];
    let mut pot_table: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();

    for line in lines {
        if let Some(cap) = REX_POT.captures(line) {
            pot_table.push(format!("| {} | {} |", &cap[1], &cap[2]));
        } else {
            if line.contains("潜能") {
                continue;
            }
            others.push(line.clone());
        }
    }

    if !others.is_empty() {
        out.extend(others);
        out.push(String::new());
    }

    if !pot_table.is_empty() {
        out.push("| 潜能级别 | 属性跃升 |".to_string());
        out.push("| :---: | :--- |".to_string());
        out.extend(pot_table);
    }
    out.push(String::new());
    out
}

/// 渲染技能区块。
fn render_skills(skills: &[SkillData]) -> Vec<String> {
    let mut out: Vec<String> = vec!["### 技能".to_string()];
    let level_order = ["1", "2", "3", "4", "5", "6", "7", "专精1", "专精2", "专精3"];

    for (i, skill) in skills.iter().enumerate() {
        out.push(format!("\n#### {}", skill.name));
        out.push(format!("- **技能序号**: {}", i + 1));

        if !skill.types.is_empty() {
            out.push(format!("- **技能类型**: {}", skill.types.join(" / ")));
        }

        out.extend(skill.base_props.iter().cloned());

        let levels_to_use: Vec<&str> = level_order
            .iter()
            .filter(|l| skill.levels.contains_key(**l))
            .copied()
            .collect();

        if !levels_to_use.is_empty() {
            let first_key = levels_to_use[0];
            let first_lvl = &skill.levels[first_key];
            let ExtractedVars {
                clean: desc_template,
                vars: first_vars,
            } = extract_vars(first_lvl.get("描述").map_or("-", String::as_str));

            out.push(format!(
                "- **技能描述**: {}",
                REX_BR.replace_all(&desc_template, " ")
            ));

            let mut header_cells: Vec<String> = vec!["等级".to_string()];
            let var_count = first_vars.len();
            header_cells.extend((1..=var_count).map(|v| format!("var{v}")));
            header_cells.extend(["初始", "消耗", "持续"].map(String::from));

            out.push(format!("\n| {} |", header_cells.join(" | ")));
            out.push(format!(
                "| {} |",
                header_cells
                    .iter()
                    .map(|_| ":---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));

            for lvl_key in &levels_to_use {
                let lvl = &skill.levels[*lvl_key];
                let ExtractedVars { vars, .. } =
                    extract_vars(lvl.get("描述").map_or("-", String::as_str));

                let display_key = match *lvl_key {
                    "专精1" => "RANK I".to_string(),
                    "专精2" => "RANK II".to_string(),
                    "专精3" => "RANK III".to_string(),
                    other => other.to_string(),
                };

                let mut row = vec![display_key];
                for vi in 0..var_count {
                    row.push(
                        vars.get(vi)
                            .map_or_else(|| "-".to_string(), |v| clean_wikitext(v)),
                    );
                }
                row.push(
                    lvl.get("初始")
                        .map_or_else(|| "-".to_string(), |v| clean_wikitext(v)),
                );
                row.push(
                    lvl.get("消耗")
                        .map_or_else(|| "-".to_string(), |v| clean_wikitext(v)),
                );
                row.push(
                    lvl.get("持续")
                        .map_or_else(|| "-".to_string(), |v| clean_wikitext(v)),
                );
                out.push(format!("| {} |", row.join(" | ")));
            }
        }

        if !skill.notes.is_empty() {
            out.push(String::new());
            out.extend(skill.notes.iter().cloned());
        }
    }

    out.push(String::new());
    out
}

/// 解析模组区块并生成 Markdown 行（仅战斗词条，不含材料消耗）。
///
/// 对应 JS `parseModulesRaw(rawText)`。
pub fn parse_modules_raw(raw_text: &str) -> Vec<String> {
    let Some(mod_start) = raw_text.find("==模组==") else {
        return Vec::new();
    };

    let mod_section = &raw_text[mod_start..];
    let mut out: Vec<String> = Vec::new();

    // 将模组块按 `{{` 模组 分块
    for chunk in split_module_chunks(mod_section) {
        let title = REX_MOD_TITLE
            .captures(&chunk)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();

        let mut params: HashMap<String, String> = HashMap::new();
        for (k, v) in parse_mod_kv_block(&chunk) {
            params.entry(k).or_insert(v);
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

        let mod_type = params.get("类型").cloned().unwrap_or_default();

        out.push(format!("#### {name}"));
        if !mod_type.is_empty() {
            let suffix = if mod_type.contains('-') {
                mod_type.split('-').next_back().unwrap_or(&mod_type)
            } else {
                &mod_type
            };
            out.push(format!("- **模组类型**: {suffix}模"));
        }

        if let Some(branch) = params.get("分支") {
            out.push(format!("- **分支**: {}", clean_wikitext(branch)));
        }

        let possible_stats = ["生命", "攻击", "防御", "法术抗性", "阻挡数", "攻击速度"];
        let present_stats: Vec<&str> = possible_stats
            .iter()
            .filter(|s| params.contains_key(**s))
            .copied()
            .collect();

        out.push(String::new());
        let col_names: Vec<String> = std::iter::once("模组等级".to_string())
            .chain(present_stats.iter().map(|s| (*s).to_string()))
            .chain(std::iter::once("战斗效果更新".to_string()))
            .collect();
        out.push(format!("| {} |", col_names.join(" | ")));
        let divider: Vec<&str> = std::iter::once(":---:")
            .chain(present_stats.iter().map(|_| ":---:"))
            .chain(std::iter::once(":---"))
            .collect();
        out.push(format!("| {} |", divider.join(" | ")));

        for (suffix, lvl) in &[("", "1"), ("2", "2"), ("3", "3")] {
            let mut row = vec![(*lvl).to_string()];
            for stat in &present_stats {
                let key = format!("{stat}{suffix}");
                let mut val = params.get(&key).cloned().unwrap_or_else(|| "-".to_string());
                if val != "-" && !val.starts_with('-') && !val.starts_with('+') {
                    val = format!("+{val}");
                }
                row.push(val);
            }
            let effect = if *lvl == "1" {
                params.get("特性").map(|v| {
                    format!(
                        "**特性更新**: {}",
                        REX_BR.replace_all(&clean_wikitext(v), " ")
                    )
                })
            } else {
                let talent_key = format!("天赋{lvl}");
                params.get(&talent_key).map(|v| {
                    format!(
                        "**天赋更新**: {}",
                        REX_BR.replace_all(&clean_wikitext(v), " ")
                    )
                })
            }
            .unwrap_or_else(|| "-".to_string());
            row.push(effect);
            out.push(format!("| {} |", row.join(" | ")));
        }

        out.push(String::new());
    }

    out
}

/// 将 wikitext 展开加 `{{` 模组 `}}` 块按模组分割。
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
fn parse_mod_kv_block(text: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in text.lines() {
        // 跳过 {{ }} 封装行
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
