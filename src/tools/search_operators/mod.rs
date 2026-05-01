#![allow(missing_docs)]
//! `search_operators` MCP Tool 模块。
//!
//! 在 PRTS Wiki 搜索明日方舟干员，支持关键词全文搜索加多维度属性过滤
//! （职业、稀有度、站位、获取途径、公招词缀）。
//!
//! 搜索结果（集合 A）与属性过滤结果（集合 B）取交集返回；
//! 无过滤参数时退化为纯搜索模式，与原有行为完全兼容。

pub(super) mod api;
pub(super) mod format;
pub mod strings;
pub mod tool;

pub use tool::SearchOperatorsTool;
