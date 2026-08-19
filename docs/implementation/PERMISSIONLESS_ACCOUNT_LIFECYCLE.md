# Permissionless account lifecycle

Status: **PROPOSED construction audit, 2026-08-19.** This document is an ABI
and reachability checkpoint. It does not claim that the listed instructions
have executed on a blank bank unless the evidence column says so.

## Promotion gate

The release gate starts an empty local bank containing only:

- the Clutch program ELF, the System program, Token-2022, sysvars, and the
  compute-budget program;
- a funded ephemeral payer and two ephemeral actor wallets; and
- a collateral mint and actor token accounts created by ordinary System and
  Token-2022 instructions during the scenario.

No Clutch-owned state account, zero-filled PDA, encoded protocol account,
outcome mint, Hoard token account, policy body, grid body, terms body,
observation page, resolution window, epoch, order page, or clearing artifact
may be installed in validator genesis. Every protocol PDA is derived from the
same seed constants the SBF program checks. Every accepted transaction is
signed, committed, reloaded from the bank, and decoded through the owning
codec. A failed late CPI must leave all watched accounts absent or byte-exactly
unchanged.

This is deliberately stricter than the current bring-up and committed-bank
harnesses. Those are valuable processor evidence, but their injected account
planes cannot establish permissionless construction.

## Exact construction inventory

| object | canonical address | current creation path | bank evidence | release status |
| --- | --- | --- | --- | --- |
| collateral-policy body | **no seed or persisted codec** | caller supplies an arbitrary 266-byte evidence account | genesis fixture only | **STOP: no ordinary-wallet producer or durable semantic owner** |
| Realm | `realm_pda(realm_id)` | `InitRealm`, System CPI | host only | drivable after policy transport exists |
| Profile | `profile_pda(realm, profile)` | `InitProfile`, System CPI | host only | drivable after policy transport exists |
| Price grid | `grid_pda(realm, grid_digest)` | `InitPriceGrid`, copies a caller-supplied 589-byte evidence account | host only | **STOP: no ordinary-wallet producer for the evidence account** |
| Terms | `terms_pda(realm, terms_digest)` | `InitTerms`, copies a caller-supplied 1,656-byte evidence account | host only | **STOP: body exceeds a Solana packet and no staged upload exists** |
| Market | `market_pda(realm, market)` | `CreateMarket` writes a preallocated PDA | real SVM, genesis-assisted | **STOP until the program System-CPI-creates the state target** |
| Hoard state | `hoard_pda(market)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same stop |
| founding Position | `position_pda(market, creator)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same stop |
| kernel aggregate | `kernel_pda(market)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same stop; reference-only state should not silently become a deployment ABI |
| legacy external shadow | `external_pda(market, owner, generation)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | **ABI decision pending actual Token-2022 supply cutover** |
| replay lane | `replay_pda(market, owner, generation)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same state-creation stop |
| supply ledger | `supply_pda(market)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same state-creation stop; external term is under redesign |
| resolution record | `resolution_pda(market)` | same `CreateMarket` preallocated plane | real SVM, genesis-assisted | same state-creation stop |
| outcome mints | `outcome_mint_pda(market, index)` | `CreateMarket`, real System and Token-2022 CPIs | real SVM from absent accounts | implemented for the currently admitted mint profile |
| Hoard token account | `hoard_token_pda(market)` | `CreateMarket`, real System and Token-2022 CPIs | real SVM from absent account | implemented for the currently admitted account profile |
| Hoard signing authority | `hoard_authority_pda(market)` | address only; no account should be allocated | real SVM authority use | implemented as a signing PDA |
| later Position/replay generation | seeds above | no intent creates it | none | **STOP: only the market creator can hold an internal Position** |
| feed head | `feed_pda(feed)` | no initializer | none | **STOP: `FeedAdvance` requires injected state** |
| authenticated source/archive page | source proposal has archive derivation; production wire has none | no initializer or writer | none | **STOP: caller buffers are not source-authenticated state** |
| Epoch | `epoch_pda(market, index)` | no initializer or freeze transition | none | **STOP: pages cannot be reached from a blank bank** |
| order page | `page_pda(epoch, page_index)` | `InitOrderPage`, System CPI | host only | blocked on Epoch creation; page-count ownership also needs one freeze rule |
| candidate record | `candidate_pda(epoch, digest)` | no initializer/submission path | none | **STOP** |
| final pot | `pot_pda(epoch)` | no initializer/selection path | none | **STOP** |
| settlement receipt | `receipt_pda(epoch, candidate, slice)` | no initializer/settlement path | none | **STOP** |
| candidate feed | **no seed yet** | no initializer | none | **STOP: address, admission, and writer authority are unspecified** |
| clearing checkpoint | **no seed yet** | no initializer; 48,750 bytes cannot be grown in one CPI | none | **STOP: address plus a resumable five-step allocate/write/seal ABI are unspecified** |

