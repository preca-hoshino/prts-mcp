//! 干员列表资源
//!
//! 从 PRTS Wiki 的「干员一览」页面获取干员数据，提供两种接口：
//! - [`fetch_operator_meta_list`]：返回结构化元数据列表（供 `search_operators` 筛选使用）
//! - [`fetch_operator_list_markdown`]：返回 Markdown 表格（供 MCP 资源 `prts://operators` 使用）

use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::fmt::Write;

// ─── 干员一览 API URL ─────────────────────────────────────────────────────

/// 干员一览的渲染 HTML API 地址
const OPERATOR_LIST_URL: &str = "https://prts.wiki/api.php?action=parse&page=%E5%B9%B2%E5%91%98%E4%B8%80%E8%A7%88&prop=text&format=json";

// ─── 结构体定义 ───────────────────────────────────────────────────────────

/// 从干员一览页面提取的干员属性元数据，用于 `search_operators` 的本地筛选。
///
/// 所有字段均来自干员一览 HTML 中 `div[data-zh]` 元素的 `data-*` 属性。
#[derive(Debug, Clone)]
pub struct OperatorMeta {
    /// 干员中文名（`data-zh`）
    pub zh_name: String,
    /// 职业（`data-profession`）：先锋/近卫/重装/狙击/术师/医疗/辅助/特种
    pub class: String,
    /// 稀有度（`data-rarity` + 1）：1-6
    pub rarity: u8,
    /// 站位（`data-position`）：近战位 / 远程位
    pub position: String,
    /// 获取途径列表（`data-obtain_method` 按逗号分隔）
    pub obtain: Vec<String>,
    /// 公招词缀列表（`data-tag` 按空格分隔）
    pub tags: Vec<String>,
    /// 职业分支（`data-subprofession`）：尖兵/冲锋手/战术家/…
    pub subprofession: String,
}

// ─── 核心拉取函数 ─────────────────────────────────────────────────────────

/// 拉取干员一览页面 HTML，解析所有 `div[data-zh]`，返回结构化元数据列表。
///
/// # Errors
///
/// 若 HTTP 请求失败、JSON 解析失败或 CSS 选择器无效，则返回错误。
pub async fn fetch_operator_meta_list(
    client: &Client,
) -> Result<Vec<OperatorMeta>, Box<dyn std::error::Error + Send + Sync>> {
    let res = client
        .get(OPERATOR_LIST_URL)
        .send()
        .await?
        .json::<Value>()
        .await?;
    let html_text = res["parse"]["text"]["*"].as_str().unwrap_or("");

    let document = Html::parse_document(html_text);
    let selector =
        Selector::parse("div[data-zh]").map_err(|_| "Invalid CSS selector: div[data-zh]")?;

    let mut meta_list: Vec<OperatorMeta> = Vec::new();

    for element in document.select(&selector) {
        let attr = element.value();

        let zh_name = attr.attr("data-zh").unwrap_or("").to_string();
        if zh_name.is_empty() {
            continue;
        }

        let class = attr.attr("data-profession").unwrap_or("").to_string();
        let rarity = attr
            .attr("data-rarity")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0)
            .saturating_add(1);
        let position = attr.attr("data-position").unwrap_or("").to_string();

        // data-obtain_method 存储为逗号分隔字符串（模板已做替换）
        let obtain: Vec<String> = attr
            .attr("data-obtain_method")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        // data-tag 存储为空格分隔字符串
        let tags: Vec<String> = attr
            .attr("data-tag")
            .unwrap_or("")
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        let subprofession = attr.attr("data-subprofession").unwrap_or("").to_string();

        meta_list.push(OperatorMeta {
            zh_name,
            class,
            rarity,
            position,
            obtain,
            tags,
            subprofession,
        });
    }

    Ok(meta_list)
}

// ─── 上层 Markdown 包装 ───────────────────────────────────────────────────

/// 拉取干员一览并格式化为 Markdown 表格（供 MCP 资源 `prts://operators` 使用）。
///
/// # Errors
///
/// 若 HTTP 请求失败、JSON 解析失败或 CSS 选择器无效，则返回错误。
pub async fn fetch_operator_list_markdown() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::builder().user_agent("prts-mcp/0.1.0").build()?;
    let res = client
        .get(OPERATOR_LIST_URL)
        .send()
        .await?
        .json::<Value>()
        .await?;
    let text = res["parse"]["text"]["*"].as_str().unwrap_or("");

    let document = Html::parse_document(text);
    let selector = Selector::parse("div[data-zh]").map_err(|_| "Invalid selector")?;

    let mut markdown = String::from(
        "| 编号 | 中文名 | 外文名 | 职业 | 职业分支 | 稀有度 |\n|---|---|---|---|---|---|\n",
    );

    for element in document.select(&selector) {
        let value = element.value();
        let id = value.attr("data-id").unwrap_or("");
        let zh = value.attr("data-zh").unwrap_or("");
        let en = value.attr("data-en").unwrap_or("");
        let ja = value.attr("data-ja").unwrap_or("");
        let foreign_name = if en.is_empty() {
            ja.to_string()
        } else {
            en.to_string()
        };
        let profession = value.attr("data-profession").unwrap_or("");
        let sub_prof = value.attr("data-subprofession").unwrap_or("");
        let rarity_str = value.attr("data-rarity").unwrap_or("0");
        let rarity = rarity_str.parse::<u32>().unwrap_or(0) + 1;
        let rarity_stars = "★".repeat(rarity as usize);

        writeln!(
            markdown,
            "| {id} | {zh} | {foreign_name} | {profession} | {sub_prof} | {rarity_stars} |"
        )?;
    }

    Ok(markdown)
}
