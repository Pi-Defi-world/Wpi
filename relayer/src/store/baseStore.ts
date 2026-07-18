import type { DepositRecord, DepositStatus, RedemptionRecord, RedemptionStatus } from '../types.js';
import type { IdempotencyStore } from './idempotencyStore.js';

export interface StoreState {
  piCursor: string;
  stellarEventCursor: number;
  deposits: Record<string, DepositRecord>;
  redemptions: Record<string, RedemptionRecord>;
}

export function emptyState(): StoreState {
  return { piCursor: '', stellarEventCursor: 0, deposits: {}, redemptions: {} };
}

/**
 * Shared read/write logic for `IdempotencyStore` implementations. Subclasses
 * only need to supply how `state` is loaded and persisted.
 */
export abstract class BaseStore implements IdempotencyStore {
  protected state: StoreState;

  protected constructor(initial: StoreState) {
    this.state = initial;
  }

  protected abstract persist(): void;

  getPiCursor(): string {
    return this.state.piCursor;
  }

  setPiCursor(cursor: string): void {
    this.state.piCursor = cursor;
    this.persist();
  }

  hasDeposit(depositId: string): boolean {
    return depositId in this.state.deposits;
  }

  getDeposit(depositId: string): DepositRecord | undefined {
    return this.state.deposits[depositId];
  }

  upsertDeposit(record: DepositRecord): void {
    this.state.deposits[record.depositId] = record;
    this.persist();
  }

  updateDepositStatus(
    depositId: string,
    status: DepositStatus,
    patch: Partial<DepositRecord> = {},
  ): void {
    const existing = this.state.deposits[depositId];
    if (!existing) {
      throw new Error(`Unknown deposit: ${depositId}`);
    }
    this.state.deposits[depositId] = {
      ...existing,
      ...patch,
      status,
      updatedAt: new Date().toISOString(),
    };
    this.persist();
  }

  listDepositsByStatus(status: DepositStatus): DepositRecord[] {
    return Object.values(this.state.deposits).filter((d) => d.status === status);
  }

  getStellarEventCursor(): number {
    return this.state.stellarEventCursor;
  }

  setStellarEventCursor(ledger: number): void {
    this.state.stellarEventCursor = ledger;
    this.persist();
  }

  hasRedemption(redemptionId: string): boolean {
    return redemptionId in this.state.redemptions;
  }

  upsertRedemption(record: RedemptionRecord): void {
    this.state.redemptions[record.redemptionId] = record;
    this.persist();
  }

  updateRedemptionStatus(
    redemptionId: string,
    status: RedemptionStatus,
    patch: Partial<RedemptionRecord> = {},
  ): void {
    const existing = this.state.redemptions[redemptionId];
    if (!existing) {
      throw new Error(`Unknown redemption: ${redemptionId}`);
    }
    this.state.redemptions[redemptionId] = {
      ...existing,
      ...patch,
      status,
      updatedAt: new Date().toISOString(),
    };
    this.persist();
  }

  listRedemptionsByStatus(status: RedemptionStatus): RedemptionRecord[] {
    return Object.values(this.state.redemptions).filter((r) => r.status === status);
  }
}
