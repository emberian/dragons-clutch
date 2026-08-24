# Project method

dClutch changes both the architecture and how the work is conducted.

## One workstream

`main` is the canonical integration line. Agent branches and worktrees are
temporary delivery mechanisms, not independent projects. A delegated lane must
have:

- one bounded outcome;
- named files or a non-overlapping semantic owner;
- an explicit input commit;
- a short handoff containing commits, invariants, and remaining blockers; and
- immediate convergence or deletion after review.

No lane receives an open-ended mandate such as "finish the subsystem."

## Vertical slices

Work proceeds through executable product slices:

1. pure total transition;
2. Solana account/CPI adapter;
3. chain-derived unsigned operator transaction;
4. adversarial tests and rollback evidence;
5. user-visible lifecycle/status projection; and
6. deletion of the path it supersedes.

A pure contract, inner composer, DTO, frontend mock, or review document is not
an enabled slice by itself.

The planned first slices are:

1. categorical Market Core: create, split, resolve, redeem;
2. Direct complementary signed intents: transfer, split-cross, merge-cross;
3. release-bound Pyth update plus Source consumption on a local validator;
4. covered Dealer capital and one immutable potential epoch;
5. General portfolio batching as an optional venue; and
6. recurring Product/Series creation over the same small core.

## Design budgets

Budgets are alarms, not arbitrary consensus restrictions:

- every universal persistent account requires an invariant and contention
  justification;
- every public wire action requires a distinct economic or authority
  transition;
- every additional program or CPI requires an atomicity and release-binding
  argument;
- every fixed width states whether it is mathematical, Solana-derived,
  measured-profile, or provisional; and
- every user workflow must be constructible from chain state without pasted
  semantic IDs or account metas.

Crossing a budget triggers a design decision in the same implementation cycle,
not a recurring audit project.

## Evidence ladder

Evidence is accumulated once at the appropriate boundary:

1. kernel unit and adversarial tests;
2. adapter/account tests;
3. local SBF execution;
4. local validator with real external program ABI/crypto;
5. bounded devnet execution;
6. reproducible release artifact;
7. independent security review; and
8. mainnet evidence.

Higher levels do not relabel lower ones. Re-running an unchanged lower-level
campaign is not progress unless a relevant boundary changed.

## Decision and deletion discipline

- Pre-release wire formats have no compatibility entitlement.
- A successor does not coexist indefinitely with its predecessor.
- Review findings are converted into an accepted decision, a rejected proposal,
  or an owned implementation; they are not recopied into another gap audit.
- Product/research work challenges the instrument and priorities continuously,
  but it may not silently become runtime authority.
- Commit messages describe the invariant or capability that changed.
