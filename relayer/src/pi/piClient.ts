import type { PiPayment } from '../types.js';

export interface IncomingPaymentsPage {
  payments: PiPayment[];
  /** Horizon paging token to resume from on the next poll. */
  nextCursor: string;
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
}
