// @flow

import {sendAndConfirmTransaction as realSendAndConfirmTransaction} from '@solana/web3.js';
import type {Account, Connection, Transaction} from '@solana/web3.js';
import YAML from 'json-to-pretty-yaml';

type TransactionNotification = (string, string) => void;

let notify: TransactionNotification = () => undefined;

export function onTransaction(callback: TransactionNotification) {
  notify = callback;
}

/**
 * Parse Solana program error and return human-readable message
 */
function parseError(error: Error): string {
  const errorStr = error.toString();
  const errorMsg = error.message || errorStr;

  // Check for custom program errors (GSN errors)
  if (errorMsg.includes('custom program error')) {
    const errorCodeMatch = errorMsg.match(/custom program error: (0x)?([0-9a-f]+)/i);
    if (errorCodeMatch) {
      const errorCode = parseInt(errorCodeMatch[2], errorCodeMatch[1] ? 16 : 10);
      
      // Map GSN error codes to human-readable messages
      const gsnErrors = {
        0: 'GSN account already in use',
        1: 'Invalid GSN state',
        2: 'Unauthorized: not the governance authority',
        3: 'Governance not initialized',
        4: 'Invalid fee mode',
        5: 'Insufficient balance: top-up balance does not cover expected fee',
        6: 'Replay attack detected: nonce already used',
        7: 'Invalid nonce: expected next nonce',
        8: 'Unauthorized fee claim: only the executor who executed the transaction can claim',
      };
      
      if (gsnErrors[errorCode] !== undefined) {
        return gsnErrors[errorCode];
      }
    }
  }

  // Check for insufficient funds
  if (errorMsg.includes('insufficient funds') || errorMsg.includes('InsufficientFunds')) {
    return 'Insufficient funds: account balance is too low to complete the transaction';
  }

  // Check for insufficient balance in top-up
  if (errorMsg.includes('InsufficientBalance') || errorMsg.includes('insufficient balance')) {
    return 'Insufficient top-up balance: your top-up balance does not cover the required fee';
  }

  // Check for invalid account
  if (errorMsg.includes('InvalidAccountData') || errorMsg.includes('invalid account')) {
    return 'Invalid account data: the account is not properly initialized or has invalid data';
  }

  // Check for unauthorized
  if (errorMsg.includes('Unauthorized') || errorMsg.includes('unauthorized')) {
    return 'Unauthorized: you do not have permission to perform this action';
  }

  // Check for invalid nonce
  if (errorMsg.includes('InvalidNonce') || errorMsg.includes('invalid nonce')) {
    return 'Invalid nonce: the transaction nonce is incorrect. Please use the next expected nonce.';
  }

  // Check for replay attack
  if (errorMsg.includes('ReplayAttack') || errorMsg.includes('replay')) {
    return 'Replay attack detected: this transaction has already been executed';
  }

  // Check for unsupported token
  if (errorMsg.includes('token') && (errorMsg.includes('not allowed') || errorMsg.includes('unsupported'))) {
    return 'Unsupported token: this token is not allowed for top-up or fee payment';
  }

  // Check for account not found
  if (errorMsg.includes('AccountNotFound') || errorMsg.includes('account not found')) {
    return 'Account not found: the specified account does not exist';
  }

  // Return original error message if no specific pattern matches
  return errorMsg;
}

export async function sendAndConfirmTransaction(
  title: string,
  connection: Connection,
  transaction: Transaction,
  ...signers: Array<Account>
): Promise<void> {
  const when = Date.now();

  try {
    const signature = await realSendAndConfirmTransaction(
      connection,
      transaction,
      signers,
      {
        skipPreflight: true,
        commitment: 'recent',
        preflightCommitment: null,
      },
    );

    const body = {
      time: new Date(when).toString(),
      signature,
      instructions: transaction.instructions.map(i => {
        return {
          keys: i.keys.map(keyObj => keyObj.pubkey.toBase58()),
          programId: i.programId.toBase58(),
          data: '0x' + i.data.toString('hex'),
        };
      }),
    };

    notify(title, YAML.stringify(body).replace(/"/g, ''));
    return signature;
  } catch (error) {
    const humanReadableError = parseError(error);
    const enhancedError = new Error(`Transaction failed (${title}): ${humanReadableError}`);
    enhancedError.stack = error.stack;
    console.error(`[TRANSACTION_ERROR] ${title}:`, humanReadableError);
    console.error('Original error:', error.message || error.toString());
    throw enhancedError;
  }
}
