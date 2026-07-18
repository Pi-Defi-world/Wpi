import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { JsonFileStore } from '../src/store/jsonFileStore.js';
import { MemoryStore } from '../src/store/memoryStore.js';
import type { IdempotencyStore } from '../src/store/idempotencyStore.js';
import type { DepositRecord, RedemptionRecord } from '../src/types.js';

function sampleDeposit(overrides: Partial<DepositRecord> = {}): DepositRecord {
  return {
    piTxId: 'pi-tx-1',
    depositId: 'deposit-1',
    amountStroops: '1000',
    destinationStellarAddress: 'GDEST',
    observedAtLedger: 100,
    status: 'pending_confirmation',
    updatedAt: new Date(0).toISOString(),
    ...overrides,
  };
}

function sampleRedemption(overrides: Partial<RedemptionRecord> = {}): RedemptionRecord {
  return {
    redemptionId: 'evt-1',
    nonce: 1,
    amountStroops: '500',
    piDestination: 'GPI',
    status: 'observed',
    updatedAt: new Date(0).toISOString(),
    ...overrides,
  };
}

describe.each([
  ['MemoryStore', () => new MemoryStore()],
  [
    'JsonFileStore',
    () => {
      const dir = mkdtempSync(join(tmpdir(), 'wpi-relayer-store-'));
      return new JsonFileStore(join(dir, 'state.json'));
    },
  ],
] as const)('%s', (_name, makeStore) => {
  let store: IdempotencyStore;

  beforeEach(() => {
    store = makeStore();
  });

  it('tracks the Pi payment cursor', () => {
    expect(store.getPiCursor()).toBe('');
    store.setPiCursor('cursor-123');
    expect(store.getPiCursor()).toBe('cursor-123');
  });

  it('tracks the Stellar event cursor', () => {
    expect(store.getStellarEventCursor()).toBe(0);
    store.setStellarEventCursor(42);
    expect(store.getStellarEventCursor()).toBe(42);
  });

  it('upserts and retrieves deposits by id', () => {
    expect(store.hasDeposit('deposit-1')).toBe(false);
    store.upsertDeposit(sampleDeposit());
    expect(store.hasDeposit('deposit-1')).toBe(true);
    expect(store.getDeposit('deposit-1')?.status).toBe('pending_confirmation');
  });

  it('updates deposit status and applies a patch', () => {
    store.upsertDeposit(sampleDeposit());
    store.updateDepositStatus('deposit-1', 'minted', { mintTxHash: 'tx-hash' });
    const record = store.getDeposit('deposit-1');
    expect(record?.status).toBe('minted');
    expect(record?.mintTxHash).toBe('tx-hash');
  });

  it('throws updating an unknown deposit', () => {
    expect(() => store.updateDepositStatus('missing', 'minted')).toThrow();
  });

  it('lists deposits filtered by status', () => {
    store.upsertDeposit(sampleDeposit({ depositId: 'a', status: 'pending_confirmation' }));
    store.upsertDeposit(sampleDeposit({ depositId: 'b', status: 'confirmed' }));
    expect(store.listDepositsByStatus('pending_confirmation').map((d) => d.depositId)).toEqual([
      'a',
    ]);
    expect(store.listDepositsByStatus('confirmed').map((d) => d.depositId)).toEqual(['b']);
  });

  it('upserts and retrieves redemptions by id', () => {
    expect(store.hasRedemption('evt-1')).toBe(false);
    store.upsertRedemption(sampleRedemption());
    expect(store.hasRedemption('evt-1')).toBe(true);
  });

  it('updates redemption status and applies a patch', () => {
    store.upsertRedemption(sampleRedemption());
    store.updateRedemptionStatus('evt-1', 'released', { piReleaseTxId: 'pi-tx' });
    const record = store.listRedemptionsByStatus('released')[0];
    expect(record?.piReleaseTxId).toBe('pi-tx');
  });
});

describe('JsonFileStore persistence', () => {
  let dir: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'wpi-relayer-store-'));
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('survives being reloaded from disk', () => {
    const path = join(dir, 'nested', 'state.json');
    const store = new JsonFileStore(path);
    store.setPiCursor('cursor-abc');
    store.upsertDeposit(sampleDeposit());

    expect(existsSync(path)).toBe(true);
    expect(JSON.parse(readFileSync(path, 'utf8')).piCursor).toBe('cursor-abc');

    const reloaded = new JsonFileStore(path);
    expect(reloaded.getPiCursor()).toBe('cursor-abc');
    expect(reloaded.hasDeposit('deposit-1')).toBe(true);
  });

  it('starts empty when no file exists yet', () => {
    const store = new JsonFileStore(join(dir, 'does-not-exist.json'));
    expect(store.getPiCursor()).toBe('');
    expect(store.listDepositsByStatus('pending_confirmation')).toEqual([]);
  });
});
