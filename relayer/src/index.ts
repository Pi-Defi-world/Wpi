import { loadConfig } from './config.js';
import { createLogger } from './log.js';
import { DryRunPiPayoutClient } from './pi/dryRunPiPayoutClient.js';
import { DepositWatcher } from './pi/depositWatcher.js';
import { HorizonPiClient } from './pi/horizonPiClient.js';
import { HorizonPiPayoutClient } from './pi/horizonPiPayoutClient.js';
import type { PiPayoutClient } from './pi/piPayoutClient.js';
import { Orchestrator } from './orchestrator.js';
import { JsonFileStore } from './store/jsonFileStore.js';
import { DryRunWpiContractClient } from './stellar/dryRunWpiContractClient.js';
import { MintSubmitter } from './stellar/mintSubmitter.js';
import { RedemptionWatcher } from './stellar/redemptionWatcher.js';
import { SorobanWpiContractClient } from './stellar/sorobanWpiContractClient.js';
import type { WpiContractClient } from './stellar/wpiContractClient.js';

async function main(): Promise<void> {
  const config = loadConfig();
  const log = createLogger('relayer');
  const store = new JsonFileStore(config.storePath);

  const piClient = new HorizonPiClient(config.pi.horizonUrl, config.pi.bridgeDepositAddress);
  const depositWatcher = new DepositWatcher(
    piClient,
    store,
    { confirmationDepth: config.pi.confirmationDepth },
    createLogger('deposit-watcher'),
  );

  let contractClient: WpiContractClient = await SorobanWpiContractClient.connect(
    {
      rpcUrl: config.stellar.rpcUrl,
      networkPassphrase: config.stellar.networkPassphrase,
      contractId: config.stellar.wpiContractId,
      adminSecretKey: config.stellar.adminSecretKey,
    },
    createLogger('soroban-client'),
  );

  let payoutClient: PiPayoutClient = new HorizonPiPayoutClient(
    config.pi.horizonUrl,
    config.pi.networkPassphrase,
    config.pi.custodianSecretKey,
  );

  if (config.dryRun) {
    contractClient = new DryRunWpiContractClient(contractClient, createLogger('dry-run-contract'));
    payoutClient = new DryRunPiPayoutClient(createLogger('dry-run-payout'));
    log.warn('DRY_RUN enabled: mints and Pi releases will be logged, not submitted');
  }

  const mintSubmitter = new MintSubmitter(contractClient, store, createLogger('mint-submitter'));
  const redemptionWatcher = new RedemptionWatcher(
    contractClient,
    payoutClient,
    store,
    createLogger('redemption-watcher'),
  );

  const orchestrator = new Orchestrator(
    depositWatcher,
    mintSubmitter,
    redemptionWatcher,
    {
      piPollIntervalMs: config.pi.pollIntervalMs,
      stellarPollIntervalMs: config.stellar.pollIntervalMs,
    },
    log,
  );

  for (const signal of ['SIGINT', 'SIGTERM'] as const) {
    process.on(signal, () => {
      log.info(`received ${signal}, shutting down`);
      orchestrator.stop();
      process.exit(0);
    });
  }

  log.info('relayer starting', {
    confirmationDepth: config.pi.confirmationDepth,
    dryRun: config.dryRun,
  });
  orchestrator.start();
}

main().catch((err: unknown) => {
  console.error('relayer failed to start', err);
  process.exit(1);
});
