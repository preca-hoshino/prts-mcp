#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `search_operators` MCP Tool 实现。
//!
//! 通过 PRTS Wiki 的 `MediaWiki` 搜索 API 模糊搜索干员，
//! 并使用 `分类:干员` 分类验证确保结果只包含干员页面。

use reqwest::Client;
use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};
use serde::Deserialize;
use std::collections::HashSet;

// ─── MediaWiki API URL 常量 ────────────────────────────────────────────────

/// `OpenSearch` API（标题前缀补全，支持外文名重定向解析）
const OPENSEARCH_API: &str =
    "https://prts.wiki/api.php?action=opensearch&format=json&redirects=resolve";

/// 全文搜索 API（按页面正文内容检索）
const FULLTEXT_SEARCH_API: &str =
    "https://prts.wiki/api.php?action=query&list=search&format=json&srnamespace=0&srwhat=text";

/// 分类验证 API（批量核查页面是否属于 `分类:干员`）
const CATEGORY_CHECK_API: &str = "https://prts.wiki/api.php?action=query&format=json&prop=categories&clcategories=%E5%88%86%E7%B1%BB:%E5%B9%B2%E5%91%98";

/// 干员页面的标准分类标题
const OPERATOR_CATEGORY: &str = "分类:干员";

/// PRTS Wiki 干员页面的基础 URL 前缀
const PRTS_PAGE_BASE: &str = "https://prts.wiki/w/";

// ─── 内部数据结构 ──────────────────────────────────────────────────────────

/// 搜索结果条目（中间态，未验证是否为干员页面）
#[derive(Debug, Clone)]
struct SearchCandidate {
    /// 页面中文标题
    title: String,
}

// ─── OpenSearch API 响应反序列化 ──────────────────────────────────────────

/// `action=opensearch` 的响应格式：
/// `[query, [title, ...], [desc, ...], [url, ...]]`
type OpenSearchResponse = (String, Vec<String>, Vec<String>, Vec<String>);

// ─── 全文搜索 API 响应反序列化 ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FulltextResponse {
    query: FulltextQuery,
}

#[derive(Debug, Deserialize)]
struct FulltextQuery {
    search: Vec<FulltextHit>,
}

#[derive(Debug, Deserialize)]
struct FulltextHit {
    title: String,
}

// ─── 分类验证 API 响应反序列化 ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CategoryCheckResponse {
    query: CategoryCheckQuery,
}

#[derive(Debug, Deserialize)]
struct CategoryCheckQuery {
    pages: std::collections::HashMap<String, CategoryPage>,
}

#[derive(Debug, Deserialize)]
struct CategoryPage {
    title: String,
    /// 仅在页面属于所请求的分类时才存在此字段
    #[serde(default)]
    categories: Vec<CategoryEntry>,
}

#[derive(Debug, Deserialize)]
struct CategoryEntry {
    title: String,
}

// ─── Tool 定义 ────────────────────────────────────────────────────────────

/// 在 PRTS Wiki 搜索明日方舟干员的 MCP Tool。
#[mcp_tool(
    name = "search_operators",
    description = "在 PRTS Wiki 中搜索明日方舟干员，返回匹配的干员名列表。\n\n\
<when_to_use>\n\
- 用户只知道干员名称的一部分（中文或外文），需要确认正式名称\n\
- 用户询问某个职业/分支有哪些干员（如「有哪些神射手？」）\n\
- 用户想按势力/阵营查找干员（如「企鹅物流有哪些干员？」）\n\
- 在调用 get_operator 之前，需要确认干员的精确中文名\n\
</when_to_use>\n\n\
<when_not_to_use>\n\
- 已知干员精确中文名时，直接调用 get_operator 更高效\n\
- 需要干员详细数据（技能/属性/档案）时，直接调用 get_operator\n\
</when_not_to_use>\n\n\
<workflow>\n\
推荐的两步工作流：\n\
1. search_operators(query=关键词) → 获得干员准确中文名列表\n\
2. get_operator(name=精确中文名, category=...) → 获取详细数据\n\
</workflow>\n\n\
<parameters>\n\
- query: 搜索关键词，支持：干员中文名或部分名（如「银」）、外文名或部分外文名（如「exus」）、\n\
  职业分支名（如「神射手」）、势力名（如「企鹅物流」）等任何出现在干员页面中的词\n\
- limit: 最大返回结果数，默认 10，最大 20\n\
</parameters>\n\n\
<output_format>\n\
返回 Markdown 列表，包含干员中文名与 PRTS Wiki 页面链接。\n\
若未找到匹配干员，则返回提示信息。\n\
</output_format>\n\n\
<important>\n\
此 Tool 依赖 PRTS Wiki API，需要两次网络请求（并发执行）加一次分类验证请求。\n\
响应时间约为 1~2 秒。\n\
</important>",
    read_only_hint = true,
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = true
)]
#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SearchOperatorsTool {
    /// 搜索关键词。
    /// 支持干员中文名（部分匹配）、外文名（部分匹配）、职业分支词、势力名等。
    query: String,

    /// 最大返回结果数。默认 10，最大 20。
    limit: Option<u8>,
}

