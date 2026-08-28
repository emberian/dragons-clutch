import { AddressLookupTableAccount, PublicKey, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import {
  CUSTODY_ABI_VERSION_V1,
  CUSTODY_REPLAY_BYTES_V1,
  CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_REALM_OFFSET_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1,
  CUSTODY_REPLAY_STATUS_OFFSET_V1,
  CUSTODY_REPLAY_VERSION_OFFSET_V1,
  EXECUTION_ROLE_CLAIMS_V1,
} from './generated/claimsCustodyReplayV1';
import {
  CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
  CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1,
  CUSTODY_VAULT_PDA_DOMAIN_V1,
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_PRODUCT_OFFSET,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_POSITION_BASIS_OFFSET,
  LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_MAGIC_V2,
  LIABILITY_BASIS_POSITION_MARKET_OFFSET,
  LIABILITY_BASIS_POSITION_OWNER_OFFSET,
  LIABILITY_BASIS_POSITION_REVISION_OFFSET,
  LIABILITY_BASIS_STATE_VERSION_V2,
} from './generated/coreFound';
import {
  TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
  TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3,
  TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
  TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
} from './generated/walletTerminalPayoutV3';
import { deriveClaimsAggregateAddressV2, deriveClaimsPositionAddressV2 } from './marketCoreV2';
import { type RpcAccount, type TransactionMetaObservation } from './rpc';
import {
  buildWalletTerminalPayoutV3,
  canonicalWalletTerminalPayoutLookupAddressesV3,
  compileWalletTerminalPayoutV0,
  encodeWalletTerminalPayoutRequestV3,
  finalizeWalletTerminalPayoutV3,
  parseWalletTerminalPayoutManifestV3,
  verifyWalletTerminalPayoutPostconditionV3,
  verifyFinalizedWalletTerminalPayoutTransactionV3,
  type PreparedWalletTerminalPayoutV3,
  type WalletTerminalPayoutBuildInputV3,
  type WalletTerminalPayoutReportV3,
  type WalletTerminalPayoutRouteV3,
} from './walletTerminalPayoutV3';

const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const LEGACY_TOKEN_PROGRAM = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
const UPGRADEABLE_LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');

function key(value: number): string { return new PublicKey(new Uint8Array(32).fill(value)).toBase58(); }
function id(value: number): string { return value.toString(16).padStart(2, '0').repeat(32); }
function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true); }
function concatenate(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0; for (const part of parts) { output.set(part, offset); offset += part.length; } return output;
}
async function hashv(...parts: ReadonlyArray<Uint8Array>): Promise<Uint8Array> { return sha256(concatenate(...parts)); }
function le64(value: bigint): Uint8Array { const output = new Uint8Array(8); putU64(output, 0, value); return output; }

function token(mint: string, owner: string, amount: bigint): Uint8Array {
  const output = new Uint8Array(165);
  output.set(new PublicKey(mint).toBytes(), 0); output.set(new PublicKey(owner).toBytes(), 32);
  putU64(output, 64, amount); output[108] = 1; return output;
}

