/**
 * Releases native Pi to a redeeming user, in response to an observed wPi
 * burn. Kept as its own interface (distinct from `PiClient`, which is
 * read-only) because production releases go through a custodial Pi wallet
 * that a security review should scope separately from deposit observation.
 */
export interface PiPayoutClient {
  releaseFunds(args: {
    toPiAddress: string;
    amountStroops: bigint;
    /** Best-effort trace id attached as the payout tx's memo (truncated to Stellar's 28-byte memo limit). */
    memo: string;
  }): Promise<{ piTxId: string }>;
}
