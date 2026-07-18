import { describe, expect, it } from 'vitest';
import { decimalToStroops, ledgerFromPagingToken, stroopsToDecimal } from '../src/util/amount.js';

describe('decimalToStroops', () => {
  it('converts a whole-number amount', () => {
    expect(decimalToStroops('12')).toBe(120_000_000n);
  });

  it('converts a fractional amount', () => {
    expect(decimalToStroops('12.5')).toBe(125_000_000n);
  });

  it('handles full 7-decimal precision', () => {
    expect(decimalToStroops('0.0000001')).toBe(1n);
  });

  it('truncates extra precision beyond 7 decimals', () => {
    expect(decimalToStroops('1.00000009')).toBe(10_000_000n);
  });

  it('rejects malformed input', () => {
    expect(() => decimalToStroops('abc')).toThrow();
    expect(() => decimalToStroops('1.2.3')).toThrow();
  });
});

describe('stroopsToDecimal', () => {
  it('round-trips through decimalToStroops', () => {
    expect(stroopsToDecimal(125_000_000n)).toBe('12.5000000');
    expect(decimalToStroops(stroopsToDecimal(125_000_000n))).toBe(125_000_000n);
  });

  it('pads small fractional amounts', () => {
    expect(stroopsToDecimal(1n)).toBe('0.0000001');
  });
});

describe('ledgerFromPagingToken', () => {
  it('extracts the ledger sequence from the high 32 bits', () => {
    const ledger = 12345;
    const toid = (BigInt(ledger) << 32n) | 4096n;
    expect(ledgerFromPagingToken(toid.toString())).toBe(ledger);
  });
});
