#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! 语音台词解析器（VOICE 域）。
//!
//! 对应 JS `temp/src/parsers/voice.js`。
//! 从 `/{name}/语音记录` 子页 wikitext 中提取中日文台词及音频下载链接。

use crate::utils::wikitext::clean_wikitext;
use regex::Regex;
use std::sync::LazyLock;

static REX_VOICE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|语音key=([^\n|]+)").expect("regex VOICE_KEY"));

static REX_PATHS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|路径=([^\n]+)").expect("regex PATHS"));

static REX_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+)=(.*?)(?:\n|$)").expect("regex TITLE"));

static REX_VOICE_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|语音\d+=([^\n|]+)").expect("regex VOICE_FILE"));

static REX_VOICEDATA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{VoiceData/word\|([^|}]+)\|([\s\S]*?)\}\}").expect("regex VOICEDATA")
});

const BASE_CN: &str = "https://torappu.prts.wiki/assets/audio/voice_cn";
const BASE_JP: &str = "https://torappu.prts.wiki/assets/audio/voice";

/// 解析语音记录页 wikitext，生成语音台词 Markdown 行列表。
///
/// 对应 JS `parseVoice(voiceWikitext)`。
///
/// - `voice_wikitext` — 干员 `/语音记录` 子页的 wikitext。
#[must_use]
pub fn parse_voice(voice_wikitext: &str) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    // 提取语音 key
    let voice_key = REX_VOICE_KEY
        .captures(voice_wikitext)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    // 解析路径字段，分别获取中文和日文目录
    let mut cn_folder = voice_key.clone();
    let mut jp_folder = voice_key.clone();

    if let Some(paths_cap) = REX_PATHS.captures(voice_wikitext) {
        for part in paths_cap[1].split(',') {
            let mut kv = part.splitn(2, ':');
            let lang = kv.next().map_or("", str::trim);
            let path = kv.next().map_or("", str::trim);
            if lang == "中文-普通话" {
                cn_folder = path.trim_start_matches("voice_cn/").to_string();
            } else if lang == "日语" {
                jp_folder = path.trim_start_matches("voice/").to_string();
            }
        }
    }

    // 按 |标题 切分语音块
    let chunks: Vec<&str> = voice_wikitext.split("|标题").collect();
    for chunk in chunks.iter().skip(1) {
        // 提取标题行
        let Some(title_cap) = REX_TITLE.captures(chunk) else {
            continue;
        };
        let title = title_cap[2].trim();

        // 提取台词块：找到 `|台词\d+=` 开头的行，取到下一个 `|` 开头的行之前
        let raw_lyrics = extract_lyrics_text(chunk);
        if raw_lyrics.is_empty() {
            continue;
        }
        let text_chunk = clean_wikitext(&raw_lyrics);

        // 提取音频文件名
        let voice_file = REX_VOICE_FILE
            .captures(chunk)
            .map(|c| c[1].trim().to_lowercase())
            .unwrap_or_default();

        let cn_url = if voice_file.is_empty() {
            String::new()
        } else {
            format!("{BASE_CN}/{cn_folder}/{voice_file}")
        };
        let jp_url = if voice_file.is_empty() {
            String::new()
        } else {
            format!("{BASE_JP}/{jp_folder}/{voice_file}")
        };

        result.push(format!("\n### {title}"));

        let vd_matches: Vec<_> = REX_VOICEDATA.captures_iter(&text_chunk).collect();

        if vd_matches.is_empty() {
            // 旧干员裸文本回退
            result.push(format!("> [中] {text_chunk}"));
            if !cn_url.is_empty() {
                result.push(format!("> [中]({cn_url})\n>"));
            }
            if !jp_url.is_empty() {
                result.push(format!("> [日]({jp_url})"));
            }
        } else {
            let mut cn_texts: Vec<String> = Vec::new();
            let mut jp_texts: Vec<String> = Vec::new();

            for m in &vd_matches {
                let lang = m[1].trim();
                let content = m[2].trim();

                // 忽略皮肤追加语音（括号标注）
                let is_skin = content.contains('(') || content.contains('（');
                if lang.starts_with("中文") && !is_skin {
                    cn_texts.push(format!("> [中] {content}"));
                } else if lang.starts_with("日文") && !is_skin {
                    jp_texts.push(format!("> [日] {content}"));
                }
            }

            if !cn_texts.is_empty() {
                result.extend(cn_texts);
                if !cn_url.is_empty() {
                    result.push(format!("> [中]({cn_url})\n>"));
                }
            }
            if !jp_texts.is_empty() {
                result.extend(jp_texts);
                if !jp_url.is_empty() {
                    result.push(format!("> [日]({jp_url})"));
                }
            }
        }
    }

    result
}

/// 从语音块 chunk 中提取台词内容。
/// 找 `|台词\d+=` 开头的行，收集到下一个 `|` 开头的行之前。
fn extract_lyrics_text(chunk: &str) -> String {
    static REX_LYRIC_START: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\|台词\d+=").expect("regex LYRIC_START"));

    let mut collecting = false;
    let mut buf = String::new();

    for line in chunk.lines() {
        if REX_LYRIC_START.is_match(line) {
            let eq = line.find('=').unwrap_or(line.len());
            buf = line[eq + 1..].to_string();
            collecting = true;
        } else if collecting {
            if line.starts_with('|') {
                break;
            }
            buf.push('\n');
            buf.push_str(line);
        }
    }

    buf.trim().to_string()
}
