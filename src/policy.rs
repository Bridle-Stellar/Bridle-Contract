//! Pure policy types and helpers — no storage access, no auth, no events.
//!
//! Keeping this module free of side effects means the rules that decide
//! whether a spend is allowed can be read (and unit tested) in isolation
//! from *how* Bridle persists state or *what* it emits when a rule fires.

use soroban_sdk::{contracttype, Address};

use crate::storage::SpendState;

/// One calendar day, in seconds. Bridle's daily cap resets on UTC calendar
/// days rather than a rolling 24h window from each call — a calendar day
/// is what "daily spend cap" means to the owner setting the guardrail,
/// and it's trivial to reason about and audit ("the cap reset at
/// midnight UTC"), unlike a rolling window whose reset time depends on
/// call history.
pub const SECONDS_PER_DAY: u64 = 86_400;

/// Start-of-day (00:00:00 UTC) timestamp for the day containing `ts`.
pub fn day_start(ts: u64) -> u64 {
    (ts / SECONDS_PER_DAY) * SECONDS_PER_DAY
}

/// Returns the spend state to evaluate a request against *right now*,
/// without persisting anything. If the stored state belongs to an earlier
/// day, the correct current state is "zero spent so far today" — the
/// caller (either a read-only query or `check_and_record_spend`, which
/// persists the roll-over only when it actually records a spend) decides
/// whether that needs to be written back.
pub fn effective_spend_state(now: u64, stored: &SpendState) -> SpendState {
    let today = day_start(now);
    if stored.period_start == today {
        stored.clone()
    } else {
        SpendState {
            period_start: today,
            spent_today: 0,
        }
    }
}

/// Why `check_and_record_spend` declined a request. This is a normal,
/// expected outcome — not an error — so it's returned as data (and
/// mirrored into a `spend_rejected` event) rather than raised as a
/// contract error. See `crate::errors::Error` for the distinction.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RejectReason {
    /// The owner has flipped the kill switch on. Overrides every other
    /// check.
    KillSwitchActive,
    /// `agent` signed the request but isn't in the registered agent set.
    UnknownAgent,
    /// `amount` was zero or negative.
    InvalidAmount,
    /// `token` doesn't match the policy's governed token.
    WrongToken,
    /// `destination` is not on the allowlist.
    DestinationNotAllowed,
    /// `amount` exceeds the per-call maximum.
    ExceedsPerCallMax,
    /// `amount` would push today's cumulative spend over the daily cap.
    ExceedsDailyCap,
}

/// Details of a spend that passed every check.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendReceipt {
    pub spent_today: i128,
    pub remaining_today: i128,
}

/// Result of `check_and_record_spend`. Deliberately never a panic/contract
/// error for a policy decision — see the doc comment on
/// `check_and_record_spend` in `lib.rs` for why a declined spend still
/// needs the transaction to succeed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpendOutcome {
    Approved(SpendReceipt),
    Rejected(RejectReason),
}

/// Public snapshot of policy state, for dashboards and the relayer.
/// Combines the individually-stored pieces into one read so a caller
/// doesn't need four round trips to render "what are the guardrails right
/// now".
#[contracttype]
#[derive(Clone)]
pub struct PolicySnapshot {
    pub owner: Address,
    pub agents: soroban_sdk::Vec<Address>,
    pub token: Address,
    pub daily_cap: i128,
    pub per_call_max: i128,
    pub kill_switch: bool,
    pub allowlist: soroban_sdk::Vec<crate::storage::AllowlistEntry>,
}

/// Public snapshot of the current spend period, for dashboards.
#[contracttype]
#[derive(Clone)]
pub struct SpendStatus {
    pub period_start: u64,
    pub spent_today: i128,
    pub remaining_today: i128,
    pub daily_cap: i128,
}
