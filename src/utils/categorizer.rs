#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! Wikitext 行语义分类器。
//!
//! 将整页 wikitext 按内容类型路由到 5 个语义桶，对应 JS `index.js:categorizeLines()`。

/// 按语义分类后的 wikitext 行集合。
#[derive(Debug, Default)]
pub struct WikiBlocks {
    /// 基础干员信息行（CharinfoV2 / 人员档案set 等）。
    pub basic: Vec<String>,
    /// 战斗数据行（属性、天赋、技能、模组、召唤物）。
    pub combat: Vec<String>,
    /// 基建后勤行（基建技能、精英化/专精材料）。
    pub build: Vec<String>,
    /// 档案文本行（干员档案、临床诊断、人员档案）。
    pub lore: Vec<String>,
    /// 图鉴立绘行（时装、立绘、图鉴）。
    pub gallery: Vec<String>,
}

/// 按语义将 wikitext 的各行路由到对应的语义桶。
///
/// 对应 JS `index.js:categorizeLines()`。
#[must_use]
pub fn categorize_lines(wikitext: &str) -> WikiBlocks {
    let mut blocks = WikiBlocks::default();
    let mut current: &str = "basic";

    for line in wikitext.lines() {
        // 跳过全局导航/广告行
        if line.starts_with("<references/>")
            || line.starts_with("{{干员导航}}")
            || line.contains("{{ads/")
        {
            continue;
        }

        // 处理 ==标题== 行，决定当前桶
        if let Some(header) = extract_section_header(line) {
            current = route_by_header(header);
            let pushed_line = format!("\n### {header}");
            push_to(&mut blocks, current, pushed_line);
            continue;
        }

        // 处理模板起始行，决定当前桶
        if line.starts_with("{{干员/技能")
            || line.starts_with("{{干员/面板")
            || line.starts_with("{{属性")
            || line.starts_with("{{干员/潜能提升")
            || line.starts_with("{{潜能")
        {
            current = "combat";
        } else if line.starts_with("{{干员/基建") || line.starts_with("{{后勤技能") {
            current = "build";
        } else if line.starts_with("{{人员档案") {
            current = "lore";
        } else if line.starts_with("{{干员信息") || line.starts_with("{{Charinfo") {
            current = "basic";
        }

        push_to(&mut blocks, current, line.to_string());
    }

    blocks
}

/// 从形如 `== 标题文字 ==` 的行中提取标题文字。
fn extract_section_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with("==") && trimmed.ends_with("==") {
        // 去掉前后的 `=` 号
        let inner = trimmed.trim_matches('=').trim();
        if !inner.is_empty() {
            return Some(inner);
        }
    }
    None
}

/// 根据标题文字将其路由到对应的语义桶名称。
fn route_by_header(header: &str) -> &'static str {
    let gallery_keys = ["干员模型", "时装", "立绘", "图鉴"];
    let build_keys = ["材料", "后勤"];
    let combat_keys = [
        "天赋",
        "技能",
        "潜能",
        "模组",
        "召唤物",
        "攻击范围",
        "特性",
        "属性",
        "面板",
    ];
    let lore_keys = ["干员档案", "背景故事", "履历"];

    if gallery_keys.iter().any(|k| header.contains(k)) {
        "gallery"
    } else if build_keys.iter().any(|k| header.contains(k)) {
        "build"
    } else if combat_keys.iter().any(|k| header.contains(k)) {
        "combat"
    } else if lore_keys.iter().any(|k| header.contains(k)) {
        "lore"
    } else {
        "basic"
    }
}

/// 将字符串追加到对应语义桶。
fn push_to(blocks: &mut WikiBlocks, current: &str, line: String) {
    match current {
        "combat" => blocks.combat.push(line),
        "build" => blocks.build.push(line),
        "lore" => blocks.lore.push(line),
        "gallery" => blocks.gallery.push(line),
        _ => blocks.basic.push(line),
    }
}
