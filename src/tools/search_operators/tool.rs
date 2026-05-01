#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `search_operators` MCP Tool 实现。
//!
//! 工作流：
//! 1. 用 `query` 并发调用 PRTS Wiki opensearch + fulltext 搜索 API，
//!    经 `分类:干员` 验证得到「搜索集合 A」。
//! 2. 若提供了任意属性过滤参数，拉取「干员一览」HTML 提取全量元数据，
//!    内存过滤后得到「属性集合 B」。
//! 3. 若无过滤参数返回 A；否则返回 A ∩ B。

use std::collections::HashSet;

use reqwest::Client;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};
use serde::Deserialize;

use super::strings::{
    ERR_CATEGORY_NO_MATCH, ERR_QUERY_EMPTY, ERR_SEARCH_NO_MATCH, FILTER_HINT_NONE, FMT_FILTER_ITEM,
    FMT_FILTER_OBTAIN, FMT_GET_OPERATOR_HINT, FMT_LIMIT_HINT, FMT_TITLE_FILTERED, FMT_TITLE_SEARCH,
    WARN_FILTER_NO_MATCH,
};
use crate::resources::operator_list::{OperatorMeta, fetch_operator_meta_list};

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

/// 搜索结果候选（来自 API）
#[derive(Debug, Clone)]
struct SearchCandidate {
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
///
/// 先用关键词搜索，若提供属性过滤参数则进一步筛选，两者取交集返回匹配干员列表。
///
/// # Examples
///
/// ```json
/// {"query": "银灰", "class": "近卫", "rarity": 6}
/// {"query": "exusiai", "rarity": 6}
/// {"query": "天使", "limit": 5}
/// ```
#[mcp_tool(
    name = "search_operators",
    description = "在 PRTS Wiki 中搜索明日方舟干员。先用关键词搜索，若提供属性参数则进一步过滤，两者取交集返回匹配干员列表。\n\n\
<when_to_use>\n\
- 用户知道干员名称的一部分（中文或外文），需要确认正式名称\n\
- 用户想按职业/星级/获取途径/词缀筛选干员\n\
- 调用 get_operator 前确认干员精确中文名\n\
</when_to_use>\n\n\
<when_not_to_use>\n\
- 已知干员精确中文名时，直接调用 get_operator 更高效\n\
- 需要获取干员详细数据（技能、面板、档案）时，使用 get_operator\n\
</when_not_to_use>\n\n\
<parameters>\n\
- query: 搜索关键词（必填）。支持干员中文名/部分名（银）、外文名（exus）、技能名、档案内容等任何干员页面中的词汇。\n\
- class: 按职业筛选，精确匹配。省略时不限制职业。合法值：先锋 / 近卫 / 重装 / 狙击 / 术师 / 医疗 / 辅助 / 特种。\n\
- rarity: 按稀有度筛选，整数对应游戏内星级。省略时不限制稀有度。合法值：1 / 2 / 3 / 4 / 5 / 6。\n\
- position: 按站位类型筛选，精确匹配。省略时不限制站位。合法值：近战位 / 远程位。\n\
- obtain: 按获取途径筛选，支持关键词模糊匹配（如「寻访」可命中所有寻访类型）。省略时不限制获取途径。\n\
- tag: 按公招词缀筛选，精确匹配。省略时不限制词缀。注意：「近战」「远程」不是词缀，请用 position 参数。合法值：治疗 / 支援 / 输出 / 群攻 / 减速 / 生存 / 防护 / 削弱 / 位移 / 控场 / 爆发 / 召唤 / 快速复活 / 费用回复 / 支援机械 / 元素 / 高空。\n\
- limit: 最大返回结果数，默认 10，超出自动截断为 50。\n\
</parameters>\n\n\
<output_format>\n\
返回 Markdown 列表，含干员中文名与 PRTS Wiki 页面链接。\n\
若有过滤参数，说明搜索命中数、过滤后数量与筛选条件。\n\
</output_format>\n\n\
<important>\n\
此工具依赖 PRTS Wiki 外部网络请求，响应受网络状况影响。\n\
当过滤后结果为空时，工具会返回未过滤的命中数供参考，便于调整筛选条件。\n\
推荐两步工作流: search_operators → get_operator。\n\
</important>",
    read_only_hint = true,
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = true
)]
#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct SearchOperatorsTool {
    /// 搜索关键词（必填）。支持干员中文名（部分匹配，如「银」）、
    /// 外文名（部分匹配，如「exus」）、技能名、档案内容等任何干员页面中的词汇。
    /// 示例：`"query": "能天使"` 或 `"query": "银灰"`。
    query: String,

    /// 按职业精确过滤，省略时不限制职业。
    /// 合法值：先锋 / 近卫 / 重装 / 狙击 / 术师 / 医疗 / 辅助 / 特种。
    /// 示例：`"class": "狙击"` 仅返回狙击干员。
    class: Option<String>,

    /// 按稀有度过滤，整数对应游戏内星级，省略时不限制稀有度。
    /// 合法值：1 / 2 / 3 / 4 / 5 / 6。
    /// 示例：`"rarity": 6` 仅返回六星干员。
    rarity: Option<u8>,

    /// 按站位类型精确过滤，省略时不限制站位。
    /// 合法值：近战位 / 远程位。
    /// 示例：`"position": "远程位"` 仅返回远程位干员。
    position: Option<String>,

    /// 按获取途径筛选（contains 模糊匹配），省略时不限制获取途径。
    /// 支持关键词模糊匹配，如「寻访」可命中「标准寻访」「中坚寻访」「限定寻访」等。
    /// 示例：`"obtain": "寻访"` 返回所有寻访类途径干员。
    obtain: Option<String>,

    /// 按公招词缀精确过滤，省略时不限制词缀。
    /// 注意：「近战」「远程」不是公招词缀，请使用 position 参数。
    /// 合法值：治疗 / 支援 / 输出 / 群攻 / 减速 / 生存 / 防护 / 削弱 / 位移 / 控场 / 爆发 / 召唤 / 快速复活 / 费用回复 / 支援机械 / 元素 / 高空。
    /// 示例：`"tag": "治疗"` 仅返回含治疗词缀的干员。
    tag: Option<String>,

    /// 最大返回结果数（1-50）。省略时默认 10，超出上限时自动截断为 50。
    /// 示例：`"limit": 20` 最多返回 20 条匹配结果。
    limit: Option<u8>,
}

