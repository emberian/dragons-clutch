import { VersionedTransaction } from '@solana/web3.js';
import { hex, sha256 } from './bytes';
import {
  decodeGeneralHotReceiptV3, decodeGeneralLocalStateV3, decodeGeneralVerifiedCandidateV2,
  reacquireGeneralSuccessorStatusV5, transactionBytesV5,
  type GeneralPlanInspectionV5,
} from './generalPlanV5';
import { LIABILITY_BASIS_POSITION_MAGIC_V2 } from './generated/coreFound';
import { TOKEN_ACCOUNT_BYTES_V1, TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1 } from './generated/walletTerminalPayoutV3';
import { decodeToken2022BehaviorAccountV2, TOKEN_2022_PROGRAM_ID } from './rationalTokenV2';
import { decodeClaimsPositionV2 } from './marketCoreV2';
import { MAX_MULTIPLE_ACCOUNTS, type MultipleAccountObservation, type RpcAccount, type SolanaRpcClient } from './rpc';

export function generalPacketBase64V5(bytes: Uint8Array): string { return btoa(String.fromCharCode(...bytes)); }
function same(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((byte, index) => byte === right[index]); }

/** Wallet authority is the user's selected deployment, never the imported plan's program field. */
export function assertGeneralDeploymentV5(inspection: GeneralPlanInspectionV5, selectedTradingProgram: string): void {
  if (inspection.plan.tradingProgram !== selectedTradingProgram) throw new Error('General native plan differs from the selected Trading deployment');
}

export async function generalAccountCommitmentV5(account: RpcAccount | null): Promise<string> {
  return hex(await sha256(new TextEncoder().encode(JSON.stringify(account === null ? null : {
    owner: account.owner, executable: account.executable, lamports: account.lamports,
    space: account.space, data: hex(await sha256(account.data)),
  }))));
}

export type GeneralAccountReviewV5 = Readonly<{
  address: string; beforeCommitment: string; beforeLamports: string; afterLamports: string;
  beforeToken: Readonly<{ mint: string; owner: string; amount: string }> | null;
  afterToken: Readonly<{ mint: string; owner: string; amount: string }> | null;
  beforeClaims: ReturnType<typeof decodeClaimsPositionV2> | null;
  afterClaims: ReturnType<typeof decodeClaimsPositionV2> | null;
}>;
export type GeneralExecutionPreviewV5 = Readonly<{
  messageDigest: string; observedSlot: string; computeUnits: string | null; rootPoststateDigest: string;
  accounts: ReadonlyArray<GeneralAccountReviewV5>;
}>;

function claims(address: string, account: RpcAccount | null): ReturnType<typeof decodeClaimsPositionV2> | null {
  if (account === null || !same(account.data.subarray(0, LIABILITY_BASIS_POSITION_MAGIC_V2.length), LIABILITY_BASIS_POSITION_MAGIC_V2)) return null;
  return decodeClaimsPositionV2(address, account.data);
}

function token(address: string, account: RpcAccount | null): GeneralAccountReviewV5['beforeToken'] {
  if (account === null || account.owner !== TOKEN_2022_PROGRAM_ID || (account.data.length !== TOKEN_ACCOUNT_BYTES_V1 && account.data.length !== TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1)) return null;
  const decoded = decodeToken2022BehaviorAccountV2(address, account);
  return { mint: decoded.mint, owner: decoded.owner, amount: decoded.rawAmount.toString() };
}

