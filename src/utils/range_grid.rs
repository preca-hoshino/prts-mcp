#![allow(clippy::expect_used)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(missing_docs)]
//! 攻击范围 SVG → ASCII 网格转换工具。
//!
//! 对应 JS `utils.js` 中的 `svgToGrid()` 与 `getRangeGrid()`。
//! 本实现不做全局缓存，每次调用均发起 HTTP 请求。

use super::wiki_client::fetch_wikitext;
use regex::Regex;
use std::sync::LazyLock;

static REX_USE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<use\s+([^>]+)/?>(?:</use>)?").expect("regex USE"));

static REX_HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r##"(?:xlink:)?href="#(\d+)""##).expect("regex HREF"));

static REX_X: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"x="([-\d.]+)""#).expect("regex X"));

static REX_Y: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"y="([-\d.]+)""#).expect("regex Y"));

/// SVG 元素中的格子单元。
struct Cell {
    /// 格子类型：`'■'` 表示实心（`href="#1"`），`'□'` 表示空心。
    kind: char,
    /// 列坐标（已归一化为网格坐标）。
    x: i32,
    /// 行坐标（已归一化为网格坐标）。
    y: i32,
}

/// 将攻击范围 SVG wikitext 转换为 ASCII 文本网格。
///
/// 对应 JS `utils.js:svgToGrid()`。
///
/// # Returns
/// 若解析成功返回多行文本（`■`/`□`/全角空格），否则返回空字符串。
#[must_use]
pub fn svg_to_grid(svg: &str) -> String {
    let mut cells: Vec<Cell> = Vec::new();

    for cap in REX_USE.captures_iter(svg) {
        let attrs = cap.get(1).map_or("", |m| m.as_str());
        let Some(href_m) = REX_HREF.captures(attrs) else {
            continue;
        };
        let Some(x_m) = REX_X.captures(attrs) else {
            continue;
        };
        let Some(y_m) = REX_Y.captures(attrs) else {
            continue;
        };

        let kind = if href_m[1].trim() == "1" {
            '■'
        } else {
            '□'
        };
        let x_raw: f64 = x_m[1].parse().unwrap_or(0.0);
        let y_raw: f64 = y_m[1].parse().unwrap_or(0.0);

        #[allow(clippy::cast_possible_truncation)]
        cells.push(Cell {
            kind,
            x: (x_raw / 26.0).round() as i32,
            y: (y_raw / 26.0).round() as i32,
        });
    }

    if cells.is_empty() {
        return String::new();
    }

    let min_x = cells.iter().map(|c| c.x).min().unwrap_or(0);
    let max_x = cells.iter().map(|c| c.x).max().unwrap_or(0);
    let min_y = cells.iter().map(|c| c.y).min().unwrap_or(0);
    let max_y = cells.iter().map(|c| c.y).max().unwrap_or(0);

    let mut rows: Vec<String> = Vec::new();
    for row_y in min_y..=max_y {
        let mut row = String::new();
        for col_x in min_x..=max_x {
            let ch = cells
                .iter()
                .find(|c| c.y == row_y && c.x == col_x)
                .map_or('　', |c| c.kind);
            row.push(ch);
            row.push(' ');
        }
        rows.push(row.trim_end().to_string());
    }
    rows.join("\n")
}

/// 获取指定范围 ID 的 ASCII 网格。
///
/// 对应 JS `utils.js:getRangeGrid()`。
/// 若请求失败或解析结果为空，则回退返回 `range_id` 本身。
///
/// # Errors
/// 内部错误均被捕获并回退，不对外传播。
pub async fn get_range_grid(range_id: &str) -> String {
    let page = format!("Widget:Range/{range_id}");
    match fetch_wikitext(&page).await {
        Ok(svg) => {
            let grid = svg_to_grid(&svg);
            if grid.is_empty() {
                range_id.to_string()
            } else {
                grid
            }
        }
        Err(_) => range_id.to_string(),
    }
}
