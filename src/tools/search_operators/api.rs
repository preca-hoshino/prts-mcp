#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `search_operators` 的 `MediaWiki` API 交互层。
//!
//! 包含 API URL 常量、响应反序列化类型及所有网络请求函数。
//! 与 Tool 业务逻辑解耦，供 [`super::tool`] 调用。

use reqwest::Client;
use serde::Deserialize;

// ─── MediaWiki API URL 常量 ────────────────────────────────────────────────

/// `OpenSearch` API（标题前缀补全，支持外文名重定向解析）
pub(crate) const OPENSEARCH_API: &str =
    "https://prts.wiki/api.php?action=opensearch&format=json&redirects=resolve";

/// 全文搜索 API（按页面正文内容检索）
pub(crate) const FULLTEXT_SEARCH_API: &str =
    "https://prts.wiki/api.php?action=query&list=search&format=json&srnamespace=0&srwhat=text";

/// 分类验证 API（批量核查页面是否属于 `分类:干员`）
pub(crate) const CATEGORY_CHECK_API: &str = "https://prts.wiki/api.php?action=query&format=json&prop=categories&clcategories=%E5%88%86%E7%B1%BB:%E5%B9%B2%E5%91%98";

/// 干员页面的标准分类标题
pub(crate) const OPERATOR_CATEGORY: &str = "分类:干员";

/// PRTS Wiki 干员页面的基础 URL 前缀
pub(crate) const PRTS_PAGE_BASE: &str = "https://prts.wiki/w/";

// ─── 内部数据结构 ──────────────────────────────────────────────────────────

/// 搜索结果候选（来自 API）。
///
/// `title` 字段设为 `pub(crate)` 以便上层模块读取。
#[derive(Debug, Clone)]
pub(crate) struct SearchCandidate {
    pub(crate) title: String,
}

// ─── OpenSearch API 响应反序列化 ──────────────────────────────────────────

/// `action=opensearch` 的响应格式：
/// `[query, [title, ...], [desc, ...], [url, ...]]`
type OpenSearchResponse = (String, Vec<String>, Vec<String>, Vec<String>);

// ─── 全文搜索 API 响应反序列化 ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct FulltextResponse {
    pub(crate) query: FulltextQuery,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FulltextQuery {
    pub(crate) search: Vec<FulltextHit>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FulltextHit {
    pub(crate) title: String,
}

// ─── 分类验证 API 响应反序列化 ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CategoryCheckResponse {
    pub(crate) query: CategoryCheckQuery,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CategoryCheckQuery {
    pub(crate) pages: std::collections::HashMap<String, CategoryPage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CategoryPage {
    pub(crate) title: String,
    /// 仅在页面属于所请求的分类时才存在此字段
    #[serde(default)]
    pub(crate) categories: Vec<CategoryEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CategoryEntry {
    pub(crate) title: String,
}

// ─── 网络请求函数 ─────────────────────────────────────────────────────────

/// 构建带 User-Agent 的 HTTP 客户端。
pub(crate) fn build_client() -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    Ok(Client::builder().user_agent("prts-mcp/0.1.0").build()?)
}

/// 调用 `action=opensearch` API，返回标题前缀匹配的候选列表。
///
/// 借助 `redirects=resolve`，外文名重定向页面会自动解析为中文标题。
pub(crate) async fn call_opensearch(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{OPENSEARCH_API}&search={}&limit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client.get(&url).send().await?;
    let data: OpenSearchResponse = resp.json().await?;

    // 返回格式：[query, [title,...], [desc,...], [url,...]]
    Ok(data
        .1
        .into_iter()
        .map(|title| SearchCandidate { title })
        .collect())
}

/// 调用 `action=query&list=search` API，返回全文匹配的候选列表。
pub(crate) async fn call_fulltext_search(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchCandidate>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "{FULLTEXT_SEARCH_API}&srsearch={}&srlimit={}",
        urlencoding::encode(query),
        limit
    );

    let resp = client.get(&url).send().await?;
    let data: FulltextResponse = resp.json().await?;

    Ok(data
        .query
        .search
        .into_iter()
        .map(|hit| SearchCandidate { title: hit.title })
        .collect())
}

/// 批量验证候选标题是否属于 `分类:干员`，返回通过验证的干员中文名列表。
///
/// `MediaWiki` `prop=categories&clcategories=分类:干员` 仅在页面属于该分类时
/// 才会在响应中包含 `categories` 字段，利用此特性精确识别干员页面。
pub(crate) async fn verify_operator_category(
    client: &Client,
    titles: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    if titles.is_empty() {
        return Ok(Vec::new());
    }

    // MediaWiki 支持 `|` 分隔多个标题，一次请求验证所有候选
    let titles_param = titles
        .iter()
        .map(|t| urlencoding::encode(t).into_owned())
        .collect::<Vec<_>>()
        .join("%7C"); // `|` 的 URL 编码

    let url = format!("{CATEGORY_CHECK_API}&titles={titles_param}");

    let resp = client.get(&url).send().await?;
    let data: CategoryCheckResponse = resp.json().await?;

    // 仅保留 categories 字段中包含「分类:干员」的页面
    let mut operators: Vec<String> = data
        .query
        .pages
        .into_values()
        .filter(|page| {
            page.categories
                .iter()
                .any(|cat| cat.title == OPERATOR_CATEGORY)
        })
        .map(|page| page.title)
        .collect();

    // 按名称排序，保证输出稳定
    operators.sort_unstable();
    Ok(operators)
}