impl SearchOperatorsTool {
    /// 判断是否提供了任意属性过滤参数。
    fn has_filter_params(&self) -> bool {
        self.class.is_some()
            || self.rarity.is_some()
            || self.position.is_some()
            || self.obtain.is_some()
            || self.tag.is_some()
    }

    /// 判断一条 `OperatorMeta` 是否满足所有过滤条件。
    fn matches_filters(&self, meta: &OperatorMeta) -> bool {
        // 职业：精确匹配
        if let Some(c) = &self.class
            && &meta.class != c
        {
            return false;
        }
        // 稀有度：精确匹配
        if let Some(r) = self.rarity
            && meta.rarity != r
        {
            return false;
        }
        // 位置：精确匹配
        if let Some(p) = &self.position
            && &meta.position != p
        {
            return false;
        }
        // 获取途径：contains 模糊匹配（用户输入"寻访"可命中所有寻访类型）
        if let Some(o) = &self.obtain
            && !meta.obtain.iter().any(|v| v.contains(o.as_str()))
        {
            return false;
        }
        // 词缀：精确匹配单个词缀
        if let Some(t) = &self.tag
            && !meta.tags.iter().any(|v| v == t)
        {
            return false;
        }

        true
    }

    /// 执行 Tool 逻辑：搜索 → [并发过滤] → 取交集 → 返回 Markdown。
    ///
    /// # Errors
    ///
    /// 若网络请求失败，返回协议级 `CallToolError`。
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        let query = self.query.trim();
        if query.is_empty() {
            return Ok(CallToolResult::with_error(CallToolError::from_message(
                ERR_QUERY_EMPTY.to_string(),
            )));
        }

        let limit = self.limit.unwrap_or(10).min(50) as usize;
        let has_filter = self.has_filter_params();

        let client = build_client().map_err(|e| CallToolError::new(std::io::Error::other(e)))?;

        // ── Step 1 & 2 并发执行 ─────────────────────────────────────────
        // Step 1: opensearch + fulltext（必执行）
        // Step 2: 干员一览拉取（仅有过滤参数时执行）
        let (open_res, full_res, meta_res) = tokio::join!(
            call_opensearch(&client, query, limit),
            call_fulltext_search(&client, query, limit),
            async {
                if has_filter {
                    fetch_operator_meta_list(&client).await.ok()
                } else {
                    None
                }
            }
        );

