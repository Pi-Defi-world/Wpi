import { describe, expect, it } from 'vitest';
import { createLogger } from '../src/log.js';
import type { PiPayoutClient } from '../src/pi/piPayoutClient.js';
import { RedemptionWatcher } from '../src/stellar/redemptionWatcher.js';
import type { MintOutcome, WpiContractClient } from '../src/stellar/wpiContractClient.js';
import { MemoryStore } from '../src/store/memoryStore.js';
import type { BurnEvent } from '../src/types.js';

const silentLogger = createLogger('test', 'error');

class FakeContractClient implements WpiContractClient {
  private events: BurnEvent[] = [];
  latestLedger = 0;

  queueEvent(event: BurnEvent): void {
    this.events.push(event);
  }

  mintFromDeposit(): Promise<MintOutcome> {
    throw new Error('not used in redemption tests');
  }

  isDepositProcessed(): Promise<boolean> {
    return Promise.resolve(false);
  }

  getRedemptionBurnEvents(
    sinceLedger: number,
  ): Promise<{ events: BurnEvent[]; nextLedger: number }> {
    const events = this.events.filter((e) => e.ledger >= sinceLedger);
    this.events = [];
    return Promise.resolve({ events, nextLedger: this.latestLedger + 1 });
  }
}

class FakePayoutClient implements PiPayoutClient {
  readonly releases: { toPiAddress: string; amountStroops: bigint; memo: string }[] = [];
  failNext = false;

  releaseFunds(args: {
    toPiAddress: string;
    amountStroops: bigint;
    memo: string;
  }): Promise<{ piTxId: string }> {
    if (this.failNext) {
      this.failNext = false;
      return Promise.reject(new Error('simulated payout failure'));
    }
    this.releases.push(args);
    return Promise.resolve({ piTxId: `pi-tx-${this.releases.length}` });
  }
}

function burnEvent(overrides: Partial<BurnEvent> = {}): BurnEvent {
  return {
    ledger: 100,
    txHash: 'stellar-tx-1',
    eventId: '0000000429496729600-0000000001',
    nonce: 1,
    from: 'GBURN',
    amountStroops: '500',
    piDestination: 'GPI'.padEnd(56, 'A'),
    ...overrides,
  };
}

describe('RedemptionWatcher', () => {
  it('releases Pi funds for a newly observed burn', async () => {
    const contract = new FakeContractClient();
    contract.latestLedger = 100;
    contract.queueEvent(burnEvent());
    const payout = new FakePayoutClient();
    const store = new MemoryStore();

    const watcher = new RedemptionWatcher(contract, payout, store, silentLogger);
    await watcher.pollOnce();

    expect(payout.releases).toHaveLength(1);
    expect(payout.releases[0]).toMatchObject({ toPiAddress: 'GPI'.padEnd(56, 'A'), amountStroops: 500n });
    const record = store.listRedemptionsByStatus('released')[0];
    expect(record?.piReleaseTxId).toBe('pi-tx-1');
  });

  it('never releases the same event twice', async () => {
    const contract = new FakeContractClient();
    contract.latestLedger = 100;
    const store = new MemoryStore();
    store.upsertRedemption({
      redemptionId: burnEvent().eventId,
      nonce: 1,
      amountStroops: '500',
      piDestination: 'GPI'.padEnd(56, 'A'),
      status: 'released',
      piReleaseTxId: 'already-released',
      updatedAt: new Date(0).toISOString(),
    });
    contract.queueEvent(burnEvent());
    const payout = new FakePayoutClient();

    const watcher = new RedemptionWatcher(contract, payout, store, silentLogger);
    await watcher.pollOnce();

    expect(payout.releases).toHaveLength(0);
  });

  it('retries a failed release on the next poll', async () => {
    const contract = new FakeContractClient();
    contract.latestLedger = 100;
    contract.queueEvent(burnEvent());
    const payout = new FakePayoutClient();
    payout.failNext = true;
    const store = new MemoryStore();

    const watcher = new RedemptionWatcher(contract, payout, store, silentLogger);
    await watcher.pollOnce();

    expect(store.listRedemptionsByStatus('failed')).toHaveLength(1);

    await watcher.pollOnce();

    expect(store.listRedemptionsByStatus('released')).toHaveLength(1);
    expect(payout.releases).toHaveLength(1);
  });

  it('advances the stored Stellar event cursor', async () => {
    const contract = new FakeContractClient();
    contract.latestLedger = 200;
    const store = new MemoryStore();
    const payout = new FakePayoutClient();

    const watcher = new RedemptionWatcher(contract, payout, store, silentLogger);
    await watcher.pollOnce();

    expect(store.getStellarEventCursor()).toBe(201);
  });
});
