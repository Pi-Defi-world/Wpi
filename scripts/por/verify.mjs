#!/usr/bin/env node
/**
 * Verify a Wpi proof-of-reserve attestation signature and basic health checks.
 *
 * Usage:
 *   node scripts/por/verify.mjs <attestation.json> [--pubkey HEX|PATH]
 *
 * Historical keys (after rotation):
 *   node scripts/por/verify.mjs <file> --pubkey path/to/old-pubkey.hex
 */

import { createPublicKey, verify } from "node:crypto";
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(__dirname, "../..");

/** Immutable v1 signed-field order — must match schema + attest.mjs */
const DEFAULT_SIGNED_FIELDS = [
  "schema_version",
  "issued_at",
  "network",
  "wpi_contract_id",
  "wpi_total_supply",
  "pi_custody_account",
  "pi_custody_balance",
  "pi_balance_source",
  "safety_margin_bps",
];

const ALLOWED_NETWORKS = new Set(["testnet", "public"]);
/** Allow small clock skew on issued_at (ms). */
const FUTURE_SKEW_MS = 5 * 60 * 1000;

function hexToBuf(hex) {
  const h = hex.trim().replace(/^0x/i, "");
  if (!/^[0-9a-fA-F]+$/.test(h) || h.length % 2 !== 0) {
    throw new Error("invalid hex");
  }
  return Buffer.from(h, "hex");
}

function loadExpectedPubkey(argPubkey, attestation) {
  if (argPubkey) {
    if (existsSync(argPubkey)) {
      return readFileSync(argPubkey, "utf8").trim().replace(/^0x/i, "");
    }
    return argPubkey.trim().replace(/^0x/i, "");
  }
  const committed = join(REPO_ROOT, "attestations/ATTESTOR_PUBLIC_KEY");
  if (existsSync(committed)) {
    return readFileSync(committed, "utf8").trim().replace(/^0x/i, "");
  }
  return (attestation.attestor_public_key || "").trim();
}

function rawPubToKeyObject(rawHex) {
  const raw = hexToBuf(rawHex);
  if (raw.length !== 32) {
    throw new Error("public key must be 32 bytes hex");
  }
  // SPKI DER prefix for Ed25519 public key
  const prefix = Buffer.from("302a300506032b6570032100", "hex");
  const der = Buffer.concat([prefix, raw]);
  return createPublicKey({ key: der, format: "der", type: "spki" });
}

function assertSignedFields(attestation) {
  const fields = attestation.signed_fields;
  if (!Array.isArray(fields)) {
    throw new Error("signed_fields must be an array");
  }
  if (fields.length !== DEFAULT_SIGNED_FIELDS.length) {
    throw new Error(
      `signed_fields length ${fields.length} != v1 expected ${DEFAULT_SIGNED_FIELDS.length}`,
    );
  }
  for (let i = 0; i < DEFAULT_SIGNED_FIELDS.length; i++) {
    if (fields[i] !== DEFAULT_SIGNED_FIELDS[i]) {
      throw new Error(
        `signed_fields[${i}] is "${fields[i]}", expected "${DEFAULT_SIGNED_FIELDS[i]}" (v1 list is immutable)`,
      );
    }
  }
}

function parseNonNegativeBigInt(label, raw) {
  const v = String(raw).trim();
  if (!/^[0-9]+$/.test(v)) {
    throw new Error(`${label} must be a non-negative integer string`);
  }
  return BigInt(v);
}

function canonicalPayload(attestation) {
  assertSignedFields(attestation);
  const body = {};
  for (const k of DEFAULT_SIGNED_FIELDS) {
    if (!(k in attestation)) {
      throw new Error(`attestation missing signed field: ${k}`);
    }
    body[k] = attestation[k];
  }
  return JSON.stringify(body);
}

function recomputeStatusAndRatio(attestation) {
  const pi = parseNonNegativeBigInt(
    "pi_custody_balance",
    attestation.pi_custody_balance,
  );
  const supply = parseNonNegativeBigInt(
    "wpi_total_supply",
    attestation.wpi_total_supply,
  );
  const margin = Number(attestation.safety_margin_bps);
  if (!Number.isInteger(margin) || margin < 0 || margin > 10000) {
    throw new Error("safety_margin_bps must be integer 0..10000");
  }

  if (supply === 0n) {
    return { status: "healthy", collateral_ratio: "inf" };
  }
  const cap = (pi * BigInt(10000 - margin)) / 10000n;
  const ratio = Number(pi) / Number(supply);
  const collateral_ratio = Number.isFinite(ratio)
    ? ratio.toFixed(8)
    : "unknown";
  const status = supply <= cap ? "healthy" : "under_collateralized";
  return { status, collateral_ratio };
}

