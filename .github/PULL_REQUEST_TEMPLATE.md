## Summary

<!-please describe what this PR does and why -->

Closes #<!-- issue number -->

## Type of change

<!-- Mark the boxes that apply with an `x` -->

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor (no behavioral change)
- [ ] Test-only
- [ ] Docs only
- [ ] CI/workflow change

## Checklist

<!-- Completed items get an `x` -->

- [ ] Branch is based on the latest `main` and the PR targets `main`.
- [ ] Local checks pass:
  - Rust: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
  - Relayer: `npm run typecheck`, `npm run lint`, `npm test`
- [ ] No secrets, keys, or `.env` files are committed.
- [ ] Tests cover the new behavior (or existing tests were updated where
      behavior intentionally changed).
- [ ] `CHANGELOG.md` has an `[Unreleased]` entry if this is user-visible or
      operationally significant.
- [ ] `COMPATIBILITY.md` / `protocol-versions.json` were updated if this
      touches SDK versions, deployment, or either network's protocol pins.
- [ ] Policy docs were updated if this touches deposit/redemption behavior
      ([docs/deposit-eligibility.md](../docs/deposit-eligibility.md)).

## Impact

Describe any effect on bridge safety (mint/redeem), the eligibility policy,
protocol compatibility, or operational tooling. If none, say "none".

## Screenshots / logs

Optional: CI output, WASM size deltas, or logs supporting the change.

## Additional context

Anything else reviewers should know.