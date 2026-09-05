// Generate the persisted state-machine tag tables from their Rust owners.
//
// WHAT THIS IS FOR. `lib/generated/marketPhaseAdmissionV1.ts` publishes, per
// route, the machines a gate is over that the Core Market's phase cannot
// answer -- `direct-root`, `series-ticket`, `funding-ledger`,
// `projected-custody`, `dealer-checkpoint`, `source`. Until this file existed
// no client surface could DECODE any of them, so every act driving such a
// route reported `needs-chain` forever: the census read a gate and the reader
// had no instrument. This emits the instrument: for each machine, the record
// that persists its discriminant, the byte the discriminant sits at, and the
// exact wire tag of every state its own hostile decoder admits.
//
// WHERE EACH FACT'S AUTHOR IS. Every one of these eight machines persists its
// record at Lean-emitted offsets AND names its wire tags in the same emission,
// and this file reads that emission for both (`generated_successor.rs`,
// `generated_source_resolution_state_v2.rs`, `generated_abi.rs`,
// `generated_dealer_*.rs`, `generated_scenario_checkpoint_v1.rs`,
// `generated_scenario_reservation_state_v1.rs`,
// `generated_projected_state_v2.rs`, `generated_ticket_state_v3.rs`) and never
// the hand-written mirror beside it. That is new twice over: until the
// LEAN-TAGS lane FOUR of the eight had no Lean module at all, and until
// LEAN-TAGS-2 three still authored their tags in Rust.
//
// The TAGS were a separate question with a separate answer per machine, and
// they are not any more: ALL EIGHT now have Lean-emitted discriminants, so
// every number in the table below comes from a `generated_*.rs`. The Rust
// scrape that used to recover them is gone with its last consumer --
// `agreedOffset` for the coordinates, `literalTag` for the tags -- and what
// remains of the Rust reading is which ARM HEAD each decoder admits, which is
// a fact about the decoder and cannot be emitted.
//
// THE TAGS COME FROM THE HOSTILE DECODER, not from the enum declaration. Three
// of these enums carry no `#[repr(u8)]` discriminants at all, one resolves its
// arms through named constants in a generated module, and one has no `decode`
// method -- its match is inline in the record's own decoder. So the reader
// below takes the match arms of whatever the chain actually runs, which is the
// only text that decides which byte is admitted. Where the enum ALSO declares
// explicit discriminants they are read and compared, and a disagreement throws:
// that is the one cross-check available here, and it is stronger than trusting
// either half alone.
//
// The machine LABELS are not typed here either. They are read from
// `tools/gauntlet/census/src/phases.rs`, which is the table the census walks
// when it attributes a guard to a machine, so a machine added there and not
// here is a hard failure rather than a silent omission.
import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  census: readFileSync(new URL('tools/gauntlet/census/src/phases.rs', root), 'utf8'),

  directSuccessor: readFileSync(new URL('crates/dclutch-trading/src/successor.rs', root), 'utf8'),
  directGenerated: readFileSync(new URL('crates/dclutch-trading/src/generated_successor.rs', root), 'utf8'),

  dealerLib: readFileSync(new URL('crates/dclutch-trading/src/dealer/mod.rs', root), 'utf8'),
  dealerLiquidityGenerated: readFileSync(new URL('crates/dclutch-trading/src/dealer/generated_dealer_liquidity.rs', root), 'utf8'),
  dealerProfileGenerated: readFileSync(new URL('crates/dclutch-trading/src/dealer/generated_dealer_trading_profile.rs', root), 'utf8'),
  dealerCheckpoint: readFileSync(new URL('crates/dclutch-trading/src/dealer/scenario_checkpoint_v1.rs', root), 'utf8'),
  dealerCheckpointGenerated: readFileSync(new URL('crates/dclutch-trading/src/dealer/generated_scenario_checkpoint_v1.rs', root), 'utf8'),
  dealerReservation: readFileSync(new URL('crates/dclutch-trading/src/dealer/scenario_custody_reservation_v1.rs', root), 'utf8'),
  dealerReservationGenerated: readFileSync(new URL('crates/dclutch-trading/src/dealer/generated_scenario_reservation_state_v1.rs', root), 'utf8'),

  projectedCustody: readFileSync(new URL('crates/dclutch-custody/src/projected.rs', root), 'utf8'),
  projectedCustodyGenerated: readFileSync(new URL('crates/dclutch-custody/src/generated_projected_state_v2.rs', root), 'utf8'),

  seriesReplay: readFileSync(new URL('crates/dclutch-trading/src/series/replay.rs', root), 'utf8'),
  seriesGenerated: readFileSync(new URL('crates/dclutch-trading/src/series/generated.rs', root), 'utf8'),
  seriesTicketGenerated: readFileSync(new URL('crates/dclutch-trading/src/series/generated_ticket_state_v3.rs', root), 'utf8'),

  funding: readFileSync(new URL('crates/dclutch-market/src/capability_manifest/funding.rs', root), 'utf8'),
  fundingGenerated: readFileSync(new URL('crates/dclutch-market/src/capability_manifest/generated_abi.rs', root), 'utf8'),

  source: readFileSync(new URL('crates/dclutch-source/src/lib.rs', root), 'utf8'),
  // The one place the Source state's ADDRESS is derived. A seed order is not a
  // constant, so the expression itself is pinned below, exactly as
  // `generate-direct-participant-v1.mjs` pins the Direct token PDA's.
  resolutionOperator: readFileSync(new URL('crates/dclutch-resolution-core-v3-operator/src/lib.rs', root), 'utf8'),
  sourceGenerated: readFileSync(new URL('crates/dclutch-source/src/generated_source_resolution_state_v2.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/stateMachinesV1.ts', import.meta.url);

/** One `const NAME: <int> = 123;` in a named Rust source. */
function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

/**
 * One eight-byte record magic, in either spelling the tree uses.
 *
 * The two spellings are `*b"DCLTFL02"` and the byte array, and BOTH appear in
 * Lean emissions -- `generated_abi.rs` writes the first for the funding
 * ledger, `generated_successor.rs` and the rest write the second -- so which
 * one a record uses says nothing about who authors it. All eight magics read
 * here now come from an emission. The spelling used to correlate with
 * authorship, and this comment used to say so by naming a hand-written
 * `*b"DCLTDSC1"`; that record's magic is emitted since the LEAN-TAGS lane and
 * the correlation was never the rule anyway.
 *
 * A byte array may run to the next line, which `[^\]]+` spans; a magic split
 * after the `=` does not match either branch, and the failure is a thrown
 * `missing Rust magic`, not a silent absence.
 */
function magic(source, name) {
  const literal = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; 8\\] = \\*b"([^"]+)";`));
  if (literal) return literal[1];
  const array = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; 8\\] = \\[([^\\]]+)\\];`));
  if (!array) throw new Error(`missing Rust magic ${source}.${name}`);
  const bytes = array[1].split(',').map((part) => part.trim()).filter((part) => part.length > 0);
  if (bytes.length !== 8) throw new Error(`Rust magic ${source}.${name} is not eight bytes`);
  return bytes.map((byte) => String.fromCharCode(Number(byte))).join('');
}

/** One PDA seed domain, in either spelling the tree uses. */
function domain(source, name) {
  const literal = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: &\\[u8\\] = b"([^"]+)";`));
  if (literal) return literal[1];
  const array = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: &\\[u8\\] = &\\[([^\\]]+)\\];`));
  if (!array) throw new Error(`missing Rust PDA domain ${source}.${name}`);
  return array[1].split(',').map((part) => part.trim()).filter((part) => part.length > 0)
    .map((byte) => String.fromCharCode(Number(byte))).join('');
}

/** The body of one `enum Name { .. }`, for the discriminant cross-check. */
function enumBody(source, name) {
  const body = sources[source].match(new RegExp(`(?:pub )?enum ${name} \\{([\\s\\S]*?)\\n\\}`));
  if (!body) throw new Error(`missing Rust enum ${source}.${name}`);
  return body[1];
}

/**
 * Explicit discriminants, or null when the enum declares none.
 *
 * A discriminant is either a byte literal or, once a machine's tags are
 * Lean-emitted, the NAME of an emitted constant -- `Prepared =
 * SERIES_TICKET_PHASE_PREPARED_V3,`. Both are read, because the regex that saw
 * only literals would have returned null for the second and silently dropped
 * the one cross-check this file has: it would have called an enum that names
 * its tags "an enum that declares none".
 */
function declaredDiscriminants(source, name, resolve = null) {
  const declared = [...enumBody(source, name).matchAll(/\n\s*([A-Z][A-Za-z0-9]*) = ([A-Za-z0-9_:]+),/g)];
  if (declared.length === 0) return null;
  return declared.map(([, variant, value]) => {
    if (/^[0-9_]+$/.test(value)) return [variant, Number(value.replaceAll('_', ''))];
    if (resolve === null) {
      throw new Error(`${name}::${variant} declares ${value} and this machine names no resolver`);
    }
    return [variant, resolve(value)];
  });
}

/** Every `Variant,` an enum declares, in declaration order. */
function declaredVariants(source, name) {
  return [...enumBody(source, name).matchAll(/\n\s*([A-Z][A-Za-z0-9]*)(?:\s*=\s*[A-Za-z0-9_:]+)?,/g)].map((match) => match[1]);
}

/**
 * The wire tags one hostile decoder admits, read off its own match arms.
 *
 * `block` is the exact Rust text of the decoding match. `resolve` turns an arm
 * head into a number: a bare literal for most machines, a named constant in a
 * generated module for the Dealer root, whose `Phase::decode` deliberately
 * routes through `generated::PHASE_*` so the emission is the author.
 */
function decodedTags(block, resolve) {
  const arms = [...block.matchAll(/\n\s*([A-Za-z0-9_:]+) =>\s*(?:Ok\()?(?:Self|[A-Za-z0-9_]+)::([A-Z][A-Za-z0-9]*)/g)];
  if (arms.length === 0) throw new Error('a state machine decoder admitted no arm at all');
  const tags = arms.map(([, head, variant]) => [variant, resolve(head)]);
  const seen = new Set();
  for (const [variant, tag] of tags) {
    if (seen.has(tag)) throw new Error(`two states of one machine decode from byte ${tag}`);
    seen.add(tag);
    if (!Number.isInteger(tag) || tag < 0 || tag > 255) throw new Error(`${variant} decodes from a byte outside 0..255`);
  }
  return tags;
}

/**
 * The text from one UNIQUE anchor to the end of the match it opens.
 *
 * The anchor is an `impl <Enum> {` line rather than a `fn decode` signature,
 * because a signature is not unique -- `successor.rs` carries three identical
 * `fn decode(value: u8) -> SuccessorResult<Self>` and `dealer-codec/src/lib.rs`
 * three identical `fn decode(value: u8) -> Result<Self>`, so anchoring on one
 * silently reads a different machine's arms. A non-unique anchor throws.
 *
 * The block ends at the hostile arm `_ =>`, which every one of these decoders
 * closes with, so nothing after the decode -- an encoder's inverse match, a
 * `terminal()` predicate -- can contribute an arm.
 */
function block(source, anchor) {
  const start = sources[source].indexOf(anchor);
  if (start < 0) throw new Error(`missing Rust decoder anchor in ${source}: ${anchor}`);
  if (sources[source].indexOf(anchor, start + anchor.length) >= 0) {
    throw new Error(`ambiguous Rust decoder anchor in ${source}: ${anchor}`);
  }
  const end = sources[source].indexOf('_ =>', start + anchor.length);
  if (end < 0) throw new Error(`unterminated Rust decoder block in ${source}: ${anchor}`);
  return sources[source].slice(start, end);
}

// `literalTag` used to live here: a decoder arm head that IS a byte literal,
// resolved by reading the number out of the Rust text. It is deleted rather
// than kept, because all eight machines now name their tags and it had no
// remaining consumer -- and a resolver that accepts a literal is exactly the
// arm through which a hand-written tag re-enters. `emittedTag` is now the only
// way an arm head becomes a number here, so a machine that reverts to `0 =>
// Ok(..)` fails loudly instead of being read.

/**
 * A decoder arm head that NAMES its tag, resolved in the Lean emission.
 *
 * A machine whose discriminants are emitted writes its decoder as
 * `SERIES_TICKET_PHASE_PREPARED_V3 => Ok(Self::Prepared)` rather than
 * `0 => ..`, so the arm head is a constant name and the number comes from the
 * generated module. The module path prefix is stripped because a crate may
 * reach the constant either way (`generated::PHASE_OPEN` or a bare import) and
 * which spelling it picked is not an ABI fact.
 */
const emittedTag = (source) => (head) => scalar(source, head.replace(/^[A-Za-z0-9_]+::/, ''));

// `agreedOffset` used to live here: `projected.rs` and `replay.rs` each wrote
// their phase byte at a bare literal, so the offset was recovered by reading
// BOTH the encode and the decode expression and refusing when they disagreed.
// It is deleted rather than kept, because both machines now emit that
// coordinate and a second authority for a fact is what this file exists to
// remove. It is also worth recording WHY it was never as strong as it read:
// the encode expression it matched for `series-ticket` belonged to
// `SeriesStateV3`, two hundred lines above `TicketStateV3`'s identical line,
// and a first-match regex takes the former. The two agreed, so nothing noticed.

// The Source state's PDA seed ORDER, which no constant can express. A reorder
// here would move every Source address while every emitted constant stayed
// correct, so the expression is read and refused when it moves.
if (!/Pubkey::find_program_address\(\s*&\[\s*SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,\s*market_key\.as_ref\(\),\s*&generation\.to_le_bytes\(\),\s*\],\s*&resolution_program,/.test(sources.resolutionOperator)) {
  throw new Error('the Source resolution state PDA seed order changed');
}

// Every machine label the census attributes a guard to. Read rather than
// typed, so a machine that gains a guard in Rust and never reaches a browser
// is a red generator here instead of an absence nobody notices.
const censusMachines = [...sources.census.matchAll(/admission_type: "([A-Za-z0-9]+)",\s*\n\s*label: "([a-z-]+)",\s*\n\s*primary: "([A-Za-z0-9]+)",/g)]
  .map(([, admission, label, primary]) => ({ admission, label, primary }));
if (censusMachines.length === 0) throw new Error('the census MACHINES table did not parse');
if (!censusMachines.some((machine) => machine.label === 'market')) {
  throw new Error('the census MACHINES table no longer carries the Core Market');
}

const machines = [
  {
    label: 'direct-root',
    record: 'DirectRootStateV1',
    magic: magic('directGenerated', 'DIRECT_ROOT_MAGIC_V1'),
    bytes: scalar('directGenerated', 'DIRECT_ROOT_STATE_BYTES_V1'),
    header: [[scalar('directGenerated', 'DIRECT_ROOT_VERSION_OFFSET_V1'), scalar('directGenerated', 'DIRECT_SUCCESSOR_ABI_VERSION_V1')]],
    tagOffset: scalar('directGenerated', 'DIRECT_ROOT_PHASE_OFFSET_V1'),
    rowBytes: null,
    headerBytes: null,
    // `require_closable` is the phase AND this count, so a client that reads
    // only the phase can still call a refused global close ready.
    counters: [['openMakerRootCount', scalar('directGenerated', 'DIRECT_ROOT_OPEN_MAKER_COUNT_OFFSET_V1')]],
    pdaDomain: null,
    discriminant: 'DirectRootPhaseV1',
    states: decodedTags(
      block('directSuccessor', 'impl DirectRootPhaseV1 {'),
      emittedTag('directGenerated'),
    ),
    declared: declaredDiscriminants('directSuccessor', 'DirectRootPhaseV1', emittedTag('directGenerated')),
    variants: declaredVariants('directSuccessor', 'DirectRootPhaseV1'),
    authority: 'crates/dclutch-trading/src/{successor,generated_successor}.rs',
  },
  {
    label: 'dealer-root',
    record: 'RootTail',
    magic: magic('dealerProfileGenerated', 'ROOT_TAIL_MAGIC'),
    bytes: scalar('dealerProfileGenerated', 'ROOT_TAIL_BYTES'),
    header: [[scalar('dealerProfileGenerated', 'ROOT_TAIL_VERSION_OFFSET'), scalar('dealerProfileGenerated', 'ROOT_TAIL_ABI_VERSION')]],
    tagOffset: scalar('dealerProfileGenerated', 'ROOT_TAIL_PHASE_OFFSET'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    pdaDomain: null,
    discriminant: 'DealerRootPhaseV1',
    // `Phase::decode` resolves its arms through the Lean emission on purpose
    // (`root_admission_v1.rs` says why), so the arm heads are constant names
    // and the numbers come from the emitted module.
    states: decodedTags(
      block('dealerLib', 'impl Phase {'),
      emittedTag('dealerLiquidityGenerated'),
    ),
    declared: declaredDiscriminants('dealerLib', 'Phase'),
    variants: declaredVariants('dealerLib', 'Phase'),
    authority: 'crates/dclutch-trading/src/dealer/{lib,generated_dealer_liquidity,generated_dealer_trading_profile}.rs',
  },
  {
    label: 'dealer-checkpoint',
    record: 'DealerScenarioCheckpointV1',
    magic: magic('dealerCheckpointGenerated', 'DEALER_SCENARIO_CHECKPOINT_MAGIC_V1'),
    bytes: scalar('dealerCheckpointGenerated', 'DEALER_SCENARIO_CHECKPOINT_BYTES_V1'),
    header: [[
      scalar('dealerCheckpointGenerated', 'DEALER_SCENARIO_CHECKPOINT_VERSION_OFFSET_V1'),
      scalar('dealerCheckpointGenerated', 'DEALER_SCENARIO_CHECKPOINT_VERSION_V1'),
    ]],
    tagOffset: scalar('dealerCheckpointGenerated', 'DEALER_SCENARIO_CHECKPOINT_PHASE_OFFSET_V1'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    pdaDomain: null,
    discriminant: 'DealerScenarioCheckpointPhaseV1',
    states: decodedTags(
      block('dealerCheckpoint', 'impl DealerScenarioCheckpointPhaseV1 {'),
      emittedTag('dealerCheckpointGenerated'),
    ),
    declared: declaredDiscriminants(
      'dealerCheckpoint',
      'DealerScenarioCheckpointPhaseV1',
      emittedTag('dealerCheckpointGenerated'),
    ),
    variants: declaredVariants('dealerCheckpoint', 'DealerScenarioCheckpointPhaseV1'),
    authority: 'crates/dclutch-trading/src/dealer/{scenario_checkpoint_v1,generated_scenario_checkpoint_v1}.rs',
  },
  {
    label: 'dealer-reservation',
    record: 'DealerScenarioReservationStateV1',
    magic: magic('dealerReservationGenerated', 'DEALER_SCENARIO_RESERVATION_STATE_MAGIC_V1'),
    bytes: scalar('dealerReservationGenerated', 'DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1'),
    // This record's OWN version coordinate, not the four-record family header
    // constant that used to be read here. `VERSION_OFFSET` and `TAG_OFFSET` are
    // the spelling four records in that file share, and the regex that found
    // them takes the first.
    header: [[
      scalar('dealerReservationGenerated', 'DEALER_SCENARIO_RESERVATION_STATE_VERSION_OFFSET_V1'),
      scalar('dealerReservationGenerated', 'DEALER_SCENARIO_CUSTODY_STATE_VERSION_V1'),
    ]],
    tagOffset: scalar('dealerReservationGenerated', 'DEALER_SCENARIO_RESERVATION_STATE_STATUS_OFFSET_V1'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    pdaDomain: null,
    discriminant: 'DealerScenarioReservationStateStatusV1',
    states: decodedTags(
      block('dealerReservation', 'impl DealerScenarioReservationStateStatusV1 {'),
      emittedTag('dealerReservationGenerated'),
    ),
    declared: declaredDiscriminants(
      'dealerReservation',
      'DealerScenarioReservationStateStatusV1',
      emittedTag('dealerReservationGenerated'),
    ),
    variants: declaredVariants('dealerReservation', 'DealerScenarioReservationStateStatusV1'),
    authority: 'crates/dclutch-trading/src/dealer/{scenario_custody_reservation_v1,generated_scenario_reservation_state_v1}.rs',
  },
  {
    label: 'projected-custody',
    record: 'ProjectedCustodyStateV2',
    magic: magic('projectedCustodyGenerated', 'PROJECTED_CUSTODY_STATE_MAGIC_V2'),
    bytes: scalar('projectedCustodyGenerated', 'PROJECTED_CUSTODY_STATE_BYTES_V2'),
    header: [[
      scalar('projectedCustodyGenerated', 'PROJECTED_CUSTODY_STATE_VERSION_OFFSET_V2'),
      scalar('projectedCustodyGenerated', 'PROJECTED_CUSTODY_STATE_SCHEMA_VERSION_V2'),
    ]],
    // Emitted, where it used to be inferred from two bare expressions agreeing.
    tagOffset: scalar('projectedCustodyGenerated', 'PROJECTED_CUSTODY_STATE_PHASE_OFFSET_V2'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    pdaDomain: null,
    discriminant: 'ProjectedCustodyPhaseV1',
    // No `decode` method exists: the match is inline in the record's decoder,
    // which is why the anchor is the record's own statement rather than an
    // `impl`.
    states: decodedTags(
      block('projectedCustody', 'let phase = match read_u8(input,'),
      emittedTag('projectedCustodyGenerated'),
    ),
    declared: declaredDiscriminants(
      'projectedCustody',
      'ProjectedCustodyPhaseV1',
      emittedTag('projectedCustodyGenerated'),
    ),
    variants: declaredVariants('projectedCustody', 'ProjectedCustodyPhaseV1'),
    authority: 'crates/dclutch-custody/src/{projected,generated_projected_state_v2}.rs',
  },
  {
    label: 'series-ticket',
    record: 'TicketStateV3',
    magic: magic('seriesGenerated', 'SERIES_TICKET_STATE_MAGIC_V3'),
    bytes: scalar('seriesTicketGenerated', 'SERIES_TICKET_STATE_BYTES_V3'),
    // The two header words' VALUES are the Series family's and stay in
    // `replay.rs`; the coordinates they are written at belong to this record
    // and are emitted.
    header: [
      [scalar('seriesTicketGenerated', 'SERIES_TICKET_STATE_SCHEMA_OFFSET_V3'), scalar('seriesReplay', 'SCHEMA_V3')],
      [scalar('seriesTicketGenerated', 'SERIES_TICKET_STATE_PROFILE_OFFSET_V3'), scalar('seriesReplay', 'PROFILE_V3')],
    ],
    // Emitted, where it used to be inferred from two bare expressions agreeing
    // -- and one of the two belonged to `SeriesStateV3`, whose `encode` writes
    // the identical `output[12] = self.phase as u8;` line two hundred lines
    // earlier and which a first-match regex reads instead.
    tagOffset: scalar('seriesTicketGenerated', 'SERIES_TICKET_STATE_PHASE_OFFSET_V3'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    pdaDomain: null,
    discriminant: 'TicketPhaseV3',
    states: decodedTags(
      block('seriesReplay', 'impl TicketPhaseV3 {'),
      emittedTag('seriesTicketGenerated'),
    ),
    declared: declaredDiscriminants('seriesReplay', 'TicketPhaseV3', emittedTag('seriesTicketGenerated')),
    variants: declaredVariants('seriesReplay', 'TicketPhaseV3'),
    authority: 'crates/dclutch-trading/src/series/{replay,generated,generated_ticket_state_v3}.rs',
  },
  {
    label: 'funding-ledger',
    record: 'FundingLedgerV2',
    magic: magic('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_MAGIC_V2'),
    // The only per-ROW machine of the eight: one ledger account holds a slot
    // for every selected manifest entry, so the record has no single width and
    // the tag's address is `headerBytes + rowBytes * row + tagOffset`.
    bytes: null,
    header: [[scalar('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_SCHEMA_OFFSET_V2'), scalar('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_SCHEMA_VERSION_V2')]],
    tagOffset: scalar('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_SLOT_STATUS_OFFSET_V2'),
    rowBytes: scalar('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_SLOT_BYTES_V2'),
    headerBytes: scalar('fundingGenerated', 'CAPABILITY_FUNDING_LEDGER_HEADER_BYTES_V2'),
    counters: [],
    pdaDomain: null,
    discriminant: 'FundingLedgerStatusV2',
    states: decodedTags(
      block('funding', 'impl FundingLedgerStatusV2 {'),
      emittedTag('fundingGenerated'),
    ),
    declared: declaredDiscriminants('funding', 'FundingLedgerStatusV2', emittedTag('fundingGenerated')),
    variants: declaredVariants('funding', 'FundingLedgerStatusV2'),
    authority: 'crates/dclutch-market/src/capability_manifest/{funding,generated_abi}.rs',
  },
  {
    label: 'source',
    record: 'SourceResolutionStateV2',
    magic: magic('sourceGenerated', 'SOURCE_RESOLUTION_STATE_V2_MAGIC'),
    bytes: scalar('sourceGenerated', 'SOURCE_RESOLUTION_STATE_V2_BYTES'),
    header: [[scalar('sourceGenerated', 'SOURCE_RESOLUTION_STATE_V2_VERSION_OFFSET'), scalar('sourceGenerated', 'SOURCE_RESOLUTION_STATE_V2_SCHEMA_VERSION')]],
    tagOffset: scalar('sourceGenerated', 'SOURCE_RESOLUTION_STATE_V2_PHASE_OFFSET'),
    rowBytes: null,
    headerBytes: null,
    counters: [],
    // The one machine whose account address a reader holding only a Market can
    // derive: `[domain, market, generation]` under the Resolution program.
    pdaDomain: domain('sourceGenerated', 'SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2_GENERATED'),
    discriminant: 'SourceResolutionPhaseV1',
    states: decodedTags(
      block('source', 'impl SourceResolutionPhaseV1 {'),
      emittedTag('sourceGenerated'),
    ),
    declared: declaredDiscriminants('source', 'SourceResolutionPhaseV1', emittedTag('sourceGenerated')),
    variants: declaredVariants('source', 'SourceResolutionPhaseV1'),
    authority: 'crates/dclutch-source/src/{lib,generated_source_resolution_state_v2}.rs',
  },
];

// Every machine the census names except the Core Market's own phase, which is
// decoded by `marketCoreV2.ts` and is the thing these are all NOT.
const expected = censusMachines.filter((machine) => machine.label !== 'market');
for (const machine of expected) {
  const emitted = machines.find((candidate) => candidate.label === machine.label);
  if (emitted === undefined) throw new Error(`the census names machine ${machine.label} and nothing here decodes it`);
  if (emitted.discriminant !== machine.primary) {
    throw new Error(`${machine.label} is over ${machine.primary} in the census and ${emitted.discriminant} here`);
  }
  emitted.admission = machine.admission;
}
for (const machine of machines) {
  if (!expected.some((candidate) => candidate.label === machine.label)) {
    throw new Error(`${machine.label} is decoded here and the census names no such machine`);
  }
  // The cross-check the header promises: where an enum declares its own
  // discriminants, every one of them must be the byte its decoder admits, and
  // every variant must be decodable. An enum that grew a variant its decoder
  // refuses is a state a client would render as an unknown byte.
  const byVariant = new Map(machine.states);
  for (const variant of machine.variants) {
    if (!byVariant.has(variant)) throw new Error(`${machine.discriminant}::${variant} is declared and its decoder admits no byte for it`);
  }
  if (machine.declared !== null) {
    for (const [variant, tag] of machine.declared) {
      if (byVariant.get(variant) !== tag) {
        throw new Error(`${machine.discriminant}::${variant} declares ${tag} and decodes from ${byVariant.get(variant)}`);
      }
    }
  }
  if (machine.magic.length !== 8) throw new Error(`${machine.label} has a record magic that is not eight bytes`);
  if (machine.states.length === 0) throw new Error(`${machine.label} admits no state`);
}

const ts = (value) => `'${value}'`;
const pairs = (list) => `[${list.map(([left, right]) => `[${left}, ${right}]`).join(', ')}]`;

let output = '// @generated by scripts/generate-state-machines-v1.mjs; do not edit.\n';
output += '// Regenerate with: npm run abi:state-machines\n';
output += '//\n';
output += '// Machine labels and admission types: tools/gauntlet/census/src/phases.rs.\n';
for (const machine of machines) output += `// ${machine.label}: ${machine.authority}\n`;
output += '\n';
output += '/**\n';
output += ' * Every persisted state machine a route gate can be over, except the Market.\n';
output += ' *\n';
output += " * These are the labels `ROUTES_GATED_ON_ANOTHER_MACHINE_V1` names, read from\n";
output += ' * the census table that assigns them. The Core Market phase is deliberately\n';
output += ' * absent: it has its own decoder and its own snapshot field, and the whole\n';
output += ' * reason these carry separate types is that a Market phase can answer for\n';
output += ' * none of them.\n';
output += ' */\n';
output += `export type StateMachineV1 =\n${machines.map((machine) => `  | ${ts(machine.label)}`).join('\n')};\n\n`;
output += '/** One state of one machine, and the byte its own decoder admits it from. */\n';
output += 'export interface StateMachineStateV1 {\n';
output += '  readonly state: string;\n';
output += '  readonly tag: number;\n';
output += '}\n\n';
output += '/**\n';
output += ' * Where one machine keeps its discriminant, and what the bytes mean.\n';
output += ' *\n';
output += ' * `header` is the list of `[offset, value]` u16 words the record pins --\n';
output += ' * a schema version, and for the ticket state a profile beside it. `bytes` is\n';
output += ' * the exact record width, or `null` for the one machine that has none:\n';
output += ' * the funding ledger holds a slot per selected manifest entry, so its tag\n';
output += ' * sits at `headerBytes + rowBytes * row + tagOffset` and the record width is\n';
output += ' * a function of how many entries were selected.\n';
output += ' *\n';
output += ' * `bytes` is the width of the RECORD, which for the Direct root is the\n';
output += ' * 24-byte mutable tail and not the account: that tail follows the composite\n';
output += " * capability-root header, whose width is `directInlineV3.ts`'s own generated\n";
output += ' * `CAPABILITY_ROOT_HEADER_BYTES_V1`. Slicing the tail out is the caller\'s,\n';
output += ' * because the header is that module\'s fact and not this one\'s.\n';
output += ' */\n';
output += 'export interface StateMachineRecordV1 {\n';
output += '  readonly machine: StateMachineV1;\n';
output += '  /** The Rust admission type whose sets the census reads for this machine. */\n';
output += '  readonly admission: string;\n';
output += '  /** The Rust enum that owns the wire tags. */\n';
output += '  readonly discriminant: string;\n';
output += '  /** The Rust record that persists the discriminant. */\n';
output += '  readonly record: string;\n';
output += "  readonly magic: string;\n";
output += '  readonly bytes: number | null;\n';
output += '  readonly header: ReadonlyArray<readonly [offset: number, value: number]>;\n';
output += '  readonly tagOffset: number;\n';
output += '  readonly headerBytes: number | null;\n';
output += '  readonly rowBytes: number | null;\n';
output += '  /**\n';
output += '   * The PDA seed domain of the account, when a reader can derive it.\n';
output += '   *\n';
output += '   * `null` for seven of the eight, and that is a fact about SEED SHAPE\n';
output += '   * rather than about reach. The Source state is the one addressed by a\n';
output += '   * single domain -- `[domain, market, generation]` under the Resolution\n';
output += '   * program -- so it is the one this table can carry. Two of the other\n';
output += '   * seven are composite derivations over a capability manifest entry and\n';
output += "   * are perfectly reachable from a Market; `capabilityManifest.ts` owns\n";
output += '   * those, and reading a `null` here as "no client can find this account"\n';
output += '   * is what left a Direct root unread for as long as it was.\n';
output += '   */\n';
output += '  readonly pdaDomain: string | null;\n';
output += '  /** Other u64 lifecycle counters the record carries beside its tag. */\n';
output += '  readonly counters: ReadonlyArray<{ readonly field: string; readonly offset: number }>;\n';
output += '  readonly states: ReadonlyArray<StateMachineStateV1>;\n';
output += '  readonly authority: string;\n';
output += '}\n\n';
output += 'export const STATE_MACHINE_RECORDS_V1: ReadonlyArray<StateMachineRecordV1> = [\n';
for (const machine of machines) {
  output += `  {\n`;
  output += `    machine: ${ts(machine.label)},\n`;
  output += `    admission: ${ts(machine.admission)},\n`;
  output += `    discriminant: ${ts(machine.discriminant)},\n`;
  output += `    record: ${ts(machine.record)},\n`;
  output += `    magic: ${ts(machine.magic)},\n`;
  output += `    bytes: ${machine.bytes === null ? 'null' : machine.bytes},\n`;
  output += `    header: ${pairs(machine.header)},\n`;
  output += `    tagOffset: ${machine.tagOffset},\n`;
  output += `    headerBytes: ${machine.headerBytes === null ? 'null' : machine.headerBytes},\n`;
  output += `    rowBytes: ${machine.rowBytes === null ? 'null' : machine.rowBytes},\n`;
  output += `    pdaDomain: ${machine.pdaDomain === null ? 'null' : ts(machine.pdaDomain)},\n`;
  output += `    counters: [${machine.counters.map(([field, offset]) => `{ field: ${ts(field)}, offset: ${offset} }`).join(', ')}],\n`;
  output += `    states: [${machine.states.map(([state, tag]) => `{ state: ${ts(state)}, tag: ${tag} }`).join(', ')}],\n`;
  output += `    authority: ${ts(machine.authority)},\n`;
  output += `  },\n`;
}
output += '];\n\n';
output += '/** The record for one machine, or `null` when the name is not a machine. */\n';
output += 'export function stateMachineRecordV1(machine: string): StateMachineRecordV1 | null {\n';
output += '  return STATE_MACHINE_RECORDS_V1.find((entry) => entry.machine === machine) ?? null;\n';
output += '}\n\n';
output += '/** Every state name one machine has, in wire-tag order. */\n';
output += 'export function stateMachineStateNamesV1(machine: string): ReadonlyArray<string> {\n';
output += '  const record = stateMachineRecordV1(machine);\n';
output += '  if (record === null) return [];\n';
output += '  return [...record.states].sort((left, right) => left.tag - right.tag).map((entry) => entry.state);\n';
output += '}\n';

if (!output.startsWith('// @generated by scripts/generate-state-machines-v1.mjs; do not edit.\n')
    || !output.includes("machine: 'direct-root'")
    || !output.includes("{ state: 'Open', tag: 0 }, { state: 'Retiring', tag: 1 }")
    || !output.includes("machine: 'funding-ledger'")
    || !output.includes('rowBytes: 72,')
    || !output.includes("counters: [{ field: 'openMakerRootCount', offset: 16 }]")
    || !output.includes("pdaDomain: 'dclutch/source-state/v2'")) {
  throw new Error('generated state-machine table failed its header or literal validation');
}

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('state-machine TypeScript table is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporary = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, output);
    if (!readFileSync(temporary, 'utf8').startsWith('// @generated by scripts/generate-state-machines-v1.mjs; do not edit.\n')) {
      throw new Error('staged state-machine table failed validation');
    }
    renameSync(temporary, outputPath);
  } finally {
    try { unlinkSync(temporary); } catch { /* renamed or never written */ }
  }
}
