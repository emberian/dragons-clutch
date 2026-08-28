#!/usr/bin/env node
// Drive `apps/dclutch-web` against a live successor chain in a real browser and
// record what it actually rendered.
//
// This is deliberately dumb: it types into the page's own inputs, presses the
// page's own buttons, waits for the page's own live status region to settle,
// and then harvests every label/value pair, provenance chip and refusal on the
// page. It asserts nothing. `compare.mjs` decides whether the harvested text
// agrees with an independent decode of the same chain, so the browser never
// gets to grade its own work.
//
// A REAL browser is the point. The first run of this driver found that every
// read surface answered `Failed to execute 'fetch' on 'Window': Illegal
// invocation` — a defect no unit test in the suite could see, because every one
// of them injects its own fetcher.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0) {
    if (fallback === undefined) throw new Error(`missing required --${name}`);
    return fallback;
  }
  return process.argv[index + 1];
}

const baseUrl = argument('base-url');
const endpoint = argument('endpoint');
const core = argument('core');
const registry = argument('registry');
const claims = argument('claims', '');
// The app reads its whole deployment from one manifest now, so the driver
// supplies all seven role programs. The defaults are the fixed-seed campaign
// layout every tier-1 gauntlet run deploys (lib/deployments.ts LOCAL rows);
// override any of them for a differently-keyed chain.
const rent = argument('rent', '2gUDaLHEAdfs44vDWyjj3cCkJDVTEjrBPntDKHQxD9U8');
const custody = argument('custody', '7H6H9NabHSQtLVpiAaMKgmuRV5VVWD5VPGzpFYNGDbiD');
const resolution = argument('resolution', 'EBH6zun6a9PRcQtAaSHjTCArqUd8ArRGMciRHmJFu34x');
const trading = argument('trading', 'H3yrZV5ekNATUhhydami88bahXYtrP5fZig31LEU8UM8');
const market = argument('market');
const owner = argument('owner');
const outDir = argument('out-dir');

mkdirSync(outDir, { recursive: true });

// Playwright is deliberately NOT a repository dependency: this driver is
// evidence tooling, not shipped code, and adding a browser download to
// `apps/dclutch-web`'s install would be a poor trade. Resolve it from wherever
// the operator has it — a normal install, `--playwright`, or `PLAYWRIGHT_MODULE`
// pointing at an `npx` cache — and say so plainly when it is not there.
async function loadPlaywright() {
  const explicit = argument('playwright', process.env.PLAYWRIGHT_MODULE ?? '');
  if (explicit !== '') return import(explicit.startsWith('/') ? `file://${explicit}` : explicit);
  try {
    return await import('playwright');
  } catch {
    throw new Error('playwright is not resolvable; install it or pass --playwright /abs/path/to/node_modules/playwright/index.mjs');
  }
}

const { chromium } = await loadPlaywright();

/** Every label/value pair the page rendered, in document order. */
async function harvest(page) {
  return page.evaluate(() => {
    const text = (node) => (node?.textContent ?? '').replace(/\s+/g, ' ').trim();
    const facts = [];
    for (const term of document.querySelectorAll('dt')) {
      const value = term.parentElement?.querySelector('dd');
      if (value !== null && value !== undefined) facts.push({ label: text(term), value: text(value) });
    }
    return {
      title: document.title,
      heading: text(document.querySelector('h1')),
      status: [...document.querySelectorAll('.direct-status, [aria-live]')].map(text),
      provenance: [...document.querySelectorAll('.provenance-chip')].map(text),
      phaseChips: [...document.querySelectorAll('.phase-chip')].map(text),
      capabilityBadges: [...document.querySelectorAll('.capability-badge')].map(text),
      refusals: [...document.querySelectorAll('.market-refusal, .market-capability-refusal, .market-empty')].map(text),
      bindings: [...document.querySelectorAll('.market-bindings li')].map((item) => ({
        ok: item.classList.contains('check-pass'),
        text: text(item),
      })),
      facts,
      // Economics tiles, the per-claim vector and the portfolio claim panel are
      // not definition lists, so they need their own harvest or they would be
      // silently unverified.
      tiles: [...document.querySelectorAll('.trade-v3-preview > div, .trade-v3-evidence article, .portfolio-claim')].map((tile) => ({
        label: text(tile.querySelector('span')),
        value: text(tile.querySelector('strong')),
      })),
      outcomeVector: [...document.querySelectorAll('.outcome-vector li')].map((item) => ({
        label: text(item.querySelector('span')),
        value: text(item.querySelector('strong')),
      })),
      bodyText: text(document.body),
    };
  });
}

