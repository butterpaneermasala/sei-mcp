# Sei Faucet API (TypeScript)

HTTP faucet for both Sei EVM testnet and Sei Native (Cosmos). Includes:

- IP rate limit (default: 2 requests / 60s)
- Per-address cooldown (default: 24h)
- EVM transfers via `ethers`
- Native transfers via `@cosmjs/*`
- Redis (Upstash) for durable limits/cooldowns

## Endpoints

- `GET /` — health + config snapshot
- `POST /faucet/request`
  - Body (EVM): `{ "address": "0x...", "chain": "sei-evm-testnet" }`
  - Body (Native): `{ "address": "sei1...", "chain": "sei-native-testnet" }`
  - Responses:
    - 200: `{ "txHash": "0x...|<hash>", "amount": "100000", "denom": "usei", "chain": "..." }`
    - 429: cooldown or rate limit info
    - 4xx/5xx: error details

## Environment Variables

Create a `.env` file (not committed) with:

```
# Server
PORT=3000

# Redis (Upstash free)
REDIS_URL=rediss://<user>:<pass>@<host>:<port>

# RPCs
CHAIN_RPC_URLS={"sei-evm-testnet":"https://evm-rpc-testnet.sei-apis.com","sei-native-testnet":"https://rpc-testnet.sei-apis.com"}

# Amount/denom
FAUCET_AMOUNT_USEI=100000
FAUCET_DENOM=usei

# Rate limits
FAUCET_RATE_WINDOW_SECS=60
FAUCET_RATE_MAX=2
FAUCET_ADDRESS_COOLDOWN_SECS=86400

# Native chain params
NATIVE_CHAIN_ID=atlantic-2
NATIVE_BECH32_HRP=sei

# Keys (set one of mnemonic or private key for native)
FAUCET_PRIVATE_KEY_EVM=0xYOUR_EVM_PVK
FAUCET_MNEMONIC_NATIVE="your mnemonic here"
# or
# FAUCET_PRIVATE_KEY_NATIVE=hex_without_0x
```

## Local Development

```
npm ci
npm run dev
# or
npm run build && npm start
```

## Deploy (Render free + Upstash)

1) Create Upstash Redis (free). Copy `REDIS_URL`.
2) Push this repo to GitHub.
3) Create a new Render Web Service:
   - Root: `faucet-api/`
   - Build: `npm ci && npm run build`
   - Start: `node dist/index.js`
   - Environment: set variables from `.env` (do not commit secrets)
4) Deploy; note the base URL.

## Notes

- Keep secrets off the client and MCP configs; only set them in the hosting platform environment.
- The service enforces cooldowns and IP rate limits via Redis TTLs.
- Adjust `FAUCET_AMOUNT_USEI`, fee/gas in code if needed.

### Single key for both chains

You can provide a single 0x-prefixed secp256k1 key and the server will use it for both EVM and native:

```
FAUCET_PRIVATE_KEY=0xYOUR_PRIVATE_KEY
```

Per-chain envs are also supported and override the shared key if set:

```
FAUCET_PRIVATE_KEY_EVM=0x...
FAUCET_PRIVATE_KEY_NATIVE=0x... # can also be hex without 0x
```

### Curl examples

EVM faucet request:

```bash
curl -sS -X POST http://localhost:3000/faucet/request \
  -H 'content-type: application/json' \
  -d '{"address":"0xYourEvmAddress","chain":"sei-evm-testnet"}'
```

Native faucet request:

```bash
curl -sS -X POST http://localhost:3000/faucet/request \
  -H 'content-type: application/json' \
  -d '{"address":"sei1YourNativeAddress","chain":"sei-native-testnet"}'
```
