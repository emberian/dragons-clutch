import { readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  route: readFileSync(new URL('programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs', root), 'utf8'),
  codec: readFileSync(new URL('crates/dclutch-market-core-codec/src/generic_founding_v1.rs', root), 'utf8'),
  projected: readFileSync(new URL('crates/dclutch-custody-contract/src/projected.rs', root), 'utf8'),
  claims: readFileSync(new URL('crates/dclutch-claims-svm/src/founding_v5.rs', root), 'utf8'),
  client: readFileSync(new URL('tools/local-validator/bootstrap/successor/src/market.rs', root), 'utf8'),
  clientRpc: readFileSync(new URL('tools/local-validator/bootstrap/successor/src/rpc.rs', root), 'utf8'),
  coreFrame: readFileSync(new URL('programs/dclutch-core-sbf/src/frame.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/genericFoundingV1.ts', import.meta.url);

/**
 * Read one integer `const` out of a Rust source.
 *
 * Several of these constants are *defined in terms of each other* rather than
 * as literals -- `PREFIX_ACCOUNT_COUNT = INSTRUCTIONS_SYSVAR_INDEX + 1` is the
 * whole reason the sysvar sits where it does. Transcribing the resolved number
 * here would erase that relationship and let the two drift apart silently, so
 * the emitter evaluates the same expression Rust does, over the same file.
 * Anything more complicated than a chain of `NAME` and `+ literal` refuses.
 */
function scalar(source, name, seen = new Set()) {
  if (seen.has(`${source}.${name}`)) throw new Error(`cyclic Rust const ${source}.${name}`);
  seen.add(`${source}.${name}`);
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: (?:usize|u8|u16|u32|u64|i64) =\\s*([^;]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  const expression = match[1].trim();
  // Sum of products of literals and other named constants. `256 * 1024` is a
  // size written the way a human reads it, and evaluating it here keeps the
  // emitted number identical to the Rust rather than rounding the intent into
  // a magic 262144. Anything outside that grammar refuses.
  return expression.split('+').reduce((total, term) => (
    total + term.split('*').reduce((product, factor) => {
      const token = factor.trim();
      if (/^[0-9_]+$/.test(token)) return product * Number(token.replaceAll('_', ''));
      if (/^[A-Z][A-Z0-9_]*$/.test(token)) return product * scalar(source, token, seen);
      throw new Error(`unparsable Rust const ${source}.${name}: ${expression}`);
    }, 1)
  ), 0);
}

/** Read `const NAME: [u8; N] = *b"TEXT";`. */
function magic(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; [0-9]+\\] = \\*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust magic ${source}.${name}`);
  return match[1];
}

/**
 * Read the offset an encoder actually writes a magic at.
 *
 * `exact_header` checks the magic at the front, and `encode` writes it with a
 * literal `put(&mut output, 0, &MAGIC)`. Zero is the obvious answer and that is
 * exactly why it should be read rather than assumed: a decoder that hard-codes
 * a coordinate the encoder owns is the drift `abi-coverage` exists to refuse.
 */
function magicOffset(source, name) {
  const match = sources[source].match(new RegExp(`put\\(\\s*&mut output,\\s*([0-9]+),\\s*&${name}\\)`));
  if (!match) throw new Error(`missing Rust magic write for ${source}.${name}`);
  return Number(match[1]);
}

/** Read `const NAME: &[u8] = b"TEXT";`, which the emitters wrap across lines. */
function domain(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: &\\[u8\\] =\\s*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust domain ${source}.${name}`);
  return match[1];
}

/**
 * Rust visibility, which this emitter reads past rather than depending on.
 *
 * How widely a constant is exported says nothing about the wire. When the
 * founding route was split into two outers the raw-request indices had to
 * become `pub(crate)` so the new stage module could import them, and every
 * emitter that matched a bare `const` stopped finding constants whose value had
 * not changed by a single byte. An ABI mirror must key off the name and the
 * value, never the keyword in front of them.
 */
const VISIBILITY = '(?:pub(?:\\((?:crate|super|self|in [a-z_:]+)\\))? )?';

/** Resolve a plain `const NAME: usize = <decimal>;` from either Rust source. */
function namedUsize(name) {
  for (const key of ['route', 'codec']) {
    const match = sources[key].match(new RegExp(`\\n${VISIBILITY}const ${name}: usize = ([0-9]+);`));
    if (match) return Number(match[1]);
  }
  throw new Error(`missing Rust usize constant ${name}`);
}

/**
 * Read a `const NAME: usize = …;` inside the route module.
 *
 * Literals first, then the one derived form the route uses: `NAMED ± k`, where
 * NAMED is a frame count the codec publishes. Route indices started as bare
 * numbers and are becoming derivations — `CORE_FOUND_TRADING_PROGRAM` is
 * `GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 - 2` — which is the better
 * way to write them, so this reads them rather than asking Rust to go back to
 * transcribed numbers. Same shape as `codecOffset` below, for the same reason:
 * evaluating the expression keeps the emitter honest about which Rust it is
 * reading, and `expected` refuses a silent drift if the form ever outgrows
 * this parser.
 */
function routeIndex(name, expected) {
  const literal = sources.route.match(new RegExp(`\\n${VISIBILITY}const ${name}: usize = ([0-9]+);`));
  if (literal) return Number(literal[1]);
  const expression = sources.route.match(new RegExp(`\\n${VISIBILITY}const ${name}: usize = ([^;]+);`));
  if (!expression) throw new Error(`missing Rust route index ${name}`);
  const derived = expression[1].trim().match(/^([A-Z0-9_]+) ([+-]) ([0-9]+)$/);
  if (!derived) throw new Error(`unparsable Rust route index ${name}: ${expression[1].trim()}`);
  const base = namedUsize(derived[1]);
  const value = derived[2] === '+' ? base + Number(derived[3]) : base - Number(derived[3]);
  if (expected !== undefined && value !== expected) throw new Error(`${name} evaluated to ${value}, not ${expected}`);
  return value;
}

/** Read the codec's private layout offsets, which are expressions as often as literals. */
function codecOffset(name, expected) {
  const literal = sources.codec.match(new RegExp(`\\nconst ${name}: usize = ([0-9]+);`));
  if (literal) return Number(literal[1]);
  const expression = sources.codec.match(new RegExp(`\\nconst ${name}: usize = ([^;]+);`));
  if (!expression) throw new Error(`missing Rust codec offset ${name}`);
  // The only non-literal form in this file is `BASE + k * 32`. Evaluating it
  // here rather than transcribing the number keeps the emitter honest about
  // which Rust expression it is reading, and `expected` refuses a silent drift
  // if that expression ever becomes something this parser cannot evaluate.
  const derived = expression[1].match(/^([A-Z_]+) \+ ([0-9]+) \* ([0-9]+)$/);
  if (!derived) throw new Error(`unparsable Rust codec offset ${name}: ${expression[1]}`);
  const value = codecOffset(derived[1]) + Number(derived[2]) * Number(derived[3]);
  if (expected !== undefined && value !== expected) throw new Error(`${name} evaluated to ${value}, not ${expected}`);
  return value;
}

/**
 * Read the stage discriminants out of the `GenericFoundingStageV1` enum.
 *
 * The enum carries explicit `= 1` / `= 2` discriminants and the route encodes
 * `self.stage as u8`, so these are wire values and not internal ordering.
 */
function stages() {
  const block = sources.codec.match(/pub enum GenericFoundingStageV1 \{([\s\S]*?)\n\}/);
  if (!block) throw new Error('missing Rust GenericFoundingStageV1 enumeration');
  const found = [...block[1].matchAll(/\n\s*([A-Za-z0-9]+) = ([0-9]+),/g)].map((entry) => [entry[1], Number(entry[2])]);
  if (found.length === 0) throw new Error('GenericFoundingStageV1 declared no explicit discriminants');
  return found;
}

const request = {
  identities: codecOffset('REQUEST_IDENTITIES_OFFSET', 16),
  capabilityRoot: codecOffset('REQUEST_CAPABILITY_ROOT_OFFSET', 80),
  generation: codecOffset('REQUEST_GENERATION_OFFSET', 336),
  entryIndex: codecOffset('REQUEST_ENTRY_INDEX_OFFSET', 392),
  tailReserved: codecOffset('REQUEST_TAIL_RESERVED_OFFSET', 394),
  tailReservedBytes: codecOffset('REQUEST_TAIL_RESERVED_BYTES', 6),
};

const lines = [];
lines.push('// @generated by scripts/generate-generic-founding.mjs; do not edit.');
lines.push('// Regenerate with: npm run abi:generic-founding');
lines.push('//');
lines.push('// Sources, in the order the wire is assembled:');
lines.push('//   programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs  (the route)');
lines.push('//   crates/dclutch-market-core-codec/src/generic_founding_v1.rs     (the request)');
lines.push('//   crates/dclutch-custody-contract/src/projected.rs                (Lock, Realize)');
lines.push('//   crates/dclutch-claims-svm/src/founding_v5.rs                    (Claims)');
lines.push('//   tools/local-validator/bootstrap/successor/src/market.rs         (the frame width)');
lines.push('');
lines.push('/** Sole top-level DCLTGMF3 discriminator followed by five invocation-evidence bumps. */');
lines.push(`export const GENERIC_MARKET_FOUNDING_MAGIC_V3 = '${magic('route', 'GENERIC_MARKET_FOUNDING_MAGIC_V3')}' as const;`);
lines.push(`export const GENERIC_MARKET_FOUNDING_CALLER_BUMP_COUNT_V3 = ${scalar('route', 'GENERIC_MARKET_FOUNDING_CALLER_BUMP_COUNT_V3')} as const;`);
lines.push(`export const GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3 = ${scalar('route', 'GENERIC_MARKET_FOUNDING_INSTRUCTION_BYTES_V3')} as const;`);
lines.push(`export const GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V3 = ${scalar('route', 'GENERIC_MARKET_FOUNDING_RAW_ACCOUNT_COUNT_V3')} as const;`);
lines.push(`export const GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V3 = ${scalar('route', 'GENERIC_MARKET_FOUNDING_INSTRUCTIONS_SYSVAR_INDEX_V3')} as const;`);
lines.push(`export const GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3 = ${scalar('route', 'GENERIC_MARKET_FOUNDING_PREFIX_ACCOUNT_COUNT_V3')} as const;`);
lines.push('');
lines.push('/** Index of each readonly request account inside the frame prefix. */');
for (const [name, index] of [['FOUND', routeIndex('FOUND_RAW')], ['LOCK', routeIndex('LOCK_RAW')], ['REALIZE', routeIndex('REALIZE_RAW')], ['CLAIMS', routeIndex('CLAIMS_RAW')]]) {
  lines.push(`export const GENERIC_FOUNDING_${name}_RAW_INDEX_V1 = ${index} as const;`);
}
lines.push('');
lines.push('/** Stage account widths. The frame is exactly the concatenation of these. */');
lines.push(`export const PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1 = ${scalar('projected', 'PROJECTED_CUSTODY_LOCK_CLOSE_ACCOUNT_COUNT_V1')} as const;`);
lines.push(`export const PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1 = ${scalar('projected', 'PROJECTED_CUSTODY_REALIZE_ACCOUNT_COUNT_V1')} as const;`);
lines.push(`export const GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 = ${scalar('codec', 'GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1')} as const;`);
lines.push(`export const GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1 = ${scalar('codec', 'GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1')} as const;`);
lines.push(`export const CLAIMS_FOUNDING_ACCOUNT_COUNT_V6 = ${scalar('claims', 'CLAIMS_FOUNDING_ACCOUNT_COUNT_V6')} as const;`);
lines.push(`export const GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1 = ${scalar('codec', 'GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1')} as const;`);
lines.push('');
lines.push('/**');
lines.push(' * Frame width at `funding_count = 0`, restated by the reference client.');
lines.push(' *');
lines.push(' * The client pins it as one number so a stage-width change shows up as a');
lines.push(' * disagreement rather than as a silently wider frame; the assertion that the');
lines.push(' * six widths above sum to it lives in `genericMarketFounding.test.ts`.');
lines.push(' */');
lines.push(`export const GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 = ${scalar('client', 'GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3')} as const;`);
lines.push(`export const PROJECTED_FOUND_ACCOUNT_COUNT_V2 = ${scalar('coreFrame', 'PROJECTED_FOUND_ACCOUNT_COUNT_V2')} as const;`);
lines.push('');
lines.push('/** Exact distinct writable keys the outer requires, asserted by the client. */');
lines.push(`export const GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3 = ${scalar('client', 'GENERIC_MARKET_FOUNDING_DISTINCT_WRITABLE_V3')} as const;`);
lines.push('');
lines.push('/** Devnet transaction account-lock limit without the raise feature. */');
lines.push('export const MAX_TX_ACCOUNT_LOCKS_V2 = 64 as const;');
lines.push('');
lines.push('/** Indexes into the Found stage that the route itself dereferences. */');
lines.push(`export const CORE_FOUND_MARKET_INDEX_V1 = ${routeIndex('CORE_FOUND_MARKET')} as const;`);
lines.push(`export const CORE_FOUND_CORE_PROGRAM_INDEX_V1 = ${routeIndex('CORE_FOUND_CORE_PROGRAM')} as const;`);
lines.push(`export const CORE_FOUND_TRADING_PROGRAM_INDEX_V1 = ${routeIndex('CORE_FOUND_TRADING_PROGRAM')} as const;`);
lines.push(`export const CORE_FOUND_PERMIT_SUFFIX_INDEX_V1 = ${routeIndex('CORE_FOUND_PERMIT_SUFFIX')} as const;`);
lines.push(`export const CORE_FOUND_CLAIMS_PROGRAM_SUFFIX_INDEX_V1 = ${routeIndex('CORE_FOUND_CLAIMS_PROGRAM_SUFFIX')} as const;`);
lines.push(`export const CORE_FOUND_CUSTODY_PROGRAM_SUFFIX_INDEX_V1 = ${routeIndex('CORE_FOUND_CUSTODY_PROGRAM_SUFFIX')} as const;`);
lines.push('');
lines.push('/** The four readonly request bodies and the return acknowledgement. */');
lines.push(`export const GENERIC_FOUNDING_REQUEST_MAGIC_V1 = '${magic('codec', 'GENERIC_FOUNDING_REQUEST_MAGIC_V1')}' as const;`);
lines.push(`export const GENERIC_FOUNDING_ACK_MAGIC_V1 = '${magic('codec', 'GENERIC_FOUNDING_ACK_MAGIC_V1')}' as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_BYTES_V1 = ${scalar('codec', 'GENERIC_FOUNDING_REQUEST_BYTES_V1')} as const;`);
lines.push(`export const GENERIC_FOUNDING_ACK_BYTES_V1 = ${scalar('codec', 'GENERIC_FOUNDING_ACK_BYTES_V1')} as const;`);
lines.push(`export const PROJECTED_CUSTODY_REQUEST_BYTES_V1 = ${scalar('projected', 'PROJECTED_CUSTODY_REQUEST_BYTES_V1')} as const;`);
lines.push(`export const CLAIMS_FOUNDING_REQUEST_BYTES_V5 = ${scalar('claims', 'CLAIMS_FOUNDING_REQUEST_BYTES_V5')} as const;`);
lines.push(`export const GENERIC_FOUNDING_MAX_FUNDING_STATES_V1 = ${scalar('codec', 'GENERIC_FOUNDING_MAX_FUNDING_STATES_V1')} as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_MAGIC_OFFSET_V1 = ${magicOffset('codec', 'GENERIC_FOUNDING_REQUEST_MAGIC_V1')} as const;`);
lines.push(`export const GENERIC_FOUNDING_ACK_MAGIC_OFFSET_V1 = ${magicOffset('codec', 'GENERIC_FOUNDING_ACK_MAGIC_V1')} as const;`);
lines.push('');
lines.push('/** `GenericFoundingRequestV1` byte layout. */');
lines.push(`export const GENERIC_FOUNDING_REQUEST_VERSION_V1 = ${scalar('codec', 'VERSION_V1')} as const;`);
lines.push('export const GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1 = 8 as const;');
lines.push('export const GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1 = 10 as const;');
lines.push('export const GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1 = 11 as const;');
lines.push('export const GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1 = 12 as const;');
lines.push('export const GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_BYTES_V1 = 4 as const;');
lines.push(`export const GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1 = ${request.identities} as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1 = ${request.generation} as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1 = ${request.entryIndex} as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1 = ${request.tailReserved} as const;`);
lines.push(`export const GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_BYTES_V1 = ${request.tailReservedBytes} as const;`);
lines.push(`export const GENERIC_FOUNDING_ACK_IDENTITIES_OFFSET_V1 = ${codecOffset('ACK_IDENTITIES_OFFSET', 16)} as const;`);
lines.push(`export const GENERIC_FOUNDING_ACK_SCALARS_OFFSET_V1 = ${codecOffset('ACK_GENERATION_OFFSET', 240)} as const;`);
lines.push('');
lines.push('/** The ten request identities, in their exact encoded order. */');
lines.push('export const GENERIC_FOUNDING_REQUEST_IDENTITIES_V1 = Object.freeze([');
for (const name of ['releaseSet', 'market', 'capabilityRoot', 'context', 'founder', 'beneficiary', 'fundingSource', 'hoard', 'projectedReplay', 'fundingListId']) {
  lines.push(`  '${name}',`);
}
lines.push('] as const);');
lines.push('');
lines.push('/** The seven request scalars, in their exact encoded order. */');
lines.push('export const GENERIC_FOUNDING_REQUEST_SCALARS_V1 = Object.freeze([');
for (const name of ['generation', 'quantity', 'basisScale', 'expirySlot', 'marketRent', 'permitRent', 'projectedResultingRevision']) {
  lines.push(`  '${name}',`);
}
lines.push('] as const);');
lines.push('');
lines.push('/** Wire discriminants of `GenericFoundingStageV1`. */');
lines.push('export const GENERIC_FOUNDING_STAGES_V1 = Object.freeze([');
for (const [name, value] of stages()) {
  lines.push(`  Object.freeze({ name: '${name}', tag: ${value} }),`);
}
lines.push('] as const);');
lines.push('');
lines.push('/**');
lines.push(' * ComputeBudget declarations the reference client puts on every transaction.');
lines.push(' *');
lines.push(' * `bounded_instructions` owns these and refuses a caller-supplied duplicate.');
lines.push(' * The configured limit is part of the emitted bytes, not a performance claim.');
lines.push(' * Current V2 routes require their own pass-count and 20-seed mean evidence.');
lines.push(' */');
lines.push(`export const SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1 = ${scalar('clientRpc', 'SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT')} as const;`);
lines.push(`export const REQUEST_HEAP_FRAME_DISCRIMINANT_V1 = ${scalar('clientRpc', 'REQUEST_HEAP_FRAME_DISCRIMINANT')} as const;`);
lines.push(`export const LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1 = ${scalar('clientRpc', 'LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT')} as const;`);
lines.push(`export const FOUNDING_HEAP_FRAME_BYTES_V1 = ${scalar('clientRpc', 'FOUNDING_HEAP_FRAME_BYTES')} as const;`);
lines.push('');
lines.push('/** Preimage domain of the ordered FundingState address list. */');
lines.push(`export const GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1 = new TextEncoder().encode('${domain('codec', 'GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1')}');`);
lines.push('');

const emitted = `${lines.join('\n')}`;
if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== emitted) {
    process.stderr.write('lib/generated/genericFoundingV1.ts is stale; run npm run abi:generic-founding\n');
    process.exit(1);
  }
  process.stdout.write('lib/generated/genericFoundingV1.ts matches its Rust sources\n');
} else {
  const temporaryUrl = new URL(`../lib/generated/.genericFoundingV1.ts.${process.pid}.tmp`, import.meta.url);
  try {
    writeFileSync(temporaryUrl, emitted, { flag: 'wx' });
    const candidate = readFileSync(temporaryUrl, 'utf8');
    if (!candidate.startsWith('// @generated by scripts/generate-generic-founding.mjs; do not edit.\n')
        || !candidate.includes('export const GENERIC_MARKET_FOUNDING_FIXED_ACCOUNTS_V3 = 129 as const;')) {
      throw new Error('generated generic-founding ABI failed header/width validation');
    }
    renameSync(temporaryUrl, outputUrl);
  } finally {
    rmSync(temporaryUrl, { force: true });
  }
  process.stdout.write('wrote lib/generated/genericFoundingV1.ts\n');
}
