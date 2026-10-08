# Security Policy

This contract decides whether an autonomous AI agent is allowed to spend
money. A bug here can let an agent, a compromised relayer, or an outsider
spend past the owner's limits, or lock the owner out of their own
guardrails. **Security reports are welcome and are prioritized over all
other work.**

## Reporting a vulnerability

Report privately through GitHub:

**[Report a vulnerability](https://github.com/Bridle-Stellar/Bridle-Contract/security/advisories/new)**

Only the maintainers can see these reports. Please don't open a public
issue, PR, or discussion for a suspected vulnerability until it has been
fixed.

Include whatever you have:

- what an attacker can do, and what they need first (which key or role);
- the affected function(s) and commit;
- steps or a test case that reproduces it. A failing test against
  `src/test.rs` is ideal.

## What to expect

- Acknowledgement within 3 days.
- An initial assessment (confirmed, needs more info, or not an issue)
  within 7 days.
- For confirmed issues, we'll agree a disclosure timeline with you, fix it,
  and credit you in the advisory unless you'd rather stay anonymous.

This is a small open-source project with no bug bounty.

## Scope

In scope, roughly in order of how much we care:

- Anything that lets a spend be approved that the policy should reject:
  bypassing the kill switch, allowlist, per-call max, or daily cap, or
  breaking the daily rollover.
- Anything that lets someone other than the current owner change policy,
  agents, or ownership, or lets the owner be permanently locked out.
- Auth problems around `agent.require_auth()`, such as replaying or
  reusing an agent's authorization for a different spend.
- Ways to make the spend counter disagree with what was actually approved.
- Denial of service that stops the owner from using the kill switch.

Out of scope:

- Bridle Backend and Bridle Frontend. Report those in their own repos.
- Bugs in soroban-sdk, the Soroban host, or Stellar itself. Report those
  upstream.
- The testnet deployment's throwaway keys and balances.
- The documented design choice that the relayer decides which spends to
  *ask* the agent to sign. See the README's auth model.

## Supported versions

Only `main` is supported. There is no mainnet deployment.
