import { PublicKey } from '@solana/web3.js';

import { classifyHeader } from './decoders';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const OPERATOR_ROLES = ['registry', 'core', 'trading', 'claims', 'custody', 'resolution'] as const;
export type OperatorRole = (typeof OPERATOR_ROLES)[number];

export type OperatorCoordinatesV1 = Readonly<Record<OperatorRole, string>> & Readonly<{
  market?: string;
}>;

export type OperatorRoleObservationV1 = Readonly<{
  role: OperatorRole;
  address: string;
  owner: string;
  executable: boolean;
  dataBytes: number;
}>;

export type OperatorSurfaceSnapshotV1 = Readonly<{
  observedSlot: string;
  roles: ReadonlyArray<OperatorRoleObservationV1>;
  market: null | Readonly<{
    address: string;
    owner: string;
    dataBytes: number;
    header: string | null;
  }>;
}>;

export type WorkflowStatus = 'constructible' | 'request-only' | 'awaiting-abi';

export type OperatorWorkflowV1 = Readonly<{
  family: 'Release' | 'Creation' | 'Direct' | 'Source' | 'Series' | 'General' | 'Dealer' | 'Claims';
  action: string;
  status: WorkflowStatus;
  route: string | null;
  exactBoundary: string;
}>;

export const OPERATOR_WORKFLOWS: ReadonlyArray<OperatorWorkflowV1> = Object.freeze([
  { family: 'Release', action: 'Activate checked multiprogram release', status: 'constructible', route: '/release', exactBoundary: 'Registry cache, six executable roles, Loader deployment identity, and recent blockhash are reacquired.' },
  { family: 'Release', action: 'Reauthenticate one program role', status: 'constructible', route: '/release', exactBoundary: 'The active Registry cache selects the exact role program and ProgramData.' },
  { family: 'Creation', action: 'Compile Product V2 result domain', status: 'constructible', route: '/product-v2', exactBoundary: 'Canonical Product bytes and content identities are emitted; no Market is claimed.' },
  { family: 'Creation', action: 'Found physical economic projection', status: 'constructible', route: '/economic', exactBoundary: 'Market, Realm, release, vacancy, Hoard custody, rent, and payer are reacquired at one finalized floor.' },
  { family: 'Creation', action: 'Found common Core Market', status: 'awaiting-abi', route: null, exactBoundary: 'Unavailable until immutable Core infrastructure binds exact Registry and Rent program plus ArtifactRelease identities; caller-entered programs are not authority.' },
  { family: 'Direct', action: 'Create registered order', status: 'awaiting-abi', route: null, exactBoundary: 'The current screen targets the superseded registration ABI; successor config and maker-root ownership are not frozen.' },
  { family: 'Direct', action: 'Fill inline or registered intents', status: 'awaiting-abi', route: null, exactBoundary: 'TransitionVM V2 exists, but Ed25519 evidence, AccountProfile projection, and child receipts are not yet one accepted outer.' },
  { family: 'Direct', action: 'Cancel / expire / CancelThrough', status: 'awaiting-abi', route: null, exactBoundary: 'Successor maker-root replay and terminal account frames are still being integrated.' },
  { family: 'Source', action: 'Create and fund resolution', status: 'request-only', route: '/local', exactBoundary: 'The Rust operator constructs the current frame; the browser has canonical chain decoding but no accepted successor transaction encoder.' },
  { family: 'Source', action: 'Resolve from real provider / failure path', status: 'request-only', route: '/local', exactBoundary: 'Real provider execution is inspectable; the common Resolution child request/receipt outer is not frozen in the browser.' },
  { family: 'Source', action: 'Recover / archive / retire Source', status: 'awaiting-abi', route: null, exactBoundary: 'Terminal successor dispatch and exact rent destinations are not yet a frozen browser ABI.' },
  { family: 'Series', action: 'Prepare occurrence and ticket', status: 'request-only', route: '/local', exactBoundary: 'The Rust operator owns the current creation frame; generated Series V2 account projection is still landing.' },
  { family: 'Series', action: 'Consume ticket into Found Market', status: 'awaiting-abi', route: null, exactBoundary: 'Atomic ticket-to-Found composition must consume the accepted common Core Found outer.' },
  { family: 'Series', action: 'Expire ticket / close occurrence / root', status: 'awaiting-abi', route: null, exactBoundary: 'Successor terminal frames and child-count closure are not frozen.' },
  { family: 'General', action: 'Consider candidate / freeze selection', status: 'constructible', route: '/general', exactBoundary: 'Generated account bodies, policy, selection revision, Registry activation, and Loader identity are reacquired.' },
  { family: 'General', action: 'Initialize settlement', status: 'constructible', route: '/general', exactBoundary: 'The verified certificate and vacant settlement PDA are exact finalized inputs.' },
  { family: 'General', action: 'Collect / materialize / distribute', status: 'request-only', route: '/general', exactBoundary: 'Exact 64-byte requests are available; Claims/Custody child wires remain deliberately unavailable.' },
  { family: 'General', action: 'Close settlement / General root', status: 'awaiting-abi', route: null, exactBoundary: 'Terminal postconditions and rent routing have not reached an accepted common outer.' },
  { family: 'Dealer', action: 'Activate custodied pool', status: 'request-only', route: null, exactBoundary: 'The Rust operator preflights current custody, funding, and rent; browser successor activation is not frozen.' },
  { family: 'Dealer', action: 'Create LP / add / remove liquidity', status: 'request-only', route: null, exactBoundary: 'The current Rust operator derives all accounts and custody movements; no successor browser encoder is accepted.' },
  { family: 'Dealer', action: 'Inventory-bounded immediate trade', status: 'awaiting-abi', route: null, exactBoundary: 'The shared Trading outer and conditional fixed-role Claims/Custody receipts must land first.' },
  { family: 'Dealer', action: 'Reset ladder / close LP / retire pool', status: 'request-only', route: null, exactBoundary: 'Current Rust routes exist; successor replay and terminal account profiles are not frozen.' },
  { family: 'Claims', action: 'Split / merge complete set', status: 'constructible', route: '/economic', exactBoundary: 'The economic successor re-decodes release authority and exact conservative supply before building.' },
  { family: 'Claims', action: 'Materialize / dematerialize representation', status: 'constructible', route: '/economic', exactBoundary: 'Exact native/materialized supply and custody effects are simulated from chain state.' },
  { family: 'Claims', action: 'Bearer mint / unwrap / redeem / retire', status: 'awaiting-abi', route: null, exactBoundary: 'LiabilityBasisV2 and rational representation exist, but the accepted Token-2022 outer is not yet frozen.' },
]);

