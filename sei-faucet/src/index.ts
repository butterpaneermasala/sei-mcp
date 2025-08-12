import express from 'express';
import { fromHex, toBech32 } from '@cosmjs/encoding';
import { canRequest, recordRequest } from './db';
import { sendTokens } from './faucet';
import { AppConfig } from './config';

const app = express();

// Middleware to parse JSON bodies
app.use(express.json());

// Trust the first proxy in front of the app.
app.set('trust proxy', 1);

/**
 * Converts an Ethereum-style (0x) address to a Sei bech32 address.
 * @param ethAddress The 0x-prefixed Ethereum address.
 * @returns The corresponding sei-prefixed bech32 address.
 */
function convertEthAddressToSei(ethAddress: string): string {
  // Remove the '0x' prefix and convert the hex string to a byte array.
  const data = fromHex(ethAddress.substring(2));
  // Convert the byte array to a bech32 string with the 'sei' prefix.
  return toBech32(AppConfig.faucet.prefix, data);
}

// Health check endpoint
app.get('/', (req, res) => {
  res.status(200).send('🚰 Faucet is running!');
});

app.post('/request', async (req, res) => {
  const ip = req.ip;
  const { address: providedAddress } = req.body;

  if (!ip) {
    return res.status(400).json({ error: 'Could not determine request IP address.' });
  }

  if (!providedAddress || typeof providedAddress !== 'string') {
    return res.status(400).json({ error: 'Address must be provided as a string.' });
  }

  let recipientAddress: string;

  try {
    if (providedAddress.startsWith('0x')) {
      recipientAddress = convertEthAddressToSei(providedAddress);
      console.log(`Converted EVM address ${providedAddress} to ${recipientAddress}`);
    } else if (providedAddress.startsWith(AppConfig.faucet.prefix)) {
      recipientAddress = providedAddress;
    } else {
      throw new Error('Invalid address format.');
    }
  } catch (e) {
    console.error("Address conversion error:", e);
    return res.status(400).json({ error: "Invalid address. Please provide a valid 'sei...' or '0x...' address." });
  }

  if (!canRequest(ip, recipientAddress, AppConfig.cooldown.ms)) {
    const cooldownHours = AppConfig.cooldown.hours;
    return res.status(429).json({
      error: `Too many requests. Please wait ${cooldownHours} hour(s) before requesting again.`,
    });
  }

  try {
    console.log(`Processing request for ${recipientAddress} from IP ${ip}`);
    const result = await sendTokens(recipientAddress, AppConfig.faucet.amountuSei);
    recordRequest(ip, recipientAddress);
    res.status(200).json({ success: true, txHash: result.transactionHash });
  } catch (err) {
    console.error(`Faucet error for address ${recipientAddress}:`, err);
    res.status(500).json({ error: 'The faucet encountered an error and could not complete the transfer.' });
  }
});

app.listen(AppConfig.port, () => {
  console.log(`🚰 Faucet server running on http://localhost:${AppConfig.port}`);
});
