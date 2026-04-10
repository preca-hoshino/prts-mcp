#![allow(missing_docs)]
//! Implementation of the `HelloWorld` tool.

use rust_mcp_sdk::macros::JsonSchema;
use rust_mcp_sdk::macros::mcp_tool;
use rust_mcp_sdk::schema::schema_utils::CallToolError;
use rust_mcp_sdk::schema::{CallToolResult, TextContent};

/// A simple tool that accepts an argument and returns "hello,world".
#[mcp_tool(name = "helloworld", description = "helloword")]
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize, JsonSchema)]
pub struct HelloWorldTool {
    /// The dummy argument (unused).
    #[allow(dead_code)]
    arg: String, // Accepts exactly one parameter
}

impl HelloWorldTool {
    /// Executes the tool logic and returns the `helloworld` string.
    ///
    /// # Errors
    /// Returns `CallToolError` if there's any failure (not applicable here).
    pub fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        Ok(CallToolResult::text_content(vec![TextContent::from(
            "hello,world".to_string(),
        )]))
    }
}
