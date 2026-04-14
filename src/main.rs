//! PRTS MCP Server
//!
//! This is the main entry point for the PRTS MCP server, using `rust-mcp-sdk`.

pub mod handler;
pub mod prompts;
pub mod resources;
pub mod tools;
pub mod utils;

use handler::MyServerHandler;
use rust_mcp_sdk::{
    McpServer, StdioTransport, ToMcpServerHandler, TransportOptions,
    error::SdkResult,
    mcp_server::{McpServerOptions, ServerRuntime, server_runtime},
    schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesTools,
    },
};
use std::sync::Arc;

/// The main entry point to start the MCP server.
///
/// # Errors
/// Returns an error if the stdio transport fails to initialize or server fails to start.
#[tokio::main]
async fn main() -> SdkResult<()> {
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "prts-mcp".into(),
            version: "0.1.0".into(),
            title: Some("PRTS MCP Server".into()),
            description: Some("Arknights PRTS Wiki Data Provider".into()),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            resources: Some(rust_mcp_sdk::schema::ServerCapabilitiesResources {
                subscribe: Some(false),
                list_changed: Some(false),
            }),
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        meta: None,
        instructions: Some("Provides Arknights operator data from PRTS wiki".into()),
        protocol_version: ProtocolVersion::V2025_11_25.into(),
    };

    let transport = StdioTransport::new(TransportOptions::default())?;
    let handler = MyServerHandler {};

    let server: Arc<ServerRuntime> = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });

    if let Err(start_error) = server.start().await {
        eprintln!("{start_error}");
    }

    Ok(())
}
