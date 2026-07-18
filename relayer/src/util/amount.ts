const STROOPS_PER_PI = 10_000_000n;

/** Converts a Horizon-style decimal amount string (e.g. "12.5000000") to stroops. */
export function decimalToStroops(decimal: string): bigint {
  const parts = decimal.split('.');
  if (parts.length > 2) {
    throw new Error(`Invalid decimal amount: ${decimal}`);
  }
  const whole = parts[0] ?? '';
  const fraction = parts[1] ?? '';
  if (!/^\d+$/.test(whole) || !/^\d*$/.test(fraction)) {
    throw new Error(`Invalid decimal amount: ${decimal}`);
  }
  const paddedFraction = fraction.padEnd(7, '0').slice(0, 7);
  return BigInt(whole) * STROOPS_PER_PI + BigInt(paddedFraction || '0');
}

/**
 * Decodes a Horizon-style paging token (TOID) into its ledger sequence.
 * TOIDs pack `ledger_sequence << 32 | tx_order << 12 | op_order`.
 */
export function ledgerFromPagingToken(pagingToken: string): number {
  return Number(BigInt(pagingToken) >> 32n);
}

/** Converts stroops to a Horizon-style decimal amount string (e.g. "12.5000000"). */
export function stroopsToDecimal(stroops: bigint): string {
  const whole = stroops / STROOPS_PER_PI;
  const fraction = stroops % STROOPS_PER_PI;
  return `${whole}.${fraction.toString().padStart(7, '0')}`;
}
