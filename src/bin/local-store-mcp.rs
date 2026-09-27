fn main() {
    if let Err(error) = local_store::agent_mcp::serve_stdio() {
        eprintln!("Local Store MCP: {error}");
        std::process::exit(1);
    }
}
