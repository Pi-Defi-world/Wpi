import { describe, expect, it } from 'vitest';
import {
  DEFAULT_ELIGIBILITY_ENABLED,
  DEFAULT_PI_CONFIRMATION_DEPTH,
  loadConfig,
} from '../src/config.js';

function validEnv(overrides: Record<string, string | undefined> = {}): NodeJS.ProcessEnv {
  return {
    PI_HORIZON_URL: 'https://api.testnet.example',
    PI_NETWORK_PASSPHRASE: 'Pi Testnet',
    PI_BRIDGE_DEPOSIT_ADDRESS: 'GDEPOSIT',
    PI_BRIDGE_CUSTODIAN_SECRET_KEY: 'SDEPOSIT',
    STELLAR_SOROBAN_RPC_URL: 'https://soroban-testnet.stellar.org',
    STELLAR_NETWORK_PASSPHRASE: 'Test SDF Network ; September 2015',
    WPI_CONTRACT_ID: 'CCONTRACT',
    BRIDGE_STELLAR_ADMIN_SECRET_KEY: 'SADMIN',
    ...overrides,
  };
}

describe('loadConfig', () => {
  it('loads a fully-specified config', () => {
    const config = loadConfig(validEnv());
    expect(config.pi.horizonUrl).toBe('https://api.testnet.example');
    expect(config.stellar.wpiContractId).toBe('CCONTRACT');
    expect(config.dryRun).toBe(false);
  });

  it('applies the documented default confirmation depth', () => {
    const config = loadConfig(validEnv());
    expect(config.pi.confirmationDepth).toBe(DEFAULT_PI_CONFIRMATION_DEPTH);
  });

  it('honors an overridden confirmation depth', () => {
    const config = loadConfig(validEnv({ PI_CONFIRMATION_DEPTH: '60' }));
    expect(config.pi.confirmationDepth).toBe(60);
  });

  it('rejects a non-positive confirmation depth', () => {
    expect(() => loadConfig(validEnv({ PI_CONFIRMATION_DEPTH: '0' }))).toThrow();
    expect(() => loadConfig(validEnv({ PI_CONFIRMATION_DEPTH: 'nope' }))).toThrow();
  });

  it('throws when a required var is missing', () => {
    const env = validEnv({ WPI_CONTRACT_ID: undefined });
    delete env.WPI_CONTRACT_ID;
    expect(() => loadConfig(env)).toThrow(/WPI_CONTRACT_ID/);
  });

  it('parses DRY_RUN as a boolean', () => {
    expect(loadConfig(validEnv({ DRY_RUN: 'true' })).dryRun).toBe(true);
    expect(loadConfig(validEnv({ DRY_RUN: '1' })).dryRun).toBe(true);
    expect(loadConfig(validEnv({ DRY_RUN: 'false' })).dryRun).toBe(false);
  });

  it('defaults the eligibility policy to enabled (fail closed)', () => {
    const config = loadConfig(validEnv());
    expect(config.pi.eligibilityEnabled).toBe(DEFAULT_ELIGIBILITY_ENABLED);
    expect(config.pi.eligibilityAllowlist).toEqual([]);
    expect(config.pi.eligibilityBlocklist).toEqual([]);
  });

  it('honors PI_ELIGIBILITY_ENABLED=false', () => {
    const config = loadConfig(validEnv({ PI_ELIGIBILITY_ENABLED: 'false' }));
    expect(config.pi.eligibilityEnabled).toBe(false);
  });

  it('parses the KYC allowlist and blocklist as comma-separated lists', () => {
    const config = loadConfig(
      validEnv({
        PI_ELIGIBILITY_ALLOWLIST: ' GABC, GDEF ,',
        PI_ELIGIBILITY_BLOCKLIST: 'GXXX',
      }),
    );
    expect(config.pi.eligibilityAllowlist).toEqual(['GABC', 'GDEF']);
    expect(config.pi.eligibilityBlocklist).toEqual(['GXXX']);
  });

  it('treats an empty allowlist/blocklist as unset', () => {
    const config = loadConfig(
      validEnv({ PI_ELIGIBILITY_ALLOWLIST: '', PI_ELIGIBILITY_BLOCKLIST: ' , ' }),
    );
    expect(config.pi.eligibilityAllowlist).toEqual([]);
    expect(config.pi.eligibilityBlocklist).toEqual([]);
  });
});
