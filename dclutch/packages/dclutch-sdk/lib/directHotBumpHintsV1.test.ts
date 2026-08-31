import { readFileSync } from 'node:fs';

import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { activationCacheFixtureV1 } from '../fixtures/hotBumpHintSource';
import {
  HOT_BUMP_HINTS_ABSENT_V1,
  HOT_BUMP_HINTS_OFFSET_V1,
  HOT_BUMP_HINT_COUNT_V1,
  HOT_BUMP_HINT_SLOT_NAMES_V1,
  custodyProgramFromActivationCacheV1,
  decodeHotBumpHintsV1,
  directInlineHotBumpSeedSitesV3,
  encodeHotBumpHintsV1,
  hotBumpHintsAreAbsentV1,
  mineChildCallerAuthorityBumpV1,
  mineDirectInlineHotBumpHintsV3,
  type DirectHotBumpHintSourceV3,
} from './directHotBumpHintsV1';
import {
  CAPABILITY_ROOT_HEADER_BYTES_V1,
  HOT_EXECUTION_ENVELOPE_BYTES_V3,
} from './generated/directInlineV3';
import { CORE_STATE_BYTES } from './generated/coreFound';

/**
 * `fixtures/direct-hot-bump-hints.json` is emitted by
 * `crates/dclutch-operator/tests/browser_bump_hint_vector.rs` through the same
 * exported seed constructors `direct_inline_hot_bump_hints_v1` mines through.
 * The Rust crates are the authority: if a seed order moves, that test fails
 * before this one, and this one then names which slot the browser got wrong.
 */
const vector = JSON.parse(
  readFileSync(new URL('../fixtures/direct-hot-bump-hints.json', import.meta.url), 'utf8'),
) as Readonly<{
  format: string;
  coreProgram: string;
  tradingProgram: string;
  custodyProgram: string;
  market: string;
  generation: string;
  releaseSetHex: string;
  sellerMaker: string;
  buyerMaker: string;
  marketCoreStateHex: string;
  capabilityRootHeaderHex: string;
  buyerMakerReplay: string;
  childCaller: Readonly<{
    claimsContextHex: string;
    claimsRequestDigestHex: string;
    custodyRequestDigestHex: string;
  }>;
  hintBlockHex: string;
}>;