function validateIssuedAt(issuedAt) {
  const ms = Date.parse(issuedAt);
  if (!Number.isFinite(ms)) {
    throw new Error(`issued_at is not a valid timestamp: ${issuedAt}`);
  }
  if (ms > Date.now() + FUTURE_SKEW_MS) {
    throw new Error(
      `issued_at is in the future beyond allowed skew: ${issuedAt}`,
    );
  }
  return ms;
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0 || args[0] === "-h" || args[0] === "--help") {
    console.log(
      "Usage: node scripts/por/verify.mjs <attestation.json> [--pubkey HEX|PATH]",
    );
    process.exit(args.length === 0 ? 1 : 0);
  }

  const path = resolve(args[0]);
  let pubkeyArg;
  const idx = args.indexOf("--pubkey");
  if (idx !== -1) pubkeyArg = args[idx + 1];

  const attestation = JSON.parse(readFileSync(path, "utf8"));

  if (attestation.schema_version !== "1.0") {
    console.error(
      `FAIL: unsupported schema_version "${attestation.schema_version}"`,
    );
    process.exit(1);
  }

  if (!ALLOWED_NETWORKS.has(attestation.network)) {
    console.error(
      `FAIL: network must be testnet|public (got "${attestation.network}")`,
    );
    process.exit(1);
  }

  // Fail closed on bad timestamps before claiming signature OK
  let issuedMs;
  try {
    issuedMs = validateIssuedAt(attestation.issued_at);
  } catch (e) {
    console.error(`FAIL: ${e.message}`);
    process.exit(1);
  }

  const expectedPub = loadExpectedPubkey(pubkeyArg, attestation);
  if (!expectedPub) {
    throw new Error(
      "no public key: pass --pubkey or commit ATTESTOR_PUBLIC_KEY",
    );
  }

  if (
    attestation.attestor_public_key &&
    attestation.attestor_public_key.toLowerCase() !== expectedPub.toLowerCase()
  ) {
    console.error(
      "FAIL: attestor_public_key in file does not match expected pubkey",
    );
    console.error(
      "  Hint: for historical attestations after key rotation, pass --pubkey <old-key>",
    );
    process.exit(1);
  }

  if (!attestation.signature) {
    console.error("FAIL: missing signature");
    process.exit(1);
  }

  let payload;
  try {
    payload = canonicalPayload(attestation);
  } catch (e) {
    console.error(`FAIL: ${e.message}`);
    process.exit(1);
  }

  const pub = rawPubToKeyObject(expectedPub);
  const ok = verify(
    null,
    Buffer.from(payload, "utf8"),
    pub,
    hexToBuf(attestation.signature),
  );

  if (!ok) {
    console.error("FAIL: invalid signature");
    process.exit(1);
  }

  let recomputed;
  try {
    recomputed = recomputeStatusAndRatio(attestation);
  } catch (e) {
    console.error(`FAIL: ${e.message}`);
    process.exit(1);
  }

  if (attestation.status !== recomputed.status) {
    console.error(
      `FAIL: status field "${attestation.status}" != recomputed "${recomputed.status}"`,
    );
    process.exit(1);
  }

  // collateral_ratio is NOT signed — recompute and compare (or reject spoofed display)
  if (attestation.collateral_ratio !== recomputed.collateral_ratio) {
    console.error(
      `FAIL: collateral_ratio "${attestation.collateral_ratio}" != recomputed "${recomputed.collateral_ratio}"`,
    );
    process.exit(1);
  }

  const ageMs = Date.now() - issuedMs;
  const ageHours = ageMs / (1000 * 60 * 60);
  const stale = ageHours > 2;

  console.log("OK: signature valid");
  console.log(`  file:     ${path}`);
  console.log(`  status:   ${recomputed.status}`);
  console.log(`  ratio:    ${recomputed.collateral_ratio} (recomputed)`);
  console.log(`  supply:   ${attestation.wpi_total_supply}`);
  console.log(`  reserve:  ${attestation.pi_custody_balance}`);
  console.log(`  issued:   ${attestation.issued_at}`);
  if (stale) {
    console.warn(
      `WARN: attestation is ~${ageHours.toFixed(1)}h old (threshold 2h)`,
    );
  }
  if (recomputed.status !== "healthy") {
    process.exitCode = 2;
  }
}

try {
  main();
} catch (e) {
  console.error("error:", e.message || e);
  process.exit(1);
}
