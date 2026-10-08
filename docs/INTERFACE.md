# On-chain interface reference

This is the exact wire format of everything the contract returns and emits,
for anyone decoding it off-chain (Bridle Backend, indexers, dashboards).
It's taken from two sources, not written from memory:

1. **The contract spec embedded in the built wasm**, which is what every
   Soroban SDK and the CLI use to encode and decode calls:

   ```bash
   cargo build --target wasm32v1-none --release
   stellar contract info interface \
     --wasm target/wasm32v1-none/release/bridle_contract.wasm --output json-formatted
   ```

2. **Real values read back from the testnet deployment** in
   [DEPLOYMENTS.md](DEPLOYMENTS.md) (contract
   `CAICNEKY4YT7M2K56RTI47WBFAUE2KZRKHZIYPFPQWGB23DDERZJVUK5`), via Soroban
   RPC `getTransaction`, `simulateTransaction` and `getEvents` with
   `"xdrFormat": "json"`. The JSON below is the stellar-xdr JSON form of
   each `ScVal`.

If you change a `#[contracttype]`, `#[contractevent]` or a function
signature, regenerate the spec and update this file in the same PR.

## Encoding rules

These are how soroban-sdk maps the Rust types in this contract onto
`ScVal`. Every example below follows them.

| Rust shape | ScVal |
|---|---|
| `struct` with named fields (`#[contracttype]`) | `ScMap` with one entry per field. Keys are `ScSymbol` field names, **sorted by key** (not in declaration order). |
| `enum` unit variant, e.g. `RejectReason::WrongToken` | `ScVec` holding one element, `[Symbol("WrongToken")]` |
| `enum` tuple variant, e.g. `SpendOutcome::Approved(r)` | `ScVec` `[Symbol("Approved"), <r encoded>]` |
| `i128` | `ScVal::I128`. The CLI and stellar-xdr JSON print it as a decimal **string**. |
| `u64` | `ScVal::U64`. stellar-xdr JSON prints a string; the CLI's decoded output prints a number. |
| `Address` | `ScVal::Address`. `G…` for accounts, `C…` for contracts. |
| `Symbol` | `ScVal::Symbol` |
| `Vec<T>` | `ScVec` of `T` |

## Functions

| Function | Arguments | Returns |
|---|---|---|
| `initialize` | `owner: Address, agent: Address, token: Address, daily_cap: i128, per_call_max: i128` | `()` |
| `check_and_record_spend` | `agent: Address, destination: Address, token: Address, amount: i128` | `SpendOutcome` |
| `get_policy` | none | `PolicySnapshot` |
| `get_spend_status` | none | `SpendStatus` |
| `update_daily_cap` | `new_cap: i128` | `()` |
| `update_per_call_max` | `new_max: i128` | `()` |
| `add_allowlist_entry` | `destination: Address, category: Symbol` | `()` |
| `remove_allowlist_entry` | `destination: Address` | `()` |
| `add_agent` | `agent: Address` | `()` |
| `remove_agent` | `agent: Address` | `()` |
| `set_kill_switch` | `active: bool` | `()` |
| `transfer_ownership` | `new_owner: Address` | `()` |
| `accept_ownership` | none | `()` |

`()` is `ScVal::Void` on the wire.

## Return types

### `SpendOutcome` (from `check_and_record_spend`)

A two-variant union. Spec:

```text
union SpendOutcome {
  Approved(SpendReceipt)
  Rejected(RejectReason)
}
struct SpendReceipt { remaining_today: i128, spent_today: i128 }
```