The table is about constructibility, not merely codecs. A codec for an account
does not create it; a seed function does not fund it; a host test does not run
`invoke_signed`; and an arbitrary evidence-buffer role is not a transport that
an ordinary wallet can necessarily produce.

## Coherent minimum repairs

### 1. `CreateMarket` creates its state plane

`CreateMarket` already carries the System program and Rent sysvar for its token
plane. Before the first write it must require every state target to be genuinely
absent: writable, non-executable, System-owned, zero lamports, and zero data.
It then derives every target and `CreateAccount`s each with its own canonical
signer seeds. Only the three immutable inputs (Realm, Profile, Terms) are
validated as existing program-owned state. The resulting accounts are encoded
and re-admitted exactly as today. Any state CPI, Token-2022 CPI, encode, or
post-write validation failure aborts the whole transaction.

This change must follow the external-supply ABI decision. Creating a legacy
per-owner shadow in the first supposedly deployable market would freeze it as a
rent-bearing public interface even if actual Token-2022 mint supply becomes the
only external truth.

### 2. First deposit may open an actor plane

The narrowest second-owner interface is not necessarily a new `InitPosition`
tag. `Endow { market, owner, amount }` already binds the owner, authenticates
their signature, and moves real collateral into the pooled Hoard. It can admit
one of two exact prestates:

1. an existing canonical Position/replay generation, which receives the
   deposit; or
2. absent generation-zero Position/replay targets, which it System-CPI-creates
   and initializes before making the same deposit.

That avoids unfunded orphan Positions and makes the first real deposit the
account-opening transaction. It requires System and Rent roles in the fixed
account list and a frozen decision on whether any per-owner external state
survives. It must not silently reopen a closed generation or infer a generation
from missing bytes.

### 3. Artifact transport is protocol state, not a fixture detail

The current evidence-buffer initializers are not blank-bank complete. The
policy, grid, and terms bodies have no public writer. Terms cannot be embedded
in one transaction because its encoded body alone is 1,656 bytes.

A coherent transport is a resumable artifact stage:

- `BeginArtifact(kind, realm, digest, exact_len)` creates a small stage PDA and
  the exact final target PDA;
- bounded `WriteArtifact(offset, chunk)` writes each byte range once and keeps
  an authenticated coverage bitmap/fold in the stage account;
- `SealArtifact` requires complete coverage, recomputes the policy/grid/terms
  identity through the owning codec, checks the target PDA and stored bump,
  changes the stage to terminal, and makes the final account admissible; and
- abort/close can return only the uploader's stage rent, never Hoard principal.

Intermediate target bytes are intentionally invalid protocol state, so every
consumer fails closed until sealing. Chunk identity, overlap behavior,
out-of-order writes, duplicate writes, close authority, and payer/rent recovery
must be specified before this becomes wire code. A generic untyped blob that
later consumers reinterpret would create a second semantic truth and is not an
acceptable shortcut.

The collateral policy also needs a durable account type or an equally exact
decision to inline and persist its canonical 266 bytes. Continuing to require
an arbitrary caller-owned buffer on every collateral instruction is not an
operatorless availability story.

### 4. Remaining missing lifecycle ABIs

After artifacts and actor opening, the next construction tags must be designed
with their semantic transitions, not just their byte allocation:

- `InitFeed` binds the Realm, frozen source/adaptor release, canonical empty
  accumulator summary, first cursor/boundary, clock/finality policy, and feed
  PDA. Archive/page creation and advancement must commit the exact history that
  resolution later consumes.
- `InitEpoch` binds market, terms, grid, policy, price scale, owner count,
  outcome count, relation version, and remainder seed in the open phase.
  `FreezeEpoch` is the one transition that binds the verified complete page set
  and moves it out of open placement.
- candidate submission creates both the canonical candidate record and its
  fill/witness feed under one candidate digest; verification creates or grows
  the checkpoint under resumable authenticated cursors; selection initializes
  the final pot once.
- settlement creates one receipt per canonical slice. A receipt is created only
  alongside the exact reservation/claim/cash movements it makes replay-safe;
  allocating empty receipts in advance would create authority-free junk state.

No `u64` truncation or host-only tag may stand in for a 32-byte onchain identity.

## Evidence ladder

The minimum tests, in order, are:

1. host refusal tests for nonzero-lamport targets, System-owned data targets,
   wrong seeds, wrong payer/writability, wrong System/Rent accounts, and every
   late-create rollback point;
2. a real-SVM `CreateMarket` test in which all state and token targets are
   absent before the transaction and rent-exempt, correctly owned and
   byte-exact afterwards;
3. a real-SVM second-wallet deposit that creates only that wallet's canonical
   actor plane and cannot initialize it for another signer;
4. a signed committed-bank sequence from artifact upload through two actor
   deposits, restart, and byte reload, with `genesis_assisted = false`; and
5. only after Feed/Epoch/settlement ABIs exist, the full promotion gate stated
   above.

Until step 4 is green, use **permissionless state creation in progress**, not
**permissionless lifecycle**. Until step 5 is green, use **blank-bank core
construction**, not **operatorless venue**.
