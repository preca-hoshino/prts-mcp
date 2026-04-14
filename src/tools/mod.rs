#![allow(missing_docs)]
//! This module groups all the tool definitions and handles the macro to expose them as a unified set.

pub mod get_operator;

use get_operator::GetOperatorTool;
use rust_mcp_sdk::tool_box;

tool_box!(OperatorTools, [GetOperatorTool]);
