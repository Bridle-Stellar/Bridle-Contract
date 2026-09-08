//! # Bridle Contract
//!
//! On-chain policy-enforcement layer for Bridle. Bridle lets a human
//! ("the owner") set spending guardrails for an autonomous AI agent's
//! Stellar wallet; this contract is the source of truth for those
//! guardrails. A separate off-chain relayer (not part of this repo) calls
//! [`BridleContract::check_and_record_spend`] before forwarding any agent
//! payment, and must not forward a payment this contract declined.
//!
//! Think of it as a programmable allowance: the owner sets a daily cap, a
//! per-call maximum, and a merchant allowlist once; the contract enforces
//! them on every subsequent spend attempt, with no code path that lets
//! the agent — or a compromised relayer — change the rules or spend past
//! them.
//!
//! ## Auth model (read this before integrating)
//!
//! There are two identities involved in a spend: the **relayer**, an
//! off-chain service that submits the Stellar transaction and pays its
//! fee, and the **agent**, the AI system whose spending is being guarded.
//! The question this contract has to answer is: whose authorization is
//! sufficient to approve a spend?
//!
//! **Decision: the agent's own key authorizes every spend, via Soroban's
//! native authorization framework — not the relayer's identity.**
//! `check_and_record_spend` calls `agent.require_auth()`. Soroban's auth
//! framework binds a signed authorization entry to the *exact* invocation
//! it was signed for — this contract, this function, these exact
//! arguments (destination, token, amount) — so a valid authorization here
//! is cryptographic proof that the agent itself requested precisely this
//! spend. The relayer submits the transaction and supplies that
//! authorization entry, but it cannot fabricate one, alter its arguments,
//! or reuse it for a different spend. **This means a compromised relayer
//! cannot approve a spend the agent never signed off on** — the whole
//! point of putting this guardrail on-chain instead of purely in the
//! relayer's backend.
//!
//! What the relayer's identity *is* trusted for: submitting the
//! transaction, and by implication which agent+destination+token+amount
//! it chooses to construct and ask the agent to sign in the first place.
//! It cannot bypass the owner's caps, allowlist, or kill switch, because
//! those are enforced by this contract regardless of who submits the
//! call. If the agent key itself is compromised, this contract's
//! guardrails are the last line of defense — which is exactly the
//! scenario the daily cap, per-call max, and allowlist exist for.
//!
//! We did not need to simplify this to a "v1 trusts the relayer" model —
//! requiring `agent.require_auth()` is the standard, idiomatic way to
//! authorize a Soroban contract call and costs nothing extra to implement
//! correctly, so there is no weaker fallback documented here.
//!
//! ## Why declined spends don't revert
//!
//! `check_and_record_spend` never panics or returns a contract error for
//! a policy decision (kill switch, allowlist, caps) — seeing "your spend
//! was rejected because X" is a normal, expected outcome that the
//! relayer and dashboard need recorded as history, not an exceptional
//! failure. If a policy rejection instead reverted the transaction,
//! Soroban would roll back the whole call — on most RPC configurations
//! that also discards the very events this function exists to produce,
//! leaving no on-chain trace of the attempt. So a declined request still
//! completes successfully and emits a `spend_rejected` event describing
//! why; the relayer reads the returned [`policy::SpendOutcome`] and must
//! not forward payment when it's `Rejected`.
//!
//! The one thing that *does* still hard-fail the transaction is
//! `agent.require_auth()` itself: an invocation lacking a valid signed
//! authorization from the named agent traps immediately. That's not a
//! policy decision to log, it's a missing signature — the same as any
//! other unauthorized Soroban call.
//!
//! ## Relationship to OpenZeppelin's `stellar-accounts`
//!
//! OpenZeppelin's Stellar contracts (`stellar-accounts`) ship a spending
//! limit *policy* module, and we evaluated it before writing this
//! contract, per the project's own guidance to build on solid primitives
//! rather than reimplement them. We chose not to depend on it:
//!
//! - It's a plug-in for OpenZeppelin's own smart-account /
//!   context-rule / verifier framework (multisig, WebAuthn, etc.) —
//!   adopting it means adopting that whole account architecture, not
//!   just a spend-limit check, for a contract that just needs a single
//!   owner and a small agent set.
//! - Its limit is a rolling ledger-window total, not a calendar-day cap,
//!   and it has no concept of a merchant allowlist, a per-call max, or a
//!   kill switch — the three guardrails this project is specifically
//!   about.
//!
//! What we *did* take from it: the pattern of tracking a running spend
//! total against a window and lazily rolling it over on read, rather than
//! needing a scheduled/cron-style reset. See [`policy::effective_spend_state`].
//! A future version could go the other direction and expose Bridle's
//! checks as a policy plugin *for* an OpenZeppelin smart account, if
//! multisig ownership becomes a goal — the storage layout here was kept
//! deliberately simple so that migration stays possible.

