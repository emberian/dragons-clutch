# Client developers

You are building something that talks to dClutch — a bot, a dashboard, an
integration, a tool of your own. This guide shows you the pieces you build
with and one working example of each core flow. The exact tables — every
account layout, every instruction, every error code — live in the
[generated reference](../reference/README.md); this page shows you the code.

Two packages in this repository are yours:

- **`@dclutch/sdk`** (`packages/dclutch-sdk`) — the client library. Typed
  decoders for every on-chain record, transaction builders for the core
  flows, and a small read-only RPC client. It never opens a connection,
  touches a key, or submits a transaction by itself: you hand it an endpoint,
  and a caller-specific durable workflow owns signing and submission.
- **`@dclutch/cli`** (`packages/dclutch-cli`) — the `dclutch` terminal
  client, built entirely on the SDK. When you wonder how to wire a flow up,
  read the command that already does it (`src/commands/` is ~200 lines per
  flow).

Neither is on npm yet. Inside the repository, depend on the SDK with
`"@dclutch/sdk": "file:../../packages/dclutch-sdk"` and import modules
directly: `@dclutch/sdk/rpc`, `@dclutch/sdk/marketDiscovery`, and so on.

## Reading markets

Everything starts with a `SolanaRpcClient` and a set of program ids. Program
ids come from whatever run you are targeting — a local run spec, or the
session file `dclutch-terminal found` writes.

```ts
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
} from '@dclutch/sdk/marketDiscovery';

const client = new SolanaRpcClient('http://127.0.0.1:19790/');
const found = await enumerateCoreMarketAddressesV1(client, CORE_PROGRAM_ID);
const discovery = await inspectMarketDiscoveryV1(client, {
  coreProgramId: CORE_PROGRAM_ID,
  addresses: found.addresses,
});
for (const card of discovery.cards) {
  console.log(card.address, card.status === 'decoded' ? card.phase : card.refusal);
}

if (found.mode === 'program-scan') {
  for (const old of found.incompatibleMarketAccounts) {
    console.log(old.address, old.magic, old.accountBytes, 'historical; not current');
  }
}
```

A card decodes fully or comes back as `refused` with a reason. A refusal
here means the account did not match the layout exactly — treat it as "not
a market I can use", not as an error to retry.

The scan lists current 360-byte `DCLTCOR3` Markets separately from exact
historical 352-byte `DCLTCOR2` accounts. It never decodes the historical bytes
as current state. Read `incompatibleMarketAccounts` when you need to explain
why an older address is absent from the current list.

Each read starts from a finalized floor. A multi-account response is
internally consistent at its returned slot, which may be later than that
floor. A longer workflow is not one atomic snapshot: carry the returned
slot forward as the minimum for the next dependent read, and recheck mutable
facts before you ask for a signature. `inspectMarketDetailV1`
(`@dclutch/sdk/marketDetail`) gives you one market in full;
`inspectPortfolioV1` (`@dclutch/sdk/portfolio`) derives a user's positions
directly from market addresses — no indexer involved.

For a terminal Market, Core's `terminal_receipt` is the address of the
Resolution-owned `ResolutionCertificateV2` account. Do not use that pubkey as
a content digest or derive Registry raw/staging records from it. Decode the
exact 312-byte certificate, preserve its i128 numerator and u64 denominator,
and bind its receipt account, Market, Source material, Product-record digest,
generation, and native u32 selector back to Core before displaying a payout.
The SDK's `decodeResolutionCertificateV2` and
`bindTerminalResolutionCertificateV2` do those checks. A graded payout remains
display-only until Claims is upgraded to consume that certificate directly.

## Checking the founding principal cap

Import the generated floor identity and the decoder from the SDK. The decoder
checks the exact 160-byte layout, magic, version, reserved bytes, basis, and all
four nonzero identities. Bind the decoded floor to your authenticated Source,
adapter configuration, and collateral unit before you use its number.

```ts
import {
  admitFoundingPrincipalV1,
  admitGenericFoundingQuantityV1,
  decodeManipulationFloorV1,
  projectPrincipalCapSetsV1,
} from '@dclutch/sdk';

const floor = decodeManipulationFloorV1(floorRecordBytes);
const atomVerdict = admitFoundingPrincipalV1(capacity, floor, {
  sourceSpecId,
  adapterConfigId,
  collateralUnitId,
}, proposedPrincipalAtoms);

const principalCapSets = projectPrincipalCapSetsV1(
  atomVerdict.largestAdmittedPrincipal ?? 0n,
  basisScale,
);
const quantityVerdict = admitGenericFoundingQuantityV1(
  principalCapSets,
  proposedCompleteSets,
);
```

That last verdict mirrors the current Core generic Found check. On chain,
ProjectFound authenticates the whole Source graph and performs the same named
floor division before generic Found mutates anything; Core then persists
`principal_cap_sets` in the Market root. Your calculation is a preview until
those chain bindings are authenticated.

## Reviewing a Direct trade

A Direct trade settles between two signed **intents**: one seller and one
buyer, crossed at one execution price. The public SDK currently lets you
authenticate a route and preview that arithmetic. It deliberately does not
offer a safe signing or submission workflow yet.

