import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  definesConstant,
  followToDefinition,
  modulePathToSource,
  requireGeneratorFollowsRoute,
  requireRouteConjunct,
  resolveUseBinding,
  parseUseTrees,
} from './route-binding.mjs';

const ROOT = new URL('../../../', import.meta.url);
const ROUTE_FILE = 'crates/dclutch-direct-codec/src/artifacts_v4.rs';
const ROUTE_CRATE = 'dclutch_direct_codec';

const readSource = (file: string): string => readFileSync(new URL(file, ROOT), 'utf8');
const ROUTE_TEXT = readSource(ROUTE_FILE);

const EFFECT_BINDING = {
  routeName: 'EFFECT_SCHEMA_ID_V4',
  conjunct: 'descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4',
  sourceFile: 'crates/dclutch-effect-kernel/src/v4.rs',
  sourceConstant: 'SCHEMA_RELEASE_ID_V4',
};

const gate = (routeText: string, binding = EFFECT_BINDING): void => {
  requireGeneratorFollowsRoute({ routeText, routeCrate: ROUTE_CRATE, readSource, binding });
};

describe('use-tree resolution', () => {
  it('reads an alias out of a nested use-tree', () => {
    const text = [
      'use dclutch_effect_kernel::{',
      '    v2::FixedRole,',
      '    v3::ProgramV3 as EffectProgramV3,',
      '    v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4},',
      '};',
    ].join('\n');
    expect(resolveUseBinding(text, 'EFFECT_SCHEMA_ID_V4'))
      .toBe('dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4');
    expect(resolveUseBinding(text, 'EffectProgramV3'))
      .toBe('dclutch_effect_kernel::v3::ProgramV3');
    expect(resolveUseBinding(text, 'FixedRole'))
      .toBe('dclutch_effect_kernel::v2::FixedRole');
    expect(resolveUseBinding(text, 'SCHEMA_RELEASE_ID_V4')).toBeNull();
  });

  it('reads the real route the same way', () => {
    expect(resolveUseBinding(ROUTE_TEXT, 'EFFECT_SCHEMA_ID_V4'))
      .toBe('dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4');
    expect(resolveUseBinding(ROUTE_TEXT, 'ACCOUNT_PROFILE_SCHEMA_ID_V2'))
      .toBe('dclutch_account_profile_contract::v2::SCHEMA_RELEASE_ID');
    expect(resolveUseBinding(ROUTE_TEXT, 'SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5'))
      .toBe('dclutch_capability_program_contract::v4::SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5');
  });

  it('takes `pub use` re-exports and ignores commented-out ones', () => {
    expect(resolveUseBinding('pub use a::b::C as D;', 'D')).toBe('a::b::C');
    expect(resolveUseBinding('// use a::b::C as D;', 'D')).toBeNull();
    expect(resolveUseBinding('//! use a::b::C as D;', 'D')).toBeNull();
  });

  it('resolves `self` and drops globs, which bind no followable name', () => {
    expect(resolveUseBinding('use a::b::{self, C};', 'b')).toBe('a::b');
    expect(parseUseTrees('use a::b::*;')).toEqual([]);
  });

  it('maps a module path to this repo\'s file layout', () => {
    expect(modulePathToSource('dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4', ROUTE_CRATE))
      .toMatchObject({ file: 'crates/dclutch-effect-kernel/src/v4.rs', constant: 'SCHEMA_RELEASE_ID_V4' });
    // `crate::` is the route's own crate, not a crate literally named `crate`.
    expect(modulePathToSource('crate::successor::DIRECT_ROOT_SCHEMA_ID_V1', ROUTE_CRATE))
      .toMatchObject({ file: 'crates/dclutch-direct-codec/src/successor.rs' });
    expect(modulePathToSource('some_crate::CONST', ROUTE_CRATE))
      .toMatchObject({ file: 'crates/some-crate/src/lib.rs', constant: 'CONST' });
  });

  it('tells a definition from a re-export', () => {
    expect(definesConstant('pub const X: [u8; 32] = [];', 'X')).toBe(true);
    expect(definesConstant('pub use a::X;', 'X')).toBe(false);
  });

  // Two real chains in this repo state their constant through
  // `mod generated { include!("…") } pub use generated::*;`. Before the walker
  // understood `include!` these reported "neither defines nor re-exports",
  // which reads as a defect in the source rather than a gap in the walker.
  it('follows an include!-stated generated module to the file that defines it', () => {
    expect(followToDefinition(
      'dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3',
      ROUTE_CRATE, readSource,
    )).toMatchObject({
      file: 'crates/dclutch-product-payoff-v2-codec/src/generated_admission_v3.rs',
      constant: 'GRADED_BASIS_RECORD_SCHEMA_ID_V3',
    });
  });

  it('follows a re-export whose target is itself include!-stated', () => {
    // source-contract re-exports from principal_capacity_v1, which states the
    // constant through an include! -- a `pub use` hop and an include! hop in
    // the same chain.
    expect(followToDefinition(
      'dclutch_source_contract::MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1',
      ROUTE_CRATE, readSource,
    ).constant).toBe('MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1');
  });

  it('follows a real two-hop re-export to the defining file', () => {
    // The route names the capability contract's v4; v4.rs `pub use`s it from
    // lifecycle_v3. Both hops are read from actual source.
    expect(followToDefinition(
      'dclutch_capability_program_contract::v4::SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5',
      ROUTE_CRATE, readSource,
    )).toMatchObject({
      file: 'crates/dclutch-account-profile-contract/src/lifecycle_v3.rs',
      constant: 'CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5',
    });
  });
});

