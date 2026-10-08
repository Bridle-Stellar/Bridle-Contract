## What and why

<!-- What does this change, and why? Link the issue: Closes #N -->

## How it was tested

<!-- New tests, and anything you ran by hand (e.g. testnet invocations). -->

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test` passes, and new behavior has a test
- [ ] Policy declines still return `SpendOutcome::Rejected` and never panic
- [ ] `check_and_record_spend` is still the only writer of the spend counter
- [ ] `docs/INTERFACE.md` updated if a return type, event, error, or signature changed
- [ ] No secrets, keys, or `.env` files in the diff
