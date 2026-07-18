import type { Logger } from '../log.js';
import type { IdempotencyStore } from '../store/idempotencyStore.js';
import type { ConfirmedDeposit } from '../types.js';
import { depositIdFromPiTxId, isStrKeyAccountAddress } from '../util/depositId.js';
import type { PiClient } from './piClient.js';

export interface DepositWatcherOptions {
  /** Ledgers of depth required before a deposit is considered final. See config.ts. */
  confirmationDepth: number;
}

/**
 * Polls Pi Network for payments to the bridge deposit address, tracks each
 * one until it clears `confirmationDepth`, and surfaces newly-confirmed
 * deposits for the mint submitter to act on.
 *
 * A deposit's destination wPi address is read from its transaction memo (a
 * `G...` Stellar StrKey); deposits with a missing or malformed memo are
 * recorded as `unroutable` rather than silently dropped or guessed at.
 */
export class DepositWatcher {
  constructor(
    private readonly pi: PiClient,
    private readonly store: IdempotencyStore,
    private readonly opts: DepositWatcherOptions,
    private readonly log: Logger,
  ) {}

  /** Runs a single ingest-and-promote cycle. Returns deposits newly confirmed this cycle. */
  async pollOnce(): Promise<ConfirmedDeposit[]> {
    await this.ingestNewPayments();
    return this.promoteConfirmedDeposits(await this.pi.getLatestLedger());
  }

  private async ingestNewPayments(): Promise<void> {
    const cursor = this.store.getPiCursor();
    const { payments, nextCursor } = await this.pi.getIncomingPayments(cursor);

    for (const payment of payments) {
      const depositId = depositIdFromPiTxId(payment.txId);
      if (this.store.hasDeposit(depositId)) continue;

      const destination = payment.memoText?.trim();
      const routable = destination !== undefined && isStrKeyAccountAddress(destination);

      this.store.upsertDeposit({
        piTxId: payment.txId,
        depositId,
        amountStroops: payment.amountStroops,
        ...(routable && destination ? { destinationStellarAddress: destination } : {}),
        observedAtLedger: payment.ledger,
        status: routable ? 'pending_confirmation' : 'unroutable',
        updatedAt: new Date().toISOString(),
      });

      if (!routable) {
        this.log.warn('deposit memo missing/invalid Stellar destination; marked unroutable', {
          piTxId: payment.txId,
          memoText: payment.memoText,
        });
      } else {
        this.log.info('observed new Pi deposit, awaiting confirmations', {
          piTxId: payment.txId,
          depositId,
          destination,
        });
      }
    }

    if (nextCursor !== cursor) {
      this.store.setPiCursor(nextCursor);
    }
  }

  private promoteConfirmedDeposits(latestLedger: number): ConfirmedDeposit[] {
    const confirmed: ConfirmedDeposit[] = [];
    for (const record of this.store.listDepositsByStatus('pending_confirmation')) {
      const confirmations = latestLedger - record.observedAtLedger;
      if (confirmations < this.opts.confirmationDepth) continue;

      // Routable deposits always carry a destination; enforced at ingest time.
      const destinationStellarAddress = record.destinationStellarAddress;
      if (!destinationStellarAddress) continue;

      this.store.updateDepositStatus(record.depositId, 'confirmed');
      this.log.info('deposit reached confirmation depth', {
        piTxId: record.piTxId,
        depositId: record.depositId,
        confirmations,
      });
      confirmed.push({
        piTxId: record.piTxId,
        depositId: record.depositId,
        amountStroops: record.amountStroops,
        destinationStellarAddress,
        confirmedAtLedger: latestLedger,
      });
    }
    return confirmed;
  }
}
