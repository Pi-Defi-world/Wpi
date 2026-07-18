import { contract, Keypair, nativeToScVal, rpc, scValToNative, StrKey } from '@stellar/stellar-sdk';
import type { Logger } from '../log.js';
import type { BurnEvent } from '../types.js';
import { piDestinationToStrKey } from '../util/piAddress.js';
import type { MintOutcome, WpiContractClient } from './wpiContractClient.js';

const REDEMPTION_BURNED_TOPIC = 'redemption_burned';

export interface SorobanWpiContractClientOptions {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
  adminSecretKey: string;
}

/**
 * `WpiContractClient` backed by the real wPi Soroban contract, via
 * `@stellar/stellar-sdk`'s dynamic contract `Client` (built from the
 * on-chain contract spec) for calls, and `rpc.Server` directly for
 * `getEvents` (the dynamic Client doesn't expose event queries).
 */
export class SorobanWpiContractClient implements WpiContractClient {
  private constructor(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- dynamically generated per contract spec
    private readonly contractClient: any,
    private readonly server: rpc.Server,
    private readonly contractId: string,
    private readonly log: Logger,
  ) {}

  static async connect(
    opts: SorobanWpiContractClientOptions,
    log: Logger,
  ): Promise<SorobanWpiContractClient> {
    const keypair = Keypair.fromSecret(opts.adminSecretKey);
    const signer = contract.basicNodeSigner(keypair, opts.networkPassphrase);
    const contractClient = await contract.Client.from({
      contractId: opts.contractId,
      networkPassphrase: opts.networkPassphrase,
      rpcUrl: opts.rpcUrl,
      publicKey: keypair.publicKey(),
      ...signer,
    });
    const server = new rpc.Server(opts.rpcUrl);
    return new SorobanWpiContractClient(contractClient, server, opts.contractId, log);
  }

  async isDepositProcessed(depositIdHex: string): Promise<boolean> {
    const tx = await this.contractClient.is_deposit_processed({
      pi_deposit_id: Buffer.from(depositIdHex, 'hex'),
    });
    return tx.result as boolean;
  }

  async mintFromDeposit(args: {
    to: string;
    amountStroops: bigint;
    depositIdHex: string;
  }): Promise<MintOutcome> {
    try {
      const tx = await this.contractClient.mint_from_deposit({
        admin: this.contractClient.options.publicKey,
        to: args.to,
        amount: args.amountStroops,
        pi_deposit_id: Buffer.from(args.depositIdHex, 'hex'),
      });
      const sent = await tx.signAndSend();
      const hash = sent.sendTransactionResponse?.hash ?? sent.getTransactionResponse?.txHash ?? '';
      return { minted: true, txHash: hash };
    } catch (err) {
      // The submission's outcome is ambiguous from here (it may have
      // failed before or after committing, or the response may have been
      // dropped) — reconcile against on-chain state rather than parse SDK
      // error text, which varies across error paths and SDK versions.
      if (await this.isDepositProcessed(args.depositIdHex)) {
        this.log.warn(
          'mint submission raised an error but deposit is already processed on-chain',
          { depositIdHex: args.depositIdHex, error: err instanceof Error ? err.message : String(err) },
        );
        return { minted: false, alreadyProcessed: true };
      }
      throw err;
    }
  }

  async getRedemptionBurnEvents(
    sinceLedger: number,
  ): Promise<{ events: BurnEvent[]; nextLedger: number }> {
    const topicScVal = nativeToScVal(REDEMPTION_BURNED_TOPIC, { type: 'symbol' });
    const response = await this.server.getEvents({
      startLedger: sinceLedger,
      filters: [
        {
          type: 'contract',
          contractIds: [this.contractId],
          topics: [[topicScVal.toXDR('base64'), '*']],
        },
      ],
      limit: 200,
    });

    const events: BurnEvent[] = [];
    for (const event of response.events) {
      const nonceTopic = event.topic[1];
      if (!nonceTopic) continue;
      const nonce = Number(scValToNative(nonceTopic) as bigint);
      const data = scValToNative(event.value) as {
        from: string;
        amount: bigint;
        pi_destination: Buffer;
      };
      events.push({
        ledger: event.ledger,
        txHash: event.txHash,
        eventId: event.id,
        nonce,
        from: data.from,
        amountStroops: data.amount.toString(),
        piDestination: piDestinationToStrKey(Buffer.from(data.pi_destination)),
      });
    }

    return { events, nextLedger: response.latestLedger + 1 };
  }
}

export { StrKey };