```ts
import {
  previewDirectInlineV3,
} from '@dclutch/sdk/directInlineV3';
import { inspectDirectHotRouteV3 } from '@dclutch/sdk/directHotChain';

// 1. Acquire the route: the market's fixed trading accounts, read back and
//    checked at one finalized slot.
const inspection = await inspectDirectHotRouteV3(client, routeManifest);

// 2. Describe unsigned terms. Do not ask either maker for a signature.
const buyerIntent = {
  side: 1, lifecycle: 0, outcome: 1,
  market: inspection.route.market,
  generation: inspection.route.generation,
  nonce: 1n,
  validFrom: BigInt(inspection.observedSlot),
  validThrough: BigInt(inspection.observedSlot) + 150n,
  maximumFill: 5n, limitPrice: 400_000n,
  feeBasisPoints: inspection.route.feeBasisPoints,
  collateralAccount: MY_COLLATERAL_ACCOUNT,
} as const;

// 3. Review exact integer arithmetic only.
const preview = previewDirectInlineV3(
  inspection.route,
  { intent: sellerIntent },
  { intent: buyerIntent },
  5n,
  400_000n,
  BigInt(inspection.observedSlot),
);
```

`preview` tells you the exact collateral debit, credit, and fees at the
chosen integer rounding boundary. It is not a transaction and does not prove
that a fill landed. The CLI can still create an authenticated off-chain intent
file, but `buy` and `sell` refuse before session, key, transaction, or RPC
access. They reopen only after one caller owns a durable exact-packet journal,
the authenticated Trading acknowledgement, and all ten ordered writable
poststates from finalized history.

## Redeeming

Payout is not open. There is no devnet Market that can use this route, and
the accepted local run stops after admitting a participant, so there is no
chain you can execute this against today either. Read what follows as the
shape the flow will have, not as a path you can run — the constructor and
finalizer below are real and tested, and what is missing is a resolved
market to point them at. The full flow has three
separately finalized parts: create the market's
payment record if it does not exist, publish and freeze the payout lookup
table, then sign the payout itself. Do not combine those steps into one
optimistic submit loop.

The current CLI accepts an already-finalized payout lookup table. When a table
still needs creation or extension, it saves the checked plan and stops before
loading your key. Those mutations remain closed until each exact packet has its
own durable Submitted journal and finalized readback.

`inspectClaimsCustodyReplayV1` (`@dclutch/sdk/claimsCustodyReplay`) can inspect
that first record and compile the exact unsigned plan when it is absent. The
public SDK deliberately stops there: it does not expose a transaction-submit
transport. Give the plan to the same kind of caller-specific journal described
below, persist its unsigned bytes before opening a wallet, and reacquire the
finalized record before preparing the payout. Do not sign or submit directly
from an inspection branch.

After that payment record and the lookup table are finalized, prepare the
payout from the current accounts. Save the unsigned plan before opening a
wallet. After the wallet signs, save both the complete signed bytes and the
transaction id before the only submission. If the RPC response is lost,
poll that saved id; never rebuild, resign, or resend the payout. The public SDK
does not expose a generic sign-and-send helper: such a helper cannot prove that
the workflow durably crossed those phases. Use the CLI's payout journal or
build a caller-specific journal with the same phase boundary.

```ts
import {
  finalizeWalletTerminalPayoutV3,
  prepareWalletTerminalPayoutV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';

const plan = await prepareWalletTerminalPayoutV3(client, manifest, owner);
await saveUnsignedPlan(plan); // durable before the wallet opens

// Your caller-specific journal now owns wallet signing, Signed persistence,
// Submitted persistence, and the sole maxRetries=0 send. It returns the exact
// saved id and bytes; it never rebuilds or resends a Submitted packet.
const { transactionId, signedWireBytes } = await payoutJournal.submitOnce(plan);
const completed = await finalizeWalletTerminalPayoutV3(
  client,
  transactionId,
  plan,
  signedWireBytes,
);
```

Your completion check must read the exact finalized transaction bytes,
message, signatures, fee payer and lamport changes, Claims return receipt,
and the five changed accounts. It starts its account read at a finalized
floor at or above the transaction slot; the response may be at a later
slot. The SDK finalizer performs those checks and refuses altered wire
bytes, signatures, fees, return data, account order, or payout poststate.
The `dclutch-terminal redeem` command adds a durable filesystem journal and is the
reference for local/custom-deployment crash recovery. Its presence does not
mean a devnet payout is currently available.

## Founding a market

Founding is driven by a run spec — a JSON file naming the programs, the
market recipe, and where the evidence goes. The producer binary does the
work; wrap it the way `dclutch-terminal found` does
(`packages/dclutch-cli/src/commands/found.ts`). Do not start from
`tools/gauntlet/run.sh --mode full`: at HEAD it is intentionally unavailable
and refuses before work or a build because no supported top-level planner is
complete. A supported named family runner documents its own inputs and evidence
boundary.

## When a transaction fails

A failed dClutch transaction carries a `custom program error` code, and the
code alone tells you which program refused: `code >> 12` is the program's
band (codes below `0x1000` are some other program's — SPL Token, the
loader). The SDK turns any code into a name and a sentence:

```ts
import { renderRefusal, customCodeFromTransactionError } from '@dclutch/sdk/refusals';

const code = customCodeFromTransactionError(rpcError); // {"InstructionError":[0,{"Custom":n}]}
if (code !== null) console.error(renderRefusal(code).text);
// -> claims refused: ClaimsSbfError::Instruction (0x5000) — Instruction
//    bytes were hostile or selected no supported family.
```

Always surface the rendered name. Your user can act on "claims refused:
identity did not join the packet"; they cannot act on `0x5002`. The full
table is [refusals.md](../reference/refusals.md).

## The layouts are generated — treat them that way

Every byte offset, magic, and account table in `@dclutch/sdk` under
`lib/generated/` is emitted from the protocol's own source and checked
byte-for-byte by `npm test` in the SDK package. If you need a layout the
SDK does not export yet, add a generator the way the existing eighteen work
(`packages/dclutch-sdk/scripts/`) instead of typing offsets in — hand-typed
offsets are how a client ends up confidently wrong about what an account
says. The [ABI tables](../reference/abi/README.md) are the same data,
rendered for reading.
