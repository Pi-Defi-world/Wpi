import type { BurnEvent } from '../types.js';

export type MintOutcome =
  | { minted: true; txHash: string }
  | { minted: false; alreadyProcessed: true }
  | { minted: false; rateLimited: true; txHash: string };

/**
 * Abstraction over the on-chain wPi Soroban contract (see
 * `Stellar-contracts-v1/wpi-token`). Kept separate from any specific SDK so
 * the mint submitter and redemption watcher can be unit tested against a
 * fake, with `SorobanWpiContractClient` as the real implementation used at
 * runtime.
 */
export interface WpiContractClient {
  /**
   * Submits `mint_from_deposit`. A committed circuit-breaker rejection is
   * returned as `{ minted: false, rateLimited: true }` so the deposit stays
   * pending, while an idempotent retry returns `alreadyProcessed`.
   */
  mintFromDeposit(args: {
    to: string;
    amountStroops: bigint;
    depositIdHex: string;
  }): Promise<MintOutcome>;

  /** Reads the contract's on-chain idempotency flag for a deposit id directly. */
  isDepositProcessed(depositIdHex: string): Promise<boolean>;

  /**
   * `redemption_burned` events emitted by `burn`, in ascending ledger order,
   * starting at `sinceLedger` (inclusive). Returns the ledger to resume
   * from on the next poll.
   */
  getRedemptionBurnEvents(sinceLedger: number): Promise<{ events: BurnEvent[]; nextLedger: number }>;
}
