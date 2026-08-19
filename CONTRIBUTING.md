# Contributing to Wpi

Thanks for considering a contribution to the Wrapped Pi (wPi) bridge. This
guide explains how to build, test, and submit changes consistently so outside
reviewers and contributors can engage without friction.

- **Issues** — use the issue templates in
  [.github/ISSUE_TEMPLATE/](./.github/ISSUE_TEMPLATE/).
- **Pull requests** — use the PR template in
  [.github/PULL_REQUEST_TEMPLATE.md](./.github/PULL_REQUEST_TEMPLATE.md).
- **Code ownership** — see [CODEOWNERS](./CODEOWNERS).

## Project layout

| Path | What lives here |
|------|-----------------|
| `Stellar-contracts-v1/` | Soroban contracts (`wpi-token`, `mock-amm`) in Rust |
| `relayer/` | TypeScript bridge relayer (deposit watcher, mint submitter, redemption watcher) |
| `scripts/` | Deployment / PoR / checksum shell and Node scripts |
| `docs/` | Design docs, proof-of-reserve, deposit-eligibility policy |
| `.github/workflows/` | CI, release, and network-drift check workflows |

## Building and testing

### Contracts (Rust / Soroban)

Requirements: Rust `1.88.0` (pinned in
`Stellar-contracts-v1/rust-toolchain.toml`) with the `wasm32-unknown-unknown`
target, and a Soroban CLI aligned with `soroban-sdk 23.0.1`.

```bash
make build     # cargo build --release --target wasm32-unknown-unknown
make test      # cargo test
```

Formatting and linting (must be clean before merge):

```bash
cd Stellar-contracts-v1
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

### Relayer (TypeScript / Node)

Requirements: Node `>= 20`.

```bash
cd relayer
npm ci
npm run typecheck
npm run lint
npm test
npm run build
```

A dependency-free end-to-end demo runs against in-process fakes:

```bash
npm run demo:e2e
```

## Making changes

1. **Pick or open an issue first.** External reviewers rely on issue
   numbering; reference the issue in your PR description (e.g. `Closes #27`).
2. **Branch from `main`.** Use a descriptive branch name, typically
   `fix/…`, `feat/…`, `docs/…`, or `chore/…`.
3. **Keep changes focused.** One logical change per PR. Resist bundling
   unrelated refactors with a bug fix.
4. **Add or update tests.** New behavior ships with tests. Existing suites
   must stay green.
5. **Run the full local checks** listed above before pushing.

## Pull request expectations

- The CI workflow runs on every PR and **must pass**: Rust fmt/clippy/tests and
  the WASM-size regression check, plus the relayer typecheck/lint/tests.
- Changes that touch the protocol-version pins (`COMPATIBILITY.md`,
  `protocol-versions.json`) must keep them in sync — the
  **Protocol Version Drift Check** validates them against the live networks.
- Changes that alter bridge deposit/redemption behavior must update the
  relevant policy doc ([docs/deposit-eligibility.md](./docs/deposit-eligibility.md),
  [Compatibility](./COMPATIBILITY.md)) and note it in the PR description.
- User-visible or operationally significant changes get a `CHANGELOG.md` entry
  under `[Unreleased]`.

### Commit messages

The repository uses [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <summary>

<optional body>

<optional footer, e.g. Closes #27>
```

Common types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `ci`.
Examples from this repo's history:

```
feat(wpi-token): cap mint size per transaction
fix(bridge): add mint and burn replay protection (#6)
chore(release): add changelog and contract release versioning strategy
docs: document Pi/Stellar protocol-version compatibility
```

## Review process

- All PRs require at least one approving review from a CODEOWNER.
- Be responsive to review comments; small iterative fixups in new commits are
  preferred over force-push rewrites (the final merge is squash-merged by
  maintainers).
- Never merge your own PRs unless you are a maintainer doing an explicitly
  delegated release.

## Code conventions

- **Rust:** `cargo fmt` formatting, `#![no_std]` for contracts, no panics in
  reachable paths, explicit error handling via contract errors/enums.
- **TypeScript:** strict TypeScript (`tsconfig.json`), ESM imports with
  explicit `.js` extensions, ESLint with the repo config, no unused vars.
- **Docs:** keep policy documents authoritative and dated. When you change
  behavior, update the doc that describes it in the same PR.

## Security

- Do not commit secrets, keys, or `.env` files. The root `.gitignore` and
  `relayer/.gitignore` already exclude these — do not work around them.
- Report security issues privately to the maintainers (see
  [CODEOWNERS](./CODEOWNERS)) rather than opening a public issue.

## Compatibility with Pi ↔ Stellar

Before changing SDK versions, deployment scripts, or anything that interacts
with either network, read [COMPATIBILITY.md](./COMPATIBILITY.md) and follow its
update policy — the CI drift check will enforce that the pins stay honest.