describe('the route-binding gate', () => {
  it('passes against the route text that ships today', () => {
    expect(() => gate(ROUTE_TEXT)).not.toThrow();
  });

  // RED PROOF (a): the exact conviction. The route moves its effect binding to
  // another generation while keeping the alias; the generator keeps scraping
  // v4.rs. The byte gate would stay green -- this must not.
  it('reds when the route rebinds the alias to another generation', () => {
    const doctored = ROUTE_TEXT.replace(
      'v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4}',
      'v5::{ProgramV5 as EffectProgramV4, SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_ID_V4}',
    );
    expect(doctored).not.toEqual(ROUTE_TEXT);
    expect(() => gate(doctored)).toThrow(/dclutch-effect-kernel\/src\/v5\.rs|cannot read/);
  });

  // The subtler shape of the same defect: the route stays on the effect kernel
  // but points the alias at v3.rs, whose preimage misleadingly reads
  // `effect-program-v4-...`. This is the file the generator actually scraped
  // when the bug shipped.
  it('reds when the route rebinds the alias to the misleadingly-named v3', () => {
    const doctored = ROUTE_TEXT.replace(
      'v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4}',
      'v3::{ProgramV3 as EffectProgramV4, SCHEMA_RELEASE_ID as EFFECT_SCHEMA_ID_V4}',
    );
    expect(doctored).not.toEqual(ROUTE_TEXT);
    expect(() => gate(doctored)).toThrow(/scrapes SCHEMA_RELEASE_ID_V4/);
    expect(() => gate(doctored)).toThrow(/v3\.rs/);
  });

  // RED PROOF (b): the conjunct disappears. A binding check over a conjunct
  // the route no longer evaluates proves nothing, so the gate must not quietly
  // keep passing.
  it('reds when the authentication conjunct is gone', () => {
    const doctored = ROUTE_TEXT.replace(
      'descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4',
      'descriptor.effect().schema().to_bytes() != descriptor.effect().schema().to_bytes()',
    );
    expect(doctored).not.toEqual(ROUTE_TEXT);
    expect(() => gate(doctored)).toThrow(/no longer contains the authentication conjunct/);
  });

  it('reds when the route stops binding the name at all', () => {
    const doctored = ROUTE_TEXT.replace(
      'SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4',
      'SCHEMA_RELEASE_ID_V4',
    );
    expect(doctored).not.toEqual(ROUTE_TEXT);
    expect(() => gate(doctored)).toThrow(/does not bind EFFECT_SCHEMA_ID_V4/);
  });

  // The gate must survive rustfmt rewrapping the conjunct across lines, or it
  // becomes a tripwire people delete rather than a check they trust.
  it('is insensitive to how rustfmt wraps the conjunct', () => {
    expect(() => requireRouteConjunct(
      'a\n    || descriptor.effect().schema().to_bytes()\n        != EFFECT_SCHEMA_ID_V4\n',
      'descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4',
    )).not.toThrow();
  });

  // A generator pointed at the right file but the wrong constant in it is the
  // same class of defect, and reds the same way.
  it('reds when the generator names another constant in the right file', () => {
    expect(() => gate(ROUTE_TEXT, { ...EFFECT_BINDING, sourceConstant: 'SCHEMA_RELEASE_ID' }))
      .toThrow(/scrapes SCHEMA_RELEASE_ID from/);
  });
});