#![no_std]

mod errors;
mod events;
mod policy;
mod storage;

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env, Symbol, Vec};

use errors::Error;
use policy::{
    day_start, effective_spend_state, PolicySnapshot, RejectReason, SpendOutcome, SpendReceipt,
    SpendStatus,
};
use storage::{AllowlistEntry, Policy, SpendState};

#[contract]
pub struct BridleContract;

#[contractimpl]
impl BridleContract {
    /// One-time setup. Registers `owner`, one initial `agent`, and the
    /// starting policy (governed `token`, `daily_cap`, `per_call_max`).
    /// The kill switch starts off and the allowlist starts empty — the
    /// owner must explicitly approve destinations before the agent can
    /// spend anywhere.
    ///
    /// Requires `owner`'s authorization, so the address that ends up
    /// controlling policy is always the one that actually asked to
    /// initialize the contract, not whichever address happened to submit
    /// the transaction.
    ///
    /// # Panics
    /// - If the contract has already been initialized ([`Error::AlreadyInitialized`]).
    /// - If `daily_cap` or `per_call_max` is not positive ([`Error::NonPositiveAmount`]).
    pub fn initialize(
        env: Env,
        owner: Address,
        agent: Address,
        token: Address,
        daily_cap: i128,
        per_call_max: i128,
    ) {
        if env.storage().instance().has(&storage::DataKey::Owner) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }
        owner.require_auth();
        require_positive(&env, daily_cap);
        require_positive(&env, per_call_max);

