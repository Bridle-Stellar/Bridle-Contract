# Bridle Contract

The on-chain policy-enforcement layer of **Bridle**. Bridle lets a human
("the owner") set spending guardrails for an autonomous AI agent's
Stellar wallet; this Soroban contract is the source of truth for those
guardrails. A separate off-chain relayer (not part of this repo) calls
`check_and_record_spend` before forwarding any agent payment, and must
treat a rejection as a hard stop.

Think of it as a programmable allowance: the owner sets a daily spend
cap, a per-call maximum, and a merchant allowlist once; the contract
enforces them on every subsequent spend attempt, with no code path that
lets the agent — or a compromised relayer — change the rules or spend
past them.

This repo is **only** the contract. There's no UI, no relayer/backend
service, and no token custody here — see [Non-goals](#non-goals) below.

## What it enforces

- **Daily spend cap** — total spend in the governed token allowed within
  one UTC calendar day.
- **Per-call maximum** — the largest single payment allowed, independent
  of remaining daily budget.
- **Merchant allowlist** — spends to a destination not explicitly approved
  by the owner are rejected. Each entry carries a `category` tag (e.g.
  `compute`, `data`, `api`) for future category-based rules; v1 only
  checks allowlist membership.
- **Kill switch** — a boolean the owner can flip to instantly block every
  spend, regardless of every other setting, and flip back to resume.

All four are checked on every call to `check_and_record_spend`, which is
the *only* function that can move the daily spend counter — there is no
other code path that writes it.

## Auth model

There are two identities involved in a spend: the **relayer**, an
off-chain service that submits the Stellar transaction, and the
**agent**, the AI system whose spending is being guarded.

**Decision: the agent's own key authorizes every spend, via Soroban's
native authorization framework — not the relayer's identity.**
`check_and_record_spend` calls `agent.require_auth()`. Soroban binds a
signed authorization entry to the exact invocation it was signed for —
this contract, this function, these exact `destination`/`token`/`amount`
arguments — so a valid authorization is cryptographic proof the agent
itself requested precisely this spend. The relayer submits the
transaction and supplies that authorization entry but cannot fabricate
one, alter its arguments, or reuse it for a different spend.

This means **a compromised relayer cannot approve a spend the agent never
signed off on** — the reason this guardrail lives on-chain instead of
purely in the relayer's backend. What the relayer *is* trusted for:
choosing which agent+destination+token+amount to construct and ask the
agent to sign, and submitting the resulting transaction. It cannot bypass
the owner's caps, allowlist, or kill switch, because those are enforced
here regardless of who submits the call. If the agent's key itself is
compromised, this contract's guardrails — the daily cap, the per-call
max, the allowlist — are the intended last line of defense.

We did not need a weaker v1 fallback here: requiring
`agent.require_auth()` is the standard, idiomatic way to authorize a
Soroban call and was no harder to implement than trusting the relayer's
identity would have been.

### Why a rejected spend doesn't revert the transaction

`check_and_record_spend` never panics or returns a contract error for a
*policy* decision (kill switch, allowlist, either cap) — a decline is a
normal, expected outcome that needs to be recorded as history, not an
exceptional failure. If it reverted instead, Soroban would roll back the
whole call and, on most RPC configurations, discard the very events this
function exists to produce. So a declined request still completes
successfully and emits a `spend_rejected` event describing why; it
returns a `SpendOutcome` enum (`Approved { .. } | Rejected(reason)`), and
**the relayer must not forward payment when it reads `Rejected`.**

The one thing that does still hard-fail the transaction is
`agent.require_auth()` itself — an invocation lacking a valid signed
authorization from the named agent traps immediately, the same as any
other unauthorized Soroban call. That's a missing signature, not a policy
decision, so it isn't reflected in `SpendOutcome`.

## Relationship to OpenZeppelin's `stellar-accounts`

OpenZeppelin's Stellar contracts ship a spending-limit *policy* module
([`stellar-accounts`](https://github.com/OpenZeppelin/stellar-contracts)),
and it was evaluated before writing this contract rather than
reimplementing spend-limit primitives from scratch. It wasn't adopted
as a dependency, for two reasons:

1. It's a plug-in for OpenZeppelin's own smart-account /
   context-rule / verifier framework (multisig, WebAuthn signers, etc.).
   Using it means adopting that whole account architecture, not just a
   spend check, for a contract that only needs a single owner and a
   small, fixed agent set.
2. Its limit is a **rolling ledger-window** total, with no concept of a
   merchant allowlist, a per-call max, or a kill switch — three of
   Bridle's four guardrails.

What *was* carried over from it: the pattern of tracking a running spend
total against a window and rolling it over lazily on read rather than
needing a scheduled reset (see `policy::effective_spend_state`). A future
version could go the other direction — expose Bridle's checks as a policy
plugin *for* an OpenZeppelin smart account — if multisig ownership
becomes a goal; storage was kept simple enough that this migration stays
open.

## Contract interface

| Function | Caller | Effect |
|---|---|---|
| `initialize(owner, agent, token, daily_cap, per_call_max)` | owner (once) | Sets up the contract. Kill switch starts off, allowlist starts empty. |
| `check_and_record_spend(agent, destination, token, amount) -> SpendOutcome` | agent | The only function that evaluates and records a spend. |
| `get_policy() -> PolicySnapshot` | anyone | Owner, agents, token, caps, kill switch, full allowlist. |
| `get_spend_status() -> SpendStatus` | anyone | Today's spend so far and remaining budget. |
| `update_daily_cap(new_cap)` | owner | Changes the daily cap. |
| `update_per_call_max(new_max)` | owner | Changes the per-call max. |
| `add_allowlist_entry(destination, category)` | owner | Approves a destination. |
| `remove_allowlist_entry(destination)` | owner | Revokes a destination. |
| `add_agent(agent)` | owner | Registers an additional agent address. |
| `remove_agent(agent)` | owner | Deregisters an agent (refuses to remove the last one). |
| `set_kill_switch(active)` | owner | Instantly blocks or resumes all spending. |
| `transfer_ownership(new_owner)` | owner | Step 1 of a two-step ownership transfer. |
| `accept_ownership()` | pending owner | Step 2 — finalizes the transfer. |

Every owner-only function checks `require_auth()` against the address
*currently stored* as owner, never against a caller-supplied parameter —
so holding any other key, including an agent's, can never satisfy it.
Ownership transfer is two-step (`transfer_ownership` + `accept_ownership`)
so a mistyped or unreachable new-owner address can't permanently lock out
policy control.

### Events

Each is a `#[contractevent]` struct; the topic list starts with the event
name in snake_case, followed by any `#[topic]`-marked fields (bold
below), so an indexer can filter by event kind, by agent, or by
destination without decoding data payloads.

| Event (topic) | Further topics | Data fields | Emitted when |
|---|---|---|---|
| `spend_approved` | **agent**, **destination** | token, amount, spent_today, remaining_today | A spend is approved. |
| `spend_rejected` | **agent**, **destination** | token, amount, reason | A spend is rejected. |
| `daily_cap_updated` | — | new_cap | Daily cap changed. |
| `per_call_max_updated` | — | new_max | Per-call max changed. |
| `kill_switch_toggled` | — | active | Kill switch toggled. |
| `allowlist_entry_added` | **destination** | category | Destination approved. |
| `allowlist_entry_removed` | **destination** | — | Destination revoked. |
| `agent_added` | **agent** | — | Agent registered. |
| `agent_removed` | **agent** | — | Agent deregistered. |
| `ownership_transfer_proposed` | **current_owner** | pending_owner | Ownership transfer proposed. |
| `ownership_transferred` | **previous_owner** | new_owner | Ownership transfer finalized. |

## Non-goals

- No UI or dashboard (see Bridle Frontend).
- No relayer/backend service beyond what's needed to exercise the
  contract in tests (see Bridle Backend).
- No token custody: this contract only *authorizes* — it never holds or
  moves tokens. The relayer performs the actual SEP-41 transfer via the
  standard Soroban token contract once `check_and_record_spend` returns
  `Approved`.
- No multi-owner or organization-hierarchy support in v1 — one owner, a
  small fixed set of agents. Storage is laid out so this could extend
  later without a breaking migration, but it isn't built now.

## Build & test

Requires Rust 1.84+ with the `wasm32v1-none` target (Soroban's contracts
target a frozen WASM feature set — `reference-types`/`multi-value`, which
`wasm32-unknown-unknown` enables by default on Rust 1.82+, aren't yet
supported by the Soroban environment):

```bash
rustup target add wasm32v1-none
```

Run the unit tests (native target, fast):

```bash
cargo test
```

Build the deployable wasm:

```bash
# Soroban CLI (stellar-cli) v22+ — picks the right target automatically:
stellar contract build
# equivalent plain cargo invocation:
cargo build --target wasm32v1-none --release
```

The optimized artifact lands at
`target/wasm32v1-none/release/bridle_contract.wasm`.

## Deploy to Testnet

```bash
# One-time: configure the testnet network and an identity.
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
stellar keys generate owner --network testnet --fund
stellar keys generate agent --network testnet --fund

# Deploy.
stellar contract deploy \
  --wasm target/wasm32v1-none/release/bridle_contract.wasm \
  --source owner \
  --network testnet
# -> prints the deployed contract's C... address; use it as CONTRACT_ID below.
```

## Example CLI invocations

Substitute `CONTRACT_ID`, and the token's contract id (e.g. testnet
USDC), for real values. `--source` picks whose key signs the transaction;
`require_auth()` calls inside the contract require the *named parameter*
address to also have signed — the CLI prompts for/uses the matching
identity automatically when `--source` matches, or you pass `--source`
for each signer a call needs.

```bash
# Initialize: 1_000_0000000 stroops/day cap, 300_0000000 per call (7 decimals, SEP-41 default).
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  initialize --owner owner --agent agent --token $TOKEN_ID \
  --daily_cap 10000000000 --per_call_max 3000000000

# Approve a merchant.
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  add_allowlist_entry --destination $MERCHANT_ID --category compute

# Agent requests a spend (the transaction must be signed by `agent`).
stellar contract invoke --id $CONTRACT_ID --source agent --network testnet -- \
  check_and_record_spend --agent agent --destination $MERCHANT_ID \
  --token $TOKEN_ID --amount 500000000

# Read current policy / spend status (no signer needed).
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- get_policy
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- get_spend_status

# Emergency stop / resume.
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  set_kill_switch --active true
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  set_kill_switch --active false

# Update caps.
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  update_daily_cap --new_cap 20000000000
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  update_per_call_max --new_max 5000000000

# Two-step ownership transfer.
stellar contract invoke --id $CONTRACT_ID --source owner --network testnet -- \
  transfer_ownership --new_owner new_owner
stellar contract invoke --id $CONTRACT_ID --source new_owner --network testnet -- \
  accept_ownership
```

## Project layout

```
Cargo.toml
src/
  lib.rs      contract entry points + module-level design docs (read this first)
  storage.rs  ledger keys and the plain get/set accessors for each
  policy.rs   pure policy types and the calendar-day rollover helper
  errors.rs   hard-failure error codes (bad config/input, not policy declines)
  events.rs   every event this contract emits, one function per event
  test.rs     behavioral tests
```
