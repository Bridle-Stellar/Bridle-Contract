# Deployments

Every deployment of this contract that anyone has exercised on-chain is
recorded here, with the real transaction hashes so each step can be checked
on a block explorer.

## Testnet — 2026-10-08

| | |
|---|---|
| Network | Stellar testnet (`Test SDF Network ; September 2015`) |
| RPC | `https://soroban-testnet.stellar.org` (protocol 29) |
| Contract ID | [`CAICNEKY4YT7M2K56RTI47WBFAUE2KZRKHZIYPFPQWGB23DDERZJVUK5`](https://stellar.expert/explorer/testnet/contract/CAICNEKY4YT7M2K56RTI47WBFAUE2KZRKHZIYPFPQWGB23DDERZJVUK5) |
| Wasm hash | `90650cce54be8dd8e969d414b04eb2b08cccf08708f7cc1e25fbdd6c116c7ee1` |
| Built from | `main` at `5df6a36` with Rust 1.99.0, `cargo build --target wasm32v1-none --release` |
| Tooling | stellar-cli 28.1.0 |
| Deployed | 2026-10-08 15:02 UTC (ledger 5089631) |

> Testnet is reset periodically by SDF. If the contract ID above no longer
> resolves, testnet has been wiped since this deployment; the transaction
> hashes below stay valid as a historical record on explorers that keep
> archived data. Re-run the steps below to get a fresh deployment and add
> a new section above this one.

### Accounts used

All three are throwaway testnet identities funded by Friendbot. Their secret
keys exist only in the deployer's local `stellar keys` store and are not in
this repository.

| Role | Address |
|---|---|
| Owner | `GD4IDHGFDQAOSFCXLXAK5FDNI5GHKLDZJEEC6Z7O5J5IMP3G5ATJ6OX4` |
| Agent | `GCPV6J54IAAW7YKYF23KOEVGIHXEU5UH4LRCB3KTPECGXLPCDWGXLL5M` |
| Merchant (allowlisted destination) | `GD57XJTA237YNXWO6N3PAJKLZPIDKV77OJK7OCCFHVOZAOC6WQJ5OMDP` |
| Governed token (native XLM SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Policy: daily cap `10000000000` (1,000 XLM), per-call max `3000000000`
(300 XLM). Amounts are in stroops (7 decimals).

### What was run, and what happened

| # | Step | Result | Ledger | Transaction |
|---|---|---|---|---|
| 1 | Upload wasm | ok | 5089629 | [`1d3be34b…3351`](https://stellar.expert/explorer/testnet/tx/1d3be34b8bdefe9f1bcfb0f1fd8edc8ba74714d9550811aefe585a8b71be3351) |
| 2 | Create contract | ok | 5089631 | [`f0f09b8b…e96a`](https://stellar.expert/explorer/testnet/tx/f0f09b8b7f834dcc6101c9face257be2d27dc349d3a614b361c5fd09e279e96a) |
| 3 | `initialize` | ok | 5089634 | [`c1fae4dd…3edb`](https://stellar.expert/explorer/testnet/tx/c1fae4ddb3de305de596d0682c39b813c636669ab7d58bf16f073d9fef1c3edb) |
| 4 | `add_allowlist_entry` (merchant, `compute`) | `allowlist_entry_added` | 5089635 | [`5ff0b8c3…eb45`](https://stellar.expert/explorer/testnet/tx/5ff0b8c3c9e547c489b0b5fdf3c79078e9bfc289bef623c39f17d84b7ed4eb45) |
| 5 | `check_and_record_spend` 50 XLM | `Approved`, spent 50 / remaining 950 | 5089637 | [`d1b87880…f04d`](https://stellar.expert/explorer/testnet/tx/d1b87880c1f4998c82ce9616442d874261a1ea769bca33603adb43893e2df04d) |
| 6 | `check_and_record_spend` 400 XLM | `Rejected(ExceedsPerCallMax)` | 5089639 | [`80a7da49…8cc0`](https://stellar.expert/explorer/testnet/tx/80a7da492f7099c91238f5335f1ef7d323d39614e77d7c664d67a4e41e548cc0) |
| 7 | `set_kill_switch true` | `kill_switch_toggled` | 5089641 | [`81ce1572…4f56`](https://stellar.expert/explorer/testnet/tx/81ce1572df7312f97e96baa8f44540dcfa8727506c990423a09524c2af8c4f56) |
| 8 | `check_and_record_spend` 50 XLM (kill switch on) | `Rejected(KillSwitchActive)` | 5089643 | [`028cbb60…7e10`](https://stellar.expert/explorer/testnet/tx/028cbb609e5d5828558684ffba3974ea98b184f8311955556994a214f99b7e10) |
| 9 | `set_kill_switch false` | `kill_switch_toggled` | 5089644 | [`0f0e9513…08da`](https://stellar.expert/explorer/testnet/tx/0f0e9513297508d52e9a86d1b14375d75602c7d7260b6d6187bbae24c41008da) |

Steps 6 and 8 are rejections, and both transactions still **succeeded**.
That is by design: a policy rejection returns `SpendOutcome::Rejected` and
emits `spend_rejected` rather than reverting (see the README's "Why a
rejected spend doesn't revert the transaction"). Only the spend in step 5
moved the daily counter; the final `get_spend_status` read shows
`spent_today = 500000000`, i.e. the two rejected requests recorded nothing.

Full transaction hashes:

```text
upload wasm            1d3be34b8bdefe9f1bcfb0f1fd8edc8ba74714d9550811aefe585a8b71be3351
create contract        f0f09b8b7f834dcc6101c9face257be2d27dc349d3a614b361c5fd09e279e96a
initialize             c1fae4ddb3de305de596d0682c39b813c636669ab7d58bf16f073d9fef1c3edb
add_allowlist_entry    5ff0b8c3c9e547c489b0b5fdf3c79078e9bfc289bef623c39f17d84b7ed4eb45
spend approved         d1b87880c1f4998c82ce9616442d874261a1ea769bca33603adb43893e2df04d
spend over per-call    80a7da492f7099c91238f5335f1ef7d323d39614e77d7c664d67a4e41e548cc0
kill switch on         81ce1572df7312f97e96baa8f44540dcfa8727506c990423a09524c2af8c4f56
spend while killed     028cbb609e5d5828558684ffba3974ea98b184f8311955556994a214f99b7e10
kill switch off        0f0e9513297508d52e9a86d1b14375d75602c7d7260b6d6187bbae24c41008da
```

### Commands

These are the commands as run, in order. `bridle-owner`, `bridle-agent` and
`bridle-merchant` are local `stellar keys` aliases.

```bash
rustup target add wasm32v1-none
cargo build --target wasm32v1-none --release

stellar keys generate bridle-owner    --network testnet --fund
stellar keys generate bridle-agent    --network testnet --fund
stellar keys generate bridle-merchant --network testnet --fund

TOKEN_ID=$(stellar contract id asset --asset native --network testnet)
MERCHANT_ID=$(stellar keys address bridle-merchant)

stellar contract deploy \
  --wasm target/wasm32v1-none/release/bridle_contract.wasm \
  --source bridle-owner --network testnet --alias bridle
CONTRACT_ID=CAICNEKY4YT7M2K56RTI47WBFAUE2KZRKHZIYPFPQWGB23DDERZJVUK5

stellar contract invoke --id $CONTRACT_ID --source bridle-owner --network testnet -- \
  initialize --owner bridle-owner --agent bridle-agent --token $TOKEN_ID \
  --daily_cap 10000000000 --per_call_max 3000000000

stellar contract invoke --id $CONTRACT_ID --source bridle-owner --network testnet -- \
  add_allowlist_entry --destination $MERCHANT_ID --category compute

stellar contract invoke --id $CONTRACT_ID --source bridle-agent --network testnet -- \
  check_and_record_spend --agent bridle-agent --destination $MERCHANT_ID \
  --token $TOKEN_ID --amount 500000000
# {"Approved":{"remaining_today":"9500000000","spent_today":"500000000"}}

stellar contract invoke --id $CONTRACT_ID --source bridle-agent --network testnet -- \
  check_and_record_spend --agent bridle-agent --destination $MERCHANT_ID \
  --token $TOKEN_ID --amount 4000000000
# {"Rejected":"ExceedsPerCallMax"}

stellar contract invoke --id $CONTRACT_ID --source bridle-owner --network testnet -- \
  set_kill_switch --active true

stellar contract invoke --id $CONTRACT_ID --source bridle-agent --network testnet -- \
  check_and_record_spend --agent bridle-agent --destination $MERCHANT_ID \
  --token $TOKEN_ID --amount 500000000
# {"Rejected":"KillSwitchActive"}

stellar contract invoke --id $CONTRACT_ID --source bridle-owner --network testnet -- \
  set_kill_switch --active false

stellar contract invoke --id $CONTRACT_ID --source bridle-owner --network testnet -- get_spend_status
# {"daily_cap":"10000000000","period_start":1791417600,"remaining_today":"9500000000","spent_today":"500000000"}
```

To see the emitted events:

```bash
stellar events --network testnet --id $CONTRACT_ID --start-ledger 5089629 --output json
```

## Mainnet

Not deployed. There is no mainnet deployment and none is planned until the
contract has had an independent security review.
