/** A native-Pi payment observed on Pi Network, sent to the bridge deposit address. */
export interface PiPayment {
  /** Pi Network transaction hash. */
  txId: string;
  /** Ledger sequence the payment was included in. */
  ledger: number;
  /** Amount in Pi stroops (1 Pi = 1e7 stroops) as a decimal string, to avoid float precision loss. */
  amountStroops: string;
  /** Source Pi account (StrKey, e.g. `GABC...`). */
  from: string;
  /**
   * Memo attached to the payment's transaction. By protocol convention the
   * depositor puts their destination Stellar (wPi) address here.
   */
  memoText?: string;
  createdAt: string;
}

/** A Pi deposit that has cleared the required confirmation depth and is ready to mint. */
export interface ConfirmedDeposit {
  piTxId: string;
  /** sha256(piTxId) hex-encoded, used as the contract's `pi_deposit_id` (32 bytes). */
  depositId: string;
  amountStroops: string;
  destinationStellarAddress: string;
  confirmedAtLedger: number;
}

export type DepositStatus =
  | 'pending_confirmation'
  | 'confirmed'
  | 'minting'
  | 'minted'
  | 'unroutable'
  | 'ineligible'
  | 'failed';

export interface DepositRecord {
  piTxId: string;
  depositId: string;
  amountStroops: string;
  destinationStellarAddress?: string;
  observedAtLedger: number;
  status: DepositStatus;
  mintTxHash?: string;
  /** Machine-readable reason when `status === 'ineligible'` (Issue #28). */
  ineligibleReason?: string;
  lastError?: string;
  updatedAt: string;
}

/** A `redemption_burned` event observed on the wPi Soroban contract. */
export interface BurnEvent {
  ledger: number;
  txHash: string;
  /** RPC-assigned globally unique event id (encodes ledger/tx/op/event order). */
  eventId: string;
  /** On-chain redemption ID supplied to `burn`; this is the relayer's dedupe key. */
  redemptionId: string;
  /** Monotonic per-contract nonce assigned by the `burn` call. */
  nonce: number;
  /** Stellar address that burned wPi. */
  from: string;
  amountStroops: string;
  /** Pi Network StrKey address (decoded from the raw 32-byte `pi_destination`). */
  piDestination: string;
}

export type RedemptionStatus = 'observed' | 'releasing' | 'released' | 'failed';

export interface RedemptionRecord {
  /** Same as the source `BurnEvent.redemptionId` — the on-chain dedupe key. */
  redemptionId: string;
  nonce: number;
  amountStroops: string;
  piDestination: string;
  status: RedemptionStatus;
  piReleaseTxId?: string;
  lastError?: string;
  updatedAt: string;
}
