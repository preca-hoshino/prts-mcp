#![allow(missing_docs)]
//! 本模块统一管理所有 Tool 定义，以及各域的 wikitext 解析器。

pub mod get_operator;
pub mod parsers;

use get_operator::GetOperatorTool;
use rust_mcp_sdk::tool_box;

tool_box!(OperatorTools, [GetOperatorTool]);
