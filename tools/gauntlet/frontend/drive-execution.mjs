// Drive the real browser through the first STATE-MUTATING dClutch transaction
// a browser has ever built, signed, and submitted against a live chain.
//
// The flow is the redeem flow's opening move: the Claims-role Custody replay
// creation (DCLCCR01, ADR-0008 §7) for a resolved Market, built entirely in
// `apps/dclutch-web` from the generated ABI, signed by a Wallet Standard
// wallet, submitted through `lib/rpc.ts`'s one `sendTransaction` seam, and
// confirmed by the page itself.
//
// The wallet injected here is a MINIMAL Wallet Standard wallet whose key is
// the journey campaign's seeded founder — a TEST-ONLY loopback-only key
// (tools/local-validator/bootstrap/successor/src/seed.rs). Its signing runs
// in this Node process; the page sees only the standard wallet interface, so
// every byte the page hands the wallet crosses the same seam a Talisman user's
// bytes would cross.
//
// Like drive.mjs: playwright resolves from a normal install, `--playwright`,
// or `PLAYWRIGHT_MODULE`. Nothing here imports from `apps/`.
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(new URL('../../../apps/dclutch-web/package.json', import.meta.url));
const { Keypair, PublicKey, VersionedTransaction } = require('@solana/web3.js');

function argument(name, fallback = undefined) {
  const index = process.argv.indexOf(`--${name}`);
  if (index >= 0 && process.argv.length > index + 1) return process.argv[index + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`--${name} is required`);
}

async function playwright() {
  const explicit = argument('playwright', process.env.PLAYWRIGHT_MODULE ?? '');
  if (explicit !== '') return await import(explicit);
  try {
    return await import('playwright');
  } catch {
    throw new Error('playwright is not resolvable; install it or pass --playwright /abs/path/to/node_modules/playwright/index.mjs');
  }
}

function journeyFounderKeypair(role, index) {
  // seed.rs: material = SHA-256(DOMAIN || 0 || seed || 0 || role || 0 || index_le_u32)
  const seed = createHash('sha256').update(argument('campaign-seed-phrase', 'dclutch/gauntlet/journey/campaign-seed/v1')).digest();
  const indexBytes = Buffer.alloc(4);
  indexBytes.writeUInt32LE(index);
  const material = createHash('sha256')
    .update(Buffer.from('dclutch/local-successor-bootstrap/keypair-seed/v1'))
    .update(Buffer.from([0]))
    .update(seed)
    .update(Buffer.from([0]))
    .update(Buffer.from(role))
    .update(Buffer.from([0]))
    .update(indexBytes)
    .digest();
  return Keypair.fromSeed(material);
}

async function rpc(endpoint, method, params) {
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const payload = await response.json();
  if (payload.error) throw new Error(`${method} refused: ${JSON.stringify(payload.error)}`);
  return payload.result;
}

const baseUrl = argument('base-url', 'http://127.0.0.1:3111');
const endpoint = argument('endpoint');
const market = argument('market');
const programs = {
  core: argument('core'),
  registry: argument('registry'),
  claims: argument('claims'),
  custody: argument('custody'),
  trading: argument('trading'),
};
const outDir = argument('out-dir');
mkdirSync(outDir, { recursive: true });

const founder = journeyFounderKeypair('founding-founder', 0);
const founderAddress = founder.publicKey.toBase58();
const transcript = {
  intent: 'first browser-built state-mutating transaction against a live dClutch chain',
  honest_label: 'local-validator execution evidence; not devnet, not mainnet, no official deployment; the signing key is the journey campaign seed derivation, TEST-ONLY and loopback-only',
  base_url: baseUrl,
  endpoint,
  market,
  programs,
  wallet: { name: 'Journey Evidence Wallet', address: founderAddress, derivation: `seed.rs: SHA-256(domain||0||sha256('${argument('campaign-seed-phrase', 'dclutch/gauntlet/journey/campaign-seed/v1')}')||0||'founding-founder'||0||u32le(0))` },
  steps: [],
};
function step(name, facts) {
  transcript.steps.push({ name, at: new Date().toISOString(), ...facts });
  console.log(`== ${name}`);
}

const { chromium } = await playwright();
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1440, height: 1400 } });
page.setDefaultTimeout(90_000);

// Node-side signing the injected wallet calls; arrays cross the bridge.
await page.exposeFunction('__evidenceSignTransaction', (bytes) => {
  const transaction = VersionedTransaction.deserialize(Uint8Array.from(bytes));
  transaction.sign([founder]);
  return Array.from(transaction.serialize());
});
await page.exposeFunction('__evidenceSignMessage', (bytes) => {
  const nacl = require('tweetnacl');
  return Array.from(nacl.sign.detached(Uint8Array.from(bytes), founder.secretKey));
});

