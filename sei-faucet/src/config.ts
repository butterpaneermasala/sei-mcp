import dotenv from 'dotenv';

// Load environment variables from .env file
dotenv.config();

/**
 * Validates and retrieves an environment variable.
 * @param name The name of the environment variable.
 * @returns The value of the environment variable.
 * @throws {Error} if the environment variable is not set.
 */
function getEnvVar(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Environment variable ${name} is not set.`);
  }
  return value;
}

/**
 * Validates the private key format.
 * @param key The private key string.
 * @returns The validated private key.
 * @throws {Error} if the key is not a valid 64-character hex string.
 */
function getValidPrivateKey(key: string): string {
  if (!/^[0-9a-fA-F]{64}$/.test(key)) {
    throw new Error(
      'Invalid FAUCET_PRIVATE_KEY format. It must be a 64-character hexadecimal string without the "0x" prefix.'
    );
  }
  return key;
}


/**
 * A validated and typed configuration object.
 */
const config = {
  port: parseInt(process.env.PORT || '3001', 10),
  faucet: {
    privateKey: getValidPrivateKey(getEnvVar('FAUCET_PRIVATE_KEY')),
    rpcUrl: getEnvVar('SEI_RPC_URL'),
    amountuSei: getEnvVar('FAUCET_AMOUNT_USEI'),
    // Sei network uses 'usei' as the smallest denomination
    denom: 'usei',
    // The prefix for Sei addresses
    prefix: 'sei',
  },
  cooldown: {
    // Convert cooldown from hours to milliseconds
    hours: parseInt(process.env.COOLDOWN_HOURS || '24', 10),
    get ms(): number {
      return this.hours * 60 * 60 * 1000;
    },
  },
};

// We freeze the object to prevent accidental modifications during runtime.
export const AppConfig = Object.freeze(config);
