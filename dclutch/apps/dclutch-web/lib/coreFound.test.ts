import { AddressLookupTableAccount, PublicKey, TransactionInstruction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  compileLifecycleRentCreateTransactionV2,
  compileCoreFoundTransactionV2,
  decodeCoreFoundProductGraphV2,
  prepareCoreFoundV2,
  validateCoreFoundCapabilityManifestV1,
  validateCoreFoundSourceMaterialV2,
} from './coreFound';
import {
  CORE_FOUND_ACCOUNT_COUNT_V2,
  CORE_FOUND_ACCOUNT_ROLES_V2,
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

describe('Core Found31 browser kernel', () => {
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

    const source = new Uint8Array(208);
    put(source, 0, new TextEncoder().encode('DCLTSMV2')); putU16(source, 8, 2);
    put(source, 16, id(8)); put(source, 48, id(2)); put(source, 80, id(3)); put(source, 112, id(4)); put(source, 176, id(6));
    const productDigest = await sha256(graph.product);
    expect(() => validateCoreFoundSourceMaterialV2(source, productDigest)).toThrow(/different Product/);
  });

  it('accepts the canonical empty manifest and preflights u64 generation before RPC', async () => {
    const manifest = Uint8Array.from([...new TextEncoder().encode('DCLTCAP1'), 1, 0, 1, 0, 0, 0, 0, 0]);
    expect(() => validateCoreFoundCapabilityManifestV1(manifest)).not.toThrow();
    manifest[14] = 1;
    expect(() => validateCoreFoundCapabilityManifestV1(manifest)).toThrow(/reserved/);
    const client = { finalizedSlot: async () => { throw new Error('RPC must not run'); } } as unknown as SolanaRpcClient;
    await expect(prepareCoreFoundV2(client, {
      payer: '', registryProgram: '', activationCache: '', refundWallet: '', realmRecord: '', productRecord: '', resultDomainRecord: '', portfolioRecord: '', sourceMaterialRecord: '', capabilityManifestRecord: '', executionReleaseSetRecord: '', generation: 1n << 64n,
    })).rejects.toThrow(/outside lifecycle u64/);
  });

  it('refuses the 31-account Found frame inline, because it does not fit a packet', () => {
    // MEASURED, not predicted. With the ComputeBudget declaration Found31
    // cannot execute without -- it spends over a million CU against a 200,000
    // default -- the inline v0 message is 1,242 bytes against a 1,232-byte
    // bound. Ten bytes. This is the same overflow `4e1c4db` recorded on the
    // Rust side when it moved the route onto a finalized lookup table, and it
    // is why the browser needs one too.
    const accounts = Object.freeze(Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V2 }, (_, index) => new PublicKey(id(index + 1)).toBase58()));
    const compile = (lookupTable?: AddressLookupTableAccount) => compileCoreFoundTransactionV2({
      payer: accounts[0],
      market: accounts[1],
      coreProgram: accounts[19],
      generation: 77n,
      recentBlockhash: new PublicKey(id(99)).toBase58(),
      accountAddresses: accounts,
      lookupTable,
    });
    expect(() => compile()).toThrow(/1242 bytes inline.*requires a finalized routing table/);

    // With a table carrying every routable key, it fits and stays exact.
    const routable = routableAddressesV1([new TransactionInstruction({
      programId: new PublicKey(accounts[19]),
      keys: accounts.map((address, index) => ({
        pubkey: new PublicKey(address),
        isSigner: CORE_FOUND_ACCOUNT_ROLES_V2[index].signer,
        isWritable: CORE_FOUND_ACCOUNT_ROLES_V2[index].writable,
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
    // Two instructions now: the ComputeBudget limit this route cannot execute
    // without, then Found31 itself. The declaration takes no accounts.
    expect(compiled.transaction.message.compiledInstructions).toHaveLength(2);
    expect(compiled.transaction.message.compiledInstructions[0].accountKeyIndexes).toHaveLength(0);
    expect(compiled.transaction.message.compiledInstructions[1].accountKeyIndexes).toHaveLength(CORE_FOUND_ACCOUNT_COUNT_V2);
    // Privileges are the ones `found_metas` emits in
    // crates/dclutch-product-runtime-v2-operator/src/found.rs: payer writable
    // and signing, the Market destination writable, every other role readonly.
    expect(CORE_FOUND_ACCOUNT_ROLES_V2[0]).toEqual({ signer: true, writable: true });
    expect(CORE_FOUND_ACCOUNT_ROLES_V2[1]).toEqual({ signer: false, writable: true });
    expect(CORE_FOUND_ACCOUNT_ROLES_V2.filter((role) => role.signer)).toHaveLength(1);
    expect(CORE_FOUND_ACCOUNT_ROLES_V2.filter((role) => role.writable)).toHaveLength(2);
    // Routing moved most keys off the static list, so the frame is checked
    // against the RESOLVED key set -- which is the only list that says what
    // the program will actually be handed.
    const message = compiled.transaction.message;
    const resolved = message.getAccountKeys({ addressLookupTableAccounts: [table] });
    accounts.forEach((address, index) => {
      const position = compiled.transaction.message.compiledInstructions[1].accountKeyIndexes[index];
      expect(resolved.get(position)?.toBase58()).toBe(address);
      expect(message.isAccountWritable(position)).toBe(CORE_FOUND_ACCOUNT_ROLES_V2[index].writable);
    });
    // A signer can never live in a lookup table, so the payer stays static.
    expect(message.staticAccountKeys[0].toBase58()).toBe(accounts[0]);
    expect(message.addressTableLookups.map((lookup) => lookup.accountKey.toBase58())).toEqual([id(200) && new PublicKey(id(200)).toBase58()]);
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
    const accounts = Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V2 }, (_, index) => new PublicKey(id(index + 1)).toBase58());
    expect(() => compileCoreFoundTransactionV2({
      payer: accounts[0], market: accounts[1], coreProgram: accounts[18], generation: 1n,
      recentBlockhash: new PublicKey(id(99)).toBase58(), accountAddresses: accounts,
    })).toThrow(/wrong exact account index/);
    accounts[30] = accounts[29];
    expect(() => compileCoreFoundTransactionV2({
      payer: accounts[0], market: accounts[1], coreProgram: accounts[19], generation: 1n,
      recentBlockhash: new PublicKey(id(99)).toBase58(), accountAddresses: accounts,
    })).toThrow(/alias/);
  });
});