await page.addInitScript(({ address, publicKeyBytes }) => {
  const account = {
    address,
    publicKey: Uint8Array.from(publicKeyBytes),
    chains: ['solana:localnet'],
    features: ['solana:signTransaction', 'solana:signMessage'],
    label: 'journey founder (derived test key)',
  };
  const wallet = {
    version: '1.0.0',
    name: 'Journey Evidence Wallet',
    icon: 'data:image/svg+xml;base64,' + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#9f7"/></svg>'),
    chains: ['solana:localnet'],
    accounts: [account],
    features: {
      'standard:connect': {
        version: '1.0.0',
        connect: async () => ({ accounts: [account] }),
      },
      'standard:events': {
        version: '1.0.0',
        on: () => () => undefined,
      },
      'solana:signTransaction': {
        version: '1.0.0',
        supportedTransactionVersions: ['legacy', 0],
        signTransaction: async (...inputs) => {
          const outputs = [];
          for (const input of inputs) {
            const signed = await window.__evidenceSignTransaction(Array.from(input.transaction));
            outputs.push({ signedTransaction: Uint8Array.from(signed) });
          }
          return outputs;
        },
      },
      'solana:signMessage': {
        version: '1.0.0',
        signMessage: async (...inputs) => {
          const outputs = [];
          for (const input of inputs) {
            const signature = await window.__evidenceSignMessage(Array.from(input.message));
            outputs.push({ signedMessage: Uint8Array.from(input.message), signature: Uint8Array.from(signature) });
          }
          return outputs;
        },
      },
    },
  };
  const announce = (api) => { try { api.register(wallet); } catch { /* the page's registry refuses loudly */ } };
  window.addEventListener('wallet-standard:app-ready', (event) => announce(event.detail));
  window.dispatchEvent(new CustomEvent('wallet-standard:register-wallet', { detail: announce }));
}, { address: founderAddress, publicKeyBytes: Array.from(founder.publicKey.toBytes()) });

async function fill(label, value) {
  const input = page.locator(`label:has-text("${label}")`).first().locator('input, textarea').first();
  await input.fill(value);
}

// ------------------------------------------------------------ 1. portfolio
await page.goto(`${baseUrl}/portfolio`, { waitUntil: 'networkidle' });
await fill('Finalized RPC endpoint', endpoint);
await fill('Core program', programs.core);
await fill('Claims program', programs.claims);
await fill('Registry program · optional', programs.registry);
await fill('Custody program · optional, required to redeem', programs.custody);
await page.locator('button.wallet-choice', { hasText: 'Journey Evidence Wallet' }).first().click();
await page.waitForFunction(
  (expected) => Array.from(document.querySelectorAll('input')).some((input) => input.value === expected),
  founderAddress,
);
step('wallet connected through the Wallet Standard registry', { owner: founderAddress });
await fill('Known Market addresses', market);
await page.getByRole('button', { name: 'Derive and read Positions' }).click();
await page.locator('.portfolio-claim', { hasText: 'Winning-claim atoms admitted to redemption' }).waitFor();
const redeemable = await page.locator('.portfolio-claim strong').first().textContent();
await page.screenshot({ path: `${outDir}/1-portfolio-redeemable.png`, fullPage: true });
step('portfolio shows the founder Position redeemable on the resolved Market', { redeemableAtoms: redeemable });

// ------------------------------------------------- 2. the redeem flow opens
await page.getByRole('button', { name: 'Prepare redemption' }).click();
await page.locator('.redeem-flow', { hasText: 'Replay to create' }).waitFor();
const planFacts = await page.locator('.redeem-flow dl').textContent();
await page.screenshot({ path: `${outDir}/2-replay-plan.png`, fullPage: true });
step('the browser derived the complete replay-creation plan from the aggregate', { rendered: planFacts });

// ------------------------------ 3. sign with the wallet, submit through rpc.ts
await page.getByRole('button', { name: 'Sign with connected wallet and submit' }).click();
await page.locator('.portfolio-claim', { hasText: 'Replay created and confirmed' }).waitFor({ timeout: 120_000 });
const confirmation = await page.locator('.redeem-flow .portfolio-claim p').textContent();
await page.screenshot({ path: `${outDir}/3-replay-confirmed.png`, fullPage: true });
const signatureMatch = confirmation?.match(/Signature ([1-9A-HJ-NP-Za-km-z]{64,88})/);
const replayMatch = confirmation?.match(/exists at ([1-9A-HJ-NP-Za-km-z]{32,44}) with next revision ([0-9]+)/);
if (!signatureMatch || !replayMatch) throw new Error(`confirmation text did not carry the signature and replay: ${confirmation}`);
step('the page confirmed the browser-built transaction on chain', {
  signature: signatureMatch[1],
  replay: replayMatch[1],
  nextRevision: replayMatch[2],
  rendered: confirmation,
});

