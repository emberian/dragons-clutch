# General's on-chain gap is not a missing producer. The producer cannot exist.

**Status:** LANDED at `a517d27c`. The recommendation below is what was built;
this section records where the note was wrong.

**The note's one wrong call was the sign of the CU.** It said an inline bank
under `ChunkedBankV2` would be sent once per output chunk and warned that as a
known cost. It is sent four times, and the ladder got **cheaper**: measured with
a control and the change on one tree at `06ef1015`, General `OpenBatch` fell
895,492 to 797,238 CU at N = 2, 878,152 to 794,898 at N = 13 and 897,452 to
827,694 at N = 258. Four page accounts cost a decode, a request-binding check,
an account borrow and a digest each; four extra sends cost a memcpy. The
mechanism was right and the arithmetic of which side wins was never done.

**What did get worse is the heap**, which the note did not predict at all: peak
50,516 to 61,520, +11,004. The CPI request buffer accounts for +2,744 of it
exactly (64 to 2,808 bytes, allocated once and reused); the remaining +8,260 is
spread across the four invocations and is now named by four new
`hot_heap_mark!` labels in `admitted_composition_v3` rather than hidden inside
one span. Splitting it exactly is owed.

The frame lost four accounts (59 to 55) and four logical coordinates (13 to 9);
the operator's recorded packet geometry lost eighteen accounts and thirty-six
wire bytes on every action at N = 258.

---

**Original status:** design note with a measured proof. It changes what C-05's
remaining work is: not "write the route that produces an input scratch page",
but "stop requiring one".

## What was believed

`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3009`, from the channel lane: *"INPUT scratch pages have no live
producer — General cannot execute on a real chain today."* That is true.
`AuthenticatedScratchPageV2::encode_into` is called from five places and none of
them is inside an SBF program: the contract's own unit tests, the host bundle
builder (`program-test/bundle-builder/src/bundle.rs:850`), two accelerator
program-tests, and the successor bootstrap's genesis fixture writer
(`tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs:1575`).
Every General action requires the transport unconditionally —
`validate_request_geometry`'s first conjunct is
`request.transport() != RequestTransportV2::ScratchPages`
(`programs/dclutch-general-accelerator-sbf/src/lib.rs:710`).

The natural reading is that a route is owed. It is not.

## Three facts that close the shape

**1. The input bank contains the slot, so a page is valid for exactly one slot.**

General's AccountProfile declares a trusted-environment slot destination —
`account_rules_v3.rs:1388`, `scalar::CURRENT_SLOT = 90` — and Trading seeds it
from `Clock::get()` on every execution
(`hot_v3.rs` `observe_trusted_environment_v3`), then refuses any projection that
disagrees (`require_trusted_environment_v3`). The input bank's bytes, and
therefore `request.input_bank_digest()`, are different in every slot. Every
reader binds a page to that digest:
`AuthenticatedScratchPageV2::validate_request_input` requires
`self.total_bank_digest == request.input_bank_digest()`, and Trading
additionally compares each payload byte-for-byte against the bank it has just
projected (`admitted_composition_v3.rs:986`).

**Measured, not argued.** `open_batch.rs`
`a_caller_written_input_page_is_stale_one_slot_after_it_was_written` builds the
honest General `OpenBatch` bundle, warps the bank one slot, and submits the same
transaction: it refuses `0x4018 AdmittedTransport` after 501,968 CU — the
transport's own discriminant, reached at its own depth, not something the frame
tripped over earlier.

**2. A full page does not fit in a transaction.**

A page account is `SCRATCH_PAGE_HEADER_BYTES_V2 + payload` = `192 + 880` = 1,072
bytes at every chunk but the last. A Solana transaction is 1,232 bytes. In the
cheapest possible v0 layout — one signature (65), the message header (3), two
static keys because a program id may not come from a lookup table (1 + 64), the
blockhash (32), one lookup table (1 + 34), and the instruction framing (~9) —
209 bytes are gone before the payload, leaving 1,023. **A full page cannot be
written by any single transaction, in any layout.** It would have to be
accumulated across transactions, in one slot, which nothing can guarantee.

**3. Every reader requires the page read-only, so Trading cannot write it
either.** The Trading library reader skips a writable account
(`hot_v3.rs:2090`), the General accelerator refuses one
`0xC00D ScratchPagePrivileges` (`lib.rs:925`), and `validate_frame` requires
every accelerator account non-writable except the output page (`lib.rs:754`).

(1) and (2) together are the whole argument: the bytes are knowable only in the
slot they will be used in, and they cannot be delivered inside that slot. There
is no transaction layout in which a caller-written input page is both fresh and
present.

## What is actually wrong

`classify_bank_transport_v2` (`execution-strategy-contract/src/v2.rs:1003`) is
the function that forces paging, and its own doc says what it is:

> Classify the chain-derived **return-data** bound without imposing a semantic N
> cap.

It computes `page_count = ceil(bank_bytes / 880)` and returns `InlineReturnData`
only when that is `<= 1`. 880 is `ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2`, which is
`SVM_RETURN_DATA_BYTES_V2` (1,024) minus `ACCELERATOR_ACK_HEADER_BYTES_V2`
(144). It is an **output** limit. Trading then uses its answer to decide the
**input** transport: the General AccountProfile's scratch-page span takes its
width from that page count (`account_rules_v3.rs:185`, `minimum: 1`), Trading's
frame carving demands that many page accounts, and
`admitted_composition_v3.rs:269` selects `ScratchPages` because pages were
supplied — under a comment that already says the right thing:

> Input transport and output transport are orthogonal.

