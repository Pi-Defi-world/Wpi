import type { Logger } from '../log.js';
import type { IdempotencyStore } from '../store/idempotencyStore.js';
import type { ConfirmedDeposit } from '../types.js';
import type { WpiContractClient } from './wpiContractClient.js';

/**
 * Submits `mint_from_deposit` for confirmed deposits. Safe to call more than
 * once for the same deposit — the contract's own idempotency (see
 * `Stellar-contracts-v1/wpi-token::mint_from_deposit`) is the real
 * guarantee against a double mint; this class's job is just to avoid
 * redundant submissions and to record the outcome locally.
 */
export class MintSubmitter {
  constructor(
    private readonly contractClient: WpiContractClient,
    private readonly store: IdempotencyStore,
    private readonly log: Logger,
  ) {}

  async submit(deposit: ConfirmedDeposit): Promise<void> {
    const existing = this.store.getDeposit(deposit.depositId);
    if (existing?.status === 'minted') return;

    this.store.updateDepositStatus(deposit.depositId, 'minting');
    try {
      const outcome = await this.contractClient.mintFromDeposit({
        to: deposit.destinationStellarAddress,
        amountStroops: BigInt(deposit.amountStroops),
        depositIdHex: deposit.depositId,
      });

      if (outcome.minted) {
        this.store.updateDepositStatus(deposit.depositId, 'minted', {
          mintTxHash: outcome.txHash,
        });
        this.log.info('minted wPi for Pi deposit', {
          piTxId: deposit.piTxId,
          depositId: deposit.depositId,
          txHash: outcome.txHash,
        });
      } else {
        this.store.updateDepositStatus(deposit.depositId, 'minted');
        this.log.info('deposit was already minted; treating as success', {
          piTxId: deposit.piTxId,
          depositId: deposit.depositId,
        });
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      this.store.updateDepositStatus(deposit.depositId, 'failed', { lastError: message });
      this.log.error('mint submission failed, will retry next cycle', {
        piTxId: deposit.piTxId,
        depositId: deposit.depositId,
        error: message,
      });
    }
  }

  /** Retries deposits left in `failed` or `confirmed` (e.g. after a crash mid-submission) or stuck `minting`. */
  async retryOutstanding(): Promise<void> {
    const outstanding = [
      ...this.store.listDepositsByStatus('confirmed'),
      ...this.store.listDepositsByStatus('failed'),
      ...this.store.listDepositsByStatus('minting'),
    ];
    for (const record of outstanding) {
      if (!record.destinationStellarAddress) continue;
      await this.submit({
        piTxId: record.piTxId,
        depositId: record.depositId,
        amountStroops: record.amountStroops,
        destinationStellarAddress: record.destinationStellarAddress,
        confirmedAtLedger: record.observedAtLedger,
      });
    }
  }
}
