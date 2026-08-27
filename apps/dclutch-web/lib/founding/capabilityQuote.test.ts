import { describe, expect, it } from 'vitest';

import manifestVector from '../../fixtures/founding/campaign-manifest-vector.json';
import {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  FUNDING_COMPARTMENTS_V1,
} from '../generated/capabilityManifestV1';
import { hex, slice } from '../bytes';
import { validateCoreFoundCapabilityManifestV1 } from '../coreFound';
import {
  NOT_APPLICABLE_V1,
  canonicalCapabilityOrderV1,
  encodeCapabilityEntryV1,
  encodeCapabilityManifestV1,
  encodeFundingQuoteV1,
  nativeLamportsV1,
  realmCollateralV1,
  summarizeManifestFundingV1,
  type CapabilityEntryInputV1,
} from './capabilityQuote';

const vector = manifestVector as Readonly<{ schema: string; provenance: string; capabilityManifestHex: string }>;
const campaign = Uint8Array.from(vector.capabilityManifestHex.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));

function id(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

function entry(overrides: Partial<CapabilityEntryInputV1> = {}): CapabilityEntryInputV1 {
  return {
    kindId: id(1),
    releaseId: id(2),
    configId: id(3),
    capacityProfileId: id(4),
    childSchemaId: id(5),
    childDerivationId: id(6),
    activation: 'RequiredAtFounding',
    activationDeadlineSlot: 0n,
    dependencies: [],
    quote: { compartments: { Rent: nativeLamportsV1(1n) }, realmCollateral: null },
    ...overrides,
  };
}

describe('the capability manifest encoder against a real campaign manifest', () => {
  it('re-encodes the exact bytes the Rust encoder published for the founded Market', () => {
    // This manifest is the one whose digest went into the Market PDA of the
    // journey campaign's DCLTGMF1 founding. The three entries are read back out
    // of the Rust-produced bytes and re-encoded field by field; agreement means
    // the browser encoder would have addressed the same Registry record.
    const count = new DataView(campaign.buffer).getUint16(12, true);
    expect(count).toBe(3);
    expect(campaign.length).toBe(CAPABILITY_MANIFEST_HEADER_BYTES_V1 + count * CAPABILITY_ENTRY_BYTES_V1);

    const entries: CapabilityEntryInputV1[] = [];
    for (let index = 0; index < count; index += 1) {
      const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + index * CAPABILITY_ENTRY_BYTES_V1;
      const body = slice(campaign, offset, CAPABILITY_ENTRY_BYTES_V1);
      const quote = slice(body, CAPABILITY_ENTRY_QUOTE_OFFSET_V1, 304);
      const compartments: CapabilityEntryInputV1['quote']['compartments'] = Object.fromEntries(
        FUNDING_COMPARTMENTS_V1.flatMap((compartment) => {
          const amount = new DataView(quote.buffer, quote.byteOffset + 176 + compartment.offset + 8, 8).getBigUint64(0, true);
          return amount === 0n ? [] : [[compartment.name, nativeLamportsV1(amount)]];
        }),
      );
      entries.push({
        kindId: hex(slice(body, 0, 32)),
        releaseId: hex(slice(body, 32, 32)),
        configId: hex(slice(body, 64, 32)),
        capacityProfileId: hex(slice(body, 96, 32)),
        childSchemaId: hex(slice(body, 128, 32)),
        childDerivationId: hex(slice(body, 160, 32)),
        activation: body[192] === 0 ? 'RequiredAtFounding' : 'PrepaidLazy',
        activationDeadlineSlot: new DataView(body.buffer, body.byteOffset + 200, 8).getBigUint64(0, true),
        dependencies: Array.from({ length: body[193] }, (_, position) => body[208 + position]),
        quote: { compartments, realmCollateral: null },
      });
    }
    expect(hex(encodeCapabilityManifestV1(entries))).toBe(vector.capabilityManifestHex);
  });

  it('reproduces the campaign quote shape: Rent, Creation and Bounty native, four unfunded', () => {
    const quote = encodeFundingQuoteV1({
      compartments: { Rent: nativeLamportsV1(1n), Creation: nativeLamportsV1(1n), Bounty: nativeLamportsV1(1n) },
      realmCollateral: null,
    });
    const campaignQuote = slice(campaign, CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_QUOTE_OFFSET_V1, 304);
    expect(hex(quote)).toBe(hex(campaignQuote));
  });

  it('produces a manifest the Found path decoder accepts', () => {
    expect(() => validateCoreFoundCapabilityManifestV1(campaign)).not.toThrow();
    expect(() => validateCoreFoundCapabilityManifestV1(encodeCapabilityManifestV1([entry()]))).not.toThrow();
  });
});

describe('the funding quote grammar the encoder cannot violate', () => {
  it('recomputes both totals rather than accepting them', () => {
    const quote = encodeFundingQuoteV1({
      compartments: { Rent: nativeLamportsV1(7n), Creation: nativeLamportsV1(11n), Work: nativeLamportsV1(13n) },
      realmCollateral: null,
    });
    const view = new DataView(quote.buffer, quote.byteOffset + 176);
    expect(view.getBigUint64(112, true)).toBe(31n);
    expect(view.getBigUint64(120, true)).toBe(0n);
  });

  it('keeps native lamports and Realm collateral in separate totals, never summed', () => {
    const quote = encodeFundingQuoteV1({
      compartments: { Rent: nativeLamportsV1(5n), Bounty: realmCollateralV1(9n) },
      realmCollateral: { realmId: id(1), collateralReleaseId: id(2), tokenProgram: id(3), mint: id(4), refundTokenBeneficiary: id(5) },
    });
    const view = new DataView(quote.buffer, quote.byteOffset + 176);
    expect(view.getBigUint64(112, true)).toBe(5n);
    expect(view.getBigUint64(120, true)).toBe(9n);
  });

  it('refuses Realm collateral in the two compartments that pay for existence', () => {
    for (const name of ['Rent', 'Creation'] as const) {
      expect(() => encodeFundingQuoteV1({
        compartments: { [name]: realmCollateralV1(1n) },
        realmCollateral: { realmId: id(1), collateralReleaseId: id(2), tokenProgram: id(3), mint: id(4), refundTokenBeneficiary: id(5) },
      })).toThrow(/native lamports only/);
    }
  });

  it('binds the Realm collateral binding to the Realm total as a biconditional', () => {
    expect(() => encodeFundingQuoteV1({
      compartments: { Bounty: realmCollateralV1(1n) },
      realmCollateral: null,
    })).toThrow(/present exactly when/);
    expect(() => encodeFundingQuoteV1({
      compartments: { Rent: nativeLamportsV1(1n) },
      realmCollateral: { realmId: id(1), collateralReleaseId: id(2), tokenProgram: id(3), mint: id(4), refundTokenBeneficiary: id(5) },
    })).toThrow(/present exactly when/);
  });

  it('refuses a zero amount in a funded class, and a nonzero one in NotApplicable', () => {
    expect(() => nativeLamportsV1(0n)).toThrow(/nonzero u64/);
    expect(() => encodeFundingQuoteV1({
      compartments: { Rent: { assetClass: 'native-lamports', amount: 0n } },
      realmCollateral: null,
    })).toThrow(/asset class its amount contradicts/);
    expect(() => encodeFundingQuoteV1({
      compartments: { Rent: { assetClass: 'not-applicable', amount: 5n } },
      realmCollateral: null,
    })).toThrow(/asset class its amount contradicts/);
  });
});

describe('manifest-level rules the read-back enforces', () => {
  it('refuses entries that are not strictly ordered by kind identity', () => {
    expect(() => encodeCapabilityManifestV1([entry({ kindId: id(9) }), entry({ kindId: id(2) })])).toThrow(/not strictly ordered/);
    const ordered = canonicalCapabilityOrderV1([entry({ kindId: id(9) }), entry({ kindId: id(2) })]);
    expect(ordered.map((candidate) => candidate.kindId)).toEqual([id(2), id(9)]);
    expect(() => encodeCapabilityManifestV1(ordered)).not.toThrow();
  });

  it('refuses two entries of the same kind rather than keeping one', () => {
    expect(() => canonicalCapabilityOrderV1([entry(), entry()])).toThrow(/same kind identity/);
  });

  it('refuses a cyclic dependency graph, which no single entry can reveal', () => {
    // Entry 0 depends on 1 and entry 1 depends on 0. Both entries are
    // individually well-formed; only the manifest read-back catches it.
    expect(() => encodeCapabilityManifestV1([
      entry({ kindId: id(1), dependencies: [1] }),
      entry({ kindId: id(2), dependencies: [0] }),
    ])).toThrow(/cyclic/);
  });

  it('refuses a dependency on itself or past the end of the manifest', () => {
    expect(() => encodeCapabilityManifestV1([entry({ dependencies: [0] })])).toThrow(/invalid or noncanonical/);
    expect(() => encodeCapabilityManifestV1([entry({ dependencies: [5] })])).toThrow(/invalid or noncanonical/);
  });

  it('refuses dependencies that are not strictly increasing', () => {
    expect(() => encodeCapabilityEntryV1(entry({ dependencies: [2, 1] }))).toThrow(/strictly increasing/);
    expect(() => encodeCapabilityEntryV1(entry({ dependencies: [1, 1] }))).toThrow(/strictly increasing/);
  });

  it('joins the activation policy to its deadline and its prepaid creation funding', () => {
    expect(() => encodeCapabilityManifestV1([entry({ activation: 'RequiredAtFounding', activationDeadlineSlot: 5n })])).toThrow(/do not join/);
    expect(() => encodeCapabilityManifestV1([entry({ activation: 'PrepaidLazy', activationDeadlineSlot: 0n })])).toThrow(/do not join/);
    expect(() => encodeCapabilityManifestV1([entry({
      activation: 'PrepaidLazy',
      activationDeadlineSlot: 5n,
      quote: { compartments: { Bounty: nativeLamportsV1(1n) }, realmCollateral: null },
    })])).toThrow(/do not join/);
    expect(() => encodeCapabilityManifestV1([entry({ activation: 'PrepaidLazy', activationDeadlineSlot: 5n })])).not.toThrow();
  });

  it('refuses an empty manifest and one above the sixteen-entry bound', () => {
    expect(() => encodeCapabilityManifestV1([])).toThrow(/1\.\.16/);
    expect(() => encodeCapabilityManifestV1(Array.from({ length: 17 }, (_, index) => entry({ kindId: id(index + 1) })))).toThrow(/1\.\.16/);
  });
});

describe('summarizing a manifest for display', () => {
  it('reports the seven compartments and never adds the two assets together', () => {
    const totals = summarizeManifestFundingV1([
      entry({ kindId: id(1), quote: { compartments: { Rent: nativeLamportsV1(100n), Bounty: realmCollateralV1(7n) }, realmCollateral: { realmId: id(1), collateralReleaseId: id(2), tokenProgram: id(3), mint: id(4), refundTokenBeneficiary: id(5) } } }),
      entry({ kindId: id(2), quote: { compartments: { Rent: nativeLamportsV1(50n) }, realmCollateral: null } }),
    ]);
    expect(totals.perCompartment.map((compartment) => compartment.name)).toEqual(FUNDING_COMPARTMENTS_V1.map((compartment) => compartment.name));
    expect(totals.nativeLamports).toBe(150n);
    expect(totals.realmCollateral).toBe(7n);
    expect(totals.perCompartment.find((compartment) => compartment.name === 'Work')).toEqual({ name: 'Work', assetClass: 'not-applicable', amount: 0n });
  });

  it('refuses to summarize a compartment quoted in two asset classes', () => {
    expect(() => summarizeManifestFundingV1([
      entry({ kindId: id(1), quote: { compartments: { Bounty: nativeLamportsV1(1n) }, realmCollateral: null } }),
      entry({ kindId: id(2), quote: { compartments: { Bounty: realmCollateralV1(1n) }, realmCollateral: { realmId: id(1), collateralReleaseId: id(2), tokenProgram: id(3), mint: id(4), refundTokenBeneficiary: id(5) } } }),
    ])).toThrow(/two different asset classes/);
  });

  it('treats an unstated compartment as NotApplicable rather than as zero lamports', () => {
    const totals = summarizeManifestFundingV1([entry({ quote: { compartments: {}, realmCollateral: null } })]);
    expect(totals.perCompartment.every((compartment) => compartment.assetClass === 'not-applicable')).toBe(true);
    expect(NOT_APPLICABLE_V1.amount).toBe(0n);
  });
});
