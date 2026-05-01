#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `get_operator` MCP Tool 实现。
//!
//! 接收干员名与数据域标识，从 PRTS Wiki 拉取并解析对应的 Markdown 数据。
//! 所有文本常量（错误信息、字段键名等）统一定义于 [`super::strings`]。

use super::strings::{
    COMBAT_GLOBAL_FIELDS, ERR_INVALID_CATEGORY, ERR_OPERATOR_NOT_FOUND, FIELD_CHAR_NAME,
    FIELD_FOREIGN_NAME, VALID_CATEGORIES, VOICE_PAGE_SUFFIX,
};
use crate::utils::categorizer::categorize_lines;
use crate::utils::wiki_client::fetch_wikitext;
use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};

/// 从 PRTS Wiki 获取干员数据的 MCP Tool。
///
/// 支持以下 `category` 参数值（大小写不敏感）：
/// - `BASIC`   — 基础信息（干员简介、职业、势力、画师、CV、获得方式）
/// - `COMBAT`  — 战斗数据（属性面板、天赋、技能、潜能、模组、攻击范围）
/// - `BUILD`   — 养成材料（精英化材料、技能升级材料、模组材料）
/// - `LORE`    — 干员档案（背景故事、干员档案1-4、模组故事）
/// - `GALLERY` — 图鉴立绘（精英立绘描述、时装信息与链接）
/// - `VOICE`   — 语音台词（中日文台词文本与音频链接）
/// - `ALL`     — 按顺序获取以上全部数据域
///
/// # Examples
///
/// ```json
/// {"name": "能天使", "category": "COMBAT"}
/// {"name": "Mon3tr", "category": "ALL"}
/// {"name": "陈", "category": "LORE"}
/// ```
#[mcp_tool(
    name = "get_operator",
    description = "从 PRTS Wiki 查询明日方舟干员的详细数据，以 Markdown 格式返回。\n\n\
<when_to_use>\n\
- 用户询问干员的天赋、技能、属性面板或模组信息\n\
- 用户需要了解干员的背景故事、干员档案或世界观内容\n\
- 用户查询干员精英化、技能升级或模组所需的养成材料\n\
- 用户想获取干员语音台词的中日文原文或音频下载链接\n\
- 用户询问干员的职业分支、势力归属、画师或 CV 等基础属性\n\
</when_to_use>\n\n\
<when_not_to_use>\n\
- 查询游戏内活动、公告、限时卡池或服务器状态（此工具不支持）\n\
- 干员名称不确定时，请先询问用户确认后再调用本工具\n\
</when_not_to_use>\n\n\
<parameters>\n\
- name: 干员名称，须与 PRTS Wiki 页面标题一致。仅支持中文名（如「能天使」）。名称错误时工具将返回错误提示。\n\
- category: 数据域标识符（大小写不敏感），合法值如下：\n  · BASIC   — 基础信息（职业/势力/画师/CV/获得方式）\n  · COMBAT  — 战斗数据（属性面板/天赋/技能/模组/攻击范围）\n  · BUILD   — 养成材料（精英化/技能升级/模组所需材料）\n  · LORE    — 干员档案（背景故事/档案1-4/模组故事）\n  · GALLERY — 图鉴立绘（精英立绘说明/时装信息与链接）\n  · VOICE   — 语音台词（中日文文本与音频下载链接）\n  · ALL     — 按顺序返回以上全部数据域\n\
</parameters>\n\n\
<output_format>\n\
返回 Markdown 文档，顶级标题格式为「# 干员名 外文名」（外文名用反引号包裹）。\n\
指定单一 category 时返回对应数据块；指定 ALL 时，各数据域之间以「---」水平分隔线分隔。\n\
若干员页面不存在则返回以 ❌ 开头的错误说明。\n\
</output_format>\n\n\
<important>\n\
此工具依赖 PRTS Wiki 外部网络请求，响应受网络状况影响。\n\
选择 VOICE 或 ALL 时将额外请求语音子页面，耗时约为单域查询的 2 倍。\n\
</important>",
    read_only_hint = true,
    destructive_hint = false,
    idempotent_hint = true,
    open_world_hint = true
)]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetOperatorTool {
    /// 干员名称（必填），须与 PRTS Wiki 页面标题一致。
    /// 仅支持中文名，如「能天使」「陈」「Mon3tr」。
    /// 名称错误或页面不存在时工具将返回以 🔍 开头的错误提示，请检查拼写后重试。
    /// 示例：`"name": "能天使"`。
    name: String,

    /// 数据域标识符（大小写不敏感）。确定要返回的数据类别。
    /// 合法值：BASIC（基础信息）/ COMBAT（战斗数据）/ BUILD（养成材料）/
    /// LORE（干员档案）/ GALLERY（图鉴立绘）/ VOICE（语音台词）/ ALL（全部数据域）。
    /// 无效值将返回以 ❌ 开头的错误提示并列出所有合法选项。
    /// 示例：`"category": "COMBAT"` 返回战斗相关数据。
    category: String,
}

