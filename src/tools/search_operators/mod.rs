#![allow(missing_docs)]
//! `search_operators` MCP Tool 模块。
//!
//! 提供对 PRTS Wiki 的模糊干员搜索能力，通过 `MediaWiki` 搜索 API
//! 并结合 `分类:干员` 验证，确保结果只包含干员页面。

pub mod tool;

pub use tool::SearchOperatorsTool;