function bytes(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function hex(value: Uint8Array): string {
  return [...value].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

function source(): DirectHotBumpHintSourceV3 {
  return Object.freeze({
    coreProgram: vector.coreProgram,
    marketCoreState: bytes(vector.marketCoreStateHex),
    capabilityRootHeader: bytes(vector.capabilityRootHeaderHex),
    activationCache: activationCacheFixtureV1(bytes(vector.releaseSetHex), { custody: vector.custodyProgram }),
  });
}

function mine(overrides: Partial<Parameters<typeof mineDirectInlineHotBumpHintsV3>[0]> = {}) {
  return mineDirectInlineHotBumpHintsV3({
    source: source(),
    tradingProgram: vector.tradingProgram,
    market: vector.market,
    generation: BigInt(vector.generation),
    releaseSet: bytes(vector.releaseSetHex),
    sellerMaker: vector.sellerMaker,
    buyerMaker: vector.buyerMaker,
    childCaller: [
      mineChildCallerAuthorityBumpV1(
        vector.tradingProgram,
        bytes(vector.releaseSetHex),
        vector.market,
        bytes(vector.childCaller.claimsContextHex),
        bytes(vector.childCaller.claimsRequestDigestHex),
      ),
      mineChildCallerAuthorityBumpV1(
        vector.tradingProgram,
        bytes(vector.releaseSetHex),
        vector.market,
        new PublicKey(vector.buyerMakerReplay).toBytes(),
        bytes(vector.childCaller.custodyRequestDigestHex),
      ),
    ],
    ...overrides,
  });
}

describe('caller-mined hot bump hints', () => {
  it('reproduces the Rust seed constructors byte for byte, in every slot', () => {
    // The whole claim of this lane, stated as eight bytes. Both languages take
    // the same identities through the same seed order to the same block; a
    // browser that reconstructed any seed differently would name a different
    // address and the route would refuse rather than save anything.
    expect(vector.format).toBe('dclutch/direct-hot-bump-hints/v1');
    expect(hex(encodeHotBumpHintsV1(mine()))).toBe(vector.hintBlockHex);
    expect(bytes(vector.hintBlockHex)).toHaveLength(HOT_BUMP_HINT_COUNT_V1);
    // Zero is ABSENT, not a value. A slot that mined to zero would mean this
    // test was comparing two searches rather than two derivations.
    for (const [slot, byte] of [...bytes(vector.hintBlockHex)].entries()) {
      expect(byte, `slot ${slot} (${HOT_BUMP_HINT_SLOT_NAMES_V1[slot]}) mined to absent`).not.toBe(0);
    }
  });

  it('reaches the activated Custody deployment through the cache, not through a caller', () => {
    expect(custodyProgramFromActivationCacheV1(source().activationCache)).toBe(vector.custodyProgram);
    const foreign = activationCacheFixtureV1(bytes(vector.releaseSetHex), {});
    expect(custodyProgramFromActivationCacheV1(foreign)).not.toBe(vector.custodyProgram);
    // Both Custody relay slots move with the Custody program, so a cache that
    // named another deployment must not silently mine the same two bumps.
    const drifted = mine({ source: Object.freeze({ ...source(), activationCache: foreign }) });
    expect(drifted.childRelay).not.toEqual(mine().childRelay);
  });

  it('joins the mined lifecycle slots to the accounts the route frame carries', () => {
    // The lifecycle bumps are derived here from seeds and elsewhere from chain
    // state. Supplying the frame's addresses makes mining a joint check: a
    // disagreement is a hint that names an account the frame does not hold,
    // which Trading refuses on chain one signature later.
    const buyerReplay = vector.buyerMakerReplay;
    expect(() => mine({ expectedLifecycleAccounts: [vector.market, buyerReplay] }))
      .toThrow(/seller maker replay hint names another account/);
    expect(() => mine({ expectedLifecycleAccounts: [vector.market, vector.market] }))
      .toThrow(/names another account/);
  });

  it('refuses a block that is not exactly eight bytes, and a slot that is not one byte', () => {
    expect(() => decodeHotBumpHintsV1(bytes(vector.hintBlockHex).slice(0, 7))).toThrow(/not the exact 8/);
    expect(() => decodeHotBumpHintsV1(new Uint8Array(9))).toThrow(/not the exact 8/);
    expect(() => encodeHotBumpHintsV1({ ...HOT_BUMP_HINTS_ABSENT_V1, market: 256 })).toThrow(/not one byte/);
    expect(() => encodeHotBumpHintsV1({ ...HOT_BUMP_HINTS_ABSENT_V1, root: -1 })).toThrow(/not one byte/);
    expect(() => mine({ childCaller: [0, 300] })).toThrow(/not one byte/);
  });

  it('round-trips the block in canonical slot order and reads absence as absence', () => {
    const hints = mine();
    expect(decodeHotBumpHintsV1(encodeHotBumpHintsV1(hints))).toEqual(hints);
    expect(hotBumpHintsAreAbsentV1(HOT_BUMP_HINTS_ABSENT_V1)).toBe(true);
    expect(hotBumpHintsAreAbsentV1(hints)).toBe(false);
    expect(encodeHotBumpHintsV1(HOT_BUMP_HINTS_ABSENT_V1)).toEqual(new Uint8Array(HOT_BUMP_HINT_COUNT_V1));
    expect(HOT_BUMP_HINT_SLOT_NAMES_V1).toHaveLength(HOT_BUMP_HINT_COUNT_V1);
    // The block is the envelope's tail, which is why adding one grows no packet
    // and moves no digest.
    expect(HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1).toBe(HOT_EXECUTION_ENVELOPE_BYTES_V3);
  });

  it('refuses a Market state or capability root that is not its exact canonical body', () => {
    expect(bytes(vector.marketCoreStateHex)).toHaveLength(CORE_STATE_BYTES);
    expect(bytes(vector.capabilityRootHeaderHex)).toHaveLength(CAPABILITY_ROOT_HEADER_BYTES_V1);
    expect(() => mine({ source: Object.freeze({ ...source(), marketCoreState: new Uint8Array(CORE_STATE_BYTES - 1) }) }))
      .toThrow(/not the exact 368/);
    expect(() => mine({ source: Object.freeze({ ...source(), capabilityRootHeader: new Uint8Array(16) }) }))
      .toThrow(/shorter than its exact immutable header/);
    const wrongSelection = bytes(vector.capabilityRootHeaderHex);
    wrongSelection[88] ^= 1;
    expect(() => mine({ source: Object.freeze({ ...source(), capabilityRootHeader: wrongSelection }) }))
      .toThrow(/selection has the wrong canonical header/);
    expect(() => mine({ source: Object.freeze({ ...source(), activationCache: new Uint8Array(1_288) }) }))
      .toThrow(/activation cache has the wrong exact header/);
  });

  it('a deliberately wrong hint names a different address, slot by slot', () => {
    // The off-chain half of the refusal walk `direct_hot_bump_hints.rs` runs on
    // chain. A hint is safe because the PROGRAM rebuilds the seeds and compares
    // the reproduced address with the account its frame supplied -- so the fact
    // that makes the whole mechanism sound is that a wrong byte reproduces
    // SOMETHING ELSE. This asserts exactly that, per site, without a validator.
    //
    // The perturbation is the canonical bump MINUS ONE rather than a random
    // byte: it is the next candidate the search itself would have tried, and
    // therefore the value most likely to also be a valid program address rather
    // than an off-curve refusal. Both outcomes are correct -- an off-curve bump
    // throws where `create_program_address` would refuse, and a valid one names
    // an address the frame does not carry -- and neither may be the canonical
    // address.
    const sites = directInlineHotBumpSeedSitesV3({
      source: source(),
      tradingProgram: vector.tradingProgram,
      market: vector.market,
      generation: BigInt(vector.generation),
      releaseSet: bytes(vector.releaseSetHex),
      sellerMaker: vector.sellerMaker,
      buyerMaker: vector.buyerMaker,
    });
    // Six sites, not eight: the two child-caller slots are supplied, not derived.
    expect(sites.map((site) => site.slot)).toEqual([0, 1, 2, 3, 6, 7]);
    expect(new Set(sites.map((site) => site.address)).size).toBe(sites.length);
    const block = bytes(vector.hintBlockHex);
    for (const site of sites) {
      expect(site.bump, `${site.name} disagrees with the vector`).toBe(block[site.slot]);
      // The canonical bump reproduces the canonical address: this is the arm
      // the program takes when a hint is present, and it must be exact.
      expect(PublicKey.createProgramAddressSync(
        [...site.seeds, Uint8Array.of(site.bump)] as Uint8Array[], new PublicKey(site.programId),
      ).toBase58()).toBe(site.address);
      let reproduced: string | null = null;
      try {
        reproduced = PublicKey.createProgramAddressSync(
          [...site.seeds, Uint8Array.of(site.bump - 1)] as Uint8Array[], new PublicKey(site.programId),
        ).toBase58();
      } catch {
        reproduced = null;
      }
      expect(reproduced, `${site.name} reproduced its own address from a wrong bump`).not.toBe(site.address);
    }
  });

  it('moves every dependent slot when the identity it is derived from moves', () => {
    // A miner that ignored an input would still reproduce the vector while
    // being wrong for every other trade. One perturbation per seed coordinate,
    // each asserted to move the slots it feeds and only those.
    const canonical = mine();
    const otherMaker = new PublicKey(new Uint8Array(32).fill(0x77)).toBase58();
    const swappedMakers = mine({ sellerMaker: vector.buyerMaker, buyerMaker: vector.sellerMaker });
    expect(swappedMakers.lifecycle).toEqual([canonical.lifecycle[1], canonical.lifecycle[0]]);
    // The buyer is the Custody replay's context, so swapping the pair moves the
    // relay too -- which is why the slot order is the lifecycle order and not a
    // convention.
    expect(swappedMakers.childRelay[0]).not.toBe(canonical.childRelay[0]);
    expect(mine({ buyerMaker: otherMaker }).childRelay[1]).toBe(canonical.childRelay[1]);
    expect(mine({ generation: BigInt(vector.generation) + 1n }).lifecycle).not.toEqual(canonical.lifecycle);
    const otherRelease = new Uint8Array(32).fill(0x66);
    const moved = mine({ releaseSet: otherRelease });
    expect(moved.childRelay).not.toEqual(canonical.childRelay);
    expect(moved.market).toBe(canonical.market);
  });
});
