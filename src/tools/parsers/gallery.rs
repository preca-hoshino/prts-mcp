#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! 图鉴立绘解析器（GALLERY 域）。
//!
//! 对应 JS `temp/src/parsers/gallery.js`。
//! 输出内容：精英 0/1/2 立绘描述 + 下载链接、皮肤时装列表。

use crate::utils::wikitext::clean_wikitext;
use regex::Regex;
use std::sync::LazyLock;

static REX_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>").expect("regex BR"));

/// 从完整 wikitext 中提取形如 `|key=value`（单行）的字段值。
fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("|{key}=");
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed.strip_prefix(needle.as_str()).map(str::trim)
    })
}

/// 解析 GALLERY 域，生成立绘与时装信息的 Markdown 行列表。
///
/// 对应 JS `parseGallery(lines, mainText, characterName)`。
///
/// - `_lines` — 分类后的 gallery 桶行（当前直接从 `main_text` 提取，保留参数兼容）。
/// - `main_text` — 完整 wikitext。
/// - `character_name` — 干员页面标题（用于构造图片 URL）。
#[must_use]
pub fn parse_gallery(_lines: &[String], main_text: &str, character_name: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    if main_text.is_empty() {
        return result;
    }

    let default_artist =
        field(main_text, "画师").map_or_else(|| "未知".to_string(), clean_wikitext);
    let default_concept = field(main_text, "原案").map(clean_wikitext);

    let enc_name = urlencoding::encode(character_name).into_owned();
    let mut clothing_blocks: Vec<String> = Vec::new();

    // ── 精英 0 / 1 / 2 立绘 ──────────────────────────────────
    for i in 0u8..=2 {
        let key = format!("精英{i}介绍");
        if let Some(raw) = field(main_text, &key) {
            let text = REX_BR.replace_all(&clean_wikitext(raw), "\n").into_owned();
            let img_suffix = match i {
                0 => "1".to_string(),
                1 => "1%2B".to_string(),
                _ => "2".to_string(),
            };
            let img_url = format!(
                "https://prts.wiki/w/Special:FilePath/%E7%AB%8B%E7%BB%98_{enc_name}_{img_suffix}.png"
            );

            let mut block: Vec<String> = vec![format!("### 默认服装 - 精英{i}")];
            if let Some(ref c) = default_concept {
                block.push(format!("- **原案**: {c}"));
            }
            block.push(format!("- **原画**: {default_artist}"));
            block.push(format!("- **原图**: [下载]({img_url})"));
            block.push(format!("\n```text\n{text}\n```"));
            clothing_blocks.push(block.join("\n"));
        }
    }

    // ── 皮肤时装（最多检查 15 套）────────────────────────────
    for i in 1u8..=15 {
        let intro_key = format!("时装{i}介绍");
        if let Some(raw) = field(main_text, &intro_key) {
            let name = field(main_text, &format!("时装{i}名称"))
                .map_or_else(|| format!("时装{i}"), clean_wikitext);
            let series = field(main_text, &format!("时装{i}系列"))
                .map_or_else(|| "常规".to_string(), clean_wikitext);
            let artist = field(main_text, &format!("时装{i}画师"))
                .map_or_else(|| default_artist.clone(), clean_wikitext);
            let concept = field(main_text, &format!("时装{i}原案"))
                .map(clean_wikitext)
                .or_else(|| default_concept.clone());

            let text = REX_BR.replace_all(&clean_wikitext(raw), "\n").into_owned();
            let img_url = format!(
                "https://prts.wiki/w/Special:FilePath/%E7%AB%8B%E7%BB%98_{enc_name}_skin{i}.png"
            );

            let mut block: Vec<String> =
                vec![format!("### {name}"), format!("- **系列**: {series}")];
            if let Some(ref c) = concept {
                block.push(format!("- **原案**: {c}"));
            }
            block.push(format!("- **原画**: {artist}"));
            block.push(format!("- **原图**: [下载]({img_url})"));
            block.push(format!("\n```text\n{text}\n```"));
            clothing_blocks.push(block.join("\n"));
        }
    }

    if clothing_blocks.is_empty() {
        result.push("该干员暂无特殊服装描述与立绘信息。".to_string());
    } else {
        result.push(clothing_blocks.join("\n\n"));
    }

    result
}
