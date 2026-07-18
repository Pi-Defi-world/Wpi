import { StrKey } from '@stellar/stellar-sdk';

/**
 * Pi Network is an SCP (Stellar Consensus Protocol) fork and uses the same
 * StrKey/ed25519 account address format as Stellar, so the raw 32-byte
 * `pi_destination` emitted by `burn` round-trips through the same encoder.
 */
export function piDestinationToStrKey(raw: Buffer): string {
  return StrKey.encodeEd25519PublicKey(raw);
}

export function strKeyToPiDestination(address: string): Buffer {
  return StrKey.decodeEd25519PublicKey(address);
}
