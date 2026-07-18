import type { DepositRecord, DepositStatus, RedemptionRecord, RedemptionStatus } from '../types.js';

/**
 * Local bookkeeping the relayer uses to avoid re-processing the same Pi
 * deposit or Stellar redemption twice. This is a *cache* for efficiency and
 * crash-restart resumption, not the source of truth for double-mint safety
 * — that guarantee comes from the wPi contract's `mint_from_deposit`
 * idempotency (see `Stellar-contracts-v1/wpi-token`). If this store is
 * lost, the relayer replays from `getPiCursor()` / `getStellarEventCursor()`
 * and the contract itself rejects any deposit id it already minted.
 */
export interface IdempotencyStore {
  getPiCursor(): string;
  setPiCursor(cursor: string): void;

  hasDeposit(depositId: string): boolean;
  getDeposit(depositId: string): DepositRecord | undefined;
  upsertDeposit(record: DepositRecord): void;
  updateDepositStatus(
    depositId: string,
    status: DepositStatus,
    patch?: Partial<DepositRecord>,
  ): void;
  listDepositsByStatus(status: DepositStatus): DepositRecord[];

  getStellarEventCursor(): number;
  setStellarEventCursor(ledger: number): void;

  hasRedemption(redemptionId: string): boolean;
  upsertRedemption(record: RedemptionRecord): void;
  updateRedemptionStatus(
    redemptionId: string,
    status: RedemptionStatus,
    patch?: Partial<RedemptionRecord>,
  ): void;
  listRedemptionsByStatus(status: RedemptionStatus): RedemptionRecord[];
}
