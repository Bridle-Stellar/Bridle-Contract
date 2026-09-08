//! Behavioral tests. Each test name states one behavior the contract
//! guarantees; taken together they're meant to double as documentation of
//! what "the guardrail" actually does at the boundaries.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    Address, Env, IntoVal,
};

use crate::policy::{RejectReason, SpendOutcome};
use crate::{BridleContract, BridleContractClient};

const DAY: u64 = 86_400;

struct Setup<'a> {
    env: Env,
    client: BridleContractClient<'a>,
    owner: Address,
    agent: Address,
    token: Address,
    destination: Address,
}

/// Deploys and initializes a contract with a 1_000 daily cap, a 300
/// per-call max, one approved destination, and the clock at the start of
/// a day — the baseline every other test starts from.
fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(10 * DAY);

    let contract_id = env.register(BridleContract, ());
    let client = BridleContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let agent = Address::generate(&env);
    let token = Address::generate(&env);
    let destination = Address::generate(&env);

    client.initialize(&owner, &agent, &token, &1_000, &300);
    client.add_allowlist_entry(&destination, &symbol_short!("compute"));

    Setup {
        env,
        client,
        owner,
        agent,
        token,
        destination,
    }
}

#[test]
fn approves_spend_within_all_limits() {
    let s = setup();

    let outcome = s
        .client
        .check_and_record_spend(&s.agent, &s.destination, &s.token, &100);

    match outcome {
        SpendOutcome::Approved(receipt) => {
            assert_eq!(receipt.spent_today, 100);
            assert_eq!(receipt.remaining_today, 900);
        }
        SpendOutcome::Rejected(reason) => panic!("expected approval, got {:?}", reason),
    }

    let status = s.client.get_spend_status();
    assert_eq!(status.spent_today, 100);
    assert_eq!(status.remaining_today, 900);
}

#[test]
fn rejects_spend_over_daily_cap() {
    let s = setup();

    // Three calls at the per-call max (300 each) eat 900 of the 1_000
    // daily cap, leaving only 100 — each individually legal on its own.
    for _ in 0..3 {
        let outcome =
            s.client
                .check_and_record_spend(&s.agent, &s.destination, &s.token, &300);
        assert!(matches!(outcome, SpendOutcome::Approved(_)));
    }
    assert_eq!(s.client.get_spend_status().spent_today, 900);

    // A fourth call for 150 is well within the per-call max, but only 100
    // remains today — it must be rejected for the daily cap, not allowed
    // through because it individually looks fine.
    let outcome =
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &150);
    assert_eq!(
        outcome,
        SpendOutcome::Rejected(RejectReason::ExceedsDailyCap)
    );
    // The rejected attempt must not have moved the counter.
    assert_eq!(s.client.get_spend_status().spent_today, 900);
}

#[test]
fn rejects_spend_over_per_call_max() {
    let s = setup();

    let outcome =
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &301);

    assert_eq!(
        outcome,
        SpendOutcome::Rejected(RejectReason::ExceedsPerCallMax)
    );
    assert_eq!(s.client.get_spend_status().spent_today, 0);
}

#[test]
fn rejects_spend_to_destination_not_on_allowlist() {
    let s = setup();
    let stranger = Address::generate(&s.env);

    let outcome = s
        .client
        .check_and_record_spend(&s.agent, &stranger, &s.token, &50);

    assert_eq!(
        outcome,
        SpendOutcome::Rejected(RejectReason::DestinationNotAllowed)
    );
}

#[test]
fn rejects_every_spend_while_kill_switch_is_on() {
    let s = setup();
    s.client.set_kill_switch(&true);

    let outcome =
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &1);

    assert_eq!(
        outcome,
        SpendOutcome::Rejected(RejectReason::KillSwitchActive)
    );

    // Flipping it back off restores normal enforcement immediately.
    s.client.set_kill_switch(&false);
    let outcome =
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &1);
    assert!(matches!(outcome, SpendOutcome::Approved(_)));
}

#[test]
fn rejects_unregistered_agent_even_with_a_valid_signature() {
    let s = setup();
    let stranger = Address::generate(&s.env);

    // `mock_all_auths` means the signature check itself passes for any
    // address — this proves the allowlist-of-agents check, not the
    // signature check, is what blocks a spend request signed by someone
    // who was never registered as an agent.
    let outcome = s
        .client
        .check_and_record_spend(&stranger, &s.destination, &s.token, &1);

    assert_eq!(outcome, SpendOutcome::Rejected(RejectReason::UnknownAgent));
}

#[test]
fn rejects_spend_naming_a_different_token_than_the_policy() {
    let s = setup();
    let other_token = Address::generate(&s.env);

    let outcome = s.client.check_and_record_spend(
        &s.agent,
        &s.destination,
        &other_token,
        &1,
    );

    assert_eq!(outcome, SpendOutcome::Rejected(RejectReason::WrongToken));
}

#[test]
fn rejects_zero_and_negative_amounts() {
    let s = setup();

    assert_eq!(
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &0),
        SpendOutcome::Rejected(RejectReason::InvalidAmount)
    );
    assert_eq!(
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &-10),
        SpendOutcome::Rejected(RejectReason::InvalidAmount)
    );
}

