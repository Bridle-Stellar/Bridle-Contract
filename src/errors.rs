//! Hard-failure error codes.
//!
//! These are for calls that should never be allowed to complete at all —
//! bad configuration, malformed input, or asking for something that
//! doesn't exist. They panic the transaction (nothing they touch should
//! ever be recorded as having happened).
//!
//! This is deliberately a *different* type from [`crate::policy::RejectReason`].
//! A `RejectReason` describes an agent's spend request that was correctly
//! evaluated and declined — that's a normal, expected outcome the relayer
//! and dashboard need to see as an event. An `Error` here means the caller
//! (always the owner, for every function that can raise one) asked the
//! contract to do something nonsensical, and the transaction should fail
//! outright with no event to index.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `initialize` was called on a contract that already has an owner.
    AlreadyInitialized = 1,
    /// A read or write happened before `initialize` ran.
    NotInitialized = 2,
    /// A cap, max, or amount argument was zero or negative.
    NonPositiveAmount = 3,
    /// `add_allowlist_entry` was called with a destination already present.
    DestinationAlreadyAllowed = 4,
    /// `remove_allowlist_entry` was called with a destination that isn't there.
    DestinationNotFound = 5,
    /// `add_agent` was called with an address already registered.
    AgentAlreadyRegistered = 6,
    /// `remove_agent` targeted an address that isn't registered.
    AgentNotFound = 7,
    /// `remove_agent` would leave the agent set empty. Bridle always needs
    /// at least one agent registered, or `check_and_record_spend` could
    /// never succeed and the contract would be permanently useless.
    CannotRemoveLastAgent = 8,
    /// `accept_ownership` was called but no transfer is pending.
    NoPendingOwner = 9,
}
