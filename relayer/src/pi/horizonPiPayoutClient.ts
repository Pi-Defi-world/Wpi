import { Account, Asset, BASE_FEE, Keypair, Memo, Operation, TransactionBuilder } from '@stellar/stellar-sdk';
import { stroopsToDecimal } from '../util/amount.js';
import type { PiPayoutClient } from './piPayoutClient.js';

interface HorizonAccountResponse {
  sequence: string;
}

interface HorizonSubmitResponse {
  hash: string;
}

/**
 * `PiPayoutClient` backed by a real Pi Network custodial account, submitting
 * a native-Pi payment through Pi's Horizon-compatible REST API. Pi Network
 * is an SCP fork, so Stellar's transaction envelope format and signing
 * apply directly to the underlying custodian keypair.
 */
export class HorizonPiPayoutClient implements PiPayoutClient {
  constructor(
    private readonly horizonUrl: string,
    private readonly networkPassphrase: string,
    private readonly custodianSecretKey: string,
  ) {}

  async releaseFunds(args: {
    toPiAddress: string;
    amountStroops: bigint;
    memo: string;
  }): Promise<{ piTxId: string }> {
    const keypair = Keypair.fromSecret(this.custodianSecretKey);
    const accountRes = await fetch(`${this.horizonUrl}/accounts/${keypair.publicKey()}`);
    if (!accountRes.ok) {
      throw new Error(
        `Failed to load Pi custodian account: ${accountRes.status} ${accountRes.statusText}`,
      );
    }
    const accountData = (await accountRes.json()) as HorizonAccountResponse;
    const account = new Account(keypair.publicKey(), accountData.sequence);

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(
        Operation.payment({
          destination: args.toPiAddress,
          asset: Asset.native(),
          amount: stroopsToDecimal(args.amountStroops),
        }),
      )
      .addMemo(Memo.text(args.memo.slice(0, 28)))
      .setTimeout(60)
      .build();
    tx.sign(keypair);

    const submitRes = await fetch(`${this.horizonUrl}/transactions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: `tx=${encodeURIComponent(tx.toXDR())}`,
    });
    if (!submitRes.ok) {
      const body = await submitRes.text();
      throw new Error(`Pi payout submission failed: ${submitRes.status} ${body}`);
    }
    const result = (await submitRes.json()) as HorizonSubmitResponse;
    return { piTxId: result.hash };
  }
}
