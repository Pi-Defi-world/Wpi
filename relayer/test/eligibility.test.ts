import { describe, expect, it } from 'vitest';
import { DepositEligibilityPolicy } from '../src/pi/eligibility.js';
import type { AccountEligibility, PiClient } from '../src/pi/piClient.js';

class FakeChain implements Pick<PiClient, 'getAccountEligibility'> {
  constructor(private readonly result: AccountEligibility) {}

  getAccountEligibility(_accountId: string): Promise<AccountEligibility> {
    return Promise.resolve(this.result);
  }
}

describe('DepositEligibilityPolicy', () => {
  it('passes every account through when disabled', async () => {
    const policy = new DepositEligibilityPolicy(new FakeChain({ eligible: false }), {
      enabled: false,
    });
    await expect(policy.check('GABC')).resolves.toEqual({ eligible: true });
  });

  it('delegates to the chain lookup when enabled with no lists', async () => {
    const policy = new DepositEligibilityPolicy(new FakeChain({ eligible: true }), {
      enabled: true,
    });
    await expect(policy.check('GABC')).resolves.toEqual({ eligible: true });
  });

  it('propagates a chain rejection', async () => {
    const policy = new DepositEligibilityPolicy(
      new FakeChain({ eligible: false, reason: 'account_not_found' }),
      { enabled: true },
    );
    await expect(policy.check('GNOTMIGRATED')).resolves.toEqual({
      eligible: false,
      reason: 'account_not_found',
    });
  });

  it('rejects an account on the blocklist before consulting the chain', async () => {
    let chainCalled = false;
    const chain = {
      getAccountEligibility(): Promise<AccountEligibility> {
        chainCalled = true;
        return Promise.resolve({ eligible: true });
      },
    };
    const policy = new DepositEligibilityPolicy(chain, {
      enabled: true,
      blocklist: ['GFROZEN'],
    });

    await expect(policy.check('GFROZEN')).resolves.toEqual({
      eligible: false,
      reason: 'blocklisted',
    });
    expect(chainCalled).toBe(false);
  });

  it('rejects an account missing from the allowlist before consulting the chain', async () => {
    let chainCalled = false;
    const chain = {
      getAccountEligibility(): Promise<AccountEligibility> {
        chainCalled = true;
        return Promise.resolve({ eligible: true });
      },
    };
    const policy = new DepositEligibilityPolicy(chain, {
      enabled: true,
      allowlist: ['GAPPROVED'],
    });

    await expect(policy.check('GNOTAPPROVED')).resolves.toEqual({
      eligible: false,
      reason: 'not_allowlisted',
    });
    expect(chainCalled).toBe(false);
  });

  it('still checks the chain for an allowlisted account', async () => {
    const chain = new FakeChain({ eligible: true });
    const policy = new DepositEligibilityPolicy(chain, {
      enabled: true,
      allowlist: ['GAPPROVED'],
    });

    await expect(policy.check('GAPPROVED')).resolves.toEqual({ eligible: true });
  });

  it('gives the blocklist precedence over the allowlist', async () => {
    const chain = new FakeChain({ eligible: true });
    const policy = new DepositEligibilityPolicy(chain, {
      enabled: true,
      allowlist: ['GAPPROVED'],
      blocklist: ['GAPPROVED'],
    });

    await expect(policy.check('GAPPROVED')).resolves.toEqual({
      eligible: false,
      reason: 'blocklisted',
    });
  });
});