/** Native runtime preview, using the imported message unchanged. No TS transition evaluator. */
export async function previewGeneralExecutionV5(client: SolanaRpcClient, inspection: GeneralPlanInspectionV5, selectedTradingProgram: string): Promise<GeneralExecutionPreviewV5> {
  assertGeneralDeploymentV5(inspection, selectedTradingProgram);
  await client.assertMutationCluster();
  const chain = await reacquireGeneralSuccessorStatusV5(client, inspection);
  const addresses = chain.dependencies.dependencies.filter((entry) => entry.writable).map((entry) => entry.address);
  // One simulation must return the whole writable observation, never independent partial runs.
  if (addresses.length > MAX_MULTIPLE_ACCOUNTS) throw new Error('General preview exceeds the 32-account atomic simulation bound');
  const before = await client.multipleAccounts(addresses, chain.observedSlot);
  const root = before.accounts.find((entry) => entry.address === inspection.plan.root)?.account;
  if (!root || root.owner !== inspection.plan.tradingProgram || root.executable
      || hex(await sha256(root.data)) !== inspection.plan.rootPrestateDigest) throw new Error('General root prestate changed; reconstruct the native plan');
  const simulation = await client.simulateTransaction(transactionBytesV5(inspection), addresses, before.slot);
  if (!simulation.succeeded) throw new Error(`General simulation refused: ${simulation.errorText}; consumed ${simulation.computeUnits ?? 'unreported'} CU`);
  if (simulation.returnData?.programId !== inspection.plan.tradingProgram) throw new Error('General simulation omitted the Trading receipt');
  const receipt = decodeGeneralHotReceiptV3(generalPacketBase64V5(simulation.returnData.data), inspection);
  const simulatedRoot = simulation.accounts.find((entry) => entry.address === inspection.plan.root)?.account;
  if (!simulatedRoot || simulatedRoot.owner !== inspection.plan.tradingProgram || simulatedRoot.executable
      || hex(await sha256(simulatedRoot.data)) !== receipt.rootPoststateDigest) throw new Error('General simulation root bytes differ from its receipt');
  const after = await client.multipleAccounts(addresses, simulation.slot);
  const accounts = await Promise.all(before.accounts.map(async (entry, index) => {
    const current = after.accounts[index]; const simulated = simulation.accounts[index];
    if (current?.address !== entry.address || simulated?.address !== entry.address) throw new Error('General preview substituted account order');
    const beforeCommitment = await generalAccountCommitmentV5(entry.account);
    if (beforeCommitment !== await generalAccountCommitmentV5(current.account)) throw new Error('General inputs changed during simulation; review again');
    return Object.freeze({ address: entry.address, beforeCommitment,
      beforeLamports: entry.account?.lamports ?? '0', afterLamports: simulated.account?.lamports ?? '0',
      beforeToken: token(entry.address, entry.account), afterToken: token(entry.address, simulated.account),
      beforeClaims: claims(entry.address, entry.account), afterClaims: claims(entry.address, simulated.account),
    });
  }));
  return Object.freeze({ messageDigest: hex(await sha256(inspection.transaction.transaction.message.serialize())), observedSlot: simulation.slot, computeUnits: simulation.computeUnits, rootPoststateDigest: receipt.rootPoststateDigest, accounts });
}

/** Recheck the exact reviewed writable inputs before each wallet prompt. */
export async function assertGeneralPreviewCurrentV5(client: SolanaRpcClient, preview: GeneralExecutionPreviewV5): Promise<void> {
  const current = await client.multipleAccounts(preview.accounts.map((entry) => entry.address), preview.observedSlot);
  for (const [index, expected] of preview.accounts.entries()) {
    const actual = current.accounts[index];
    if (actual?.address !== expected.address || await generalAccountCommitmentV5(actual.account) !== expected.beforeCommitment) throw new Error('General reviewed input changed; discard unsigned operation and reconstruct the native plan');
  }
}

/** Prove the finalized packet and root commitment; child snapshots remain separately labelled observations. */
export async function observeGeneralExecutionV5(client: SolanaRpcClient, inspection: GeneralPlanInspectionV5, signature: string, signedBytes: Uint8Array) {
  const signed = VersionedTransaction.deserialize(signedBytes);
  if (!same(signed.message.serialize(), inspection.transaction.transaction.message.serialize())) throw new Error('General submitted message differs from the retained native plan');
  const transaction = await client.transaction(signature);
  if (transaction === null) return null;
  if (!same(transaction.transactionBytes, signedBytes)) throw new Error('General finalized transaction differs from the retained signed bytes');
  if (!transaction.succeeded) throw new Error(`General finalized refusal: ${transaction.errorText}`);
  if (transaction.returnData?.programId !== inspection.plan.tradingProgram) throw new Error('General finalized transaction omitted the Trading receipt');
  const receipt = decodeGeneralHotReceiptV3(generalPacketBase64V5(transaction.returnData.data), inspection);
  const states = [inspection.plan.lifecycle.primary, inspection.plan.lifecycle.secondary, inspection.plan.lifecycle.conditionalResult].filter((state) => state !== null);
  const addresses = [...new Set([inspection.plan.root, ...states.map((state) => state.account)])];
  const observation = await client.multipleAccounts(addresses, transaction.slot);
  const root = observation.accounts.find((entry) => entry.address === inspection.plan.root)?.account;
  if (!root || root.owner !== inspection.plan.tradingProgram || root.executable
      || hex(await sha256(root.data)) !== receipt.rootPoststateDigest) throw new Error('General finalized root poststate is unavailable or has advanced beyond this receipt; retain the operation');
  const lifecycle = states.map((state) => {
    const account = observation.accounts.find((entry) => entry.address === state.account)?.account ?? null;
    if (account === null) return { address: state.account, state: 'absent' as const };
    if (account.owner === '11111111111111111111111111111111' && !account.executable && account.data.length === 0) return { address: state.account, state: 'vacant' as const };
    if (account.owner !== inspection.plan.tradingProgram || account.executable) throw new Error('General lifecycle poststate has a substituted owner');
    return { address: state.account, state: state === inspection.plan.lifecycle.conditionalResult ? decodeGeneralVerifiedCandidateV2(account.data) : decodeGeneralLocalStateV3(account.data) };
  });
  return Object.freeze({ transaction, receipt, observedSlot: observation.slot, lifecycle, accounts: observation.accounts as MultipleAccountObservation['accounts'] });
}
