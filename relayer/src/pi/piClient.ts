import type { PiPayment } from '../types.js';

export interface IncomingPaymentsPage {
  payments: PiPayment[];
  /** Horizon paging token to resume from on the next poll. */
  nextCursor: string;
}

/** Result of the deposit-source eligibility check (Issue #28). */
export interface AccountEligibility {
  eligible: boolean;
  /**
   * Machine-readable reason when `eligible` is false; always present on
   * rejection so the deposit record can be audited. See
   * `docs/deposit-eligibility.md` for the canonical policy.
   */
  reason?:
    | 'account_not_found'
    | 'not_kyc_verified'
    | 'not_allowlisted'
    | 'blocklisted'
    | 'eligibility_check_failed'
    | string;
}

/**
 * Read-only access to Pi Network payment history. Pi Network is an SCP
 * (Stellar Consensus Protocol) fork and exposes a Horizon-compatible REST
 * API, so this mirrors the Stellar Horizon `/accounts/{id}/payments` and `/`
 * (root) endpoints.
 */
export interface PiClient {
  /** Latest closed ledger sequence, used to compute confirmation depth. */
  getLatestLedger(): Promise<number>;

  /**
   * Native-Pi payments sent to the bridge deposit address, in ascending
   * ledger order, starting strictly after `cursor` (an empty string starts
   * from the beginning of history).
   */
  getIncomingPayments(cursor: string): Promise<IncomingPaymentsPage>;

  /**
   * Whether an account may originate a bridge deposit, per the policy in
   * `docs/deposit-eligibility.md`. The chain-level signal is that the
   * account exists on the configured Pi Horizon ledger (`migrated`);
   * KYC/AML and blocklist decisions are layered on top by
   * `DepositEligibilityPolicy` in `eligibility.ts`.
   */
  getAccountEligibility(accountId: string): Promise<AccountEligibility>;
}