#[test]
fn daily_cap_resets_at_the_next_calendar_day() {
    let s = setup();

    s.client
        .check_and_record_spend(&s.agent, &s.destination, &s.token, &300);
    assert_eq!(s.client.get_spend_status().spent_today, 300);

    // Jump the ledger clock forward into the next calendar day.
    s.env.ledger().set_timestamp(11 * DAY);

    // The read path reports the reset without anyone having spent yet...
    let status = s.client.get_spend_status();
    assert_eq!(status.spent_today, 0);
    assert_eq!(status.remaining_today, 1_000);

    // ...and a full day's cap is available again for real spends, proving
    // the reset isn't just a read-side illusion.
    let outcome =
        s.client
            .check_and_record_spend(&s.agent, &s.destination, &s.token, &300);
    assert!(matches!(outcome, SpendOutcome::Approved(_)));
    assert_eq!(s.client.get_spend_status().spent_today, 300);
}

#[test]
fn daily_cap_does_not_reset_within_the_same_calendar_day() {
    let s = setup();

    s.client
        .check_and_record_spend(&s.agent, &s.destination, &s.token, &300);
    // Advance a few hours, still the same UTC day.
    s.env.ledger().set_timestamp(10 * DAY + 3600);

    assert_eq!(s.client.get_spend_status().spent_today, 300);
}

#[test]
fn owner_can_update_daily_cap() {
    let s = setup();
    s.client.update_daily_cap(&2_000);
    assert_eq!(s.client.get_policy().daily_cap, 2_000);
}

#[test]
fn agent_cannot_call_owner_only_functions() {
    let s = setup();

    // Mock only the agent's authorization, not the owner's, so
    // `owner.require_auth()` inside `update_daily_cap` has nothing valid
    // to check against — proving that holding the agent's key is not
    // enough to change policy, regardless of what the call claims.
    let result = s
        .client
        .mock_auths(&[MockAuth {
            address: &s.agent,
            invoke: &MockAuthInvoke {
                contract: &s.client.address,
                fn_name: "update_daily_cap",
                args: (2_000i128,).into_val(&s.env),
                sub_invokes: &[],
            },
        }])
        .try_update_daily_cap(&2_000);

    assert!(result.is_err());
    // Policy is unchanged.
    assert_eq!(s.client.get_policy().daily_cap, 1_000);
}

#[test]
fn agent_cannot_call_allowlist_management_either() {
    let s = setup();
    let destination2 = Address::generate(&s.env);

    // Same property as `agent_cannot_call_owner_only_functions`, exercised
    // against a second owner-only function to confirm it's not
    // `update_daily_cap`-specific: every owner-only function checks
    // `owner.require_auth()`, not "whoever the caller happens to be".
    let result = s
        .client
        .mock_auths(&[MockAuth {
            address: &s.agent,
            invoke: &MockAuthInvoke {
                contract: &s.client.address,
                fn_name: "remove_allowlist_entry",
                args: (destination2.clone(),).into_val(&s.env),
                sub_invokes: &[],
            },
        }])
        .try_remove_allowlist_entry(&destination2);
    assert!(result.is_err());
}

#[test]
fn owner_can_manage_the_allowlist() {
    let s = setup();
    let merchant = Address::generate(&s.env);

    s.client
        .add_allowlist_entry(&merchant, &symbol_short!("data"));
    assert_eq!(s.client.get_policy().allowlist.len(), 2);

    s.client.remove_allowlist_entry(&merchant);
    assert_eq!(s.client.get_policy().allowlist.len(), 1);

    // Removing it again fails — it's already gone.
    let result = s.client.try_remove_allowlist_entry(&merchant);
    assert!(result.is_err());
}

#[test]
fn owner_can_add_and_remove_a_second_agent_but_not_the_last_one() {
    let s = setup();
    let agent2 = Address::generate(&s.env);

    s.client.add_agent(&agent2);
    assert_eq!(s.client.get_policy().agents.len(), 2);

    s.client.remove_agent(&agent2);
    assert_eq!(s.client.get_policy().agents.len(), 1);

    // The one remaining agent can't be removed — that would brick spending.
    let result = s.client.try_remove_agent(&s.agent);
    assert!(result.is_err());
}

#[test]
fn ownership_transfer_moves_privileges_to_the_new_owner() {
    let s = setup();
    let new_owner = Address::generate(&s.env);

    s.client.transfer_ownership(&new_owner);
    // Proposing a transfer alone doesn't move anything yet.
    assert_eq!(s.client.get_policy().owner, s.owner);

    s.client.accept_ownership();
    assert_eq!(s.client.get_policy().owner, new_owner);

    // The old owner has lost every management privilege...
    let result = s
        .client
        .mock_auths(&[MockAuth {
            address: &s.owner,
            invoke: &MockAuthInvoke {
                contract: &s.client.address,
                fn_name: "update_daily_cap",
                args: (500i128,).into_val(&s.env),
                sub_invokes: &[],
            },
        }])
        .try_update_daily_cap(&500);
    assert!(result.is_err());

    // ...while the new owner has gained them.
    s.client.update_daily_cap(&500);
    assert_eq!(s.client.get_policy().daily_cap, 500);
}

#[test]
fn accept_ownership_fails_with_no_transfer_pending() {
    let s = setup();
    let result = s.client.try_accept_ownership();
    assert!(result.is_err());
}

#[test]
fn accept_ownership_requires_the_pending_owners_own_signature() {
    let s = setup();
    let new_owner = Address::generate(&s.env);
    s.client.transfer_ownership(&new_owner);

    // The current owner can't accept on the new owner's behalf.
    let result = s
        .client
        .mock_auths(&[MockAuth {
            address: &s.owner,
            invoke: &MockAuthInvoke {
                contract: &s.client.address,
                fn_name: "accept_ownership",
                args: ().into_val(&s.env),
                sub_invokes: &[],
            },
        }])
        .try_accept_ownership();
    assert!(result.is_err());
}
