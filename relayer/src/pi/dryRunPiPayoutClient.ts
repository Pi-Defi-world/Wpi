import type { Logger } from '../log.js';
import type { PiPayoutClient } from './piPayoutClient.js';

/** Logs intended Pi releases instead of submitting them. */
export class DryRunPiPayoutClient implements PiPayoutClient {
  constructor(private readonly log: Logger) {}

  releaseFunds(args: {
    toPiAddress: string;
    amountStroops: bigint;
    memo: string;
  }): Promise<{ piTxId: string }> {
    this.log.info('[dry-run] would release Pi funds', args);
    return Promise.resolve({ piTxId: '(dry-run)' });
  }
}