        storage::set_owner(&env, &owner);
        let mut agents = Vec::new(&env);
        agents.push_back(agent);
        storage::set_agents(&env, &agents);
        storage::set_policy(
            &env,
            &Policy {
                token,
                daily_cap,
                per_call_max,
                kill_switch: false,
            },
        );
        storage::set_allowlist(&env, &Vec::new(&env));
        storage::set_spend_state(
            &env,
            &SpendState {
                period_start: day_start(env.ledger().timestamp()),
                spent_today: 0,
            },
        );
    }

    // ---------------------------------------------------------------
    // Core spend check — the only path that can move the spent-so-far
    // counter. Every owner-only function below changes *rules*; this is
    // the only function that changes *state derived from those rules*.
    // ---------------------------------------------------------------

    /// Evaluate a spend request from `agent` to `destination` for `amount`
    /// of `token`, and record it if approved. This is the only function
    /// in the contract that can advance the daily spend counter — there
    /// is deliberately no other way to touch it, so the counter can never
    /// drift from what was actually approved here.
    ///
    /// Requires `agent.require_auth()`: the transaction must carry a
    /// signed authorization from `agent` for this exact call (same
    /// contract, same function, same `destination`/`token`/`amount`) —
    /// see the module-level "Auth model" docs. This traps immediately if
    /// missing; it is not a policy outcome and is not reflected in the
    /// returned [`SpendOutcome`].
    ///
    /// Checks are evaluated in this order, and the first one that fails
    /// determines the rejection reason reported in the event and return
    /// value:
    /// 1. `amount` is positive ([`RejectReason::InvalidAmount`]).
    /// 2. `agent` is in the registered agent set ([`RejectReason::UnknownAgent`]).
    /// 3. The kill switch is off ([`RejectReason::KillSwitchActive`]).
    /// 4. `token` matches the policy's governed token ([`RejectReason::WrongToken`]).
    /// 5. `destination` is on the allowlist ([`RejectReason::DestinationNotAllowed`]).
    /// 6. `amount` does not exceed the per-call max ([`RejectReason::ExceedsPerCallMax`]).
    /// 7. `amount` does not push today's cumulative spend over the daily cap
    ///    ([`RejectReason::ExceedsDailyCap`]).
    ///
    /// A rejection at any step still completes the transaction
    /// successfully and emits a `spend_rejected` event — see the
    /// module-level docs on why. **The relayer must treat
    /// `SpendOutcome::Rejected` as a hard "do not forward this payment"
    /// signal**; this contract has no custody of funds and cannot stop
    /// the relayer from moving them anyway, it can only refuse to
    /// authorize.
    pub fn check_and_record_spend(
        env: Env,
        agent: Address,
        destination: Address,
        token: Address,
        amount: i128,
    ) -> SpendOutcome {
        storage::require_initialized(&env);
        agent.require_auth();

        let policy = storage::get_policy(&env);
        let now = env.ledger().timestamp();
        let state = effective_spend_state(now, &storage::get_spend_state(&env));

        let reason = if amount <= 0 {
            Some(RejectReason::InvalidAmount)
        } else if !storage::get_agents(&env).contains(&agent) {
            Some(RejectReason::UnknownAgent)
        } else if policy.kill_switch {
            Some(RejectReason::KillSwitchActive)
        } else if token != policy.token {
            Some(RejectReason::WrongToken)
        } else if !is_allowed(&env, &destination) {
            Some(RejectReason::DestinationNotAllowed)
        } else if amount > policy.per_call_max {
            Some(RejectReason::ExceedsPerCallMax)
        } else if state.spent_today + amount > policy.daily_cap {
            Some(RejectReason::ExceedsDailyCap)
        } else {
            None
        };

        match reason {
            Some(reason) => {
                events::spend_rejected(&env, &agent, &destination, &token, amount, reason);
                SpendOutcome::Rejected(reason)
            }
            None => {
                let spent_today = state.spent_today + amount;
                storage::set_spend_state(
                    &env,
                    &SpendState {
                        period_start: state.period_start,
                        spent_today,
                    },
                );
                let receipt = SpendReceipt {
                    spent_today,
                    remaining_today: policy.daily_cap - spent_today,
                };
                events::spend_approved(&env, &agent, &destination, &token, amount, &receipt);
                SpendOutcome::Approved(receipt)
            }
        }
    }

    // ---------------------------------------------------------------
    // Public reads — no auth required; a dashboard needs to display
    // these without holding any key.
    // ---------------------------------------------------------------

    /// Full policy snapshot: owner, registered agents, governed token,
    /// caps, kill-switch state, and the allowlist. One read for a
    /// dashboard to render the current guardrails.
    pub fn get_policy(env: Env) -> PolicySnapshot {
        storage::require_initialized(&env);
        let policy = storage::get_policy(&env);
        PolicySnapshot {
            owner: storage::get_owner(&env),
            agents: storage::get_agents(&env),
            token: policy.token,
            daily_cap: policy.daily_cap,
            per_call_max: policy.per_call_max,
            kill_switch: policy.kill_switch,
            allowlist: storage::get_allowlist(&env),
        }
    }

    /// Current calendar day's cumulative spend and remaining budget.
    /// Computed on the fly against the ledger's current timestamp — if
    /// the stored counter belongs to a previous day, this reports a
    /// fresh day (spent = 0) without needing anyone to have called
    /// `check_and_record_spend` yet today to "trigger" the reset.
    pub fn get_spend_status(env: Env) -> SpendStatus {
        storage::require_initialized(&env);
        let policy = storage::get_policy(&env);
        let now = env.ledger().timestamp();
        let state = effective_spend_state(now, &storage::get_spend_state(&env));
        SpendStatus {
            period_start: state.period_start,
            spent_today: state.spent_today,
            remaining_today: policy.daily_cap - state.spent_today,
            daily_cap: policy.daily_cap,
        }
    }

    // ---------------------------------------------------------------
    // Owner-only management. Every one of these calls `require_auth()`
    // on the address currently stored as owner — not on a parameter the
    // caller supplies — so possessing any other key (including an
    // agent's) can never satisfy it.
    // ---------------------------------------------------------------

    /// Set a new daily spend cap, in the policy's governed token's
    /// smallest unit. Takes effect immediately, including for spends
    /// already recorded today (if today's spend already exceeds the new,
    /// lower cap, further spends today are rejected until the next day
    /// rolls over — this does not retroactively fail anything already
    /// approved).
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `new_cap` is not positive ([`Error::NonPositiveAmount`]).
    pub fn update_daily_cap(env: Env, new_cap: i128) {
        let owner = storage::get_owner(&env);
        owner.require_auth();
        require_positive(&env, new_cap);

        let mut policy = storage::get_policy(&env);
        policy.daily_cap = new_cap;
        storage::set_policy(&env, &policy);
        events::daily_cap_updated(&env, new_cap);
    }

    /// Set a new per-call maximum, in the policy's governed token's
    /// smallest unit.
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `new_max` is not positive ([`Error::NonPositiveAmount`]).
    pub fn update_per_call_max(env: Env, new_max: i128) {
        let owner = storage::get_owner(&env);
        owner.require_auth();
        require_positive(&env, new_max);

        let mut policy = storage::get_policy(&env);
        policy.per_call_max = new_max;
        storage::set_policy(&env, &policy);
        events::per_call_max_updated(&env, new_max);
    }

    /// Approve a destination address, tagging it with `category` (e.g.
    /// `"compute"`, `"data"`, `"api"`). The category isn't enforced by
    /// any rule yet — see [`storage::AllowlistEntry`] — it's recorded now
    /// so category-based rules can be added later without re-tagging
    /// every existing entry.
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `destination` is already on the allowlist ([`Error::DestinationAlreadyAllowed`]).
    pub fn add_allowlist_entry(env: Env, destination: Address, category: Symbol) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        let mut allowlist = storage::get_allowlist(&env);
        if allowlist.iter().any(|e| e.destination == destination) {
            panic_with_error!(&env, Error::DestinationAlreadyAllowed);
        }
        allowlist.push_back(AllowlistEntry {
            destination: destination.clone(),
            category: category.clone(),
        });
        storage::set_allowlist(&env, &allowlist);
        events::allowlist_entry_added(&env, &destination, &category);
    }

    /// Remove a destination from the allowlist. Spends already recorded
    /// against it are untouched; only future spend attempts are affected.
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `destination` is not on the allowlist ([`Error::DestinationNotFound`]).
    pub fn remove_allowlist_entry(env: Env, destination: Address) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        let allowlist = storage::get_allowlist(&env);
        let index = allowlist.iter().position(|e| e.destination == destination);
        match index {
            Some(i) => {
                let mut allowlist = allowlist;
                allowlist.remove(i as u32);
                storage::set_allowlist(&env, &allowlist);
                events::allowlist_entry_removed(&env, &destination);
            }
            None => panic_with_error!(&env, Error::DestinationNotFound),
        }
    }

    /// Register an additional address allowed to call
    /// `check_and_record_spend`. See the module doc on `DataKey::Agents`
    /// for why this is a small set rather than a single address.
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `agent` is already registered ([`Error::AgentAlreadyRegistered`]).
    pub fn add_agent(env: Env, agent: Address) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        let mut agents = storage::get_agents(&env);
        if agents.contains(&agent) {
            panic_with_error!(&env, Error::AgentAlreadyRegistered);
        }
        agents.push_back(agent.clone());
        storage::set_agents(&env, &agents);
        events::agent_added(&env, &agent);
    }

    /// Deregister an agent address; it immediately loses the ability to
    /// have spends approved (existing signed-but-unsubmitted
    /// authorizations from it still fail the membership check on
    /// submission, same as any other rejection path).
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    /// - If `agent` is not registered ([`Error::AgentNotFound`]).
    /// - If `agent` is the only registered agent ([`Error::CannotRemoveLastAgent`]) —
    ///   removing it would leave the contract unable to approve any spend
    ///   at all, which is never what "revoke this one agent" means; flip
    ///   the kill switch instead if the goal is to stop all spending.
    pub fn remove_agent(env: Env, agent: Address) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        let agents = storage::get_agents(&env);
        if agents.len() <= 1 {
            panic_with_error!(&env, Error::CannotRemoveLastAgent);
        }
        let index = agents.iter().position(|a| a == agent);
        match index {
            Some(i) => {
                let mut agents = agents;
                agents.remove(i as u32);
                storage::set_agents(&env, &agents);
                events::agent_removed(&env, &agent);
            }
            None => panic_with_error!(&env, Error::AgentNotFound),
        }
    }

    /// Flip the kill switch. `true` instantly blocks every spend
    /// regardless of caps or allowlist, taking effect on the very next
    /// `check_and_record_spend` call — there is no grace period.
    ///
    /// # Panics
    /// - If not called with the owner's authorization.
    pub fn set_kill_switch(env: Env, active: bool) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        let mut policy = storage::get_policy(&env);
        policy.kill_switch = active;
        storage::set_policy(&env, &policy);
        events::kill_switch_toggled(&env, active);
    }

    /// Step 1 of ownership transfer: propose `new_owner`. Policy control
    /// stays with the current owner — who can still call every owner-only
    /// function, including proposing a different `new_owner` or none at
    /// all by transferring to themselves — until `new_owner` calls
    /// [`Self::accept_ownership`]. Two steps rather than one so a
    /// mistyped or unreachable address can't accidentally lock the
    /// contract's policy controls forever.
    ///
    /// # Panics
    /// - If not called with the current owner's authorization.
    pub fn transfer_ownership(env: Env, new_owner: Address) {
        let owner = storage::get_owner(&env);
        owner.require_auth();

        storage::set_pending_owner(&env, &new_owner);
        events::ownership_transfer_proposed(&env, &owner, &new_owner);
    }

    /// Step 2 of ownership transfer: the proposed owner claims it. Only
    /// after this call does the previous owner lose every owner-only
    /// privilege.
    ///
    /// # Panics
    /// - If no transfer is pending ([`Error::NoPendingOwner`]).
    /// - If not called with the pending owner's authorization (calling as
    ///   the outgoing owner, or anyone else, fails here).
    pub fn accept_ownership(env: Env) {
        let pending = match storage::get_pending_owner(&env) {
            Some(pending) => pending,
            None => panic_with_error!(&env, Error::NoPendingOwner),
        };
        pending.require_auth();

        let previous_owner = storage::get_owner(&env);
        storage::set_owner(&env, &pending);
        storage::clear_pending_owner(&env);
        events::ownership_transferred(&env, &previous_owner, &pending);
    }
}

fn require_positive(env: &Env, amount: i128) {
    if amount <= 0 {
        panic_with_error!(env, Error::NonPositiveAmount);
    }
}

fn is_allowed(env: &Env, destination: &Address) -> bool {
    storage::get_allowlist(env)
        .iter()
        .any(|e| &e.destination == destination)
}

#[cfg(test)]
mod test;