impl SearchOperatorsTool {
    /// 执行 Tool 逻辑：并发搜索 → 合并去重 → 分类验证 → 返回 Markdown。
    ///
    /// # Errors
    /// 若网络请求失败，返回协议级 `CallToolError`。
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let query = self.query.trim();
        if query.is_empty() {
            return Ok(CallToolResult::with_error(CallToolError::from_message(
                "❌ 搜索关键词不能为空。".to_string(),
            )));
        }

        let limit = self.limit.unwrap_or(10).min(20) as usize;

        // 1. 并发调用 opensearch + 全文搜索
        let client = build_client().map_err(|e| CallToolError::new(std::io::Error::other(e)))?;
        let (open_res, full_res) = tokio::join!(
            call_opensearch(&client, query, limit),
            call_fulltext_search(&client, query, limit),
        );

        // 2. 合并候选，去重（以 title 为 key）
        let mut seen: HashSet<String> = HashSet::new();
        let mut candidates: Vec<SearchCandidate> = Vec::new();

        for candidate in open_res
            .unwrap_or_default()
            .into_iter()
            .chain(full_res.unwrap_or_default())
        {
            // 过滤子页面（含 `/` 分隔符的页面肯定不是顶层干员页）
            if candidate.title.contains('/') {
                continue;
            }
            if seen.insert(candidate.title.clone()) {
                candidates.push(candidate);
            }
        }

        if candidates.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                format!(
                    "未找到与「{query}」相关的干员。\n\n建议检查关键词拼写，或尝试更短的关键词。"
                ),
            )]));
        }

        // 3. 分类验证：批量核查哪些候选页面属于「分类:干员」
        let titles: Vec<&str> = candidates.iter().map(|c| c.title.as_str()).collect();
        let operator_titles = verify_operator_category(&client, &titles)
            .await
            .unwrap_or_default();

        if operator_titles.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                format!(
                    "搜索「{query}」未找到符合条件的干员页面。\n\n\
                     建议：尝试更精确的干员名称，或使用中文关键词。"
                ),
            )]));
        }

        // 4. 组装 Markdown 输出
        let output = format_result(query, &operator_titles, limit);
        Ok(CallToolResult::text_content(vec![TextContent::from(
            output,
        )]))
    }
}

// ─── 网络请求函数 ─────────────────────────────────────────────────────────

/// 构建带 User-Agent 的 HTTP 客户端。
fn build_client() -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    Ok(Client::builder().user_agent("prts-mcp/0.1.0").build()?)
}

/// 调用 `action=opensearch` API，返回标题前缀匹配的候选列表。
///
/// 借助 `redirects=resolve`，外文名重定向页面会自动解析为中文标题。
async fn call_opensearch(
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
async fn call_fulltext_search(
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
async fn verify_operator_category(
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

// ─── 输出格式化 ───────────────────────────────────────────────────────────

/// 将干员名列表格式化为 Markdown 输出。
fn format_result(query: &str, operators: &[String], limit: usize) -> String {
    let count = operators.len();
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("## 搜索「{query}」共找到 {count} 名干员\n"));

    for name in operators {
        let url = format!("{PRTS_PAGE_BASE}{}", urlencoding::encode(name));
        lines.push(format!("- **{name}** — [PRTS Wiki 页面]({url})"));
    }

    if count >= limit {
        lines.push(String::new());
        lines.push(format!(
            "> 结果已达上限 {limit} 条，可能存在更多匹配干员。建议使用更精确的关键词缩小范围。"
        ));
    }

    lines.push(String::new());
    lines.push(
        "> 提示：使用 `get_operator` 工具并传入干员中文名，可获取详细的技能、属性、档案等数据。"
            .to_string(),
    );

    lines.join("\n")
}
