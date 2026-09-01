import { AddressLookupTableAccount, PublicKey, TransactionInstruction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  acquireFinalizedAccountsInChunksV1,
  compileLifecycleRentCreateTransactionV2,
  compileCoreFoundTransactionV2,
  decodeCoreFoundProductGraphV2,
  decodeResultDomainV2,
  deriveCoreFoundRecordsV2,
  prepareCoreFoundV2,
  validateCoreFoundCapabilityManifestV1,
  validateCoreFoundSourceMaterialV3,
} from './coreFound';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import {
  MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
  SOURCE_SPEC_SCHEMA_ID_V1,
} from './generated/coreFound';
import {
  CORE_FOUND_ACCOUNT_COUNT_V3,
  CORE_FOUND_ACCOUNT_ROLES_V3,
  CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3,
  CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3,
  CORE_FOUND_PRICE_GATE_RAW_INDEX_V3,
  CORE_FOUND_PRICE_GATE_STAGING_INDEX_V3,
  CORE_REQUEST_BYTES,
  CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
} from './generated/coreFound';
import { canonicalLookupAddressesV1, routableAddressesV1 } from './founding/lookupTable';
import { type SolanaRpcClient } from './rpc';

function put(bytes: Uint8Array, offset: number, value: Uint8Array): void { bytes.set(value, offset); }
function putU16(bytes: Uint8Array, offset: number, value: number): void { new DataView(bytes.buffer).setUint16(offset, value, true); }
function putU32(bytes: Uint8Array, offset: number, value: number): void { new DataView(bytes.buffer).setUint32(offset, value, true); }
function putU64(bytes: Uint8Array, offset: number, value: bigint): void { new DataView(bytes.buffer).setBigUint64(offset, value, true); }
function putI128(bytes: Uint8Array, offset: number, value: bigint): void {
  const view = new DataView(bytes.buffer);
  view.setBigUint64(offset, BigInt.asUintN(64, value), true);
  view.setBigInt64(offset + 8, BigInt.asIntN(64, value >> 64n), true);
}
function id(byte: number): Uint8Array { return new Uint8Array(32).fill(byte); }

async function graph258(): Promise<Readonly<{ product: Uint8Array; domain: Uint8Array; portfolio: Uint8Array; domainDigest: Uint8Array; portfolioDigest: Uint8Array }>> {
  const cutCount = 256;
  const domain = new Uint8Array(240 + cutCount * 16);
  put(domain, 0, new TextEncoder().encode('DCLTPRD2'));
  putU16(domain, 8, 2); putU16(domain, 10, 240); putU32(domain, 12, domain.length);
  putU32(domain, 16, cutCount + 1); putU32(domain, 20, cutCount);
  [1, 2, 3, 4, 5, 6].forEach((byte, index) => put(domain, 32 + index * 32, id(byte)));
  putU64(domain, 224, 1n);
  for (let index = 0; index < cutCount; index += 1) putI128(domain, 240 + index * 16, BigInt(index) - 128n);
  const domainDigest = await sha256(domain);

  const outcomeCount = 258;
  const portfolio = new Uint8Array(208 + outcomeCount * 8);
  put(portfolio, 0, new TextEncoder().encode('DCLTPRF2'));
  putU16(portfolio, 8, 2); putU16(portfolio, 10, 208); putU32(portfolio, 12, portfolio.length);
  putU32(portfolio, 16, outcomeCount); portfolio[20] = 1;
  put(portfolio, 32, id(1)); put(portfolio, 64, domainDigest); put(portfolio, 96, id(7)); put(portfolio, 128, id(4)); put(portfolio, 160, id(5));
  putU64(portfolio, 192, 1n);
  for (let index = 0; index < outcomeCount; index += 1) putU64(portfolio, 208 + index * 8, 1n);
  const portfolioDigest = await sha256(portfolio);

  const product = new Uint8Array(112);
  put(product, 0, new TextEncoder().encode('DCLTPRM2')); putU16(product, 8, 2);
  put(product, 16, id(1)); put(product, 48, domainDigest); put(product, 80, portfolioDigest);
  return Object.freeze({ product, domain, portfolio, domainDigest, portfolioDigest });
}