// --------------------------- 4. INDEPENDENT verification, no apps/ imports
const replayInfo = await rpc(endpoint, 'getAccountInfo', [replayMatch[1], { encoding: 'base64', commitment: 'finalized' }]);
if (replayInfo.value === null) throw new Error('independent read: the replay account does not exist');
const replayBytes = Buffer.from(replayInfo.value.data[0], 'base64');
const facts = {
  owner: replayInfo.value.owner,
  bytes: replayBytes.length,
  magic: replayBytes.subarray(0, 8).toString('ascii'),
  // dclutch-custody-contract/src/generated.rs: caller_role at 11, market at 48,
  // rent_refund at 176, next_revision u64 LE at 208.
  callerRole: replayBytes[11],
  market: new PublicKey(replayBytes.subarray(48, 80)).toBase58(),
  rentRefund: new PublicKey(replayBytes.subarray(176, 208)).toBase58(),
  nextRevision: Number(replayBytes.readBigUInt64LE(208)),
};
if (facts.owner !== programs.custody) throw new Error(`replay owner ${facts.owner} is not the Custody program`);
if (facts.magic !== 'DCLCUSS1' || facts.callerRole !== 1 || facts.market !== market || facts.nextRevision !== 1) {
  throw new Error(`independent decode disagrees: ${JSON.stringify(facts)}`);
}
if (facts.rentRefund !== founderAddress) throw new Error('the rent refund is not the signing wallet');
const transactionRecord = await rpc(endpoint, 'getTransaction', [signatureMatch[1], { encoding: 'base64', commitment: 'finalized', maxSupportedTransactionVersion: 0 }]);
step('independent decoder agrees: Claims-role replay exists, created by the wallet that signed', {
  ...facts,
  slot: transactionRecord?.slot ?? null,
  fee: transactionRecord?.meta?.fee ?? null,
  computeUnits: transactionRecord?.meta?.computeUnitsConsumed ?? null,
  logTail: transactionRecord?.meta?.logMessages?.slice(-3) ?? null,
  transactionHistoryNote: transactionRecord === null
    ? 'this resumed node serves no transaction history (measured: getSignatureStatuses answers null even for its own executed transactions); the postcondition account above is the proof of execution'
    : null,
});

// ------------------------------------------------ 5. market detail + walls
await page.goto(`${baseUrl}/markets/${market}`, { waitUntil: 'networkidle' });
await fill('Finalized RPC endpoint', endpoint);
await fill('Core program', programs.core);
await fill('Registry program · optional', programs.registry);
await fill('Claims program · optional', programs.claims);
await fill('Custody program · optional', programs.custody);
await fill('Trading program · optional, enables the trade panel', programs.trading);
await page.getByRole('button', { name: 'Read this Market' }).click();
await page.locator('.phase-meaning').waitFor();
const phase = await page.locator('.trade-v3-hero h1 em').first().textContent();
await page.getByRole('button', { name: 'Ask the chain about trading here' }).click();
await page.locator('section.trade-v3-card', { hasText: 'Trade this Market' }).locator('.market-refusal, .market-bindings, .trade-v3-evidence').first().waitFor();
await page.screenshot({ path: `${outDir}/4-market-detail-trading.png`, fullPage: true });
const tradeStatus = await page.locator('section.trade-v3-card', { hasText: 'Trade this Market' }).locator('.direct-status').first().textContent();
const tradeRefusal = await page.locator('section.trade-v3-card', { hasText: 'Trade this Market' }).locator('.market-refusal').first().textContent().catch(() => null);
step('market detail renders the resolved phase and the trading verdict as named reasons', {
  phase,
  tradeStatus,
  tradeRefusal,
});

// ------------------------------------------------------------- 6. activity
await page.goto(`${baseUrl}/activity`, { waitUntil: 'networkidle' });
await fill('RPC endpoint', endpoint);
await fill('Owner address · wallet or pasted', founderAddress);
await fill('Claims program · required to derive Positions', programs.claims);
await fill('Core program · label only', programs.core);
await fill('Trading program · label only', programs.trading);
await fill('Market addresses · one per line', market);
await page.getByRole('button', { name: 'Read activity' }).click();
// A node without transaction history honestly answers empty; the surface must
// say that is the node speaking. Accept either the row or the honest answer.
await page.locator('.activity-row, .market-empty').first().waitFor();
const activityRows = await page.locator('.activity-row').count();
if (activityRows > 0) {
  const newest = await page.locator('.activity-row').first().textContent();
  if (!newest?.includes(signatureMatch[1].slice(0, 10))) {
    throw new Error('the newest activity row is not the browser-built transaction');
  }
  await page.screenshot({ path: `${outDir}/5-activity.png`, fullPage: true });
  step('the activity surface lists the browser-built transaction from the node history', { newestRow: newest });
} else {
  const honest = await page.locator('.market-empty').last().textContent();
  if (!honest?.includes("this node's answer")) throw new Error(`activity rendered neither rows nor the honest node answer: ${honest}`);
  await page.screenshot({ path: `${outDir}/5-activity.png`, fullPage: true });
  step('the activity surface reported the history-less node honestly instead of inventing a feed', { rendered: honest });
}

await browser.close();
writeFileSync(`${outDir}/transcript.json`, JSON.stringify(transcript, null, 2));
console.log(`transcript: ${outDir}/transcript.json`);
console.log(fileURLToPath(new URL(outDir, 'file://')));
