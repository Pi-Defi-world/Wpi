# Public attestations

This directory is the **public surface** for short-term proof-of-reserve (Issue #25).

> **Demo feed:** Files here currently hold **demonstration** attestations (placeholder contract id / manual balances). Do not treat them as production custody proofs until operators run the attestor with real config on a cadence.

| File | Purpose |
|------|---------|
| `latest.json` | Most recent signed attestation (**demo** until production cadence) |
| `history/` | Optional dated snapshots |
| `schema.json` | JSON Schema for attestations (v1 signed fields immutable) |
| `ATTESTOR_PUBLIC_KEY` | Current hex Ed25519 public key used to verify signatures |
| `sample.json` | Example attestation for docs / offline verify demos |
| `keys/retired/` | Optional retired public keys for historical verification |

## Verify

```bash
# Current key (default)
node scripts/por/verify.mjs attestations/latest.json

# Historical attestation after key rotation
node scripts/por/verify.mjs attestations/history/<file>.json \
  --pubkey attestations/keys/retired/<YYYY-MM-DD>.hex
```

Process documentation: [`docs/proof-of-reserve.md`](../docs/proof-of-reserve.md).
