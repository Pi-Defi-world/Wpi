/**
 * Confirmation-depth policy: number of Pi Network ledgers that must close on
 * top of a deposit's ledger before the relayer will mint wPi against it.
 *
 * Pi Network (an SCP/Stellar fork) closes ledgers roughly every 3-5s and,
 * like Stellar, does not fork under normal validator operation — but the
 * bridge is a one-way irreversible action (mint) triggered by an
 * observation of a chain we don't validate ourselves, so it deliberately
 * uses a conservative margin rather than trusting instant finality.
 * 30 ledgers (~2-3 minutes) is the default; operators can widen it via
 * PI_CONFIRMATION_DEPTH as real-world reorg data accumulates.
 */
export const DEFAULT_PI_CONFIRMATION_DEPTH = 30;

export const DEFAULT_PI_POLL_INTERVAL_MS = 5_000;
export const DEFAULT_STELLAR_POLL_INTERVAL_MS = 5_000;
export const DEFAULT_STORE_PATH = './data/relayer-state.json';

export interface RelayerConfig {
  pi: {
    horizonUrl: string;
    networkPassphrase: string;
    bridgeDepositAddress: string;
    /** Secret key for `bridgeDepositAddress`, used to sign outgoing Pi releases on redemption. */
    custodianSecretKey: string;
    confirmationDepth: number;
    pollIntervalMs: number;
  };
  stellar: {
    rpcUrl: string;
    networkPassphrase: string;
    wpiContractId: string;
    adminSecretKey: string;
    pollIntervalMs: number;
  };
  storePath: string;
  /** When true, log intended actions (mints/releases) instead of submitting them. */
  dryRun: boolean;
}

function required(env: NodeJS.ProcessEnv, name: string): string {
  const value = env[name];
  if (!value) {
    throw new Error(`Missing required env var: ${name}`);
  }
  return value;
}

function optionalInt(env: NodeJS.ProcessEnv, name: string, fallback: number): number {
  const raw = env[name];
  if (!raw) return fallback;
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`Env var ${name} must be a positive integer, got: ${raw}`);
  }
  return parsed;
}

function optionalBool(env: NodeJS.ProcessEnv, name: string, fallback: boolean): boolean {
  const raw = env[name];
  if (raw === undefined) return fallback;
  return raw === 'true' || raw === '1';
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): RelayerConfig {
  return {
    pi: {
      horizonUrl: required(env, 'PI_HORIZON_URL'),
      networkPassphrase: required(env, 'PI_NETWORK_PASSPHRASE'),
      bridgeDepositAddress: required(env, 'PI_BRIDGE_DEPOSIT_ADDRESS'),
      custodianSecretKey: required(env, 'PI_BRIDGE_CUSTODIAN_SECRET_KEY'),
      confirmationDepth: optionalInt(
        env,
        'PI_CONFIRMATION_DEPTH',
        DEFAULT_PI_CONFIRMATION_DEPTH,
      ),
      pollIntervalMs: optionalInt(env, 'PI_POLL_INTERVAL_MS', DEFAULT_PI_POLL_INTERVAL_MS),
    },
    stellar: {
      rpcUrl: required(env, 'STELLAR_SOROBAN_RPC_URL'),
      networkPassphrase: required(env, 'STELLAR_NETWORK_PASSPHRASE'),
      wpiContractId: required(env, 'WPI_CONTRACT_ID'),
      adminSecretKey: required(env, 'BRIDGE_STELLAR_ADMIN_SECRET_KEY'),
      pollIntervalMs: optionalInt(
        env,
        'STELLAR_POLL_INTERVAL_MS',
        DEFAULT_STELLAR_POLL_INTERVAL_MS,
      ),
    },
    storePath: env['RELAYER_STORE_PATH'] ?? DEFAULT_STORE_PATH,
    dryRun: optionalBool(env, 'DRY_RUN', false),
  };
}
