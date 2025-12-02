// @flow

import { Connection, PublicKey } from '@solana/web3.js';
import { deserialize as borshDeserialize } from 'borsh';
import BN from 'bn.js';

/**
 * GSN State schema for Borsh deserialization
 */
class GsnInfo {
  constructor(fields) {
    this.is_initialized = fields.is_initialized;
    this.consumer = fields.consumer || new Map();
    this.executor = fields.executor || new Map();
    this.governance = fields.governance || null;
    this.consumer_nonces = fields.consumer_nonces || new Map();
    this.transaction_executor = fields.transaction_executor || new Map();
  }
}

const GsnInfoSchema = new Map([
  [
    GsnInfo,
    {
      kind: 'struct',
      fields: [
        ['is_initialized', 'u8'],
        ['consumer', { kind: 'map', key: 'string', value: 'u64' }],
        ['executor', { kind: 'map', key: 'string', value: 'u64' }],
        ['governance', { kind: 'option', type: 'object' }],
        ['consumer_nonces', { kind: 'map', key: 'string', value: 'u64' }],
        ['transaction_executor', { kind: 'map', key: 'string', value: 'string' }],
      ],
    },
  ],
]);

/**
 * Get GSN state from account
 */
export async function getGsnState(
  connection: Connection,
  gsnAccountPubkey: PublicKey,
): Promise<GsnInfo> {
  const accountInfo = await connection.getAccountInfo(gsnAccountPubkey);

  if (!accountInfo) {
    throw new Error('GSN account not found');
  }

  // Convert account data to Uint8Array if needed
  let data = accountInfo.data;
  if (Buffer.isBuffer(data)) {
    data = new Uint8Array(data);
  }

  try {
    const gsnInfo = borshDeserialize(GsnInfoSchema, GsnInfo, data);
    return gsnInfo;
  } catch (error) {
    // Fallback: try to parse manually if borsh deserialization fails
    console.warn('Borsh deserialization failed, trying manual parse:', error);
    throw new Error(`Failed to deserialize GSN state: ${error.message}`);
  }
}

/**
 * Get consumer balance from GSN state
 */
export async function getConsumerBalance(
  connection: Connection,
  gsnAccountPubkey: PublicKey,
  consumerPubkey: PublicKey,
): Promise<BN> {
  const gsnInfo = await getGsnState(connection, gsnAccountPubkey);
  const consumerKey = consumerPubkey.toBase58();
  const balance = gsnInfo.consumer.get(consumerKey);
  return balance ? new BN(balance.toString()) : new BN(0);
}

/**
 * Get executor earnings from GSN state
 */
export async function getExecutorEarnings(
  connection: Connection,
  gsnAccountPubkey: PublicKey,
  executorPubkey: PublicKey,
): Promise<BN> {
  const gsnInfo = await getGsnState(connection, gsnAccountPubkey);
  const executorKey = executorPubkey.toBase58();
  const earnings = gsnInfo.executor.get(executorKey);
  return earnings ? new BN(earnings.toString()) : new BN(0);
}

/**
 * Get next nonce for a consumer
 */
export async function getConsumerNonce(
  connection: Connection,
  gsnAccountPubkey: PublicKey,
  consumerPubkey: PublicKey,
): Promise<number> {
  const gsnInfo = await getGsnState(connection, gsnAccountPubkey);
  const consumerKey = consumerPubkey.toBase58();
  const nonce = gsnInfo.consumer_nonces.get(consumerKey);
  return nonce ? nonce.toNumber() : 0;
}