describe('Core Found37 browser kernel', () => {
  it('acquires Found37 in 32-key chunks at one finalized context slot', async () => {
    const addresses = Array.from({ length: 37 }, (_, index) => new PublicKey(id(index + 1)).toBase58());
    const calls: Array<{ count: number; floor: string | undefined }> = [];
    const client = {
      multipleAccounts: async (chunk: ReadonlyArray<string>, floor?: string) => {
        calls.push({ count: chunk.length, floor });
        return Object.freeze({
          slot: '77',
          accounts: Object.freeze(chunk.map((address) => Object.freeze({ address, account: null }))),
        });
      },
    } as unknown as SolanaRpcClient;
    const observation = await acquireFinalizedAccountsInChunksV1(client, addresses, '77');
    expect(calls).toEqual([{ count: 32, floor: '77' }, { count: 5, floor: '77' }]);
    expect(observation.slot).toBe('77');
    expect(observation.accounts.map((entry) => entry.address)).toEqual(addresses);
  });

  it('refuses regressed or mixed finalized chunk contexts', async () => {
    const addresses = Array.from({ length: 33 }, (_, index) => new PublicKey(id(index + 1)).toBase58());
    let call = 0;
    const mixed = {
      multipleAccounts: async (chunk: ReadonlyArray<string>) => {
        call += 1;
        return { slot: call === 1 ? '77' : '78', accounts: chunk.map((address) => ({ address, account: null })) };
      },
    } as unknown as SolanaRpcClient;
    await expect(acquireFinalizedAccountsInChunksV1(mixed, addresses, '77')).rejects.toThrow(/different context slots/);
    const regressed = {
      multipleAccounts: async (chunk: ReadonlyArray<string>) => ({
        slot: '76',
        accounts: chunk.map((address) => ({ address, account: null })),
      }),
    } as unknown as SolanaRpcClient;
    await expect(acquireFinalizedAccountsInChunksV1(regressed, addresses, '77')).rejects.toThrow(/regressed/);
  });

  it('joins a runtime-width Product with 258 outcomes and refuses same-width substitution', async () => {
    const graph = await graph258();
    expect(decodeCoreFoundProductGraphV2(graph.product, graph.domain, graph.portfolio, graph.domainDigest, graph.portfolioDigest)).toMatchObject({ outcomeCount: 258 });
    const hostile = new Uint8Array(graph.domain);
    putI128(hostile, hostile.length - 16, 129n);
    const hostileDigest = await sha256(hostile);
    expect(() => decodeCoreFoundProductGraphV2(graph.product, hostile, graph.portfolio, hostileDigest, graph.portfolioDigest)).toThrow(/does not select/);
  });

  it('refuses noncanonical portfolio rational scale and malformed Source linkage', async () => {
    const graph = await graph258();
    const portfolio = new Uint8Array(graph.portfolio);
    putU64(portfolio, 192, 2n);
    for (let index = 0; index < 258; index += 1) putU64(portfolio, 208 + index * 8, 2n);
    const digest = await sha256(portfolio);
    const product = new Uint8Array(graph.product); put(product, 80, digest);
    expect(() => decodeCoreFoundProductGraphV2(product, graph.domain, portfolio, graph.domainDigest, digest)).toThrow(/gcd-normalized/);

    const source = new Uint8Array(240);
    put(source, 0, new TextEncoder().encode('DCLTSMV3')); putU16(source, 8, 3); source[11] = 1;
    put(source, 16, id(8)); put(source, 48, id(2)); put(source, 80, id(3)); put(source, 112, id(4)); put(source, 176, id(6));
    const productDigest = await sha256(graph.product);
    expect(() => validateCoreFoundSourceMaterialV3(source, productDigest)).toThrow(/different Product/);
  });

  it('derives the four dependent record addresses the parents already name', async () => {
    /**
     * THE DEFECT THIS CLOSES. `/found` asks a stranger to paste fourteen
     * base58 addresses, and its own field comments say five of them are
     * values another record on the same list already contains: "Derivable
     * from that record once this console reads it; today it is typed and
     * then checked." Checked is not the same as derived. A typed address
     * that survives every check is still an address somebody had to go and
     * find, and the one place a person can be silently wrong is the one the
     * console asked them to fill in by hand.
     *
     * Four of the five are derivable through the Registry's own constructor
     * from a digest the parent record carries at a named coordinate. The
     * fifth is not, and is a routed finding rather than a fifth derivation:
     * `SourceSpecV1::decode` writes its capacity-profile coordinate as a
     * bare `144` (crates/dclutch-source-contract/src/lib.rs:917), so there
     * is no named constant for the generator to emit and nothing this
     * browser could import instead of restating it.
     */
    const graph = await graph258();
    const registry = new PublicKey(new Uint8Array(32).fill(9)).toBase58();
    const productDigest = await sha256(graph.product);

    const sourceSpecDigest = id(21);
    const manipulationFloorDigest = id(22);
    const source = new Uint8Array(240);
    put(source, 0, new TextEncoder().encode('DCLTSMV3')); putU16(source, 8, 3); source[11] = 2;
    put(source, 16, productDigest);
    put(source, 48, sourceSpecDigest);
    put(source, 80, id(3)); put(source, 112, id(4)); put(source, 176, id(6));
    put(source, 208, manipulationFloorDigest);
    const sourceDigest = await sha256(source);

    const productRecord = deriveFinalizedRecordAddressesV1(registry, PRODUCT_RECORD_SCHEMA_ID_V2, productDigest).record;
    const sourceMaterialRecord = deriveFinalizedRecordAddressesV1(registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, sourceDigest).record;

    const served = new Map<string, Uint8Array>([[productRecord, graph.product], [sourceMaterialRecord, source]]);
    const asked: string[] = [];
    const client = {
      finalizedSlot: async () => '600',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => {
        asked.push(...addresses);
        return {
          slot: '600',
          accounts: addresses.map((address) => ({
            address,
            account: served.has(address)
              ? { owner: registry, executable: false, lamports: '1000000', space: served.get(address)!.length, data: served.get(address)! }
              : null,
          })),
        };
      },
    } as unknown as SolanaRpcClient;

    const derived = await deriveCoreFoundRecordsV2(client, { registryProgram: registry, productRecord, sourceMaterialRecord });

    // Each address, recomputed here through the same Registry constructor the
    // contract uses, from the digest its parent carries.
    expect(derived.resultDomainRecord)
      .toBe(deriveFinalizedRecordAddressesV1(registry, RESULT_DOMAIN_SCHEMA_ID_V2, graph.domainDigest).record);
    expect(derived.portfolioRecord)
      .toBe(deriveFinalizedRecordAddressesV1(registry, PORTFOLIO_SCHEMA_ID_V2, graph.portfolioDigest).record);
    expect(derived.sourceSpecRecord)
      .toBe(deriveFinalizedRecordAddressesV1(registry, SOURCE_SPEC_SCHEMA_ID_V1, sourceSpecDigest).record);
    expect(derived.manipulationFloorRecord)
      .toBe(deriveFinalizedRecordAddressesV1(registry, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, manipulationFloorDigest).record);

    // It asks the chain for the two parents a reader supplied and nothing
    // else: a derivation that quietly reads a third address is a third
    // address the reader is trusting without being told.
    expect([...new Set(asked)].sort()).toEqual([productRecord, sourceMaterialRecord].sort());
  });

  it('refuses a parent record that is not its own content-derived Registry PDA', async () => {
    // The check that survives derivation, and the only one that matters here:
    // the two PARENTS are still typed, so a reader can still paste an address
    // that is not the schema/content PDA of what lives there. The children
    // inherit their correctness from this refusal.
    const graph = await graph258();
    const registry = new PublicKey(new Uint8Array(32).fill(9)).toBase58();
    const wrongAddress = new PublicKey(new Uint8Array(32).fill(11)).toBase58();
    const source = new Uint8Array(240);
    put(source, 0, new TextEncoder().encode('DCLTSMV3')); putU16(source, 8, 3); source[11] = 2;
    put(source, 16, await sha256(graph.product));
    put(source, 48, id(21)); put(source, 80, id(3)); put(source, 112, id(4)); put(source, 176, id(6));
    put(source, 208, id(22));
    const sourceMaterialRecord = deriveFinalizedRecordAddressesV1(registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, await sha256(source)).record;
    const served = new Map<string, Uint8Array>([[wrongAddress, graph.product], [sourceMaterialRecord, source]]);
    const client = {
      finalizedSlot: async () => '600',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '600',
        accounts: addresses.map((address) => ({
          address,
          account: served.has(address)
            ? { owner: registry, executable: false, lamports: '1000000', space: served.get(address)!.length, data: served.get(address)! }
            : null,
        })),
      }),
    } as unknown as SolanaRpcClient;

    await expect(deriveCoreFoundRecordsV2(client, {
      registryProgram: registry, productRecord: wrongAddress, sourceMaterialRecord,
    })).rejects.toThrow(/Product is not the schema\/content-derived Registry raw PDA/);
  });

  it('refuses a SourceMaterialV3 that names another Product than the one supplied', async () => {
    const graph = await graph258();
    const registry = new PublicKey(new Uint8Array(32).fill(9)).toBase58();
    const source = new Uint8Array(240);
    put(source, 0, new TextEncoder().encode('DCLTSMV3')); putU16(source, 8, 3); source[11] = 2;
    // A Product digest that is not this Product's.
    put(source, 16, id(99));
    put(source, 48, id(21)); put(source, 80, id(3)); put(source, 112, id(4)); put(source, 176, id(6));
    put(source, 208, id(22));
    const productRecord = deriveFinalizedRecordAddressesV1(registry, PRODUCT_RECORD_SCHEMA_ID_V2, await sha256(graph.product)).record;
    const sourceMaterialRecord = deriveFinalizedRecordAddressesV1(registry, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, await sha256(source)).record;
    const served = new Map<string, Uint8Array>([[productRecord, graph.product], [sourceMaterialRecord, source]]);
    const client = {
      finalizedSlot: async () => '600',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '600',
        accounts: addresses.map((address) => ({
          address,
          account: served.has(address)
            ? { owner: registry, executable: false, lamports: '1000000', space: served.get(address)!.length, data: served.get(address)! }
            : null,
        })),
      }),
    } as unknown as SolanaRpcClient;

    await expect(deriveCoreFoundRecordsV2(client, {
      registryProgram: registry, productRecord, sourceMaterialRecord,
    })).rejects.toThrow(/different Product record digest/);
  });

  it('returns the operator\u2019s own cuts instead of discarding them', async () => {
    /**
     * THE DEFECT THIS CLOSES. These bytes were already decoded here and
     * already validated for ABI, width, identity and strict increase — and
     * then dropped, because the graph decoder only needed them well formed.
     * So the one artifact that says what a market\u2019s outcomes ARE reached
     * the browser and left no trace, while `/product-v2` rendered a parallel
     * list of "interpolation segment N" derived in TypeScript from the payoff
     * KNOTS, in the place a reader looks for the partition. Knots are where
     * the payoff bends; cuts are where the outcome changes. Different lists,
     * different lengths, and only one of them is on chain.
     */
    const graph = await graph258();
    const domain = decodeResultDomainV2(graph.domain);
    expect(domain.regionCount).toBe(257);
    expect(domain.cuts.length).toBe(256);
    expect(domain.denominator).toBe(1n);
    // Exactly the values the fixture wrote, in canonical order.
    expect(domain.cuts[0]).toBe(-128n);
    expect(domain.cuts[255]).toBe(127n);
    for (let index = 1; index < domain.cuts.length; index += 1) {
      expect(domain.cuts[index] > domain.cuts[index - 1]).toBe(true);
    }
  });

  it('refuses a result domain whose cuts are not strictly increasing', async () => {
    const graph = await graph258();
    const broken = new Uint8Array(graph.domain);
    putI128(broken, 240 + 16, -200n);
    expect(() => decodeResultDomainV2(broken)).toThrow(/not strictly increasing/);
  });

  it('accepts the canonical empty manifest and preflights u64 generation before RPC', async () => {
    const manifest = Uint8Array.from([...new TextEncoder().encode('DCLTCAP1'), 1, 0, 1, 0, 0, 0, 0, 0]);
    expect(() => validateCoreFoundCapabilityManifestV1(manifest)).not.toThrow();
    manifest[14] = 1;
    expect(() => validateCoreFoundCapabilityManifestV1(manifest)).toThrow(/reserved/);
    const client = { finalizedSlot: async () => { throw new Error('RPC must not run'); } } as unknown as SolanaRpcClient;
    await expect(prepareCoreFoundV2(client, {
      payer: '', registryProgram: '', activationCache: '', refundWallet: '', realmRecord: '', productRecord: '', resultDomainRecord: '', portfolioRecord: '', linkedBasisRecord: '', sourceMaterialRecord: '', sourceSpecRecord: '', capacityProfileRecord: '', manipulationFloorRecord: '', capabilityManifestRecord: '', generation: 1n << 64n,
    })).rejects.toThrow(/outside lifecycle u64/);
  });

  it('refuses the 37-account Found frame inline, because it does not fit a packet', () => {
    // Compiled, not inferred: the bounded Found37 message is 1,242 bytes
    // inline against a 1,232-byte packet bound. The packet geometry requires a
    // finalized lookup table independently of any later compute measurement.
    const accounts = Object.freeze(Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V3 }, (_, index) => new PublicKey(id(index + 1)).toBase58()));
    const compile = (lookupTable?: AddressLookupTableAccount) => compileCoreFoundTransactionV2({
      payer: accounts[0],
      market: accounts[1],
      coreProgram: accounts[25],
      generation: 77n,
      recentBlockhash: new PublicKey(id(99)).toBase58(),
      accountAddresses: accounts,
      lookupTable,
    });
    expect(() => compile()).toThrow(/exceeds the 1232-byte packet bound.*requires a finalized routing table/);

    // With a table carrying every routable key, it fits and stays exact.
    const routable = routableAddressesV1([new TransactionInstruction({
      programId: new PublicKey(accounts[25]),
      keys: accounts.map((address, index) => ({
        pubkey: new PublicKey(address),
        isSigner: CORE_FOUND_ACCOUNT_ROLES_V3[index].signer,
        isWritable: CORE_FOUND_ACCOUNT_ROLES_V3[index].writable,
      })),
      data: Buffer.alloc(0),
    })], accounts[0]);
    const table = new AddressLookupTableAccount({
      key: new PublicKey(id(200)),
      state: {
        deactivationSlot: 2n ** 64n - 1n,
        lastExtendedSlot: 1,
        lastExtendedSlotStartIndex: 0,
        authority: new PublicKey(accounts[0]),
        addresses: canonicalLookupAddressesV1(routable).map((address) => new PublicKey(address)),
      },
    });
    const compiled = compile(table);
    expect(compiled.requestBytes).toHaveLength(CORE_REQUEST_BYTES);
    expect(compiled.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(compiled.requiredSigners).toEqual([accounts[0]]);
    // Two instructions now: the configured ComputeBudget limit, then Found37
    // itself. The declaration takes no accounts.
    expect(compiled.transaction.message.compiledInstructions).toHaveLength(2);
    expect(compiled.transaction.message.compiledInstructions[0].accountKeyIndexes).toHaveLength(0);
    expect(compiled.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(CORE_FOUND_ACCOUNT_COUNT_V3);
    // Privileges are the ones `found_metas` emits in
    // crates/dclutch-product-runtime-v2-operator/src/found.rs: payer writable
    // and signing, the Market destination writable, every other role readonly.
    expect(CORE_FOUND_ACCOUNT_ROLES_V3[0]).toEqual({ signer: true, writable: true });
    expect(CORE_FOUND_ACCOUNT_ROLES_V3[1]).toEqual({ signer: false, writable: true });
    expect(CORE_FOUND_ACCOUNT_ROLES_V3.filter((role) => role.signer)).toHaveLength(1);
    expect(CORE_FOUND_ACCOUNT_ROLES_V3.filter((role) => role.writable)).toHaveLength(2);
    // Routing moved most keys off the static list, so the frame is checked
    // against the RESOLVED key set -- which is the only list that says what
    // the program will actually be handed.
    const message = compiled.transaction.message;
    const resolved = message.getAccountKeys({ addressLookupTableAccounts: [table] });
    accounts.forEach((address, index) => {
      const position = compiled.transaction.message.compiledInstructions[1].accountKeyIndexes[index];
      expect(resolved.get(position)?.toBase58()).toBe(address);
      expect(message.isAccountWritable(position)).toBe(CORE_FOUND_ACCOUNT_ROLES_V3[index].writable);
    });
    // A signer can never live in a lookup table, so the payer stays static.
    expect(message.staticAccountKeys[0].toBase58()).toBe(accounts[0]);
    expect(message.addressTableLookups.map((lookup) => lookup.accountKey.toBase58())).toEqual([id(200) && new PublicKey(id(200)).toBase58()]);
  });

  it('compiles the generated 39-account curved Found frame without inventing a gate projection', () => {
    expect(CORE_FOUND_PRICE_GATE_RAW_INDEX_V3).toBe(CORE_FOUND_ACCOUNT_COUNT_V3);
    expect(CORE_FOUND_PRICE_GATE_STAGING_INDEX_V3).toBe(CORE_FOUND_ACCOUNT_COUNT_V3 + 1);
    expect(CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3.slice(0, CORE_FOUND_ACCOUNT_COUNT_V3)).toEqual(CORE_FOUND_ACCOUNT_ROLES_V3);
    expect(CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3.slice(CORE_FOUND_ACCOUNT_COUNT_V3)).toEqual([
      { signer: false, writable: false },
      { signer: false, writable: false },
    ]);

    const accounts = Object.freeze(Array.from({ length: CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3 }, (_, index) => new PublicKey(id(index + 1)).toBase58()));
    const instruction = new TransactionInstruction({
      programId: new PublicKey(accounts[25]),
      keys: accounts.map((address, index) => ({
        pubkey: new PublicKey(address),
        isSigner: CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3[index].signer,
        isWritable: CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3[index].writable,
      })),
      data: Buffer.alloc(0),
    });
    const routable = routableAddressesV1([instruction], accounts[0]);
    const table = new AddressLookupTableAccount({
      key: new PublicKey(id(200)),
      state: {
        deactivationSlot: 2n ** 64n - 1n,
        lastExtendedSlot: 1,
        lastExtendedSlotStartIndex: 0,
        authority: new PublicKey(accounts[0]),
        addresses: canonicalLookupAddressesV1(routable).map((address) => new PublicKey(address)),
      },
    });
    const compiled = compileCoreFoundTransactionV2({
      payer: accounts[0],
      market: accounts[1],
      coreProgram: accounts[25],
      generation: 77n,
      recentBlockhash: new PublicKey(id(99)).toBase58(),
      accountAddresses: accounts,
      lookupTable: table,
    });
    expect(compiled.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3);
    expect(compiled.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(() => compileCoreFoundTransactionV2({
      payer: accounts[0], market: accounts[1], coreProgram: accounts[25], generation: 77n,
      recentBlockhash: new PublicKey(id(99)).toBase58(), accountAddresses: accounts.slice(0, -1), lookupTable: table,
    })).toThrow(/exactly 37 or 39/);
  });

  it('derives a Market-generation lifecycle credit and binds its sole refund wallet and release set', () => {
    const payer = new PublicKey(id(1)).toBase58();
    const refundWallet = new PublicKey(id(2)).toBase58();
    const market = new PublicKey(id(3)).toBase58();
    const rentProgram = new PublicKey(id(4)).toBase58();
    const compiled = compileLifecycleRentCreateTransactionV2({
      payer,
      refundWallet,
      market,
      releaseSet: id(5),
      generation: 7n,
      rentProgram,
      recentBlockhash: new PublicKey(id(99)).toBase58(),
    });
    expect(compiled.requestBytes).toHaveLength(CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2);
    expect(new TextDecoder().decode(compiled.requestBytes.slice(0, 8))).toBe('DCLRNCI2');
    expect(compiled.requestBytes.slice(16, 48)).toEqual(new PublicKey(refundWallet).toBytes());
    expect(compiled.requestBytes.slice(48, 80)).toEqual(new PublicKey(market).toBytes());
    expect(compiled.requestBytes.slice(80, 112)).toEqual(id(5));
    expect(compiled.requiredSigners).toEqual([payer]);
    expect(compiled.transaction.message.compiledInstructions).toHaveLength(1);
    expect(compiled.wireBytes.length).toBeLessThanOrEqual(1_232);

    const next = compileLifecycleRentCreateTransactionV2({
      payer, refundWallet, market, releaseSet: id(5), generation: 8n, rentProgram,
      recentBlockhash: new PublicKey(id(99)).toBase58(),
    });
    expect(next.rentCredit).not.toBe(compiled.rentCredit);
  });

  it('refuses zero generations and aliasing a refund wallet with lifecycle identity', () => {
    const payer = new PublicKey(id(1)).toBase58();
    const market = new PublicKey(id(3)).toBase58();
    const common = {
      payer,
      market,
      releaseSet: id(5),
      rentProgram: new PublicKey(id(4)).toBase58(),
      recentBlockhash: new PublicKey(id(99)).toBase58(),
    };
    expect(() => compileLifecycleRentCreateTransactionV2({ ...common, refundWallet: payer, generation: 0n })).toThrow(/lifecycle u64/);
    expect(() => compileLifecycleRentCreateTransactionV2({ ...common, refundWallet: market, generation: 7n })).toThrow(/alias/);
  });

  it('refuses account-index drift and aliasing before transaction construction', () => {
    const accounts = Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V3 }, (_, index) => new PublicKey(id(index + 1)).toBase58());
    expect(() => compileCoreFoundTransactionV2({
      payer: accounts[0], market: accounts[1], coreProgram: accounts[24], generation: 1n,
      recentBlockhash: new PublicKey(id(99)).toBase58(), accountAddresses: accounts,
    })).toThrow(/wrong exact account index/);
    accounts[30] = accounts[29];
    expect(() => compileCoreFoundTransactionV2({
      payer: accounts[0], market: accounts[1], coreProgram: accounts[25], generation: 1n,
      recentBlockhash: new PublicKey(id(99)).toBase58(), accountAddresses: accounts,
    })).toThrow(/alias/);
  });
});
