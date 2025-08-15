import 'dotenv/config';
import express from 'express';
import morgan from 'morgan';
import { z } from 'zod';
import { getRedis } from './lib/redis';
import { sendEvm } from './lib/evm';
import { sendNative } from './lib/native';

const app = express();
app.use(express.json());
app.use(morgan('dev'));

const PORT = parseInt(process.env.PORT || '3000', 10);

// Config & defaults
const RATE_WINDOW_SECS = parseInt(process.env.FAUCET_RATE_WINDOW_SECS || '60', 10);
const RATE_MAX = parseInt(process.env.FAUCET_RATE_MAX || '2', 10);
const COOLDOWN_SECS = parseInt(process.env.FAUCET_ADDRESS_COOLDOWN_SECS || '86400', 10);
const AMOUNT_USEI = process.env.FAUCET_AMOUNT_USEI || '100000';
const DENOM = process.env.FAUCET_DENOM || 'usei';

// RPCs
const chainRpcUrls = (() => {
  try {
    return JSON.parse(process.env.CHAIN_RPC_URLS || '{}') as Record<string, string>;
  } catch (e) {
    return {} as Record<string, string>;
  }
})();

const CHAINS = {
  evm: 'sei-evm-testnet',
  native: 'sei-native-testnet',
} as const;

type ChainKey = typeof CHAINS[keyof typeof CHAINS];

const bodySchema = z.object({
  address: z.string().min(1),
  chain: z.enum([CHAINS.evm, CHAINS.native]),
});

app.get('/', (_req, res) => {
  res.json({ ok: true, chains: CHAINS, rate: { RATE_WINDOW_SECS, RATE_MAX }, cooldown: COOLDOWN_SECS });
});

// Lightweight health endpoint with soft Redis check
app.get('/health', async (_req, res) => {
  const envSummary = {
    hasRedisUrl: Boolean(process.env.REDIS_URL),
    hasRpcEvm: Boolean(chainRpcUrls[CHAINS.evm]),
    hasRpcNative: Boolean(chainRpcUrls[CHAINS.native]),
  };
  let redisStatus: 'up' | 'down' = 'down';
  try {
    if (process.env.REDIS_URL) {
      const r = getRedis();
      // Ensure health stays snappy: bound Redis ping with a 1s timeout
      const ping = r.ping();
      await Promise.race([
        ping,
        new Promise((_, reject) => setTimeout(() => reject(new Error('redis_ping_timeout')), 1000)),
      ]);
      redisStatus = 'up';
    }
  } catch {
    redisStatus = 'down';
  }
  res.json({ ok: true, port: PORT, uptimeSecs: Math.floor(process.uptime()), redis: redisStatus, env: envSummary });
});

app.post('/faucet/request', async (req, res) => {
  const parse = bodySchema.safeParse(req.body);
  if (!parse.success) {
    return res.status(400).json({ error: 'invalid_body', details: parse.error.flatten() });
  }
  const { address, chain } = parse.data;

  const ip = (req.headers['x-forwarded-for'] as string)?.split(',')[0]?.trim() || req.socket.remoteAddress || 'unknown';

  console.log('faucet_request_in', { chain, ip, addressMasked: address.slice(0, 6) + '...' + address.slice(-4) });

  // Basic rate limiting per IP
  try {
    await ensureRateLimit(ip);
  } catch (err: any) {
    if (err?.code === 'redis_unavailable') {
      console.error('rate_limit_skipped_redis_unavailable');
      return res.status(503).json({ error: 'redis_unavailable' });
    }
    return res.status(429).json({ error: 'rate_limited', retryAfterSecs: err?.retryAfterSecs ?? RATE_WINDOW_SECS });
  }

  // Address cooldown per chain
  const cooldownKey = `cooldown:${chain}:${address.toLowerCase()}`;
  let redis;
  try {
    redis = getRedis();
  } catch (e) {
    console.error('cooldown_check_redis_unavailable');
    return res.status(503).json({ error: 'redis_unavailable' });
  }
  const ttl = await redis.ttl(cooldownKey);
  if (ttl > 0) {
    const retryAt = new Date(Date.now() + ttl * 1000).toISOString();
    return res.status(429).json({ error: 'cooldown_active', retryAt });
  }

  try {
    let result: { txHash: string };
    if (chain === CHAINS.evm) {
      const rpc = chainRpcUrls[CHAINS.evm];
      if (!rpc) return res.status(500).json({ error: 'rpc_not_configured', chain });
      result = await sendEvm({
        rpcUrl: rpc,
        privateKey: process.env.FAUCET_PRIVATE_KEY_EVM || process.env.FAUCET_PRIVATE_KEY || '',
        to: address,
        amountUsei: AMOUNT_USEI,
      });
    } else {
      const rpc = chainRpcUrls[CHAINS.native];
      if (!rpc) return res.status(500).json({ error: 'rpc_not_configured', chain });
      result = await sendNative({
        rpcUrl: rpc,
        denom: DENOM,
        amountUsei: AMOUNT_USEI,
        toBech32: address,
        bech32Hrp: process.env.NATIVE_BECH32_HRP || 'sei',
        chainId: process.env.NATIVE_CHAIN_ID || 'atlantic-2',
        mnemonic: process.env.FAUCET_MNEMONIC_NATIVE,
        privateKeyHex: process.env.FAUCET_PRIVATE_KEY_NATIVE || process.env.FAUCET_PRIVATE_KEY,
      });
    }

    // Set cooldown
    await redis.set(cooldownKey, '1', 'EX', COOLDOWN_SECS);

    const resp = { txHash: result.txHash, amount: AMOUNT_USEI, denom: DENOM, chain };
    console.log('faucet_request_success', { chain, txHash: result.txHash });
    return res.json(resp);
  } catch (err: any) {
    console.error('faucet_error', { err });
    return res.status(500).json({ error: 'faucet_failed', message: err?.message || String(err) });
  }
});

app.listen(PORT, () => {
  const envSummary = {
    hasRedisUrl: Boolean(process.env.REDIS_URL),
    hasRpcEvm: Boolean(chainRpcUrls[CHAINS.evm]),
    hasRpcNative: Boolean(chainRpcUrls[CHAINS.native]),
  };
  console.log(`Faucet API listening on :${PORT}`);
  console.log('env_summary', envSummary);
});

async function ensureRateLimit(ip: string) {
  let redis;
  try {
    redis = getRedis();
  } catch (e) {
    const err: any = new Error('redis_unavailable');
    err.code = 'redis_unavailable';
    throw err;
  }
  try {
    const key = `ratelimit:${ip}`;
    const current = await redis.incr(key);
    if (current === 1) {
      await redis.expire(key, RATE_WINDOW_SECS);
    }
    if (current > RATE_MAX) {
      const ttl = await redis.ttl(key);
      const retryAfterSecs = ttl > 0 ? ttl : RATE_WINDOW_SECS;
      const err: any = new Error('rate_limited');
      err.retryAfterSecs = retryAfterSecs;
      throw err;
    }
  } catch (e) {
    const err: any = new Error('redis_unavailable');
    err.code = 'redis_unavailable';
    throw err;
  }
}
