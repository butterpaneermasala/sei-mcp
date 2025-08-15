import { ethers } from 'ethers';

export async function sendEvm(params: {
  rpcUrl: string;
  privateKey: string; // 0x-prefixed or not
  to: string; // 0x address
  amountUsei: string; // integer string in usei (1e-6 SEI)
}): Promise<{ txHash: string }> {
  const { rpcUrl, privateKey, to, amountUsei } = params;
  if (!privateKey) throw new Error('FAUCET_PRIVATE_KEY_EVM not set');

  const pvk = privateKey.startsWith('0x') ? privateKey : `0x${privateKey}`;
  const provider = new ethers.JsonRpcProvider(rpcUrl);
  const wallet = new ethers.Wallet(pvk, provider);

  // Convert usei (1e-6 SEI) to wei (assuming 18 decimals on EVM): wei = usei * 1e12
  const usei = BigInt(amountUsei);
  const wei = usei * 10n ** 12n;

  const tx = await wallet.sendTransaction({
    to,
    value: wei,
  });
  const receipt = await tx.wait();
  // In ethers v6, status is a number (0 or 1) or null on legacy networks.
  if (receipt == null || receipt.status !== 1) {
    throw new Error('EVM transfer failed');
  }
  return { txHash: tx.hash };
}
