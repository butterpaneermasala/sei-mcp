import { DirectSecp256k1HdWallet, DirectSecp256k1Wallet } from '@cosmjs/proto-signing';
import { SigningStargateClient, coins } from '@cosmjs/stargate';
import { fromHex } from '@cosmjs/encoding';

export async function sendNative(params: {
  rpcUrl: string;
  denom: string; // e.g., 'usei'
  amountUsei: string; // integer string
  toBech32: string; // sei1...
  bech32Hrp: string; // 'sei'
  chainId: string; // 'atlantic-2'
  mnemonic?: string;
  privateKeyHex?: string; // secp256k1 raw hex (no 0x)
}): Promise<{ txHash: string }> {
  const { rpcUrl, denom, amountUsei, toBech32, bech32Hrp, chainId, mnemonic, privateKeyHex } = params;

  const { wallet, fromAddress } = await makeWallet({ bech32Hrp, mnemonic, privateKeyHex });

  const client = await SigningStargateClient.connectWithSigner(rpcUrl, wallet);

  const amount = coins(amountUsei, denom);
  const fee = {
    amount: coins('5000', denom),
    gas: '200000',
  };

  const res = await client.sendTokens(fromAddress, toBech32, amount, fee, 'Faucet transfer');
  if (res.code !== 0) {
    throw new Error(`Native transfer failed: code=${res.code} log=${res.rawLog}`);
  }
  return { txHash: res.transactionHash };
}

async function makeWallet(params: { bech32Hrp: string; mnemonic?: string; privateKeyHex?: string; }) {
  const { bech32Hrp, mnemonic, privateKeyHex } = params;
  if (mnemonic && mnemonic.trim()) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: bech32Hrp });
    const [acc] = await wallet.getAccounts();
    return { wallet, fromAddress: acc.address };
  }
  if (privateKeyHex && privateKeyHex.trim()) {
    const raw = privateKeyHex.startsWith('0x') ? privateKeyHex.slice(2) : privateKeyHex;
    const wallet = await DirectSecp256k1Wallet.fromKey(fromHex(raw), bech32Hrp);
    const [acc] = await wallet.getAccounts();
    return { wallet, fromAddress: acc.address };
  }
  throw new Error('Set FAUCET_MNEMONIC_NATIVE or FAUCET_PRIVATE_KEY_NATIVE');
}
