# MCP Server Troubleshooting Guide

## Current Status
✅ MCP server builds successfully  
✅ MCP server runs correctly  
✅ MCP server responds to all commands  
✅ All tools are available and working  

## Configuration Files
- **Global MCP Config**: `~/.cursor/mcp.json` (Cursor-wide configuration)
- **Local MCP Config**: `./mcp.json` (Project-specific configuration)
- **Binary Location**: `./target/release/sei-mcp-server-rs`

## MCP Server Features
The server provides 20 blockchain tools:
- Wallet operations (create, import, register, list, remove)
- Balance queries
- Transaction history
- Fee estimation
- Token transfers
- Event searching
- Health monitoring

## Troubleshooting Steps

### 1. Restart Cursor
After updating MCP configuration, restart Cursor completely:
- Close Cursor
- Wait a few seconds
- Reopen Cursor
- Open the project

### 2. Check MCP Server Status
Test if the server is working:
```bash
./test_mcp_server.sh
```

### 3. Verify Configuration
Ensure both configuration files are correct:
- Global: `~/.cursor/mcp.json`
- Local: `./mcp.json`

### 4. Check Cursor Logs
Look for MCP-related errors in Cursor's developer console:
- Press `Ctrl+Shift+I` (or `Cmd+Option+I` on Mac)
- Check Console tab for errors

### 5. Manual MCP Server Test
Test the server manually:
```bash
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}' | ./target/release/sei-mcp-server-rs --mcp
```

## Expected Behavior
When working correctly, Cursor should:
1. Automatically detect the MCP server
2. Load all available tools
3. Allow you to use blockchain operations directly in chat
4. Show MCP tools in the available functions list

## Common Issues
1. **Cursor not restarting**: Force quit and restart
2. **Path issues**: Ensure binary path is correct
3. **Permission issues**: Ensure binary is executable
4. **Configuration conflicts**: Check both global and local configs

## Next Steps
If the server still doesn't work automatically:
1. Check Cursor's MCP documentation
2. Verify Cursor version supports MCP
3. Check for any Cursor-specific MCP requirements
4. Consider using the HTTP API as an alternative
