#![allow(missing_docs)]
//! `get_operator` MCP Tool 完整特性模块。
//!
//! 包含 Tool 定义、路由引擎及 6 个干员数据域的渲染器。

pub mod tool;

pub(super) mod basic;
pub(super) mod build;
pub(super) mod combat;
pub(super) mod gallery;
pub(super) mod lore;
pub(super) mod strings;
pub(super) mod voice;

pub use tool::GetOperatorTool;