function canonicalKey(value: string, field: string): string {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function requireAccount(account: RpcAccount | null, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  return account;
}

export async function acquireOperatorSurfaceV1(
  client: SolanaRpcClient,
  coordinates: OperatorCoordinatesV1,
): Promise<OperatorSurfaceSnapshotV1> {
  const roleAddresses = OPERATOR_ROLES.map((role) => canonicalKey(coordinates[role], `${role} program`));
  if (new Set(roleAddresses).size !== roleAddresses.length) throw new Error('multiprogram roles must have distinct executable program identities');
  const market = coordinates.market === undefined || coordinates.market === ''
    ? null
    : canonicalKey(coordinates.market, 'Market');
  if (market !== null && roleAddresses.includes(market)) throw new Error('Market aliases an executable program role');
  const floor = await client.finalizedSlot();
  const addresses = market === null ? roleAddresses : [...roleAddresses, market];
  const observation = await client.multipleAccounts(addresses, floor);
  const roles = OPERATOR_ROLES.map((role, index) => {
    const account = requireAccount(observation.accounts[index].account, `${role} program`);
    if (!account.executable) throw new Error(`${role} program is not executable`);
    return Object.freeze({
      role,
      address: roleAddresses[index],
      owner: account.owner,
      executable: account.executable,
      dataBytes: account.data.length,
    });
  });
  let marketObservation: OperatorSurfaceSnapshotV1['market'] = null;
  if (market !== null) {
    const account = requireAccount(observation.accounts[roleAddresses.length].account, 'Market');
    if (account.executable) throw new Error('Market is executable');
    if (account.owner !== coordinates.core) throw new Error('Market is not owned by the selected Core program');
    marketObservation = Object.freeze({
      address: market,
      owner: account.owner,
      dataBytes: account.data.length,
      header: classifyHeader(account.data),
    });
  }
  return Object.freeze({ observedSlot: observation.slot, roles: Object.freeze(roles), market: marketObservation });
}
