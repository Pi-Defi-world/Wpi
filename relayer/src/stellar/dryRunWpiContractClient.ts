import type { Logger } from '../log.js';
import type { BurnEvent } from '../types.js';
import type { MintOutcome, WpiContractClient } from './wpiContractClient.js';

/** Wraps a real `WpiContractClient`, logging intended mints instead of submitting them. */
export class DryRunWpiContractClient implements WpiContractClient {
  private readonly minted = new Set<string>();

  constructor(
    private readonly inner: WpiContractClient,
    private readonly log: Logger,
  ) {}

  isDepositProcessed(depositIdHex: string): Promise<boolean> {
    return Promise.resolve(this.minted.has(depositIdHex));
  }

  mintFromDeposit(args: {
    to: string;
    amountStroops: bigint;
    depositIdHex: string;
  }): Promise<MintOutcome> {
    if (this.minted.has(args.depositIdHex)) {
      return Promise.resolve({ minted: false, alreadyProcessed: true });
    }
    this.minted.add(args.depositIdHex);
    this.log.info('[dry-run] would submit mint_from_deposit', args);
    return Promise.resolve({ minted: true, txHash: '(dry-run)' });
  }

  getRedemptionBurnEvents(sinceLedger: number): Promise<{ events: BurnEvent[]; nextLedger: number }> {
    return this.inner.getRedemptionBurnEvents(sinceLedger);
  }
}