They are orthogonal, and one of them is nevertheless deciding the other.

**The input bank does not ride return data. It rides CPI instruction data**, and
that limit is `MAX_CPI_INSTRUCTION_DATA_LEN = 10 * 1024`
(`solana-bpf-loader-program/src/syscalls/cpi.rs:14`). Trading's own caps bound
the bank at `MAX_HOT_SCALARS_V3 * 8 + MAX_HOT_IDENTITIES_V3 * 32` =
`512 * 8 + 128 * 32` = **8,192 bytes** (`hot_v3.rs:434-435`), plus a 64-byte
request header. **Every register bank Trading can legally construct fits inline
in one CPI, with 1,984 bytes to spare** — at every action and every Product
width, not just General's.

## The recommendation

Stop deriving the input transport from the return-data bound.

1. Give the input transport its own classifier, bounded by
   `MAX_CPI_INSTRUCTION_DATA_LEN` rather than by 880, and let
   `classify_bank_transport_v2` keep answering the output question it was
   written for. Under `AcceleratorTransportProfileV2::OutputPageV3` the output
   does not ride return data at all, so nothing is left that wants 880.
2. General's AccountProfile then declares no scratch-page span, its
   `validate_request_geometry` accepts `RequestTransportV2::Inline`, and
   `AdmittedAcceleratorRequestV2::new` already carries the bank inline with no
   size cap of its own (`v2.rs:1127`).
3. Nothing needs a producer, because nothing needs a page.

The cost is the four-artifact join `ea4c46e0` documented: the account profile,
request profile, transition and effect all declare the geometry and move
together, Lean first, and every General artifact digest moves. General has no
published on-chain substrate, so the re-digest strands nothing. It also deletes
four accounts and four chunk authorities from every General frame — measured
tonight, the honest `OpenBatch` frame is 59 accounts of which 4 are pages and 4
are chunk authorities.

### The one thing that has to land with it, or after it

Under `AcceleratorTransportProfileV2::ChunkedBankV2` the request is rebuilt once
per output chunk with the same bank (`admitted_composition_v3.rs:317-331`), so an
inline input would be transmitted `accelerator_invocation_count_v2` times --- four
times at General `OpenBatch` N = 2. Under `OutputPageV3` there is exactly one
invocation, and the inline bank is sent once. So the end state is inline input
**and** the output page, and the second half is ember's 0003 ruling to switch on,
not a lane's to build.

Landing inline input alone is still strictly better than being unexecutable --- it
removes four page accounts and four chunk authorities from the frame, and the
repeated bank is CU, not a wall --- but it should be stated as an interim, and the
CU should be measured before and after rather than assumed. `classify_bank_transport_v2`
must be left exactly as it is for that reason: it is what sizes the caller-authority
span and the output chunk count, and those are still the return-data question.

**Keep the page transport.** It is correct, it is tested, and it is the only
thing that can carry a bank wider than one CPI. It should be reachable when a
bank exceeds `MAX_CPI_INSTRUCTION_DATA_LEN`, which no current family does, and
its producer problem is then a problem no shipping route has.

## If the pages are kept anyway, this is the only route that can work

Trading writes them, in the instruction that reads them, from the bank it has
just projected — the only writer that can be in the right slot by construction.
That needs three things and none of them is a new instruction:

- the page accounts arrive **writable at the top level** and are passed
  **read-only in the accelerator CPI** (privileges may be downgraded across a
  CPI boundary, never raised), which satisfies `validate_frame` and every reader
  unchanged;
- `general_scratch_page_rule_v3` (`account_rules_v3.rs:198`) must grant the
  write privilege it currently denies — the four-way join again;
- Trading's own observation transcript already records `signer: false,
  writable: false` for every runtime coordinate (`runtime_transcript_digest_v3`),
  so top-level writability does not move the invocation-context digest.

The **accounts** still need a creator, and that is a genuinely new, small route
modelled on `process_capability_seal_v1` (`hot_v3/seal.rs:78`): a magic-selected
Trading instruction that `transfer`s the rent deficit from a payer,
`invoke_signed(allocate)` and `invoke_signed(assign)` a Trading PDA, and a close
half that reclaims the rent, or every General action strands
`4 x rent(1,072) ~ 0.033 SOL`. Seeds must be domain-separated and
client-derivable — `[domain, payer, page_index]` gives a **reusable** per-payer
scratch pool whose address does not move with the bank, which is what a page
holding no authority should be. Keying by `input_bank_digest` would move the
address every slot, which is the same defect one level out.

## What the successor bootstrap must do

`family_hot_campaign.rs` today writes pages as genesis `--account` fixtures
(`:645`) owned by `--caller`, at `campaign_account_key_v1` addresses that no key
signs for, and invokes the `general-accelerator-test-caller` program rather than
Trading. Against cohort-12 that is four separate changes, and the first one is
now a fork:

1. **Decide the transport first.** Under the recommendation above the campaign
   drops the page loop entirely and sets `RequestTransportV2::Inline`; under the
   page route it keeps it and the remaining three apply.
2. The page's `trading_program` field and the account owner become the real
   Trading program id, not `--caller`.
3. The address becomes a Trading PDA, because `assign` requires either the
   account's signature or the owning program's — a SHA-256 address that nothing
   can sign for works only because genesis can mint arbitrary accounts.
4. `write_genesis_account_v1` becomes a transaction, sequenced before the hot
   instruction, and the campaign additionally founds a Market whose capability
   manifest selects General — the seven conditions are already enumerated on
   chain-checked terms at `family_hot_campaign.rs:1601`.

Item 4's Market is owed under either fork, and it is the larger of the two.
