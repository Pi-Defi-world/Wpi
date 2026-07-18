import type { Logger } from '../log.js';
import type { PiPayoutClient } from '../pi/piPayoutClient.js';
import type { IdempotencyStore } from '../store/idempotencyStore.js';
import type { WpiContractClient } from './wpiContractClient.js';

/**
 * Watches the wPi contract for `redemption_burned` events and releases the
 * corresponding native Pi via `PiPayoutClient`, deduping by the event's
 * globally-unique RPC id so a burn is never paid out twice.
 */
export class RedemptionWatcher {
  constructor(
    private readonly contractClient: WpiContractClient,
    private readonly payout: PiPayoutClient,
    private readonly store: IdempotencyStore,
    private readonly log: Logger,
  ) {}

  async pollOnce(): Promise<void> {
    await this.ingestNewBurns();
    await this.releaseOutstanding();
  }

  private async ingestNewBurns(): Promise<void> {
    const since = this.store.getStellarEventCursor();
    const { events, nextLedger } = await this.contractClient.getRedemptionBurnEvents(since);

    for (const event of events) {
      if (this.store.hasRedemption(event.eventId)) continue;
      this.store.upsertRedemption({
        redemptionId: event.eventId,
        nonce: event.nonce,
        amountStroops: event.amountStroops,
        piDestination: event.piDestination,
        status: 'observed',
        updatedAt: new Date().toISOString(),
      });
      this.log.info('observed wPi redemption burn, queued for Pi release', {
        redemptionId: event.eventId,
        piDestination: event.piDestination,
      });
    }

    if (nextLedger !== since) {
      this.store.setStellarEventCursor(nextLedger);
    }
  }

  private async releaseOutstanding(): Promise<void> {
    const outstanding = [
      ...this.store.listRedemptionsByStatus('observed'),
      ...this.store.listRedemptionsByStatus('failed'),
    ];
    for (const record of outstanding) {
      this.store.updateRedemptionStatus(record.redemptionId, 'releasing');
      try {
        const { piTxId } = await this.payout.releaseFunds({
          toPiAddress: record.piDestination,
          amountStroops: BigInt(record.amountStroops),
          memo: record.redemptionId,
        });
        this.store.updateRedemptionStatus(record.redemptionId, 'released', {
          piReleaseTxId: piTxId,
        });
        this.log.info('released Pi for wPi redemption', {
          redemptionId: record.redemptionId,
          piTxId,
        });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        this.store.updateRedemptionStatus(record.redemptionId, 'failed', { lastError: message });
        this.log.error('Pi release failed, will retry next cycle', {
          redemptionId: record.redemptionId,
          error: message,
        });
      }
    }
  }
}
