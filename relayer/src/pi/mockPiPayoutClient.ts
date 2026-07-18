import { createHash } from 'node:crypto';
import type { PiPayoutClient } from './piPayoutClient.js';

export interface MockPiRelease {
  toPiAddress: string;
  amountStroops: bigint;
  memo: string;
  piTxId: string;
}

/**
 * In-memory `PiPayoutClient` for the e2e demo (see `scripts/e2e-demo.ts`)
 * and tests, standing in for a real Pi custodial payout when real Pi
 * testnet credentials aren't available in this environment.
 */
export class MockPiPayoutClient implements PiPayoutClient {
  readonly releases: MockPiRelease[] = [];

  async releaseFunds(args: {
    toPiAddress: string;
    amountStroops: bigint;
    memo: string;
  }): Promise<{ piTxId: string }> {
    const piTxId = createHash('sha256')
      .update(`${args.toPiAddress}:${args.amountStroops}:${args.memo}:${this.releases.length}`)
      .digest('hex');
    this.releases.push({ ...args, piTxId });
    return Promise.resolve({ piTxId });
  }
}
