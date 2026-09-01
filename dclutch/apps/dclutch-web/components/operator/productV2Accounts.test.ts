import { describe, expect, it } from 'vitest';
import { PublicKey } from '@solana/web3.js';

import { deriveProductV2AccountsV1, effectiveAccountV1 } from './productV2Accounts';
import { buildAdmissionInstructionV2 } from '@/lib/productRuntimeV2Admission';
import { fromHex } from '@/lib/bytes';

const REGISTRY = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
const DIGESTS = {
  product: 'a1'.repeat(32),
  domain: 'b2'.repeat(32),
  portfolio: 'c3'.repeat(32),
} as const;

describe('deriveProductV2AccountsV1', () => {
  it('derives all six accounts from the Registry and three digests alone', () => {
    const derived = deriveProductV2AccountsV1(REGISTRY, DIGESTS);
    expect(derived).not.toBeNull();
    if (derived === null) throw new Error('unreachable');
    const addresses = Object.values(derived);
    expect(addresses).toHaveLength(6);
    // Six distinct addresses -- the program refuses any duplicate.
    expect(new Set(addresses).size).toBe(6);
    for (const address of addresses) {
      expect(new PublicKey(address).toBase58()).toBe(address);
    }
  });

  it('is deterministic, so the same form always composes the same frame', () => {
    expect(deriveProductV2AccountsV1(REGISTRY, DIGESTS))
      .toEqual(deriveProductV2AccountsV1(REGISTRY, DIGESTS));
  });

  it('moves every account when the Registry changes', () => {
    const other = new PublicKey(new Uint8Array(32).fill(8)).toBase58();
    const a = deriveProductV2AccountsV1(REGISTRY, DIGESTS);
    const b = deriveProductV2AccountsV1(other, DIGESTS);
    if (a === null || b === null) throw new Error('expected both to derive');
    for (const slot of Object.keys(a) as Array<keyof typeof a>) {
      expect(a[slot]).not.toBe(b[slot]);
    }
  });

  it('separates raw from staging for the same record', () => {
    const derived = deriveProductV2AccountsV1(REGISTRY, DIGESTS);
    if (derived === null) throw new Error('expected a derivation');
    expect(derived.productRaw).not.toBe(derived.productStaging);
  });

  it('composes a real admission instruction the program would frame', () => {
    // The load-bearing claim: what this derives is what the adapter expects.
    // If the derivation were wrong, the builder's own distinctness and count
    // checks would still pass -- so the assertion is that the frame CONTAINS
    // exactly the derived addresses, in the program's declared order.
    const derived = deriveProductV2AccountsV1(REGISTRY, DIGESTS);
    if (derived === null) throw new Error('expected a derivation');
    const built = buildAdmissionInstructionV2({
      programId: new PublicKey(new Uint8Array(32).fill(9)).toBase58(),
      registry: REGISTRY,
      productRaw: derived.productRaw, productStaging: derived.productStaging,
      resultDomainRaw: derived.domainRaw, resultDomainStaging: derived.domainStaging,
      portfolioRaw: derived.portfolioRaw, portfolioStaging: derived.portfolioStaging,
    }, {
      productDigest: fromHex(DIGESTS.product, 'Product record digest'),
      resultDomainDigest: fromHex(DIGESTS.domain, 'result-domain record digest'),
      portfolioDigest: fromHex(DIGESTS.portfolio, 'portfolio record digest'),
    });
    const frame = built.instruction.keys.map((key) => key.pubkey.toBase58());
    expect(frame).toHaveLength(9);
    // Slots 2..7 are the six record accounts, in the order validate_frame reads.
    expect(frame.slice(2, 8)).toEqual([
      derived.productRaw, derived.productStaging,
      derived.domainRaw, derived.domainStaging,
      derived.portfolioRaw, derived.portfolioStaging,
    ]);
    expect(frame[1]).toBe(REGISTRY);
  });

  it('offers nothing while the Registry is unreadable', () => {
    expect(deriveProductV2AccountsV1('not-an-address', DIGESTS)).toBeNull();
    expect(deriveProductV2AccountsV1('', DIGESTS)).toBeNull();
  });

  it('offers nothing while any digest is unreadable', () => {
    expect(deriveProductV2AccountsV1(REGISTRY, { ...DIGESTS, domain: '' })).toBeNull();
    expect(deriveProductV2AccountsV1(REGISTRY, { ...DIGESTS, domain: 'ab' })).toBeNull();
    // A base58 address pasted into a digest field derives nothing, rather than
    // deriving something confidently wrong.
    expect(deriveProductV2AccountsV1(REGISTRY, { ...DIGESTS, product: REGISTRY })).toBeNull();
  });

  it('offers nothing for the all-zero content identity the chain refuses', () => {
    expect(deriveProductV2AccountsV1(REGISTRY, { ...DIGESTS, product: '00'.repeat(32) })).toBeNull();
  });
});

describe('effectiveAccountV1', () => {
  it('uses the derived address when no override is set', () => {
    expect(effectiveAccountV1('DerivedAddr', '')).toBe('DerivedAddr');
    expect(effectiveAccountV1('DerivedAddr', '   ')).toBe('DerivedAddr');
  });

  it('lets a deliberate override win, because an operator may know it moved', () => {
    expect(effectiveAccountV1('DerivedAddr', 'OverrideAddr')).toBe('OverrideAddr');
  });

  it('yields empty rather than null when nothing is derivable and nothing is set', () => {
    expect(effectiveAccountV1(null, '')).toBe('');
  });
});
