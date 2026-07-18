import { describe, expect, it } from 'vitest';
import { DepositWatcher } from '../src/pi/depositWatcher.js';
import type { IncomingPaymentsPage, PiClient } from '../src/pi/piClient.js';
import { createLogger } from '../src/log.js';
import { MemoryStore } from '../src/store/memoryStore.js';
import type { PiPayment } from '../src/types.js';
import { depositIdFromPiTxId } from '../src/util/depositId.js';

const DEST = 'G'.padEnd(56, 'A');

class FakePiClient implements PiClient {
  latestLedger = 0;
  private pages: IncomingPaymentsPage[] = [];

  queuePayments(payments: PiPayment[], nextCursor: string): void {
    this.pages.push({ payments, nextCursor });
  }

  getLatestLedger(): Promise<number> {
    return Promise.resolve(this.latestLedger);
  }

  getIncomingPayments(cursor: string): Promise<IncomingPaymentsPage> {
    const page = this.pages.shift();
    return Promise.resolve(page ?? { payments: [], nextCursor: cursor });
  }
}

function payment(overrides: Partial<PiPayment> = {}): PiPayment {
  return {
    txId: 'tx-1',
    ledger: 100,
    amountStroops: '1000',
    from: 'GFROM',
    memoText: DEST,
    createdAt: new Date(0).toISOString(),
    ...overrides,
  };
}

function paymentWithoutMemo(overrides: Partial<PiPayment> = {}): PiPayment {
  const { memoText: _memoText, ...rest } = payment(overrides);
  return rest;
}

const silentLogger = createLogger('test', 'error');

describe('DepositWatcher', () => {
  it('leaves a fresh deposit pending until it clears confirmation depth', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([payment()], 'cursor-1');
    pi.latestLedger = 100 + 29; // one short of the default-style depth used below

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    const confirmed = await watcher.pollOnce();

    expect(confirmed).toEqual([]);
    expect(store.getDeposit(depositIdFromPiTxId('tx-1'))?.status).toBe('pending_confirmation');
  });

  it('confirms a deposit once it reaches the confirmation depth', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([payment()], 'cursor-1');
    pi.latestLedger = 130;

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    const confirmed = await watcher.pollOnce();

    expect(confirmed).toHaveLength(1);
    expect(confirmed[0]).toMatchObject({
      piTxId: 'tx-1',
      destinationStellarAddress: DEST,
      amountStroops: '1000',
    });
    expect(store.getDeposit(depositIdFromPiTxId('tx-1'))?.status).toBe('confirmed');
  });

  it('marks deposits with a missing memo as unroutable and never confirms them', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([paymentWithoutMemo()], 'cursor-1');
    pi.latestLedger = 1000;

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    const confirmed = await watcher.pollOnce();

    expect(confirmed).toEqual([]);
    expect(store.getDeposit(depositIdFromPiTxId('tx-1'))?.status).toBe('unroutable');
  });

  it('marks deposits with a malformed memo as unroutable', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([payment({ memoText: 'not-an-address' })], 'cursor-1');
    pi.latestLedger = 1000;

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    await watcher.pollOnce();

    expect(store.getDeposit(depositIdFromPiTxId('tx-1'))?.status).toBe('unroutable');
  });

  it('never re-ingests the same Pi tx id twice', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([payment()], 'cursor-1');
    pi.queuePayments([payment()], 'cursor-1');
    pi.latestLedger = 1000;

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    await watcher.pollOnce();
    const confirmedSecondTime = await watcher.pollOnce();

    // Second poll re-confirms nothing new since the deposit already moved past 'pending_confirmation'.
    expect(confirmedSecondTime).toEqual([]);
  });

  it('advances the stored cursor', async () => {
    const pi = new FakePiClient();
    pi.queuePayments([payment()], 'cursor-42');
    pi.latestLedger = 100;

    const store = new MemoryStore();
    const watcher = new DepositWatcher(pi, store, { confirmationDepth: 30 }, silentLogger);

    await watcher.pollOnce();

    expect(store.getPiCursor()).toBe('cursor-42');
  });
});
