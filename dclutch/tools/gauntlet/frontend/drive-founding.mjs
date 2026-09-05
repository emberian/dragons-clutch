/**
 * Drive a real founding transaction from the real browser, through a wallet.
 *
 * `drive.mjs` grades what the app *renders* against an independent decode.
 * This program grades something else: whether the app can *originate* chain
 * state. It opens the `/create` wizard in a real Chromium, registers a Wallet
 * Standard wallet the page discovers exactly as it would discover Talisman,
 * fills the founding coordinates, and clicks sign-and-submit. Everything the
 * transaction is made of — the Market PDA, the 31-account frame, the rent
 * debit, the request bytes — is derived inside the page by `lib/coreFound.ts`.
 * Node's only job is to hold a private key and answer one signature request.
 *
 * WHAT IT PROVES AND WHAT IT DOES NOT. It proves the browser can carry the two
 * Core rungs of the founding ladder end to end against a validator: a
 * Market-scoped lifecycle RentCredit, and Found31 leaving a Core Market in
 * phase Founding. It does NOT found a Market to Open — that is the DCLTGMF1
 * rung, and it needs the projected-Custody prestate that only the Rust
 * reference client can build today. `lib/founding/ladder.ts` is the inventory
 * of that difference and this program deliberately stops where the inventory
 * says a browser stops.
 *
 * The chain readback at the end is independent of the page: it is a raw
 * JSON-RPC `getAccountInfo` decoded against offsets cited to their Rust owner,
 * in the spirit of `chain-witness.mjs`. A screenshot of a green checkmark is
 * not evidence that a Market exists.
 *
 * Playwright is deliberately not a repository dependency. Pass
 * `--playwright /abs/path/to/playwright/index.mjs` or set `PLAYWRIGHT_MODULE`.
 *
 *   node tools/gauntlet/frontend/drive-founding.mjs \
 *     --base-url http://127.0.0.1:3111 \
 *     --endpoint http://127.0.0.1:22890 \
 *     --run /private/tmp/<work>/runs/<run> \
 *     --generation 2 \
 *     --out /private/tmp/<work>/founding-evidence.json
 */

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

import { Keypair, VersionedTransaction } from '../../../apps/dclutch-web/node_modules/@solana/web3.js/lib/index.cjs.js';

function argument(name, fallback = null) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0 || index + 1 >= process.argv.length) {
    if (fallback === null) throw new Error(`--${name} is required`);
    return fallback;
  }
  return process.argv[index + 1];
}

const baseUrl = argument('base-url', 'http://127.0.0.1:3111').replace(/\/$/, '');
const endpoint = argument('endpoint', 'http://127.0.0.1:22890').replace(/\/$/, '');
const runDir = argument('run');
const generation = argument('generation', '2');
const outPath = argument('out', '/private/tmp/dclutch-fe-create/founding-evidence.json');
const playwrightModule = argument('playwright', process.env.PLAYWRIGHT_MODULE ?? 'playwright');

let requestId = 0;
async function rpc(method, params) {
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: (requestId += 1), method, params }),
  });
  const payload = await response.json();
  if (payload.error) throw new Error(`${method}: ${JSON.stringify(payload.error)}`);
  return payload.result;
}

/**
 * The Market's own coordinates, read straight off the chain.
 *
 * Offsets cited to `crates/dclutch-market/src/generated.rs`'s
 * `CoreState`: magic `DCLTCOR2` (STATE_MAGIC) at 0, schema at 8, phase at 10. Phase 0 is
 * Founding. Nothing here is imported from `apps/`.
 */
function decodeCoreState(base64) {
  const bytes = Buffer.from(base64, 'base64');
  const magic = bytes.subarray(0, 8).toString('ascii');
  return {
    magic,
    bytes: bytes.length,
    schemaVersion: bytes.readUInt16LE(8),
    phaseByte: bytes[10],
    dataSha256: createHash('sha256').update(bytes).digest('hex'),
  };
}