        // ── 合并搜索候选，去重 ──────────────────────────────────────────
        let mut seen: HashSet<String> = HashSet::new();
        let mut candidates: Vec<SearchCandidate> = Vec::new();

        for candidate in open_res
            .unwrap_or_default()
            .into_iter()
            .chain(full_res.unwrap_or_default())
        {
            // 过滤子页面（含 `/` 的页面不是顶层干员页）
            if candidate.title.contains('/') {
                continue;
            }
            if seen.insert(candidate.title.clone()) {
                candidates.push(candidate);
            }
        }

        if candidates.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                ERR_SEARCH_NO_MATCH.replace("{query}", query),
            )]));
        }

        // ── Step 1: Category 验证，得到集合 A ──────────────────────────
        let titles: Vec<&str> = candidates.iter().map(|c| c.title.as_str()).collect();
        let set_a = verify_operator_category(&client, &titles)
            .await
            .unwrap_or_default();

        if set_a.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                ERR_CATEGORY_NO_MATCH.replace("{query}", query),
            )]));
        }

        // ── Step 2: 属性过滤，得到集合 B，取 A ∩ B ────────────────────
        let search_hit_count = set_a.len();

        let final_names: Vec<String> = if let Some(meta_list) = meta_res {
            // 构建属性过滤后的干员名集合 B
            let set_b: HashSet<String> = meta_list
                .into_iter()
                .filter(|meta| self.matches_filters(meta))
                .map(|meta| meta.zh_name)
                .collect();

            // 取 A ∩ B，保持 A 的顺序（alphabetical，来自 verify_operator_category）
            set_a
                .into_iter()
                .filter(|name| set_b.contains(name))
                .collect()
        } else {
            set_a
        };

        if final_names.is_empty() {
            let filter_hint = self.filter_hint();
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                WARN_FILTER_NO_MATCH
                    .replace("{query}", query)
                    .replace("{count}", &search_hit_count.to_string())
                    .replace("{filter}", &filter_hint),
            )]));
        }

        // ── 组装 Markdown 输出 ──────────────────────────────────────────
        let output = format_result(query, &final_names, limit, has_filter);
        Ok(CallToolResult::text_content(vec![TextContent::from(
            output,
        )]))
    }

    /// 生成过滤条件的简要描述（用于无结果时的错误提示）。
    fn filter_hint(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(c) = &self.class {
            parts.push(FMT_FILTER_ITEM.replace("{k}", "职业").replace("{v}", c));
        }
        if let Some(r) = self.rarity {
            parts.push(format!("稀有度={r}星"));
        }
        if let Some(p) = &self.position {
            parts.push(FMT_FILTER_ITEM.replace("{k}", "位置").replace("{v}", p));
        }
        if let Some(o) = &self.obtain {
            parts.push(FMT_FILTER_OBTAIN.replace("{v}", o));
        }
        if let Some(t) = &self.tag {
            parts.push(FMT_FILTER_ITEM.replace("{k}", "词缀").replace("{v}", t));
        }
        if parts.is_empty() {
            FILTER_HINT_NONE.to_string()
        } else {
            parts.join("，")
        }
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
fn format_result(query: &str, operators: &[String], limit: usize, filtered: bool) -> String {
    let count = operators.len();
    let mut lines: Vec<String> = Vec::new();

    if filtered {
        lines.push(
            FMT_TITLE_FILTERED
                .replace("{query}", query)
                .replace("{count}", &count.to_string()),
        );
    } else {
        lines.push(
            FMT_TITLE_SEARCH
                .replace("{query}", query)
                .replace("{count}", &count.to_string()),
        );
    }

    for name in operators {
        let url = format!("{PRTS_PAGE_BASE}{}", urlencoding::encode(name));
        lines.push(format!("- **{name}** — [PRTS Wiki 页面]({url})"));
    }

    if count >= limit {
        lines.push(String::new());
        lines.push(FMT_LIMIT_HINT.replace("{limit}", &limit.to_string()));
    }

    lines.push(String::new());
    lines.push(FMT_GET_OPERATOR_HINT.to_string());

    lines.join("\n")
}
