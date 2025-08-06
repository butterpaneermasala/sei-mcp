# MCP Server API Documentation

## Health Check
- **GET /health**
  - Returns `{ "status": "ok" }` if the server is running.

## Get Balance
- **GET /balance/{chain_id}/{address}**
  - Returns the balance for the given address on the specified chain.
  - **Response:**
    ```json
    {
      "chain_id": "sei",
      "address": "0x...",
      "balance": "123456789",
      "denom": "usei"
    }
    ```

## Get Transaction History
- **GET /history/{chain_id}/{address}?limit=20**
  - Returns recent transactions for the address. Only the `sei` chain is supported. Data is fetched from the public Seistream indexer (not the node RPC).
  - Optional `limit` query param (default: 20 transactions, max: 100).
  - **Response:**
    ```json
    {
      "address": "0x...",
      "transactions": [
        {
          "tx_hash": "0x...",
          "from_address": "0x...",
          "to_address": "0x...",
          "amount": "1000",
          "denom": "usei",
          "timestamp": "2025-08-06T12:34:56Z"
        }
      ]
    }
    ```

## Create Wallet
- **POST /wallet/create**
  - Returns a new wallet (address, private key, mnemonic).

## Import Wallet
- **POST /wallet/import**
  - **Body:** `{ "mnemonic_or_private_key": "..." }`
  - Returns the wallet address and private key.

## Estimate Fees
- **POST /fees/estimate**
  - **Body:**
    ```json
    {
      "chain_id": "sei",
      "from": "0x...",
      "to": "0x...",
      "amount": "1000"
    }
    ```
  - Returns estimated gas, gas price, and total fee.
