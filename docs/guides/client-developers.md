# Client developers

You are building something that talks to dClutch — a bot, a dashboard, an
integration, a tool of your own. This guide shows you the pieces you build
with and one working example of each core flow. The exact tables — every
account layout, every instruction, every error code — live in the
[generated reference](../reference/README.md); this page shows you the code.

Two packages in this repository are yours:

- **`@dclutch/sdk`** (`packages/dclutch-sdk`) — the client library. Typed
  decoders for every on-chain record, transaction builders for the core
  flows, and a small RPC client. It never opens a connection or touches a
  key by itself: you hand it an endpoint, you sign what it builds.
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
session file `dclutch found` writes.

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
```

A card decodes fully or comes back as `refused` with a reason. A refusal
here means the account did not match the layout exactly — treat it as "not
a market I can use", not as an error to retry.

All reads happen at one finalized slot, so the numbers you show a user are
from a single consistent snapshot. `inspectMarketDetailV1`
(`@dclutch/sdk/marketDetail`) gives you one market in full;
`inspectPortfolioV1` (`@dclutch/sdk/portfolio`) derives a user's positions
directly from market addresses — no indexer involved.

## Buying and selling

A Direct trade settles between two signed **intents**: one seller, one
buyer, crossed at one execution price. Your client plays one side and needs
the other side's signed intent (or plays both sides, in a bench).

```ts
import {
  compileDirectInlineTransactionV3,
  encodeCompactIntentSigningMessageV2,
} from '@dclutch/sdk/directInlineV3';
import { inspectDirectHotRouteV3 } from '@dclutch/sdk/directHotChain';
import nacl from 'tweetnacl';

// 1. Acquire the route: the market's fixed trading accounts, read back and
//    checked at one finalized slot.
const inspection = await inspectDirectHotRouteV3(client, routeManifest);

// 2. Build and sign your intent (here: buying outcome 1, up to 5 units,
//    limit price in the market's own scale).
const intent = {
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
const signature = nacl.sign.detached(
  encodeCompactIntentSigningMessageV2(intent), myKeypair.secretKey);

// 3. Cross it with the counterparty's signed intent and submit.
const plan = compileDirectInlineTransactionV3({
  route: inspection.route,
  seller: theirSignedIntent,
  buyer: { maker: myKeypair.publicKey.toBase58(), signature, intent },
  fill: 5n, executionPrice: 400_000n,
  clockSlot: BigInt(inspection.observedSlot),
});
plan.transaction.sign([payerKeypair]);
await client.sendRawTransaction(plan.transaction.serialize());
```

`plan.preview` tells you the exact collateral debit, credit, and fees
before anything is signed — show it to your user. The compiler refuses
anything the chain would refuse (an intent that does not admit the fill, a
price outside either limit, a stale slot), so a transaction that compiles
is one the program will actually consider.

The CLI's `intent` / `buy` / `sell` commands are this exact code plus a
JSON file format for handing intents between machines.

## Redeeming

Redemption has two steps, and today your client can perform one of them.
The step you can build: create the market's Claims-role Custody replay,
which every payout needs to exist first.

```ts
import { inspectClaimsCustodyReplayV1 } from '@dclutch/sdk/claimsCustodyReplay';

const state = await inspectClaimsCustodyReplayV1(client, {
  marketAddress, claimsProgramId, custodyProgramId, registryProgramId,
  payer: myKeypair.publicKey.toBase58(),
});
if (state.status === 'creatable') {
  state.plan.transaction.sign([myKeypair]);
  await client.sendRawTransaction(state.plan.transaction.serialize());
}
```

The payout instruction itself is not yet callable from a wallet: the
program only accepts it from Core or Trading
([decision 0008](../decisions/0008-custody-namespace-owner.md)). Say that
to your user rather than looping on a transaction that cannot land;
`PLAIN_POSITION_PAYOUT_BLOCK_V1` in the same module is ready-made copy.

## Founding a market

Founding is driven by a run spec — a JSON file naming the programs, the
market recipe, and where the evidence goes. The producer binary does the
work; wrap it the way `dclutch found` does
(`packages/dclutch-cli/src/commands/found.ts`), or start from
`tools/gauntlet/run.sh`, which assembles a spec end to end. What you get
back is a running local validator with an open market and an evidence file
whose `accounts` map names everything the market is made of.

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
