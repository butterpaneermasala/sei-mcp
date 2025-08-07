# Sei MCP Server in Rust

A Model Context Protocol (MCP) server implementation for Sei blockchain operations, built in Rust.

## Features

This MCP server provides blockchain tools for:

- **Balance Queries**: Get wallet balances on supported chains
- **Wallet Management**: Create new wallets or import existing ones
- **Transaction History**: View transaction history (Sei chain only)
- **Fee Estimation**: Estimate transaction fees
- **Health Checks**: Server status monitoring

## Prerequisites

- Rust (latest stable version)
- Environment variables for blockchain RPC endpoints

## Environment Setup

Create a `.env` file in the project root:

```env
CHAIN_RPC_URLS=sei=https://rpc.sei.io
PORT=3000
```

## Running Modes

### MCP Server Mode (for AI assistants)

Run as an MCP server for integration with Claude Desktop, Cursor, or other MCP clients:

```bash
cargo run -- --mcp
```

### HTTP API Mode (traditional REST API)

Run as a regular HTTP server:

```bash
cargo run
```

The HTTP server will be available at `http://localhost:3000` with endpoints:
- `GET /health` - Health check
- `GET /balance/{chain_id}/{address}` - Get balance
- `GET /history/{chain_id}/{address}` - Get transaction history
- `POST /wallet/create` - Create wallet
- `POST /wallet/import` - Import wallet
- `POST /fees/estimate` - Estimate fees

## MCP Integration

### Claude Desktop Configuration

Add to your Claude Desktop `mcp.json`:

```json
{
  "mcpServers": {
    "sei-mcp-server": {
      "command": "cargo",
      "args": ["run", "--", "--mcp"],
      "cwd": "/path/to/sei-mcp-server-rs",
      "env": {
        "CHAIN_RPC_URLS": "sei=https://rpc.sei.io",
        "PORT": "3000"
      }
    }
  }
}
```

### Cursor/VS Code Configuration  

Add to your VS Code settings or `.vscode/mcp.json`:

```json
{
  "mcp": {
    "servers": {
      "sei-mcp-server": {
        "command": "cargo",
        "args": ["run", "--", "--mcp"],
        "cwd": "/path/to/sei-mcp-server-rs",
        "env": {
          "CHAIN_RPC_URLS": "sei=https://rpc.sei.io",
          "PORT": "3000"
        }
      }
    }
  }
}
```

## Available MCP Tools

### `get_balance`
Get the balance of an address on a specific blockchain.

**Parameters:**
- `chain_id` (string): The blockchain chain ID (e.g., "sei")
- `address` (string): The wallet address to check

### `create_wallet`
Create a new wallet with mnemonic phrase.

**Parameters:** None

### `import_wallet`
Import a wallet from mnemonic phrase or private key.

**Parameters:**
- `mnemonic_or_private_key` (string): The mnemonic phrase or private key

### `get_transaction_history`
Get transaction history for an address (Sei chain only).

**Parameters:**
- `chain_id` (string): The blockchain chain ID (currently only "sei" supported)
- `address` (string): The wallet address
- `limit` (integer, optional): Number of transactions to return (default: 20, max: 100)

### `estimate_fees`
Estimate transaction fees for a transfer.

**Parameters:**
- `chain_id` (string): The blockchain chain ID
- `from` (string): The sender address
- `to` (string): The recipient address  
- `amount` (string): The amount to send

## Example Usage

Once connected to an MCP client, you can use natural language to interact with the blockchain:

- "What's the balance of address 0x31781a5B8ABBFeCd35421f37397E5251fC19a344 on Sei?"
- "Create a new wallet for me"
- "Show the transaction history for address 0x... on Sei"
- "Estimate fees to send 1000 tokens from 0x... to 0x... on Sei"

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Run with logs

```bash
RUST_LOG=debug cargo run -- --mcp
```

## Project Structure

```
src/
├── main.rs              # Entry point with HTTP server configuration (CORS, rate limiting)
├── mcp.rs               # MCP server implementation
├── config.rs            # Configuration and environment management
├── api/                 # HTTP API handlers with input validation
│   ├── balance.rs       # Balance query endpoints
│   ├── wallet.rs        # Wallet management endpoints
│   ├── history.rs       # Transaction history endpoints
│   ├── fees.rs          # Fee estimation endpoints
│   └── health.rs        # Health check endpoint
└── blockchain/          # Blockchain interaction layer
    ├── client.rs        # HTTP client with timeouts and retries
    ├── models.rs        # Data structures and validation
    └── services/        # Blockchain service implementations
        ├── balance.rs   # Balance query service
        ├── fees.rs      # Fee estimation service
        ├── history.rs   # Transaction history service
        ├── wallet.rs    # Wallet management service
        └── transactions.rs # Transaction operations
```

## Security Features

- Rate limiting: 2 requests per second with burst allowance of 5
- Request timeouts: 30 seconds for blockchain RPC calls
- CORS: Configurable allowed origins
- Compression: Response compression enabled
- Input validation: Request payload validation
- Logging: Structured logging with configurable levels
- Environment separation: .env.example provided for configuration

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

[Add your license here] 