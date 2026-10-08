//! Event definitions.
//!
//! Everything the off-chain relayer and dashboard need to reconstruct
//! "what happened and why" — without re-querying contract state — is
//! defined here. Every event is published from exactly one place in
//! `lib.rs`; keeping the definitions together makes it easy to audit the
//! complete list of things this contract reports and to keep topic/data
//! shapes consistent as the contract grows.
//!
//! Each event uses `#[contractevent]`, which derives its primary topic
//! from the struct name (`SpendApproved` -> topic `"spend_approved"`).
//! Fields marked `#[topic]` are added as further topics so an indexer can
//! filter by them directly (e.g. "every event about this agent") instead
//! of decoding every event's data section; unmarked fields carry the
//! rest of the detail.

use soroban_sdk::{contractevent, Address};

use crate::policy::{RejectReason, SpendReceipt};

/// A spend was approved and recorded against today's running total.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendApproved {
    #[topic]
    pub agent: Address,
    #[topic]
    pub destination: Address,
    pub token: Address,
    pub amount: i128,
    pub spent_today: i128,
    pub remaining_today: i128,
}

/// A spend request was declined. `reason` names exactly which check
/// failed — see [`RejectReason`] — so the dashboard can surface "why"
/// without inferring it from policy state that may have since changed.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendRejected {
    #[topic]
    pub agent: Address,
    #[topic]
    pub destination: Address,
    pub token: Address,
    pub amount: i128,
    pub reason: RejectReason,
}

/// The owner changed the daily spend cap.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyCapUpdated {
    pub new_cap: i128,
}

/// The owner changed the per-call maximum.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerCallMaxUpdated {
    pub new_max: i128,
}

/// The owner flipped the kill switch.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KillSwitchToggled {
    pub active: bool,
}

/// The owner approved a new destination.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowlistEntryAdded {
    #[topic]
    pub destination: Address,
    pub category: soroban_sdk::Symbol,
}

/// The owner revoked a destination's approval.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllowlistEntryRemoved {
    #[topic]
    pub destination: Address,
}

/// The owner registered an additional agent address.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAdded {
    #[topic]
    pub agent: Address,
}

/// The owner deregistered an agent address.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRemoved {
    #[topic]
    pub agent: Address,
}

/// The owner proposed transferring ownership. Not yet in effect — see
/// [`OwnershipTransferred`], which fires when the pending owner accepts.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipTransferProposed {
    #[topic]
    pub current_owner: Address,
    pub pending_owner: Address,
}

/// A proposed ownership transfer was accepted and is now in effect.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipTransferred {
    #[topic]
    pub previous_owner: Address,
    pub new_owner: Address,
}

pub fn spend_approved(
    env: &soroban_sdk::Env,
    agent: &Address,
    destination: &Address,
    token: &Address,
    amount: i128,
    receipt: &SpendReceipt,
) {
    SpendApproved {
        agent: agent.clone(),
        destination: destination.clone(),
        token: token.clone(),
        amount,
        spent_today: receipt.spent_today,
        remaining_today: receipt.remaining_today,
    }
    .publish(env);
}

pub fn spend_rejected(
    env: &soroban_sdk::Env,
    agent: &Address,
    destination: &Address,
    token: &Address,
    amount: i128,
    reason: RejectReason,
) {
    SpendRejected {
        agent: agent.clone(),
        destination: destination.clone(),
        token: token.clone(),
        amount,
        reason,
    }
    .publish(env);
}

pub fn daily_cap_updated(env: &soroban_sdk::Env, new_cap: i128) {
    DailyCapUpdated { new_cap }.publish(env);
}

pub fn per_call_max_updated(env: &soroban_sdk::Env, new_max: i128) {
    PerCallMaxUpdated { new_max }.publish(env);
}

pub fn kill_switch_toggled(env: &soroban_sdk::Env, active: bool) {
    KillSwitchToggled { active }.publish(env);
}

pub fn allowlist_entry_added(
    env: &soroban_sdk::Env,
    destination: &Address,
    category: &soroban_sdk::Symbol,
) {
    AllowlistEntryAdded {
        destination: destination.clone(),
        category: category.clone(),
    }
    .publish(env);
}

pub fn allowlist_entry_removed(env: &soroban_sdk::Env, destination: &Address) {
    AllowlistEntryRemoved {
        destination: destination.clone(),
    }
    .publish(env);
}

pub fn agent_added(env: &soroban_sdk::Env, agent: &Address) {
    AgentAdded {
        agent: agent.clone(),
    }
    .publish(env);
}

pub fn agent_removed(env: &soroban_sdk::Env, agent: &Address) {
    AgentRemoved {
        agent: agent.clone(),
    }
    .publish(env);
}

pub fn ownership_transfer_proposed(
    env: &soroban_sdk::Env,
    current_owner: &Address,
    pending_owner: &Address,
) {
    OwnershipTransferProposed {
        current_owner: current_owner.clone(),
        pending_owner: pending_owner.clone(),
    }
    .publish(env);
}

pub fn ownership_transferred(
    env: &soroban_sdk::Env,
    previous_owner: &Address,
    new_owner: &Address,
) {
    OwnershipTransferred {
        previous_owner: previous_owner.clone(),
        new_owner: new_owner.clone(),
    }
    .publish(env);
}
