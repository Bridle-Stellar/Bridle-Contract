//! Storage layout for the Bridle policy contract.
//!
//! Every ledger key and its accessor lives here so the rest of the
//! contract reasons about "the owner" or "the policy" as plain values,
//! never as raw storage calls. This is also the one file that needs to
//! change if the storage layout is ever extended — see the comments
//! below for the extension points that were deliberately left in place
//! for that (multi-agent sets, category tags, future per-token policies).
//!
//! Everything here uses *instance* storage. Bridle's entire state — one
//! owner, a handful of agents, one policy, an allowlist of at most a few
//! dozen merchants, one running total — is small, bounded, and touched on
//! (almost) every call, which is exactly the profile instance storage is
//! for. There's no per-user or unboundedly-growing data that would call
//! for persistent/temporary storage tiers.

use soroban_sdk::{contracttype, panic_with_error, Address, Env, Symbol, Vec};

use crate::errors::Error;

#[contracttype]
pub enum DataKey {
    /// The human who controls policy. Set once in `initialize`, changed
    /// only through the two-step `transfer_ownership` / `accept_ownership`
    /// flow.
    Owner,
    /// An ownership transfer that has been proposed but not yet accepted.
    /// Absent when no transfer is pending.
    PendingOwner,
    /// Addresses allowed to *request* spends via `check_and_record_spend`.
    /// A `Vec<Address>` rather than a single `Address` because the spec
    /// for this contract allows "an agent address (or set of addresses)" —
    /// a small, owner-managed set covers that without needing a storage
    /// migration later. This is not multi-tenant/multi-org support (that's
    /// explicitly out of scope for v1) — every agent in the set shares the
    /// one policy below.
    Agents,
    /// The spend policy: governed token, daily cap, per-call max, kill
    /// switch.
    Policy,
    /// Approved destinations, each tagged with a category. Stored as a
    /// `Vec` (not a `Map`) so the full list can be handed to a dashboard
    /// in one read; the expected size — tens of merchants an agent is
    /// allowed to pay — makes whole-list add/remove cheap enough that a
    /// `Map`'s per-key storage overhead isn't worth it.
    Allowlist,
    /// Running total for the current calendar day, plus the timestamp of
    /// the day it belongs to.
    SpendState,
}

/// A destination the owner has approved, tagged with a category.
///
/// The `category` field is not enforced by any policy yet — v1 only
/// checks *membership* in the allowlist. It's captured on every entry now
/// so a future category-based rule (e.g. "cap spend to `data` vendors at
/// X/day separately from `compute`") can be added by reading data that
/// already exists, instead of requiring every existing allowlist entry to
/// be re-tagged in a migration. `general` is the default category for
/// callers that don't care about tagging yet.
#[contracttype]
#[derive(Clone)]
pub struct AllowlistEntry {
    pub destination: Address,
    pub category: Symbol,
}

/// Spend caps and the kill switch. One `Policy` governs one token — see
/// the module doc on `DataKey::Policy` for why, and the README for how a
/// per-token map could replace this without breaking the single-token
/// case.
#[contracttype]
#[derive(Clone)]
pub struct Policy {
    /// The SEP-41 token contract this policy's caps are denominated in.
    /// `check_and_record_spend` rejects any call naming a different
    /// token — see `RejectReason::WrongToken`.
    pub token: Address,
    /// Maximum total `token` spend allowed within one calendar day.
    pub daily_cap: i128,
    /// Maximum amount allowed in a single `check_and_record_spend` call,
    /// independent of how much daily budget remains.
    pub per_call_max: i128,
    /// When `true`, `check_and_record_spend` rejects every request
    /// regardless of any other policy field. The owner's emergency stop.
    pub kill_switch: bool,
}

/// Running spend total for "today", where "today" is a UTC calendar day
/// (`unix_timestamp / 86400`). Rolling over to a new day is handled lazily
/// — see `crate::policy::effective_spend_state` — so this struct can
/// legitimately describe a day that has already ended; nothing reads it
/// without first checking `period_start` against the current day.
#[contracttype]
#[derive(Clone)]
pub struct SpendState {
    /// Unix timestamp (seconds) of 00:00:00 UTC on the day `spent_today`
    /// was accumulated for.
    pub period_start: u64,
    /// Total spent so far within `period_start`'s calendar day.
    pub spent_today: i128,
}

pub fn require_initialized(env: &Env) {
    if !env.storage().instance().has(&DataKey::Owner) {
        panic_with_not_initialized(env);
    }
}

fn panic_with_not_initialized(env: &Env) -> ! {
    panic_with_error!(env, Error::NotInitialized)
}

pub fn get_owner(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Owner)
        .unwrap_or_else(|| panic_with_not_initialized(env))
}

pub fn set_owner(env: &Env, owner: &Address) {
    env.storage().instance().set(&DataKey::Owner, owner);
}

pub fn get_pending_owner(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingOwner)
}

pub fn set_pending_owner(env: &Env, pending: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::PendingOwner, pending);
}

pub fn clear_pending_owner(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingOwner);
}

pub fn get_agents(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Agents)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_agents(env: &Env, agents: &Vec<Address>) {
    env.storage().instance().set(&DataKey::Agents, agents);
}

pub fn get_policy(env: &Env) -> Policy {
    env.storage()
        .instance()
        .get(&DataKey::Policy)
        .unwrap_or_else(|| panic_with_not_initialized(env))
}

pub fn set_policy(env: &Env, policy: &Policy) {
    env.storage().instance().set(&DataKey::Policy, policy);
}

pub fn get_allowlist(env: &Env) -> Vec<AllowlistEntry> {
    env.storage()
        .instance()
        .get(&DataKey::Allowlist)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_allowlist(env: &Env, allowlist: &Vec<AllowlistEntry>) {
    env.storage().instance().set(&DataKey::Allowlist, allowlist);
}

pub fn get_spend_state(env: &Env) -> SpendState {
    env.storage()
        .instance()
        .get(&DataKey::SpendState)
        .unwrap_or(SpendState {
            period_start: 0,
            spent_today: 0,
        })
}

pub fn set_spend_state(env: &Env, state: &SpendState) {
    env.storage().instance().set(&DataKey::SpendState, state);
}
