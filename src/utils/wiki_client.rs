#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! PRTS Wiki HTTP 客户端，封装 `MediaWiki` API 请求逻辑。

use reqwest::Client;

/// 通过 `MediaWiki` `action=parse` 接口获取指定页面的原始 wikitext 内容。
///
/// # Errors
/// 若 HTTP 请求失败、JSON 解析失败或页面不存在，则返回错误。
pub async fn fetch_wikitext(
    page_title: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://prts.wiki/api.php?action=parse&page={}&format=json&prop=wikitext",
        urlencoding::encode(page_title)
    );

    let client = Client::builder().user_agent("prts-mcp/0.1.0").build()?;

    let resp = client.get(&url).send().await?;
    let data: serde_json::Value = resp.json().await?;

    let text = data
        .get("parse")
        .and_then(|p| p.get("wikitext"))
        .and_then(|w| w.get("*"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(text)
}