impl GetOperatorTool {
    /// 执行 Tool 逻辑：拉取 wikitext → 分块 → 路由到对应解析器 → 返回 Markdown。
    ///
    /// # Errors
    /// 若网络请求失败，返回协议级 `CallToolError`。
    /// 若干员不存在或 `category` 非法，返回带 `isError: true` 的工具执行错误。
    #[allow(clippy::too_many_lines)]
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        // 1. 服务端输入验证（不信任 LLM 填写的参数，防止越界调用）
        let category_upper = self.category.to_uppercase();
        if !VALID_CATEGORIES.contains(&category_upper.as_str()) {
            return Ok(CallToolResult::with_error(CallToolError::from_message(
                ERR_INVALID_CATEGORY.replace("{category}", &self.category),
            )));
        }

        // 2. 拉取主页 wikitext
        let main_text = fetch_wikitext(&self.name)
            .await
            .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;

        if main_text.is_empty() {
            return Ok(CallToolResult::with_error(CallToolError::from_message(
                ERR_OPERATOR_NOT_FOUND.replace("{name}", &self.name),
            )));
        }

        // 3. 提取全局元数据
        let char_name = extract_line_field(&main_text, FIELD_CHAR_NAME)
            .unwrap_or(self.name.as_str())
            .to_string();
        let foreign_name = extract_line_field(&main_text, FIELD_FOREIGN_NAME)
            .unwrap_or("")
            .to_string();

        // 4. 分块
        let blocks = categorize_lines(&main_text);

        // 5. 注入 combat 所需的全局字段（职业/分支/特性）
        let mut combat_lines = blocks.combat;
        for field in COMBAT_GLOBAL_FIELDS {
            if let Some(val) = extract_line_field(&main_text, field) {
                combat_lines.insert(0, format!("|{field}={val}"));
            }
        }

        // 6. 路由解析
        let parsed: Vec<String> = match category_upper.as_str() {
            "BASIC" => super::basic::parse_basic(&blocks.basic, &main_text),

            "COMBAT" => super::combat::parse_combat(&combat_lines, Some(&main_text)).await,

            "BUILD" => {
                let all_lines: Vec<String> = main_text.lines().map(str::to_string).collect();
                super::build::parse_build(&all_lines)
            }

            "LORE" => super::lore::parse_lore(&blocks.lore, &main_text),

            "GALLERY" => super::gallery::parse_gallery(&blocks.gallery, &main_text, &self.name),

            "VOICE" => {
                let voice_page = format!("{}{}", self.name, VOICE_PAGE_SUFFIX);
                let voice_text = fetch_wikitext(&voice_page)
                    .await
                    .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;
                super::voice::parse_voice(&voice_text)
            }

            "ALL" => {
                // 按 BASIC → COMBAT → BUILD → LORE → GALLERY → VOICE 顺序拼接
                let all_lines: Vec<String> = main_text.lines().map(str::to_string).collect();
                let voice_page = format!("{}{}", self.name, VOICE_PAGE_SUFFIX);
                let voice_text = fetch_wikitext(&voice_page)
                    .await
                    .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;

                let mut all: Vec<String> = Vec::new();
                all.extend(super::basic::parse_basic(&blocks.basic, &main_text));
                all.push(String::new());
                all.push("---".to_string());
                all.push(String::new());
                all.extend(super::combat::parse_combat(&combat_lines, Some(&main_text)).await);
                all.push(String::new());
                all.push("---".to_string());
                all.push(String::new());
                all.extend(super::build::parse_build(&all_lines));
                all.push(String::new());
                all.push("---".to_string());
                all.push(String::new());
                all.extend(super::lore::parse_lore(&blocks.lore, &main_text));
                all.push(String::new());
                all.push("---".to_string());
                all.push(String::new());
                all.extend(super::gallery::parse_gallery(
                    &blocks.gallery,
                    &main_text,
                    &self.name,
                ));
                all.push(String::new());
                all.push("---".to_string());
                all.push(String::new());
                all.extend(super::voice::parse_voice(&voice_text));
                all
            }

            // 兜底分支（理论上不可达，因为前置校验已拦截）
            _ => unreachable!("category 已经过服务端校验，此分支不可达"),
        };

        // 7. 组装输出 Markdown
        let mut output = format!("# {char_name}");
        if !foreign_name.is_empty() {
            use std::fmt::Write as _;
            let _ = write!(output, " `{foreign_name}`");
        }
        output.push_str("\n\n");
        output.push_str(&parsed.join("\n"));

        Ok(CallToolResult::text_content(vec![TextContent::from(
            output,
        )]))
    }
}

/// 从 wikitext 中提取形如 `|key=value`（单行）的字段值，返回裁剪后的字符串切片。
fn extract_line_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            return None;
        }
        let inner = &trimmed[1..];
        let eq = inner.find('=')?;
        if inner[..eq].trim() == key {
            Some(inner[eq + 1..].trim())
        } else {
            None
        }
    })
}
