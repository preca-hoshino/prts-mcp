//! Operator list resource
//!
//! Fetches operator data from PRTS wiki

use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::fmt::Write;

/// Fetches the PRTS operator list page and parses it into a Markdown table.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, the JSON cannot be parsed, or the selector is invalid.
pub async fn fetch_operator_list_markdown() -> Result<String, Box<dyn std::error::Error>> {
    let url = "https://prts.wiki/api.php?action=parse&page=%E5%B9%B2%E5%91%98%E4%B8%80%E8%A7%88&prop=text&format=json";

    let client = Client::builder().user_agent("prts-mcp/0.1.0").build()?;

    let res = client.get(url).send().await?.json::<Value>().await?;
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
