# Connecting External Tools (MCP Servers & Chrome)

This guide shows how to connect external tools like Google Chrome, Continue.dev, and other MCP-compatible servers to Indigo.

## MCP Server Registration

### 1. Chrome Extension/DevTools
```bash
curl -X POST http://localhost:3001/v1/mcp-servers \
  -H "Content-Type: application/json" \
  -d '{
    "server_name": "chrome",
    "server_type": "stdio",
    "args": ["chrome-devtools", "--stdio"],
    "description": "Chrome DevTools automation and page interaction"
  }'
```

### 2. Continue.dev Integration
```bash
curl -X POST http://localhost:3001/v1/mcp-servers \
  -H "Content-Type: application/json" \
  -d '{
    "server_name": "continue",
    "server_type": "http", 
    "endpoint": "http://localhost:3131/mcp",
    "description": "Continue.dev AI coding assistant integration"
  }'
```

### 3. Filesystem MCP Server (for web chat)
```bash
curl -X POST http://localhost:3001/v1/mcp-servers \
  -H "Content-Type: application/json" \
  -d '{
    "server_name": "filesystem",
    "server_type": "http",
    "endpoint": "http://localhost:3232/mcp", 
    "description": "Secure file system access for web chat"
  }'
```

## Tool Context Filtering

Indigo now supports context-aware tool filtering:

### Web Chat Context (`?context=web`)
- ✅ **Available**: web_search, analyze_project, MCP HTTP tools
- ❌ **Filtered**: list_files, read_file, write_file, run_shell
- 🔒 **Security**: No direct filesystem or shell access

### CLI Interface Context (`?context=cli`)  
- ✅ **Available**: All native tools (bash, filesystem), MCP servers
- 📁 **Full Access**: Complete system access for OpenCode/Continue
- 🌐 **Universal**: Tools marked `Both` context available everywhere

## Available Tools After Integration

### Web Chat Tools
```
📊 analyze_project - Analyze project structure and provide insights
🔍 web_search - Search web for information  
🔧 MCP tools - Chrome, Continue, and other registered MCP servers
```

### CLI Tools (OpenCode/Continue)
```
📂 list_files - List files and directories
📖 read_file - Read file contents  
✏️  write_file - Write content to files
⚡ run_shell - Execute shell commands
📊 analyze_project - Project analysis (universal tool)
🔧 MCP tools - External tool integrations
```

## MCP Server Requirements

### stdio MCP Servers
- Must accept commands via stdin
- Return responses via stdout  
- Use JSON-RPC 2.0 protocol
- Example: `chrome-devtools --stdio`

### HTTP MCP Servers  
- Must implement `/health` endpoint
- Must accept POST requests with JSON-RPC payloads
- Return structured tool responses
- Example: Continue.dev MCP server

## Testing Integration

### 1. Test Tool Registration
```bash
# List all MCP servers
curl http://localhost:3001/v1/mcp-servers

# Test individual server
curl http://localhost:3001/v1/mcp-servers/chrome
```

### 2. Test Tool Execution
```bash
# Via Web Interface (Chrome DevTools)
# Web chat → Send message → Should show Chrome tools available

# Via CLI (OpenCode/Continue) 
# Should have access to all tools + registered MCP servers
```

### 3. Verify Context Filtering
```bash
# Web context (no filesystem tools)
curl "http://localhost:3001/v1/tools?context=web"

# CLI context (all tools)
curl "http://localhost:3001/v1/tools?context=cli"
```

## Security Considerations

### Web Chat Isolation
- 🚫 No direct filesystem access
- 🚫 No shell execution
- ✅ Sandbox-protected MCP tools
- ✅ Context-enforced filtering

### CLI Interface Power
- 📁 Full filesystem access
- ⚡ Shell command execution  
- 🔧 MCP server integrations
- 🛡️ Permission-based tool access

## Troubleshooting

### MCP Server Not Connecting
```bash
# Check server health
curl http://localhost:3001/v1/mcp-servers

# Verify MCP server is running
curl http://localhost:3131/mcp/health  # Continue.dev
```

### Tools Missing in Web Chat
```bash
# Check context filtering works
curl "http://localhost:3001/v1/tools?context=web"

# Should show web_search, analyze_project, MCP tools only
```

### OpenCode Shows "Plan Mode"
- ✅ Fixed in Phase 2 - tools now execute properly
- ✅ Structured tool responses returned
- ✅ No more plain text responses

## Tool Development

### Creating Custom MCP Tools
1. Implement JSON-RPC 2.0 server
2. Add tool definitions to `/tools` endpoint
3. Register with Indigo hub via `/v1/mcp-servers`
4. Tools automatically available to appropriate contexts

### Example MCP Server
```json
{
  "name": "custom_tool",
  "description": "Custom tool functionality",
  "parameters": {"type": "object", "properties": {...}},
  "context": "both"  // "web", "cli", or "both"
}
```

This architecture provides secure, context-aware tool access across web chat and CLI interfaces while enabling rich external tool integrations.