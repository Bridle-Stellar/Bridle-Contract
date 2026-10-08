# Contributing to Bridle Contract

Thanks for helping. This contract guards an AI agent's spending authority,
so changes are held to a high bar for tests and clarity. Small, focused
PRs get reviewed fastest.

## Setup

You need Rust (stable, 1.84 or newer) and the `wasm32v1-none` target:

```bash
rustup target add wasm32v1-none
rustup component add rustfmt clippy
```

To deploy or invoke on testnet you also need
[stellar-cli](https://developers.stellar.org/docs/tools/cli) (v22+). You
don't need it just to build and test.

## Build, test, lint

These are the same checks CI runs on every PR. Run them before you push:

```bash
cargo fmt --all -- --check          # formatting
cargo clippy --all-targets -- -D warnings
cargo test                          # unit tests, native target
cargo build --target wasm32v1-none --release
```

The wasm ends up at `target/wasm32v1-none/release/bridle_contract.wasm`.

If you change a public type, function signature, or event, also update
[`docs/INTERFACE.md`](docs/INTERFACE.md). Regenerate the spec with:

```bash
stellar contract info interface \
  --wasm target/wasm32v1-none/release/bridle_contract.wasm --output json-formatted
```

Bridle Backend decodes these shapes, so an undocumented change breaks it.

## Picking up an issue

1. Look for issues labelled
   [`good first issue`](https://github.com/Bridle-Stellar/Bridle-Contract/labels/good%20first%20issue)
   or with a `complexity:` label that matches your experience. Each issue
   lists the background it needs.
2. Comment on the issue saying you'd like to take it, with a sentence or
   two on your approach. Wait for a maintainer to assign it to you before
   starting, so two people don't build the same thing.
3. If you go quiet for 7 days with no PR or update, the issue may be
   unassigned so someone else can pick it up. Just say so if you need
   more time.

For anything that changes contract behavior and isn't already an issue,
open an issue first. Docs fixes and typos can go straight to a PR.

## Branches and commits

- Branch from `main`. Name it `<type>/<short-description>`, e.g.
  `feat/category-caps`, `fix/rollover-boundary`, `docs/event-examples`.
- Write commit messages in the
  [Conventional Commits](https://www.conventionalcommits.org/) style the
  history already uses: `feat:`, `fix:`, `docs:`, `test:`, `ci:`,
  `refactor:`, `chore:`.
- One logical change per PR. If you find an unrelated problem along the
  way, open a separate issue or PR for it.

## Pull requests

- Fill in the PR template, and link the issue with `Closes #N`.
- New behavior needs tests in `src/test.rs`. Name each test after the
  behavior it guarantees. The existing names show the style, e.g.
  `rejects_every_spend_while_kill_switch_is_on`.
- Policy declines must stay non-reverting. Return
  `SpendOutcome::Rejected` and emit `spend_rejected`; never panic for a
  policy decision. The README explains why.
- `check_and_record_spend` must remain the only code path that writes the
  spend counter.
- CI must be green. A maintainer reviews every PR; security-sensitive
  changes (auth, ownership, the spend path) need extra review time.

## Security issues

Don't open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).

## Code of Conduct

Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).
