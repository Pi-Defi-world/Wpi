/**
 * End-to-end demo of the bridge pipeline: a Pi deposit is observed, clears
 * the confirmation-depth policy, and is minted as wPi; then a wPi burn is
 * observed and released as native Pi.
 *
 * This runs against in-process fakes for the Pi Network and Stellar RPC
 * edges (`FakePiClient`, `FakeWpiContractClient` below) rather than real
 * testnets, because Pi Network's testnet is invite-gated and this sandbox
 * has no funded accounts on either chain. It exercises the exact same
 * `DepositWatcher` / `MintSubmitter` / `RedemptionWatcher` classes the
 * production relayer uses — only the two network-facing edges are faked.
 *
 * To run this against real testnets instead, see
 * `relayer/docs/e2e-testnet-demo.md`.
 */
import { createLogger } from '../src/log.js';
import { DepositWatcher } from '../src/pi/depositWatcher.js';
import type { IncomingPaymentsPage, PiClient } from '../src/pi/piClient.js';
import { MockPiPayoutClient } from '../src/pi/mockPiPayoutClient.js';
import { RedemptionWatcher } from '../src/stellar/redemptionWatcher.js';
import { MintSubmitter } from '../src/stellar/mintSubmitter.js';
import type { MintOutcome, WpiContractClient } from '../src/stellar/wpiContractClient.js';
import { MemoryStore } from '../src/store/memoryStore.js';
import type { BurnEvent, PiPayment } from '../src/types.js';
import { depositIdFromPiTxId } from '../src/util/depositId.js';

const CONFIRMATION_DEPTH = 5;
const DESTINATION_STELLAR_ADDRESS = 'GDEMOUSER'.padEnd(56, 'X');
const PI_REDEEMER_ADDRESS = 'GDEMOPIREDEEM'.padEnd(56, 'X');
const PI_SENDER_ADDRESS = 'GDEMOSENDER'.padEnd(56, 'X');

class FakePiClient implements PiClient {
  ledger = 1000;
  private pending: PiPayment[] = [];

  depositObserved(payment: PiPayment): void {
    this.pending.push(payment);
  }

  advanceLedgers(n: number): void {
    this.ledger += n;
  }

  getLatestLedger(): Promise<number> {
    return Promise.resolve(this.ledger);
  }

  getIncomingPayments(cursor: string): Promise<IncomingPaymentsPage> {
    const payments = this.pending;
    this.pending = [];
    return Promise.resolve({ payments, nextCursor: cursor || 'demo-cursor' });
  }
}

class FakeWpiContractClient implements WpiContractClient {
  private readonly minted = new Map<string, string>();
  private readonly pendingBurns: BurnEvent[] = [];
  ledger = 2000;

  queueBurn(event: BurnEvent): void {
    this.pendingBurns.push(event);
  }

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
    const txHash = `demo-mint-tx-${this.minted.size + 1}`;
    this.minted.set(args.depositIdHex, txHash);
    return Promise.resolve({ minted: true, txHash });
  }

  getRedemptionBurnEvents(): Promise<{ events: BurnEvent[]; nextLedger: number }> {
    const events = this.pendingBurns.splice(0, this.pendingBurns.length);
    return Promise.resolve({ events, nextLedger: this.ledger + 1 });
  }
}

async function main(): Promise<void> {
  const log = createLogger('e2e-demo');
  const store = new MemoryStore();

  const piClient = new FakePiClient();
  const depositWatcher = new DepositWatcher(
    piClient,
    store,
    { confirmationDepth: CONFIRMATION_DEPTH },
    createLogger('deposit-watcher'),
  );
  const contractClient = new FakeWpiContractClient();
  const mintSubmitter = new MintSubmitter(contractClient, store, createLogger('mint-submitter'));
  const payoutClient = new MockPiPayoutClient();
  const redemptionWatcher = new RedemptionWatcher(
    contractClient,
    payoutClient,
    store,
    createLogger('redemption-watcher'),
  );

  log.info('--- step 1: Pi deposit observed on Pi Network ---');
  const piTxId = 'demo-pi-deposit-tx';
  piClient.depositObserved({
    txId: piTxId,
    ledger: piClient.ledger,
    amountStroops: '250000000', // 25 Pi
    from: PI_SENDER_ADDRESS,
    memoText: DESTINATION_STELLAR_ADDRESS,
    createdAt: new Date().toISOString(),
  });

  let confirmed = await depositWatcher.pollOnce();
  log.info('poll immediately after observation: not yet confirmed', {
    confirmedCount: confirmed.length,
    status: store.getDeposit(depositIdFromPiTxId(piTxId))?.status,
  });

  log.info(`--- step 2: ${CONFIRMATION_DEPTH} Pi ledgers close (confirmation-depth policy) ---`);
  piClient.advanceLedgers(CONFIRMATION_DEPTH);
  confirmed = await depositWatcher.pollOnce();
  log.info('poll after confirmation depth reached', { confirmedCount: confirmed.length });

  log.info('--- step 3: relayer submits mint_from_deposit ---');
  for (const deposit of confirmed) {
    await mintSubmitter.submit(deposit);
  }
  const depositRecord = store.getDeposit(depositIdFromPiTxId(piTxId));
  log.info('wPi minted', {
    status: depositRecord?.status,
    mintTxHash: depositRecord?.mintTxHash,
    to: depositRecord?.destinationStellarAddress,
    amountStroops: depositRecord?.amountStroops,
  });

  log.info('--- step 4: retrying the same deposit id is a no-op (idempotency) ---');
  await mintSubmitter.submit(confirmed[0]!);
  log.info('deposit id already processed; no double mint', {
    status: store.getDeposit(depositIdFromPiTxId(piTxId))?.status,
  });

  log.info('--- step 5: user burns wPi to redeem Pi ---');
  contractClient.queueBurn({
    ledger: contractClient.ledger,
    txHash: 'demo-burn-tx',
    eventId: 'demo-burn-event-1',
    nonce: 1,
    from: DESTINATION_STELLAR_ADDRESS,
    amountStroops: '100000000', // 10 Pi
    piDestination: PI_REDEEMER_ADDRESS,
  });
  await redemptionWatcher.pollOnce();
  const redemption = store.listRedemptionsByStatus('released')[0];
  log.info('Pi released for redemption', {
    redemptionId: redemption?.redemptionId,
    piReleaseTxId: redemption?.piReleaseTxId,
    to: redemption?.piDestination,
    amountStroops: redemption?.amountStroops,
  });

  log.info('--- demo complete ---', { mockPiReleases: payoutClient.releases.length });
}

main().catch((err: unknown) => {
  console.error('e2e demo failed', err);
  process.exitCode = 1;
});
