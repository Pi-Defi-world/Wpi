import type { Logger } from './log.js';
import type { DepositWatcher } from './pi/depositWatcher.js';
import type { MintSubmitter } from './stellar/mintSubmitter.js';
import type { RedemptionWatcher } from './stellar/redemptionWatcher.js';

export interface OrchestratorOptions {
  piPollIntervalMs: number;
  stellarPollIntervalMs: number;
}

/**
 * Drives the two independent polling loops (Pi deposits -> mints, Stellar
 * burns -> Pi releases) on their own intervals. Each loop guards against
 * overlapping runs so a slow RPC call can't pile up concurrent cycles.
 */
export class Orchestrator {
  private piTimer?: ReturnType<typeof setInterval>;
  private stellarTimer?: ReturnType<typeof setInterval>;
  private piRunning = false;
  private stellarRunning = false;
  private stopped = true;

  constructor(
    private readonly depositWatcher: DepositWatcher,
    private readonly mintSubmitter: MintSubmitter,
    private readonly redemptionWatcher: RedemptionWatcher,
    private readonly opts: OrchestratorOptions,
    private readonly log: Logger,
  ) {}

  start(): void {
    this.stopped = false;
    this.piTimer = setInterval(() => void this.runPiCycle(), this.opts.piPollIntervalMs);
    this.stellarTimer = setInterval(
      () => void this.runStellarCycle(),
      this.opts.stellarPollIntervalMs,
    );
    void this.runPiCycle();
    void this.runStellarCycle();
  }

  stop(): void {
    this.stopped = true;
    if (this.piTimer) clearInterval(this.piTimer);
    if (this.stellarTimer) clearInterval(this.stellarTimer);
  }

  async runPiCycle(): Promise<void> {
    if (this.piRunning || this.stopped) return;
    this.piRunning = true;
    try {
      const confirmed = await this.depositWatcher.pollOnce();
      for (const deposit of confirmed) {
        await this.mintSubmitter.submit(deposit);
      }
      await this.mintSubmitter.retryOutstanding();
    } catch (err) {
      this.log.error('Pi deposit cycle failed', {
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      this.piRunning = false;
    }
  }

  async runStellarCycle(): Promise<void> {
    if (this.stellarRunning || this.stopped) return;
    this.stellarRunning = true;
    try {
      await this.redemptionWatcher.pollOnce();
    } catch (err) {
      this.log.error('Stellar redemption cycle failed', {
        error: err instanceof Error ? err.message : String(err),
      });
    } finally {
      this.stellarRunning = false;
    }
  }
}