**Approved.** From testnet tx
[`d1b87880…f04d`](https://stellar.expert/explorer/testnet/tx/d1b87880c1f4998c82ce9616442d874261a1ea769bca33603adb43893e2df04d):

CLI output:

```json
{"Approved":{"remaining_today":"9500000000","spent_today":"500000000"}}
```

ScVal:

```json
{"vec": [
  {"symbol": "Approved"},
  {"map": [
    {"key": {"symbol": "remaining_today"}, "val": {"i128": "9500000000"}},
    {"key": {"symbol": "spent_today"},     "val": {"i128": "500000000"}}
  ]}
]}
```

**Rejected.** From testnet tx
[`80a7da49…8cc0`](https://stellar.expert/explorer/testnet/tx/80a7da492f7099c91238f5335f1ef7d323d39614e77d7c664d67a4e41e548cc0).
The reason is itself a unit-variant enum, so it's a one-element vec
nested inside the outer vec:

CLI output:

```json
{"Rejected":"ExceedsPerCallMax"}
```

ScVal:

```json
{"vec": [
  {"symbol": "Rejected"},
  {"vec": [{"symbol": "ExceedsPerCallMax"}]}
]}
```

### `RejectReason`

Unit-variant union, encoded as `{"vec": [{"symbol": "<Variant>"}]}`. Listed
in spec order. The order `check_and_record_spend` *evaluates* them in is
different; see the doc comment on that function in `src/lib.rs`.

| Variant | Meaning |
|---|---|
| `KillSwitchActive` | The kill switch is on. |
| `UnknownAgent` | `agent` signed, but is not in the registered agent set. |
| `InvalidAmount` | `amount <= 0`. |
| `WrongToken` | `token` is not the policy's governed token. |
| `DestinationNotAllowed` | `destination` is not on the allowlist. |
| `ExceedsPerCallMax` | `amount > per_call_max`. |
| `ExceedsDailyCap` | `spent_today + amount > daily_cap`. |

### `PolicySnapshot` (from `get_policy`)

```text
struct PolicySnapshot {
  agents:       Vec<Address>
  allowlist:    Vec<AllowlistEntry>
  daily_cap:    i128
  kill_switch:  bool
  owner:        Address
  per_call_max: i128
  token:        Address
}
struct AllowlistEntry { category: Symbol, destination: Address }
```

Read from the testnet contract with `simulateTransaction` after the
deployment run:

CLI output:

```json
{"agents":["GCPV6J54IAAW7YKYF23KOEVGIHXEU5UH4LRCB3KTPECGXLPCDWGXLL5M"],"allowlist":[{"category":"compute","destination":"GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP"}],"daily_cap":"10000000000","kill_switch":false,"owner":"GD4IDHGFDQAOSFCXLXAK5FDNI5GHKLDZJEEC6Z7O5J5IMP3G5ATJ6OX4","per_call_max":"3000000000","token":"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"}
```

ScVal:

```json
{"map": [
  {"key": {"symbol": "agents"}, "val": {"vec": [
    {"address": "GCPV6J54IAAW7YKYF23KOEVGIHXEU5UH4LRCB3KTPECGXLPCDWGXLL5M"}
  ]}},
  {"key": {"symbol": "allowlist"}, "val": {"vec": [
    {"map": [
      {"key": {"symbol": "category"},    "val": {"symbol": "compute"}},
      {"key": {"symbol": "destination"}, "val": {"address": "GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP"}}
    ]}
  ]}},
  {"key": {"symbol": "daily_cap"},    "val": {"i128": "10000000000"}},
  {"key": {"symbol": "kill_switch"},  "val": {"bool": false}},
  {"key": {"symbol": "owner"},        "val": {"address": "GD4IDHGFDQAOSFCXLXAK5FDNI5GHKLDZJEEC6Z7O5J5IMP3G5ATJ6OX4"}},
  {"key": {"symbol": "per_call_max"}, "val": {"i128": "3000000000"}},
  {"key": {"symbol": "token"},        "val": {"address": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"}}
]}
```

`PolicySnapshot` does **not** include a pending ownership transfer. There
is currently no read function for it; watch for the
`ownership_transfer_proposed` event instead.

### `SpendStatus` (from `get_spend_status`)

```text
struct SpendStatus {
  daily_cap:       i128
  period_start:    u64    // 00:00:00 UTC of the current day, unix seconds
  remaining_today: i128
  spent_today:     i128
}
```

CLI output:

```json
{"daily_cap":"10000000000","period_start":1791417600,"remaining_today":"9500000000","spent_today":"500000000"}
```

ScVal:

```json
{"map": [
  {"key": {"symbol": "daily_cap"},       "val": {"i128": "10000000000"}},
  {"key": {"symbol": "period_start"},    "val": {"u64": "1791417600"}},
  {"key": {"symbol": "remaining_today"}, "val": {"i128": "9500000000"}},
  {"key": {"symbol": "spent_today"},     "val": {"i128": "500000000"}}
]}
```

`remaining_today` can be negative if the owner lowers `daily_cap` below
what has already been spent today. Decode it as a signed value.

## Errors

Hard failures (the transaction fails, nothing is recorded, no event). They
surface as `Error(Contract, #N)`.

| Code | Name | Raised by |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` |
| 2 | `NotInitialized` | any call before `initialize` |
| 3 | `NonPositiveAmount` | `initialize`, `update_daily_cap`, `update_per_call_max` |
| 4 | `DestinationAlreadyAllowed` | `add_allowlist_entry` |
| 5 | `DestinationNotFound` | `remove_allowlist_entry` |
| 6 | `AgentAlreadyRegistered` | `add_agent` |
| 7 | `AgentNotFound` | `remove_agent` |
| 8 | `CannotRemoveLastAgent` | `remove_agent` |
| 9 | `NoPendingOwner` | `accept_ownership` |

A missing `require_auth` signature is also a hard failure, but it's raised
by the Soroban host's auth framework (an `InvalidAction` host error), not
by one of the contract codes above. Don't match on it by contract code.

## Events

Every event is a `#[contractevent]` with the spec's `data_format: map`:

- **topics** = `[Symbol(<event name>), <each #[topic] field, in declaration order>]`
- **data** = `ScMap` of every non-topic field, keyed by `Symbol(field name)`
  and **sorted by key**

| Event (topic 0) | Further topics, in order | Data map keys (sorted) |
|---|---|---|
| `spend_approved` | `agent: Address`, `destination: Address` | `amount: i128`, `remaining_today: i128`, `spent_today: i128`, `token: Address` |
| `spend_rejected` | `agent: Address`, `destination: Address` | `amount: i128`, `reason: RejectReason`, `token: Address` |
| `daily_cap_updated` | none | `new_cap: i128` |
| `per_call_max_updated` | none | `new_max: i128` |
| `kill_switch_toggled` | none | `active: bool` |
| `allowlist_entry_added` | `destination: Address` | `category: Symbol` |
| `allowlist_entry_removed` | `destination: Address` | none (empty map) |
| `agent_added` | `agent: Address` | none (empty map) |
| `agent_removed` | `agent: Address` | none (empty map) |
| `ownership_transfer_proposed` | `current_owner: Address` | `pending_owner: Address` |
| `ownership_transferred` | `previous_owner: Address` | `new_owner: Address` |

Events with no data fields still carry a data value: an empty `ScMap`,
not `Void` (checked against the soroban-sdk 27 test host for
`agent_added`).

### Event examples from testnet

All from the deployment in [DEPLOYMENTS.md](DEPLOYMENTS.md), fetched with
`getEvents`.

**`spend_approved`** (ledger 5089637):

```json
"topic": [
  {"symbol": "spend_approved"},
  {"address": "GCPV6J54IAAW7YKYF23KOEVGIHXEU5UH4LRCB3KTPECGXLPCDWGXLL5M"},
  {"address": "GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP"}
],
"value": {"map": [
  {"key": {"symbol": "amount"},          "val": {"i128": "500000000"}},
  {"key": {"symbol": "remaining_today"}, "val": {"i128": "9500000000"}},
  {"key": {"symbol": "spent_today"},     "val": {"i128": "500000000"}},
  {"key": {"symbol": "token"},           "val": {"address": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"}}
]}
```

The same event as base64 XDR, which is what `getEvents` returns when you
don't ask for JSON:

```text
topic[0] AAAADwAAAA5zcGVuZF9hcHByb3ZlZAAA
topic[1] AAAAEgAAAAAAAAAAn18nvEABb+FYLranEqZB7kp2h+LiIO1TeQRrreIdjXU=
topic[2] AAAAEgAAAAAAAAAA+/umYNb/ht7O83bwJUvL0DVX/3JV9whFPV2QOF60E9c=
value    AAAAEQAAAAEAAAAEAAAADwAAAAZhbW91bnQAAAAAAAoAAAAAAAAAAAAAAAAdzWUAAAAADwAAAA9yZW1haW5pbmdfdG9kYXkAAAAACgAAAAAAAAAAAAAAAjY+fwAAAAAPAAAAC3NwZW50X3RvZGF5AAAAAAoAAAAAAAAAAAAAAAAdzWUAAAAADwAAAAV0b2tlbgAAAAAAABIAAAAB15KLcsJwPM/q9+uf9O9NUEpVqLl5/JtFDqLIQrTRzmE=
```

**`spend_rejected`** (ledger 5089639). Note that `reason` is the enum
encoding, a one-element vec, not a bare symbol:

```json
"topic": [
  {"symbol": "spend_rejected"},
  {"address": "GCPV6J54IAAW7YKYF23KOEVGIHXEU5UH4LRCB3KTPECGXLPCDWGXLL5M"},
  {"address": "GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP"}
],
"value": {"map": [
  {"key": {"symbol": "amount"}, "val": {"i128": "4000000000"}},
  {"key": {"symbol": "reason"}, "val": {"vec": [{"symbol": "ExceedsPerCallMax"}]}},
  {"key": {"symbol": "token"},  "val": {"address": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"}}
]}
```

**`kill_switch_toggled`** (ledger 5089641):

```json
"topic": [{"symbol": "kill_switch_toggled"}],
"value": {"map": [{"key": {"symbol": "active"}, "val": {"bool": true}}]}
```

**`allowlist_entry_added`** (ledger 5089635):

```json
"topic": [
  {"symbol": "allowlist_entry_added"},
  {"address": "GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP"}
],
"value": {"map": [{"key": {"symbol": "category"}, "val": {"symbol": "compute"}}]}
```

The remaining events (`daily_cap_updated`, `per_call_max_updated`,
`allowlist_entry_removed`, `agent_added`, `agent_removed`,
`ownership_transfer_proposed`, `ownership_transferred`) haven't been emitted
on testnet yet. Their shapes above come from the spec. A worked example of
each is tracked as a separate docs issue.

### Filtering

To get every spend decision for one agent without decoding data payloads,
filter on topics `["spend_approved" | "spend_rejected", <agent>, *]`. With
Soroban RPC `getEvents`, the topic filter for approved spends by agent
`G…` is:

```json
{"type": "contract",
 "contractIds": ["<CONTRACT_ID>"],
 "topics": [["AAAADwAAAA5zcGVuZF9hcHByb3ZlZAAA", "<agent address ScVal, base64>", "*"]]}
```
