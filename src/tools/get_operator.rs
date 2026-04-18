#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! `get_operator` MCP Tool 实现。
//!
//! 接收干员名与数据域标识，从 PRTS Wiki 拉取并解析对应的 Markdown 数据。

use crate::tools::parsers;
use crate::utils::categorizer::categorize_lines;
use crate::utils::wiki_client::fetch_wikitext;
use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};

/// 从 PRTS Wiki 获取干员数据的 MCP Tool。
///
/// 支持以下 `category` 参数值（全大写）：
/// - `BASIC`   — 基础信息（干员简介、职业、势力、画师、CV、获得方式）
/// - `COMBAT`  — 战斗数据（属性面板、天赋、技能、潜能、模组、攻击范围）
/// - `BUILD`   — 基建/材料（精英化材料、技能升级材料、模组材料）
/// - `LORE`    — 干员档案（背景故事、干员档案1-4、模组故事）
/// - `GALLERY` — 图鉴立绘（精英立绘描述、时装信息与链接）
/// - `VOICE`   — 语音台词（中日文台词文本与音频链接）
#[mcp_tool(name = "get_operator", description = "获取干员PRTS Wiki数据")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct GetOperatorTool {
    /// 干员名（须与 PRTS Wiki 页面标题一致，例如：\"陈\"、\"Mon3tr\"）
    name: String,
    /// 数据域标识（全大写）：BASIC / COMBAT / BUILD / LORE / GALLERY / VOICE
    category: String,
}

impl GetOperatorTool {
    /// 执行 Tool 逻辑：拉取 wikitext → 分块 → 路由到对应解析器 → 返回 Markdown。
    ///
    /// # Errors
    /// 若网络请求失败或参数无效，则返回 `CallToolError`。
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        // 1. 拉取主页 wikitext
        let main_text = fetch_wikitext(&self.name)
            .await
            .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;

        if main_text.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                format!(
                    "❌ 错误：未找到干员「{}」的 Wiki 页面，请检查名称是否正确。",
                    self.name
                ),
            )]));
        }

        // 2. 提取全局元数据
        let char_name = extract_line_field(&main_text, "干员名")
            .unwrap_or(self.name.as_str())
            .to_string();
        let foreign_name = extract_line_field(&main_text, "干员外文名")
            .unwrap_or("")
            .to_string();

        // 3. 分块
        let blocks = categorize_lines(&main_text);

        // 4. 注入 combat 所需的全局字段（职业/分支/特性）
        let mut combat_lines = blocks.combat;
        for field in &["职业", "分支", "特性"] {
            if let Some(val) = extract_line_field(&main_text, field) {
                combat_lines.insert(0, format!("|{field}={val}"));
            }
        }

        // 5. 路由解析
        let parsed: Vec<String> = match self.category.to_uppercase().as_str() {
            "BASIC" => parsers::basic::parse_basic(&blocks.basic, &main_text),

            "COMBAT" => parsers::combat::parse_combat(&combat_lines, Some(&main_text)).await,

            "BUILD" => {
                let all_lines: Vec<String> = main_text.lines().map(str::to_string).collect();
                parsers::build::parse_build(&all_lines)
            }

            "LORE" => parsers::lore::parse_lore(&blocks.lore, &main_text),

            "GALLERY" => parsers::gallery::parse_gallery(&blocks.gallery, &main_text, &self.name),

            "VOICE" => {
                let voice_page = format!("{}/语音记录", self.name);
                let voice_text = fetch_wikitext(&voice_page)
                    .await
                    .map_err(|e| CallToolError::new(std::io::Error::other(e.to_string())))?;
                parsers::voice::parse_voice(&voice_text)
            }

            _ => {
                return Ok(CallToolResult::text_content(vec![TextContent::from(
                    format!(
                        "❌ 错误：无效的 `category` 参数「{}」。\n\
                         请使用以下任一标准值（全大写）：BASIC / COMBAT / BUILD / LORE / GALLERY / VOICE",
                        self.category
                    ),
                )]));
            }
        };

        // 6. 组装输出 Markdown
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
