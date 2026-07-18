import { describe, expect, it } from 'vitest';
import { createLogger } from '../src/log.js';
import { MintSubmitter } from '../src/stellar/mintSubmitter.js';
import type { MintOutcome, WpiContractClient } from '../src/stellar/wpiContractClient.js';
import { MemoryStore } from '../src/store/memoryStore.js';
import type { BurnEvent, ConfirmedDeposit } from '../src/types.js';

const silentLogger = createLogger('test', 'error');

class FakeContractClient implements WpiContractClient {
  readonly calls: { to: string; amountStroops: bigint; depositIdHex: string }[] = [];
  private readonly processed = new Set<string>();
  outcomeOverride?: MintOutcome | (() => MintOutcome);
  throwOnce = false;

  mintFromDeposit(args: {
    to: string;
    amountStroops: bigint;
    depositIdHex: string;
  }): Promise<MintOutcome> {
    this.calls.push(args);
    if (this.throwOnce) {
      this.throwOnce = false;
      return Promise.reject(new Error('simulated network failure'));
    }
    if (this.outcomeOverride) {
      return Promise.resolve(
        typeof this.outcomeOverride === 'function' ? this.outcomeOverride() : this.outcomeOverride,
      );
    }
    if (this.processed.has(args.depositIdHex)) {
      return Promise.resolve({ minted: false, alreadyProcessed: true });
    }
    this.processed.add(args.depositIdHex);
    return Promise.resolve({ minted: true, txHash: `tx-for-${args.depositIdHex}` });
  }

  isDepositProcessed(depositIdHex: string): Promise<boolean> {
    return Promise.resolve(this.processed.has(depositIdHex));
  }

  getRedemptionBurnEvents(): Promise<{ events: BurnEvent[]; nextLedger: number }> {
    return Promise.resolve({ events: [], nextLedger: 0 });
  }
}

function confirmedDeposit(overrides: Partial<ConfirmedDeposit> = {}): ConfirmedDeposit {
  return {
    piTxId: 'pi-tx-1',
    depositId: 'deposit-1',
    amountStroops: '1000',
    destinationStellarAddress: 'GDEST',
    confirmedAtLedger: 100,
    ...overrides,
  };
}

describe('MintSubmitter', () => {
  it('submits a mint and records the tx hash', async () => {
    const contract = new FakeContractClient();
    const store = new MemoryStore();
    store.upsertDeposit({
      piTxId: 'pi-tx-1',
      depositId: 'deposit-1',
      amountStroops: '1000',
      destinationStellarAddress: 'GDEST',
      observedAtLedger: 90,
      status: 'confirmed',
      updatedAt: new Date(0).toISOString(),
    });
    const submitter = new MintSubmitter(contract, store, silentLogger);

    await submitter.submit(confirmedDeposit());

    expect(contract.calls).toHaveLength(1);
    expect(contract.calls[0]).toMatchObject({ to: 'GDEST', amountStroops: 1000n });
    const record = store.getDeposit('deposit-1');
    expect(record?.status).toBe('minted');
    expect(record?.mintTxHash).toBe('tx-for-deposit-1');
  });

  it('does not resubmit a deposit already marked minted locally', async () => {
    const contract = new FakeContractClient();
    const store = new MemoryStore();
    store.upsertDeposit({
      piTxId: 'pi-tx-1',
      depositId: 'deposit-1',
      amountStroops: '1000',
      destinationStellarAddress: 'GDEST',
      observedAtLedger: 90,
      status: 'minted',
      mintTxHash: 'already-minted-tx',
      updatedAt: new Date(0).toISOString(),
    });
    const submitter = new MintSubmitter(contract, store, silentLogger);

    await submitter.submit(confirmedDeposit());

    expect(contract.calls).toHaveLength(0);
  });

  it('treats an already-processed on-chain result as success without erroring', async () => {
    const contract = new FakeContractClient();
    contract.outcomeOverride = { minted: false, alreadyProcessed: true };
    const store = new MemoryStore();
    store.upsertDeposit({
      piTxId: 'pi-tx-1',
      depositId: 'deposit-1',
      amountStroops: '1000',
      destinationStellarAddress: 'GDEST',
      observedAtLedger: 90,
      status: 'confirmed',
      updatedAt: new Date(0).toISOString(),
    });
    const submitter = new MintSubmitter(contract, store, silentLogger);

    await submitter.submit(confirmedDeposit());

    expect(store.getDeposit('deposit-1')?.status).toBe('minted');
  });

  it('marks a deposit failed (not minted) when submission throws, and retryOutstanding recovers it', async () => {
    const contract = new FakeContractClient();
    contract.throwOnce = true;
    const store = new MemoryStore();
    store.upsertDeposit({
      piTxId: 'pi-tx-1',
      depositId: 'deposit-1',
      amountStroops: '1000',
      destinationStellarAddress: 'GDEST',
      observedAtLedger: 90,
      status: 'confirmed',
      updatedAt: new Date(0).toISOString(),
    });
    const submitter = new MintSubmitter(contract, store, silentLogger);

    await submitter.submit(confirmedDeposit());
    expect(store.getDeposit('deposit-1')?.status).toBe('failed');
    expect(store.getDeposit('deposit-1')?.lastError).toContain('simulated network failure');

    await submitter.retryOutstanding();
    expect(store.getDeposit('deposit-1')?.status).toBe('minted');
    expect(contract.calls).toHaveLength(2);
  });

  it('retryOutstanding skips deposits without a routable destination', async () => {
    const contract = new FakeContractClient();
    const store = new MemoryStore();
    store.upsertDeposit({
      piTxId: 'pi-tx-1',
      depositId: 'deposit-1',
      amountStroops: '1000',
      observedAtLedger: 90,
      status: 'unroutable',
      updatedAt: new Date(0).toISOString(),
    });

    const submitter = new MintSubmitter(contract, store, silentLogger);
    await submitter.retryOutstanding();

    expect(contract.calls).toHaveLength(0);
  });
});
