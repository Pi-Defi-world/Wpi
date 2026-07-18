import { createHash } from 'node:crypto';

/**
 * Derives the contract's 32-byte `pi_deposit_id` from a Pi Network
 * transaction hash. Hashing normalizes Pi tx ids (hex strings of varying
 * length) into a fixed-size, collision-resistant contract key.
 */
export function depositIdFromPiTxId(piTxId: string): string {
  return createHash('sha256').update(piTxId, 'utf8').digest('hex');
}

const STELLAR_ACCOUNT_ADDRESS_RE = /^G[A-Z2-7]{55}$/;

/** True if `value` is a plausible Stellar/Pi ed25519 account StrKey (a `G...` address). */
export function isStrKeyAccountAddress(value: string): boolean {
  return STELLAR_ACCOUNT_ADDRESS_RE.test(value);
}