function fixture(): WalletTerminalPayoutBuildInputV3 {
  const market = key(10); const claimsProgram = key(40); const custodyProgram = key(41);
  const registryProgram = key(42); const owner = key(17); const collateralMint = key(43);
  const resolutionProgram = key(67); const terminalCertificate = key(29);
  const [resolutionProgramData] = PublicKey.findProgramAddressSync([
    new PublicKey(resolutionProgram).toBytes(),
  ], UPGRADEABLE_LOADER);
  const aggregate = deriveClaimsAggregateAddressV2(claimsProgram, market);
  const position = deriveClaimsPositionAddressV2(claimsProgram, aggregate, owner);
  const releaseSet = bytes(11); const realm = bytes(15); const context = bytes(16);
  const [custodyReplay] = PublicKey.findProgramAddressSync([
    CUSTODY_REPLAY_PDA_DOMAIN_V1, new PublicKey(market).toBytes(), releaseSet,
    Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1), context,
  ], new PublicKey(custodyProgram));
  const [custodyAuthority] = PublicKey.findProgramAddressSync([
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, new PublicKey(market).toBytes(), releaseSet,
  ], new PublicKey(custodyProgram));
  const [hoard] = PublicKey.findProgramAddressSync([
    CUSTODY_VAULT_PDA_DOMAIN_V1, new PublicKey(market).toBytes(), releaseSet, context,
    Uint8Array.of(CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1),
  ], new PublicKey(custodyProgram));
  const route: WalletTerminalPayoutRouteV3 = Object.freeze({
    aggregate, linkedBasisRaw: key(50), linkedBasisStaging: key(51), productRaw: key(52),
    productStaging: key(53), resultDomainRaw: key(54), resultDomainStaging: key(55),
    portfolioRaw: key(56), portfolioStaging: key(57), market, activationCache: key(58),
    registryProgram, claimsProgram, claimsProgramData: key(59), coreProgram: key(60),
    coreProgramData: key(61), resolutionProgram, resolutionProgramData: resolutionProgramData.toBase58(),
    position, exposureRaw: key(62), exposureStaging: key(63), custodyProgram, terminalCertificate,
    realmRaw: key(64), realmStaging: key(65),
    custodyReplay: custodyReplay.toBase58(), collateralMint, hoard: hoard.toBase58(), recipient: key(66),
    custodyAuthority: custodyAuthority.toBase58(), tokenProgram: LEGACY_TOKEN_PROGRAM,
  });
  const aggregateBytes = new Uint8Array(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16);
  aggregateBytes.set(LIABILITY_BASIS_MARKET_MAGIC_V2, 0); putU16(aggregateBytes, 8, LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(aggregateBytes, LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, 2); putU64(aggregateBytes, LIABILITY_BASIS_MARKET_REVISION_OFFSET, 7n);
  aggregateBytes.set(new PublicKey(market).toBytes(), LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  aggregateBytes.set(releaseSet, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET);
  aggregateBytes.set(new PublicKey(registryProgram).toBytes(), LIABILITY_BASIS_MARKET_REGISTRY_OFFSET);
  aggregateBytes.set(bytes(13), LIABILITY_BASIS_MARKET_PRODUCT_OFFSET); aggregateBytes.set(bytes(14), LIABILITY_BASIS_MARKET_BASIS_OFFSET);
  aggregateBytes.set(realm, LIABILITY_BASIS_MARKET_REALM_OFFSET); aggregateBytes.set(context, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET);
  putU64(aggregateBytes, LIABILITY_BASIS_MARKET_GENERATION_OFFSET, 3n); putU64(aggregateBytes, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, 5n); putU64(aggregateBytes, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8, 7n);
  const positionBytes = new Uint8Array(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16);
  positionBytes.set(LIABILITY_BASIS_POSITION_MAGIC_V2, 0); putU16(positionBytes, 8, LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(positionBytes, LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET, 2); putU64(positionBytes, LIABILITY_BASIS_POSITION_REVISION_OFFSET, 11n);
  positionBytes.set(new PublicKey(aggregate).toBytes(), LIABILITY_BASIS_POSITION_MARKET_OFFSET);
  positionBytes.set(new PublicKey(owner).toBytes(), LIABILITY_BASIS_POSITION_OWNER_OFFSET); positionBytes.set(bytes(14), LIABILITY_BASIS_POSITION_BASIS_OFFSET);
  putU64(positionBytes, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, 2n); putU64(positionBytes, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8, 4n);
  const replay = new Uint8Array(CUSTODY_REPLAY_BYTES_V1); replay.set(CUSTODY_REPLAY_MAGIC_V1, 0);
  putU16(replay, CUSTODY_REPLAY_VERSION_OFFSET_V1, CUSTODY_ABI_VERSION_V1); replay[CUSTODY_REPLAY_STATUS_OFFSET_V1] = 1;
  replay[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] = EXECUTION_ROLE_CLAIMS_V1; putU32(replay, 12, 1);
  replay.set(releaseSet, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1); replay.set(new PublicKey(market).toBytes(), CUSTODY_REPLAY_MARKET_OFFSET_V1);
  replay.set(realm, CUSTODY_REPLAY_REALM_OFFSET_V1); replay.set(context, CUSTODY_REPLAY_CONTEXT_OFFSET_V1);
  replay.set(new PublicKey(claimsProgram).toBytes(), CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1); replay.set(bytes(25), CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1);
  putU64(replay, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, 5n); putU64(replay, CUSTODY_REPLAY_GENERATION_OFFSET_V1, 3n);
  replay.set(bytes(26), 224); replay.set(bytes(27), 256);
  const request = Object.freeze({
    releaseSet: id(11), market, realm: id(15), parentContext: id(28), productRecordDigest: id(19),
    exposureId: id(20), exposureDigest: id(21), terminalRecordDigest: hex(new PublicKey(terminalCertificate).toBytes()), owner, position,
    recipientOwner: owner, recipient: route.recipient, claimsProgram, custodyProgram, collateralMint,
    tokenProgram: LEGACY_TOKEN_PROGRAM, semanticBasisId: id(14), linkedBasisRecordDigest: id(18),
    generation: '3', expectedMarketRevision: '7', expectedPositionRevision: '11', expectedCustodyRevision: '5',
    quantity: '2', claimIndex: 1, transferIndex: 0,
  });
  const requestBytes = encodeWalletTerminalPayoutRequestV3(request);
  const signedPacket = new Uint8Array(336); signedPacket.set(new TextEncoder().encode('DCLSDP03'), 0); putU16(signedPacket, 8, 3); signedPacket[10] = 1;
  signedPacket.set(releaseSet, 16); signedPacket.set(new PublicKey(market).toBytes(), 48);
  // request identity is filled asynchronously below by the fixture caller.
  signedPacket.set(bytes(19), 112); signedPacket.set(bytes(14), 144); signedPacket.set(bytes(18), 176);
  putU64(signedPacket, 208, 7n); putU32(signedPacket, 216, 2); putU32(signedPacket, 220, 1); putU32(signedPacket, 224, 1);
  signedPacket.set(new PublicKey(owner).toBytes(), 240); putU64(signedPacket, 272, 11n);
  signedPacket[296] = 2; putU64(signedPacket, 304, 2n); putU32(signedPacket, 312, 0); putU32(signedPacket, 316, 1); signedPacket[320] = 2; putU64(signedPacket, 328, 2n);
  return Object.freeze({
    observedSlot: '99', route, custodyContext: id(16), request, signedPacket, payout: '2',
    aggregateBytes, positionBytes, custodyReplayBytes: replay,
    hoardTokenBytes: token(collateralMint, custodyAuthority.toBase58(), 100n),
    recipientTokenBytes: token(collateralMint, owner, 9n),
    // Preserve the exact request bytes so the caller can fill SHA-256@80.
    __requestBytes: requestBytes,
  } as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array });
}

async function report(): Promise<WalletTerminalPayoutReportV3> {
  const input = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
  input.signedPacket.set(await sha256(input.__requestBytes), 80);
  return buildWalletTerminalPayoutV3(input);
}

describe('wallet terminal payout v3', () => {
  it('hostile-decodes the bounded payout manifest and refuses unknown authority', async () => {
    const input = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
    input.signedPacket.set(await sha256(input.__requestBytes), 80);
    const manifest = {
      format: 'dclutch-wallet-terminal-payout-v3', route: input.route,
      custodyContext: input.custodyContext, request: input.request,
      signedPacketBase64: btoa(String.fromCharCode(...input.signedPacket)),
      payout: input.payout, lookupTable: key(241),
    };
    expect(parseWalletTerminalPayoutManifestV3(JSON.stringify(manifest)).request.position).toBe(input.route.position);
    expect(() => parseWalletTerminalPayoutManifestV3(JSON.stringify({ ...manifest, unchecked: true }))).toThrow('missing or unknown');
    expect(() => parseWalletTerminalPayoutManifestV3(JSON.stringify({ ...manifest, signedPacketBase64: 'AA' }))).toThrow('canonical base64');
    const rest = Object.fromEntries(Object.entries(input.route).filter(([field]) => ![
      'terminalCertificate', 'resolutionProgram', 'resolutionProgramData',
    ].includes(field)));
    expect(() => parseWalletTerminalPayoutManifestV3(JSON.stringify({
      ...manifest,
      route: { ...rest, terminalCoordinateRaw: key(201), terminalCoordinateStaging: key(202) },
    }))).toThrow('missing or unknown');
  });

  it('emits the exact 640-byte Claims request vector and 36-account certificate frame', async () => {
    const built = await report();
    expect(built.requestBytes).toHaveLength(640);
    expect(new TextDecoder().decode(built.requestBytes.slice(0, 8))).toBe('DCLTSQ03');
    expect(built.requestBytes[10]).toBe(EXECUTION_ROLE_CLAIMS_V1);
    expect(built.requestBytes.slice(11, 16)).toEqual(new Uint8Array(5));
    expect(hex(built.requestBytes.slice(16, 48))).toBe(id(11));
    expect(new PublicKey(built.requestBytes.slice(272, 304)).toBase58()).toBe(key(17));
    expect(new DataView(built.requestBytes.buffer, built.requestBytes.byteOffset + 624, 8).getBigUint64(0, true)).toBe(2n);
    expect(built.instruction.keys).toHaveLength(TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3);
    expect(built.instruction.keys[0]).toMatchObject({ isSigner: true, isWritable: false });
    expect(built.instruction.keys[1]).toMatchObject({ isSigner: false, isWritable: true });
    expect(built.instruction.keys[20]).toMatchObject({ pubkey: new PublicKey(built.route.position), isWritable: true });
    expect(built.instruction.keys[23]?.pubkey.toBase58()).toBe(built.custodyCaller);
    expect(built.instruction.keys[TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3]?.pubkey.toBase58()).toBe(built.route.terminalCertificate);
    expect(built.instruction.keys[TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3]?.pubkey.toBase58()).toBe(built.route.resolutionProgram);
    expect(built.instruction.keys[TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3]?.pubkey.toBase58()).toBe(built.route.resolutionProgramData);
    for (const index of [TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3, TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3, TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3]) {
      expect(built.instruction.keys[index]).toMatchObject({ isSigner: false, isWritable: false });
    }
    expect(built.instruction.keys[32]).toMatchObject({ isWritable: true });
    expect(built.instruction.keys[33]).toMatchObject({ isWritable: true });
    expect(hex(built.requestDigest)).toBe('956ad1ac4483ad68bbce95466bcb64bfdf61ecf0e9fc0e00d91dead0fdecbeb2');
  });

  it('compiles only through the exact same-observation canonical ALT', async () => {
    const built = await report(); const payer = key(242);
    const addresses = canonicalWalletTerminalPayoutLookupAddressesV3(built, payer);
    const table = new AddressLookupTableAccount({ key: new PublicKey(key(241)), state: {
      deactivationSlot: MAX_U64, lastExtendedSlot: 98, lastExtendedSlotStartIndex: 0,
      authority: new PublicKey(key(240)), addresses: addresses.map((address) => new PublicKey(address)),
    } });
    const plan = compileWalletTerminalPayoutV0(built, { payer, recentBlockhash: key(31), lookupTable: table, lookupObservedSlot: '99' });
    expect(plan.requiredSigners).toEqual([payer, built.request.owner]);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    const reordered = new AddressLookupTableAccount({ key: table.key, state: { ...table.state, addresses: [table.state.addresses[1]!, table.state.addresses[0]!, ...table.state.addresses.slice(2)] } });
    expect(() => compileWalletTerminalPayoutV0(built, { payer, recentBlockhash: key(31), lookupTable: reordered, lookupObservedSlot: '99' })).toThrow('sole canonical');
    expect(() => compileWalletTerminalPayoutV0(built, { payer, recentBlockhash: key(31), lookupTable: table, lookupObservedSlot: '100' })).toThrow('payout prestate observation');
  });

  it('refuses SignedDelta and physical-route substitutions before a wallet sees a message', async () => {
    const wrongPacket = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
    wrongPacket.signedPacket.set(await sha256(wrongPacket.__requestBytes), 80); wrongPacket.signedPacket[320] = 1;
    await expect(buildWalletTerminalPayoutV3(wrongPacket)).rejects.toThrow('Position delta');
    const wrongRoute = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
    wrongRoute.signedPacket.set(await sha256(wrongRoute.__requestBytes), 80);
    await expect(buildWalletTerminalPayoutV3({ ...wrongRoute, route: { ...wrongRoute.route, recipient: key(200) } })).rejects.toThrow('substitutes a request coordinate');
    const wrongCertificate = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
    wrongCertificate.signedPacket.set(await sha256(wrongCertificate.__requestBytes), 80);
    await expect(buildWalletTerminalPayoutV3({ ...wrongCertificate, route: { ...wrongCertificate.route, terminalCertificate: key(200) } }))
      .rejects.toThrow('differs from the Core terminal receipt');
    const wrongProgramData = fixture() as WalletTerminalPayoutBuildInputV3 & { __requestBytes: Uint8Array };
    wrongProgramData.signedPacket.set(await sha256(wrongProgramData.__requestBytes), 80);
    await expect(buildWalletTerminalPayoutV3({ ...wrongProgramData, route: { ...wrongProgramData.route, resolutionProgramData: key(200) } }))
      .rejects.toThrow('canonical Loader-v3 authority coordinate');
  });

  it('accepts the exact finalized receipt/resources and refuses one changed token byte', async () => {
    const built = await report(); const request = built.request;
    const aggregate = new Uint8Array(built.preAggregateBytes); putU64(aggregate, LIABILITY_BASIS_MARKET_REVISION_OFFSET, 8n); putU64(aggregate, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8, 5n);
    const position = new Uint8Array(built.prePositionBytes); putU64(position, LIABILITY_BASIS_POSITION_REVISION_OFFSET, 12n); putU64(position, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8, 2n);
    const replay = new Uint8Array(built.preCustodyReplayBytes); putU64(replay, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, 6n); replay.set(built.custodyRequestDigest, 224);
    replay.set(await hashv(new TextEncoder().encode('dclutch:custody-poststate:v1'), built.custodyRequestDigest,
      new PublicKey(built.route.hoard).toBytes(), new PublicKey(built.route.recipient).toBytes(),
      le64(100n), le64(98n), le64(9n), le64(11n), le64(0n)), 256);
    const hoard = token(request.collateralMint, built.route.custodyAuthority, 98n);
    const recipient = token(request.collateralMint, request.recipientOwner, 11n);
    const signedPost = await hashv(new TextEncoder().encode('dclutch/claims/signed-delta-post-resources/v3'), aggregate, position);
    const replayDigest = await sha256(replay);
    const tokenPost = await hashv(new TextEncoder().encode('dclutch/claims-terminal-token-poststate/v3'), hoard, recipient);
    const custodyReceipt = bytes(30);
    const allPost = await hashv(new TextEncoder().encode('dclutch/claims-terminal-postresources/v3'), built.requestDigest, signedPost, replayDigest, tokenPost, custodyReceipt);
    const receipt = new Uint8Array(1_008); receipt.set(new TextEncoder().encode('DCLTSA03'), 0); putU16(receipt, 8, 3); receipt.set(built.requestBytes, 16);
    for (const [offset, value] of [[656, built.requestDigest], [688, built.signedPacketDigest], [720, built.signedTableDigest], [752, signedPost], [784, built.custodyRequestDigest], [816, custodyReceipt], [848, replayDigest], [880, tokenPost], [912, allPost]] as const) receipt.set(value, offset);
    for (const [offset, value] of [[944, 2n], [952, 7n], [960, 8n], [968, 11n], [976, 12n], [984, 5n], [992, 6n]] as const) putU64(receipt, offset, value);
    const post = { receiptBytes: receipt, aggregateBytes: aggregate, positionBytes: position, custodyReplayBytes: replay, hoardTokenBytes: hoard, recipientTokenBytes: recipient };
    await expect(verifyWalletTerminalPayoutPostconditionV3(built, post)).resolves.toBeUndefined();
    const changed = new Uint8Array(recipient); changed[64] ^= 1;
    await expect(verifyWalletTerminalPayoutPostconditionV3(built, { ...post, recipientTokenBytes: changed })).rejects.toThrow('token payout poststate');

    const payer = built.request.owner;
    const lookupAddress = key(241);
    const lookupAddresses = canonicalWalletTerminalPayoutLookupAddressesV3(built, payer);
    const table = new AddressLookupTableAccount({ key: new PublicKey(lookupAddress), state: {
      deactivationSlot: MAX_U64, lastExtendedSlot: 98, lastExtendedSlotStartIndex: 0,
      authority: new PublicKey(payer), addresses: lookupAddresses.map((address) => new PublicKey(address)),
    } });
    const compiled = compileWalletTerminalPayoutV0(built, {
      payer, recentBlockhash: key(31), lookupTable: table, lookupObservedSlot: '99',
    });
    const plan = Object.freeze({ ...compiled, lookupTable: lookupAddress }) as PreparedWalletTerminalPayoutV3;
    const signed = VersionedTransaction.deserialize(plan.wireBytes);
    signed.signatures[0] = new Uint8Array(64).fill(7);
    const signedWire = signed.serialize();
    const signature = 'saved-exact-payout-signature';
    const meta: TransactionMetaObservation = Object.freeze({
      signature, slot: '100', blockTime: null, succeeded: true, errorText: null,
      feeLamports: '5', accountAddresses: Object.freeze([payer]),
      preBalances: Object.freeze(['100']), postBalances: Object.freeze(['95']),
      logMessages: Object.freeze([]),
      returnData: Object.freeze({ programId: built.route.claimsProgram, data: receipt }),
      transactionBytes: signedWire,
    });
    const account = (owner: string, data: Uint8Array): RpcAccount => Object.freeze({
      owner, data, executable: false, lamports: '1', space: data.length,
    });
    const rows = Object.freeze([
      Object.freeze({ address: built.route.aggregate, account: account(built.route.claimsProgram, aggregate) }),
      Object.freeze({ address: built.route.position, account: account(built.route.claimsProgram, position) }),
      Object.freeze({ address: built.route.custodyReplay, account: account(built.route.custodyProgram, replay) }),
      Object.freeze({ address: built.route.hoard, account: account(built.route.tokenProgram, hoard) }),
      Object.freeze({ address: built.route.recipient, account: account(built.route.tokenProgram, recipient) }),
    ]);
    const client = (changedMeta = meta, observedSlot = '101', changedRows = rows) => Object.freeze({
      transaction: async () => changedMeta,
      finalizedSlot: async () => '101',
      multipleAccounts: async () => Object.freeze({ slot: observedSlot, accounts: changedRows }),
    });
    await expect(finalizeWalletTerminalPayoutV3(client(), signature, plan, signedWire)).resolves.toEqual({
      signature, observedSlot: '101', payout: built.payout,
    });
    const changedWire = new Uint8Array(signedWire); changedWire[changedWire.length - 1] ^= 1;
    expect(() => verifyFinalizedWalletTerminalPayoutTransactionV3(
      { ...meta, transactionBytes: changedWire }, signature, plan, signedWire,
    )).toThrow(/wire bytes/);
    expect(() => verifyFinalizedWalletTerminalPayoutTransactionV3(
      { ...meta, postBalances: Object.freeze(['94']) }, signature, plan, signedWire,
    )).toThrow(/payer fee/);
    expect(() => verifyFinalizedWalletTerminalPayoutTransactionV3(
      { ...meta, accountAddresses: Object.freeze([key(99)]) }, signature, plan, signedWire,
    )).toThrow(/fee payer/);
    expect(() => verifyFinalizedWalletTerminalPayoutTransactionV3(
      { ...meta, returnData: null }, signature, plan, signedWire,
    )).toThrow(/Claims-produced/);
    await expect(finalizeWalletTerminalPayoutV3(client(meta, '100'), signature, plan, signedWire))
      .rejects.toThrow(/regressed below/);
    await expect(finalizeWalletTerminalPayoutV3(
      client(meta, '101', Object.freeze([...rows].reverse())), signature, plan, signedWire,
    )).rejects.toThrow(/ordered account closure/);
  });
});