async function main() {
  const { chromium } = await import(playwrightModule);
  const evidence = readFileSync(`${runDir}/evidence.json`, 'utf8');
  const plan = JSON.parse(readFileSync(`${runDir}/plan.json`, 'utf8'));
  const accounts = JSON.parse(evidence).accounts;

  // Every coordinate the wizard needs, taken from what the campaign actually
  // left on this chain. The wizard reauthenticates all of them; this is
  // transport, not authority.
  const coordinates = {
    registryProgram: plan.registry.program_id,
    activationCache: plan.activation,
    realmRecord: accounts.realm_record.address,
    productRecord: accounts.product_record.address,
    resultDomainRecord: accounts.result_domain_record.address,
    portfolioRecord: accounts.portfolio_record.address,
    sourceMaterialRecord: accounts.source_material_record.address,
    capabilityManifestRecord: accounts.capability_manifest_record.address,
    executionReleaseSetRecord: plan.records.execution_release_set.raw,
  };

  const payer = Keypair.generate();
  const airdrop = await rpc('requestAirdrop', [payer.publicKey.toBase58(), 20_000_000_000]);
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const status = await rpc('getSignatureStatuses', [[airdrop]]);
    if (status.value[0]?.confirmationStatus === 'finalized') break;
    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }
  const funded = await rpc('getBalance', [payer.publicKey.toBase58(), { commitment: 'finalized' }]);
  if (!funded.value) throw new Error('the founding payer was never funded');

  const browser = await chromium.launch();
  const context = await browser.newContext();
  const consoleLines = [];

  // Node holds the key; the page never sees it. The page asks for a signature
  // over exactly the message bytes it built, and gets back exactly those bytes
  // plus a signature -- which is the property `walletHandoff` rechecks before
  // it accepts the answer.
  await context.exposeFunction('__dclutchSignTransaction', (serialized) => {
    const transaction = VersionedTransaction.deserialize(Uint8Array.from(serialized));
    transaction.sign([payer]);
    return Array.from(transaction.serialize());
  });

  await context.addInitScript(({ address, publicKey }) => {
    const wallet = {
      version: '1.0.0',
      name: 'FE-CREATE evidence wallet',
      icon: 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=',
      chains: ['solana:localnet'],
      // `describeWalletV1` refuses an account whose `publicKey` bytes do not encode
      // its own `address`, so the mock carries the real thirty-two bytes.
      accounts: [{ address, publicKey: Uint8Array.from(publicKey), chains: ['solana:localnet'], features: ['solana:signTransaction'], label: 'evidence payer', icon: undefined }],
      features: {
        'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: wallet.accounts }) },
        'standard:events': { version: '1.0.0', on: () => () => {} },
        'solana:signTransaction': {
          version: '1.0.0',
          supportedTransactionVersions: ['legacy', 0],
          signTransaction: async (...inputs) => {
            const outputs = [];
            for (const input of inputs) {
              const signed = await window.__dclutchSignTransaction(Array.from(input.transaction));
              outputs.push({ signedTransaction: Uint8Array.from(signed) });
            }
            return outputs;
          },
        },
      },
    };
    const announce = ({ register }) => register(wallet);
    window.addEventListener('wallet-standard:app-ready', (event) => announce(event.detail));
    window.dispatchEvent(new CustomEvent('wallet-standard:register-wallet', { detail: announce }));
  }, { address: payer.publicKey.toBase58(), publicKey: Array.from(payer.publicKey.toBytes()) });

  const payerAddress = payer.publicKey.toBase58();
  const page = await context.newPage();
  page.on('console', (message) => consoleLines.push(`${message.type()}: ${message.text()}`));
  page.on('pageerror', (error) => consoleLines.push(`pageerror: ${error.message}`));

  await page.goto(`${baseUrl}/create`, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: /Sign & submit/ }).click();

  // If the registration was refused, the directory says why in its own words.
  // Surfacing that beats a bare click timeout, because the refusal reason is
  // the actual finding whenever this step fails.
  await page.waitForSelector('.wallet-directory', { timeout: 20_000 });
  await page.evaluate(() => { document.querySelectorAll('.wallet-directory details').forEach((entry) => entry.setAttribute('open', '')); });
  const directoryText = await page.locator('.wallet-directory').innerText();
  const walletButton = page.getByRole('button', { name: /FE-CREATE evidence wallet/ });
  if ((await walletButton.count()) === 0) {
    throw new Error(`the page did not list the evidence wallet. Directory said:\n${directoryText}`);
  }
  await walletButton.click();
  // Connecting adopts the address as the payer, so the payer field carrying it
  // is the observable fact that the handshake completed. The wizard's own
  // status line lives inside the signing panel, which does not exist until a
  // plan does -- waiting on that would be waiting for the wrong thing.
  try {
    await page.waitForFunction(
      (expected) => [...document.querySelectorAll('input')].some((input) => input.value === expected),
      payerAddress,
      { timeout: 20_000 },
    );
  } catch {
    throw new Error(`the wallet never connected. Directory said:\n${await page.locator('.wallet-directory').innerText()}`);
  }

  async function fill(label, value) {
    await page.getByLabel(label, { exact: false }).first().fill(value);
  }
  await fill('Finalized RPC endpoint', endpoint);
  await fill('Market generation', generation);
  await fill('Registry program', coordinates.registryProgram);
  await fill('Release activation cache', coordinates.activationCache);
  await fill('Realm raw record', coordinates.realmRecord);
  await fill('Product Runtime V2 raw', coordinates.productRecord);
  await fill('Result domain raw', coordinates.resultDomainRecord);
  await fill('Portfolio raw', coordinates.portfolioRecord);
  await fill('SourceMaterialV2 raw', coordinates.sourceMaterialRecord);
  await fill('Capability manifest raw', coordinates.capabilityManifestRecord);
  await fill('Execution release set raw', coordinates.executionReleaseSetRecord);

  await page.getByRole('button', { name: /Construct the unsigned lifecycle/ }).click();
  // A refusal is the interesting outcome as often as an acceptance, and the
  // wizard states it in one line. Surfacing that line beats a bare timeout.
  try {
    await page.waitForFunction(
      () => /Accepted at finalized slot|^Refused:/m.test(document.body.innerText),
      null,
      { timeout: 120_000 },
    );
  } catch {
    throw new Error(`the wizard never answered. Status was:\n${await page.locator('.direct-status').allInnerTexts()}`);
  }
  const constructionStatus = (await page.locator('.wizard-chain-form .direct-status').innerText()).trim();
  if (!constructionStatus.startsWith('Accepted at finalized slot')) {
    throw new Error(`the wizard refused to construct the founding pair: ${constructionStatus}`);
  }

  const derivedMarket = await page.locator('dt:text-is("Derived Market") + dd').innerText();
  const derivedCredit = await page.locator('dt:text-is("Lifecycle RentCredit") + dd').innerText();
  const outcomeWidth = await page.locator('dt:text-is("Outcome width") + dd').innerText();

  const submissions = [];
  const buttons = page.getByRole('button', { name: /Sign & submit this transaction/ });
  // Three rungs, one click each, in order. The wizard waits for each to
  // finalize before it reports, so this driver never has to decide that a
  // predecessor landed -- which is the same reason the wizard refuses to chain
  // them behind one button.
  for (const [index, stage] of ['lifecycle RentCredit Create', 'routing table', 'Found31'].entries()) {
    await buttons.nth(index).click();
    await page.waitForFunction(
      (position) => {
        const items = [...document.querySelectorAll('.wizard-stages li')];
        return items[position] && /submitted|refused/.test(items[position].className);
      },
      index,
      { timeout: 300_000 },
    );
    const item = page.locator('.wizard-stages li').nth(index);
    const status = (await item.getAttribute('class')) ?? '';
    const detail = await item.locator('p').innerText();
    const signature = (await item.locator('code').count()) > 0 ? await item.locator('code').innerText() : null;
    submissions.push({ stage, status: status.trim(), detail, signature });
    if (!status.includes('submitted')) break;
  }

  await page.screenshot({ path: outPath.replace(/\.json$/, '.png'), fullPage: true });
  await browser.close();

  // The independent readback. Nothing below came from the page.
  const marketAccount = await rpc('getAccountInfo', [derivedMarket, { encoding: 'base64', commitment: 'finalized' }]);
  const creditAccount = await rpc('getAccountInfo', [derivedCredit, { encoding: 'base64', commitment: 'finalized' }]);
  const slot = await rpc('getSlot', [{ commitment: 'finalized' }]);

  const report = {
    schema: 'dclutch-web-browser-founding-evidence-v1',
    claim: 'The two Core rungs of the founding ladder, originated in a real browser through a Wallet Standard wallet, against a local validator. This is NOT a Market at Open; DCLTGMF1 is not driven here.',
    baseUrl,
    endpoint,
    runDir,
    generation: Number(generation),
    observedFinalizedSlot: slot,
    payer: payer.publicKey.toBase58(),
    coordinates,
    rendered: { derivedMarket, derivedCredit, outcomeWidth },
    submissions,
    chainReadback: {
      market: marketAccount.value === null ? null : {
        address: derivedMarket,
        owner: marketAccount.value.owner,
        lamports: marketAccount.value.lamports,
        executable: marketAccount.value.executable,
        ...decodeCoreState(marketAccount.value.data[0]),
      },
      rentCredit: creditAccount.value === null ? null : {
        address: derivedCredit,
        owner: creditAccount.value.owner,
        lamports: creditAccount.value.lamports,
        bytes: Buffer.from(creditAccount.value.data[0], 'base64').length,
        magic: Buffer.from(creditAccount.value.data[0], 'base64').subarray(0, 8).toString('ascii'),
      },
    },
    consoleLines: consoleLines.slice(0, 40),
  };
  writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);

  const market = report.chainReadback.market;
  if (market === null || market.magic !== 'DCLTCOR2' || market.phaseByte !== 0) {
    throw new Error('the browser did not leave a Core Market at phase Founding');
  }
}

await main();
