import type { PiPayment } from '../types.js';
import { decimalToStroops, ledgerFromPagingToken } from '../util/amount.js';
import type { AccountEligibility, IncomingPaymentsPage, PiClient } from './piClient.js';

interface HorizonRootResponse {
  history_latest_ledger: number;
}

interface HorizonTransactionEmbed {
  memo_type?: string;
  memo?: string;
}

interface HorizonPaymentRecord {
  id: string;
  paging_token: string;
  transaction_hash: string;
  created_at: string;
  type: string;
  asset_type: string;
  to: string;
  from: string;
  amount: string;
  transaction_successful?: boolean;
  transaction?: HorizonTransactionEmbed;
}

interface HorizonPaymentsResponse {
  _embedded?: { records: HorizonPaymentRecord[] };
}

/**
 * `PiClient` backed by a Pi Network Horizon-compatible REST endpoint.
 *
 * Requests `join=transactions` so each payment record embeds its parent
 * transaction (for the memo) in a single round trip rather than N+1 fetches.
 */
export class HorizonPiClient implements PiClient {
  constructor(
    private readonly horizonUrl: string,
    private readonly depositAddress: string,
  ) {}

  async getLatestLedger(): Promise<number> {
    const res = await fetch(this.horizonUrl);
    if (!res.ok) {
      throw new Error(`Pi Horizon root request failed: ${res.status} ${res.statusText}`);
    }
    const body = (await res.json()) as HorizonRootResponse;
    return body.history_latest_ledger;
  }

  async getIncomingPayments(cursor: string): Promise<IncomingPaymentsPage> {
    const url = new URL(
      `/accounts/${this.depositAddress}/payments`,
      this.horizonUrl.endsWith('/') ? this.horizonUrl : `${this.horizonUrl}/`,
    );
    url.searchParams.set('cursor', cursor);
    url.searchParams.set('order', 'asc');
    url.searchParams.set('limit', '200');
    url.searchParams.set('join', 'transactions');

    const res = await fetch(url);
    if (!res.ok) {
      throw new Error(`Pi Horizon payments request failed: ${res.status} ${res.statusText}`);
    }
    const body = (await res.json()) as HorizonPaymentsResponse;
    const records = body._embedded?.records ?? [];

    const payments: PiPayment[] = [];
    let nextCursor = cursor;
    for (const record of records) {
      nextCursor = record.paging_token;
      if (record.type !== 'payment' || record.asset_type !== 'native') continue;
      if (record.transaction_successful === false) continue;
      if (record.to !== this.depositAddress) continue;

      const memoText =
        record.transaction?.memo_type === 'text' ? record.transaction.memo : undefined;

      payments.push({
        txId: record.transaction_hash,
        ledger: ledgerFromPagingToken(record.paging_token),
        amountStroops: decimalToStroops(record.amount).toString(),
        from: record.from,
        ...(memoText !== undefined ? { memoText } : {}),
        createdAt: record.created_at,
      });
    }
    return { payments, nextCursor };
  }

  async getAccountEligibility(accountId: string): Promise<AccountEligibility> {
    // A migrated Pi mainnet account owns a live ledger entry and therefore
    // resolves on the configured Horizon (defaults to mainnet in production).
    // A pre-migration wallet balance or a testnet-only address has no ledger
    // entry here and returns 404 => ineligible. See
    // docs/deposit-eligibility.md for the full policy.
    const url = new URL(
      `/accounts/${encodeURIComponent(accountId)}`,
      this.horizonUrl.endsWith('/') ? this.horizonUrl : `${this.horizonUrl}/`,
    );

    try {
      const res = await fetch(url);
      if (res.status === 404) {
        return { eligible: false, reason: 'account_not_found' };
      }
      if (!res.ok) {
        return { eligible: false, reason: 'eligibility_check_failed' };
      }
      return { eligible: true };
    } catch {
      // Fail closed: if the Pi Horizon endpoint is unreachable we cannot
      // prove the source is migrated, so treat it as ineligible and let an
      // operator investigate rather than minting speculatively.
      return { eligible: false, reason: 'eligibility_check_failed' };
    }
  }
}
