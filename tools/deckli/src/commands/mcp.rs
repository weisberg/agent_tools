use serde_json::Value;

/// Run deckli as an MCP server over stdio.
///
/// Implements the Model Context Protocol (JSON-RPC over stdin/stdout),
/// exposing all deckli commands as MCP tools for native Claude Code integration.
pub async fn run() -> Result<Value, Box<dyn std::fmt::Display>> {
    // TODO: Implement MCP server
    //
    // The MCP server will:
    // 1. Read JSON-RPC requests from stdin
    // 2. Translate them to bridge WebSocket commands
    // 3. Write JSON-RPC responses to stdout
    //
    // Tools to expose:
    // - inspect_presentation, inspect_masters, inspect_theme
    // - get_slide, get_shape, get_selection
    // - set_text, set_fill, set_font, set_geometry
    // - add_slide, add_shape, add_image, add_table
    // - remove_slide, remove_shape
    // - move_slide
    // - render_slide (returns base64 PNG inline)
    // - batch_operations

    eprintln!("deckli mcp-serve: not yet implemented");
    Err(Box::new(McpError("MCP server not yet implemented".to_string())))
}

#[derive(Debug)]
struct McpError(String);
impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
