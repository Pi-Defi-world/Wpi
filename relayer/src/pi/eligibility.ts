import type { AccountEligibility, PiClient } from './piClient.js';

export interface DepositEligibilityPolicyOptions {
  /**
   * Master switch. Defaults to true (fail closed). Disable only for
   * dry-run/testnet demos; production must keep it enabled.
   */
  enabled: boolean;
  /**
   * Operator-attested KYC allowlist. When non-empty, ONLY these addresses may
   * originate a deposit (KYC verification is off-chain on Pi Network, so this
   * is how it is expressed).
   */
  allowlist?: string[];
  /** Accounts that are never eligible; takes precedence over the allowlist. */
  blocklist?: string[];
}

export interface DepositEligibilityResult {
  eligible: boolean;
  reason?: AccountEligibility['reason'];
}

/**
 * Fail-closed deposit-source eligibility policy — the on-chain enforcement of
 * `docs/deposit-eligibility.md` (Issue #28). Layers operator-configured
 * allowlist/blocklist over the chain-level "account exists on Pi mainnet"
 * lookup, and is evaluated by `DepositWatcher` at ingest time so an
 * ineligible source never reaches `mint_from_deposit`.
 */
export class DepositEligibilityPolicy {
  constructor(
    private readonly chain: Pick<PiClient, 'getAccountEligibility'>,
    private readonly opts: DepositEligibilityPolicyOptions,
  ) {}

  async check(from: string): Promise<DepositEligibilityResult> {
    if (!this.opts.enabled) {
      return { eligible: true };
    }

    // Blocklist wins over everything, including the allowlist.
    if (this.opts.blocklist?.includes(from)) {
      return { eligible: false, reason: 'blocklisted' };
    }

    // When an allowlist is configured, KYC-unapproved accounts are rejected before
    // any chain call — migration alone is not enough.
    if (this.opts.allowlist && this.opts.allowlist.length > 0 && !this.opts.allowlist.includes(from)) {
      return { eligible: false, reason: 'not_allowlisted' };
    }

    // Chain-level proof of migration: the account must resolve on the
    // configured Pi Horizon ledger (mainnet in production).
    return this.chain.getAccountEligibility(from);
  }
}