async function open(page, path) {
  await page.goto(`${baseUrl}${path}`, { waitUntil: 'domcontentloaded' });
  // React has to hydrate before a typed value survives; without this the form
  // submits its initial state and the page reports a refusal that says more
  // about the driver than about the chain.
  await page.waitForFunction(() => document.querySelectorAll('button').length > 0, undefined, { timeout: 30_000 });
  await page.waitForTimeout(1_200);
}

async function fill(page, labelPattern, value) {
  const field = page.locator('label', { hasText: labelPattern }).locator('input, textarea').first();
  await field.fill(value);
}

/** Wait until the page's own live region stops saying it is working. */
async function settle(page, shotPath) {
  await page.waitForFunction(() => {
    const regions = [...document.querySelectorAll('[aria-live]')].map((node) => (node.textContent ?? '').trim());
    return regions.length > 0 && regions.every((text) => !/^Reading|^Probing|^Deriving|^Attempting|^Enumerating/.test(text));
  }, undefined, { timeout: 90_000 }).catch(() => {});
  await page.waitForTimeout(600);
  await page.screenshot({ path: shotPath, fullPage: true });
}

// A bundled chromium if playwright has one, otherwise the installed Chrome.
const browser = await chromium.launch().catch(() => chromium.launch({ channel: argument('channel', 'chrome') }));
const context = await browser.newContext({ viewport: { width: 1440, height: 1400 } });

// FE-PRODUCT inverted the surfaces: they no longer render endpoint/program
// forms — every read flows from the ACTIVE DEPLOYMENT (lib/deployments.ts,
// selected in lib/deploymentStore.ts). The driver therefore does what a
// bring-your-own operator does: store a Custom deployment in localStorage
// through the app's own keys, before any page script runs. The surfaces then
// load their content on their own, which is exactly the behavior under test.
await context.addInitScript(({ clusterKey, customKey, custom }) => {
  window.localStorage.setItem(customKey, JSON.stringify(custom));
  window.localStorage.setItem(clusterKey, 'custom');
}, {
  clusterKey: 'dclutch.cluster.v1',
  customKey: 'dclutch.customDeployment.v1',
  custom: {
    endpoint,
    programs: { registry, rent, custody, resolution, claims: claims === '' ? '9fAcEn8fhVkmJmhx4xFfquNshTryNC6cQ9ieKwAPBMY6' : claims, trading, core },
    activationCache: null,
  },
});
const consoleErrors = [];
context.on('console', (message) => {
  if (message.type() === 'error') consoleErrors.push(message.text());
});
const requestFailures = [];
context.on('requestfailed', (request) => {
  requestFailures.push(`${request.method()} ${request.url()} ${request.failure()?.errorText ?? ''}`);
});

const captured = {};

// ------------------------------------------------------------------ /markets
// The list loads itself: enumeration and discovery are one auto-read now, so
// one settled harvest serves both of compare.mjs's views of the page.
{
  const page = await context.newPage();
  await open(page, '/markets');
  await settle(page, join(outDir, 'markets-discovery.png'));
  captured.enumeration = await harvest(page);
  captured.discovery = captured.enumeration;
  await page.close();
}

// ---------------------------------------------------------- /markets/:address
{
  const page = await context.newPage();
  await open(page, `/markets/${market}`);
  await settle(page, join(outDir, 'market-detail.png'));
  // Capability drawers are <details>; open them so their facts are in the DOM.
  await page.evaluate(() => {
    for (const drawer of document.querySelectorAll('details')) drawer.open = true;
  });
  await page.waitForTimeout(400);
  await page.screenshot({ path: join(outDir, 'market-detail-expanded.png'), fullPage: true });
  captured.detail = await harvest(page);
  await page.close();
}

// ---------------------------------------------------------------- /portfolio
{
  const page = await context.newPage();
  await open(page, '/portfolio');
  await fill(page, /Or paste any owner address/, owner);
  await page.getByRole('button', { name: /Read this owner/ }).click();
  await settle(page, join(outDir, 'portfolio.png'));
  captured.portfolio = await harvest(page);
  await page.close();
}

captured.consoleErrors = consoleErrors;
captured.requestFailures = requestFailures;
writeFileSync(join(outDir, 'rendered.json'), `${JSON.stringify(captured, null, 2)}\n`);
await browser.close();
process.stdout.write(`rendered: ${join(outDir, 'rendered.json')}\n`);
