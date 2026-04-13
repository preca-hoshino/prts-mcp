#![allow(warnings)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! Implementation of the `get_operator` tool.

use indexmap::IndexMap;
use regex::Regex;
use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};
use std::collections::HashMap;
use std::sync::LazyLock;

/// A tool that accepts an operator name and returns parsed data from PRTS wiki in Markdown format.
#[mcp_tool(name = "get_operator", description = "获取干员PRTS Wiki数据")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetOperatorTool {
    /// 干员名
    name: String,
    /// 类别 (例如: 干员信息, 特性, 获得方式, 技能 等)
    category: String,
}

impl GetOperatorTool {
    /// Executes the tool logic and returns the operator data.
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let url = format!(
            "https://prts.wiki/api.php?action=query&prop=revisions&rvprop=content&titles={}&format=json",
            urlencoding::encode(&self.name)
        );

        let response = reqwest::get(&url)
            .await
            .map_err(|e| CallToolError::new(e))?;
        let data: serde_json::Value = response.json().await.map_err(|e| CallToolError::new(e))?;

        // Extract wikitext
        let query = data
            .get("query")
            .and_then(|q| q.get("pages"))
            .and_then(|p| p.as_object())
            .ok_or_else(|| {
                CallToolError::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Missing pages",
                ))
            })?;
        let page_id = query.keys().next().ok_or_else(|| {
            CallToolError::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No pages returned",
            ))
        })?;
        let page = query.get(page_id).unwrap();
        let revisions = page
            .get("revisions")
            .and_then(|r| r.as_array())
            .ok_or_else(|| {
                CallToolError::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "No revisions",
                ))
            })?;
        let text = revisions
            .get(0)
            .and_then(|r| r.get("*"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                CallToolError::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "No text in revision",
                ))
            })?;

        let clean_text = clean_wikitext(text);

        let mut results: IndexMap<String, IndexMap<String, String>> = IndexMap::new();
        let mut current_template = None;
        let mut template_counts: HashMap<String, usize> = HashMap::new();
        let mut last_key: Option<String> = None;

        for line in clean_text.lines() {
            let line = line.trim();

            if let Some(captures) = REX_START.captures(line) {
                let t_name = captures.get(1).unwrap().as_str().trim().to_string();
                let count = template_counts.entry(t_name.clone()).or_insert(0);
                *count += 1;

                let unique_name = if *count > 1 {
                    format!("{}_{}", t_name, count)
                } else {
                    t_name
                };

                current_template = Some(unique_name.clone());
                results.insert(unique_name, IndexMap::new());
                last_key = None;
                continue;
            }

            if line == "}}" && current_template.is_some() {
                current_template = None;
                last_key = None;
                continue;
            }

            if let Some(ref unique_name) = current_template {
                if line.starts_with('|') {
                    if let Some(eq_idx) = line.find('=') {
                        let key = line[1..eq_idx].trim().to_string();
                        let val = line[eq_idx + 1..].trim().to_string();

                        if let Some(map) = results.get_mut(unique_name) {
                            map.insert(key.clone(), val);
                        }
                        last_key = Some(key);
                    }
                } else if let Some(ref key) = last_key {
                    if !line.is_empty() {
                        if let Some(map) = results.get_mut(unique_name) {
                            if let Some(val) = map.get_mut(key) {
                                val.push('\n');
                                val.push_str(line);
                            }
                        }
                    }
                }
            }
        }

        let mut md_output;

        match self.category.as_str() {
            "干员信息" => {
                let char_info = results.get("CharinfoV2");
                let person_info = results.get("人员档案set");

                md_output = format!(
                    "# {} {}\n\n",
                    char_info
                        .and_then(|info| info.get("干员名"))
                        .unwrap_or(&self.name),
                    char_info
                        .and_then(|info| info.get("干员外文名"))
                        .unwrap_or(&String::new())
                );

                md_output.push_str("## 干员信息\n\n");

                let faction = char_info
                    .and_then(|info| info.get("所属团队"))
                    .filter(|s| !s.is_empty())
                    .or_else(|| char_info.and_then(|info| info.get("所属组织")))
                    .filter(|s| !s.is_empty())
                    .or_else(|| char_info.and_then(|info| info.get("所属国家")))
                    .map(|s| s.as_str())
                    .unwrap_or("未知");

                let class = char_info
                    .and_then(|info| info.get("职业"))
                    .map(|s| s.as_str())
                    .unwrap_or("未知");
                let branch = char_info
                    .and_then(|info| info.get("分支"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let class_str = format!("{} ({})", class, branch);

                let rarity_num = char_info
                    .and_then(|info| info.get("稀有度"))
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let star_str = "★".repeat(rarity_num);

                let race = person_info
                    .and_then(|info| info.get("种族"))
                    .map(|s| s.as_str())
                    .unwrap_or("未知");
                let gender = person_info
                    .and_then(|info| info.get("性别"))
                    .map(|s| s.as_str())
                    .unwrap_or("未知");

                md_output.push_str("| 阵营 / 国家 | 职业与分支 | 星级 | 种族 | 性别 |\n");
                md_output.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
                md_output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n\n",
                    faction, class_str, star_str, race, gender
                ));

                let height = person_info
                    .and_then(|info| info.get("身高"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let exp = person_info
                    .and_then(|info| info.get("战斗经验"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let birth_place = person_info
                    .and_then(|info| info.get("出身地"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let bday = person_info
                    .and_then(|info| info.get("生日"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let infection = person_info
                    .and_then(|info| info.get("是否感染者"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");

                md_output.push_str("| 身高 | 战斗经验 | 出身地 | 生日 | 感染状态 |\n");
                md_output.push_str("| :--- | :--- | :--- | :--- | :--- |\n");
                md_output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n\n",
                    height, exp, birth_place, bday, infection
                ));

                let artist = char_info
                    .and_then(|info| info.get("画师"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let cv_cn = char_info
                    .and_then(|info| info.get("中文配音"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                let cv_jp = char_info
                    .and_then(|info| info.get("日文配音"))
                    .map(|s| s.as_str())
                    .unwrap_or("-");

                md_output.push_str(&format!(
                    "> ***画师**: {} | **中配**: {} | **日配**: {}*",
                    artist, cv_cn, cv_jp
                ));

                if let Some(cv_ru) = char_info.and_then(|info| info.get("俄文配音")) {
                    if !cv_ru.is_empty() {
                        md_output.push_str(&format!(" | **俄配**: {}", cv_ru));
                    }
                }
                md_output.push_str("\n");
            }
            "特性" | "获得方式" | "属性" | "攻击范围" | "天赋" | "潜能提升" | "技能"
            | "后勤技能" | "召唤物信息" | "精英化材料" | "技能升级材料" | "模组" | "相关道具"
            | "干员档案" => {
                md_output = format!("# 干员 {} 数据档案\n\n", self.name);
                md_output.push_str(&format!(
                    "## {}\n\n> 正在施工中...（当前版本暂未深度解析该类别模板）\n",
                    self.category
                ));
            }
            _ => {
                return Ok(CallToolResult::text_content(vec![TextContent::from(
                    format!(
                        "❌ 错误：无效的参数 `category`: {}。 \n请使用以下任一标准类别: 干员信息, 特性, 获得方式, 属性, 攻击范围, 天赋, 潜能提升, 技能, 后勤技能, 召唤物信息, 精英化材料, 技能升级材料, 模组, 相关道具, 干员档案",
                        self.category
                    ),
                )]));
            }
        }

        Ok(CallToolResult::text_content(vec![TextContent::from(
            md_output,
        )]))
    }
}

static REX_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\{\{([^|{}]+)$").unwrap());

fn clean_wikitext(str_val: &str) -> String {
    let mut s = str_val.to_string();

    static REX_CMT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").unwrap());
    s = REX_CMT.replace_all(&s, "").to_string();

    static REX_COLOR1: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\{color\|[^|]+\|([^}]+)\}\}").unwrap());
    s = REX_COLOR1.replace_all(&s, "$1").to_string();

    static REX_COLOR2: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\{color\|[^|]+\|([^|]+)\|[^}]+\}\}").unwrap());
    s = REX_COLOR2.replace_all(&s, "$1").to_string();

    static REX_TERM: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\{术语\|[^|]+\|([^}]+)\}\}").unwrap());
    s = REX_TERM.replace_all(&s, "$1").to_string();

    static REX_VAR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\{变动数值[a-zA-Z]*\|[^|]+\|[^|]+\|([^}]+)\}\}").unwrap());
    s = REX_VAR.replace_all(&s, "$1").to_string();

    static REX_MAT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\{\{材料消耗\|([^|]+)\|([^}]+)\}\}").unwrap());
    s = REX_MAT.replace_all(&s, "[$1×$2]").to_string();

    static REX_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
    s = REX_LINK.replace_all(&s, "$1").to_string();

    static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    s = REX_BR.replace_all(&s, "\n").to_string();

    s
}
