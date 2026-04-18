//! Request handlers for the MCP Server.

use crate::resources::operator_list::fetch_operator_list_markdown;
use crate::tools::OperatorTools;
use async_trait::async_trait;
use rust_mcp_sdk::{
    McpServer,
    mcp_server::ServerHandler,
    schema::{
        CallToolRequestParams, CallToolResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceContent, ReadResourceRequestParams, ReadResourceResult,
        Resource, RpcError, TextResourceContents, schema_utils::CallToolError,
    },
};
use std::sync::Arc;

/// A custom handler implementing the `ServerHandler` trait for this server.
#[derive(Debug)]
pub struct MyServerHandler;

#[async_trait]
impl ServerHandler for MyServerHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: OperatorTools::tools(),
        })
    }

    async fn handle_list_resources_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ListResourcesResult, RpcError> {
        Ok(ListResourcesResult {
            meta: None,
            next_cursor: None,
            resources: vec![Resource {
                uri: "prts://operators".into(),
                name: "干员列表".into(),
                description: Some(
                    "PRTS Wiki所有的干员数据列表，包含编号，中文名，外文名，职业，职业分支，稀有度"
                        .into(),
                ),
                mime_type: Some("text/markdown".into()),
                annotations: None,
                icons: vec![],
                meta: None,
                size: None,
                title: None,
            }],
        })
    }

    async fn handle_read_resource_request(
        &self,
        params: ReadResourceRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<ReadResourceResult, RpcError> {
        if params.uri == "prts://operators" {
            let markdown = fetch_operator_list_markdown().await.map_err(|e| {
                RpcError::internal_error().with_message(format!("Failed to fetch operators: {e}"))
            })?;

            Ok(ReadResourceResult {
                meta: None,
                contents: vec![ReadResourceContent::TextResourceContents(
                    TextResourceContents {
                        uri: params.uri,
                        mime_type: Some("text/markdown".into()),
                        text: markdown,
                        meta: None,
                    },
                )],
            })
        } else {
            Err(RpcError::invalid_params()
                .with_message(format!("Unknown resource URI: {}", params.uri)))
        }
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn McpServer>,
    ) -> std::result::Result<CallToolResult, CallToolError> {
        let tool_params: OperatorTools =
            OperatorTools::try_from(params).map_err(CallToolError::new)?;

        match tool_params {
            OperatorTools::GetOperatorTool(tool) => tool.call_tool().await,
            OperatorTools::SearchOperatorsTool(tool) => tool.call_tool().await,
        }
    }
}
