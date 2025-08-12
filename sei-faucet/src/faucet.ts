import { DirectSecp256k1Wallet } from '@cosmjs/proto-signing';
import { SigningStargateClient, DeliverTxResponse } from '@cosmjs/stargate';
import { coins } from '@cosmjs/amino';
import { AppConfig } from './config';

/**
 * Sends tokens from the faucet wallet to a specified recipient.
 *
 * @param recipient The bech32 address of the token recipient.
 * @param amount The amount of tokens to send, in the smallest denomination (e.g., usei).
 * @returns A promise that resolves to the transaction result.
 * @throws {Error} if the transaction fails.
 */
export async function sendTokens(recipient: string, amount: string): Promise<DeliverTxResponse> {
  const { privateKey, rpcUrl, prefix, denom } = AppConfig.faucet;

  const wallet = await DirectSecp256k1Wallet.fromKey(
    Buffer.from(privateKey, 'hex'),
    prefix
  );

  const [faucetAccount] = await wallet.getAccounts();
  console.log(`Faucet address: ${faucetAccount.address}`);

  const client = await SigningStargateClient.connectWithSigner(rpcUrl, wallet);

  const sendAmount = coins(amount, denom);

  const fee = {
    amount: coins('5000', denom),
    gas: '200000',
  };

  console.log(`Attempting to send ${amount}${denom} to ${recipient}...`);

  try {
    const result = await client.sendTokens(
      faucetAccount.address,
      recipient,
      sendAmount,
      fee,
      'Funds from faucet'
    );

    console.log(`Successfully sent tokens. Tx hash: ${result.transactionHash}`);
    return result;
  } catch (error) {
    console.error('Error sending tokens:', error);
    throw new Error('Faucet transaction failed.');
  }
}
