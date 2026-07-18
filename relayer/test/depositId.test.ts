import { describe, expect, it } from 'vitest';
import { depositIdFromPiTxId, isStrKeyAccountAddress } from '../src/util/depositId.js';

describe('depositIdFromPiTxId', () => {
  it('produces a stable 32-byte (64 hex char) id', () => {
    const id = depositIdFromPiTxId('abc123');
    expect(id).toMatch(/^[0-9a-f]{64}$/);
  });

  it('is deterministic', () => {
    expect(depositIdFromPiTxId('same-tx')).toBe(depositIdFromPiTxId('same-tx'));
  });

  it('differs for different tx ids', () => {
    expect(depositIdFromPiTxId('tx-a')).not.toBe(depositIdFromPiTxId('tx-b'));
  });
});

describe('isStrKeyAccountAddress', () => {
  it('accepts a well-formed G-address', () => {
    expect(isStrKeyAccountAddress('G'.padEnd(56, 'A'))).toBe(true);
  });

  it('rejects addresses with the wrong prefix', () => {
    expect(isStrKeyAccountAddress('C'.padEnd(56, 'A'))).toBe(false);
  });

  it('rejects the wrong length', () => {
    expect(isStrKeyAccountAddress('GAAAA')).toBe(false);
  });

  it('rejects non-base32 characters', () => {
    expect(isStrKeyAccountAddress('G'.padEnd(55, 'A') + '0')).toBe(false);
  });
});
