#!/usr/bin/env node
// Exercise the checked-release activation un-gate against a REAL chain.
//
// FD2 implemented the gate and closed saying it had never been exercised:
// "Nothing here has been run against a validator." This loads one campaign's
// own checked-release manifests into `/release` in a real browser, points it at
// the chain that campaign deployed to, and records exactly what the gate says —
// then does it again with one byte of one manifest flipped.
//
// The gate is a REFUSAL. A run that reports it stayed closed is a result, not a
// failure; what would be a failure is a gate that opened on evidence the chain
// does not support, or one whose refusal named nothing.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
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
const run = argument('run');
const work = argument('work', join(run, 'checked-release'));
const tamperedWork = argument('tampered-work', '');
const outDir = argument('out-dir');
mkdirSync(outDir, { recursive: true });

async function loadPlaywright() {
  const explicit = argument('playwright', process.env.PLAYWRIGHT_MODULE ?? '');
  if (explicit !== '') return import(explicit.startsWith('/') ? `file://${explicit}` : explicit);
  return import('playwright');
}
const { chromium } = await loadPlaywright();

const plan = JSON.parse(readFileSync(join(run, 'plan.json'), 'utf8'));
const ROLES = ['core', 'claims', 'trading', 'resolution', 'custody'];
const base64 = (path) => readFileSync(path).toString('base64');

const multiprogramRef = { value: base64(join(work, 'set', 'multiprogram.checked')) };
const manifests = Object.fromEntries(ROLES.map((role) => [role, base64(join(work, 'evidence', role, 'checked.bin'))]));

/** One byte of the trading manifest, flipped. Everything else is untouched. */
function tamper(value) {
  const bytes = Buffer.from(value, 'base64');
  // Flip a bit inside the artifact digest region rather than in a header, so
  // the manifest still decodes and the refusal has to come from the chain
  // comparison and not from a shape check.
  bytes[200] ^= 0x01;
  return bytes.toString('base64');
}

const payer = plan.core_bootstrap.upgrade_authority;

async function attempt(page, label, roleManifests) {
  await page.goto(`${baseUrl}/release`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => document.querySelectorAll('button').length > 0, undefined, { timeout: 30_000 });
  await page.waitForTimeout(1_000);
  await page.locator('label', { hasText: /Finalized RPC endpoint/ }).locator('input').first().fill(endpoint);
  await page.locator('label', { hasText: /Registry program/ }).locator('input').first().fill(plan.registry.program_id);
  await page.locator('label', { hasText: /External fee payer public key/ }).locator('input').first().fill(payer);
  await page.locator('label', { hasText: /checked multiprogram/ }).locator('textarea').first().fill(multiprogramRef.value);
  for (const role of ROLES) {
    await page.locator('label', { hasText: new RegExp(`^${role} complete checked release`) }).locator('textarea').first().fill(roleManifests[role]);
  }
  await page.getByRole('button', { name: /Reacquire finalized authority/ }).click();
  await page.waitForFunction(() => {
    const regions = [...document.querySelectorAll('[aria-live]')].map((node) => (node.textContent ?? '').trim());
    return regions.some((text) => text.length > 0 && !/^No manifest or chain request/.test(text) && !/Reacquiring|Building/.test(text));
  }, undefined, { timeout: 60_000 }).catch(() => {});
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(outDir, `ungate-${label}.png`), fullPage: true });
  return page.evaluate(() => {
    const text = (node) => (node?.textContent ?? '').replace(/\s+/g, ' ').trim();
    const signButtons = [...document.querySelectorAll('button')].filter((button) => /^Sign /.test(text(button)));
    return {
      status: [...document.querySelectorAll('[aria-live]')].map(text),
      gate: [...document.querySelectorAll('.release-ungate, .direct-status')].map(text),
      signButtons: signButtons.map((button) => ({ label: text(button), disabled: button.disabled })),
      bodyText: text(document.body),
    };
  });
}

const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1440, height: 1600 } });
const page = await context.newPage();

const honest = await attempt(page, 'honest', manifests);
const tampered = await attempt(page, 'tampered', { ...manifests, trading: tamper(manifests.trading) });

// The strongest adversarial case: a manifest set that is INTERNALLY PERFECT -
// built by the same pipeline, every create/verify/inspect pass agreeing - over
// a one-byte-altered custody ELF. Nothing about it is malformed. The only thing
// wrong with it is that the chain runs a different program, so a refusal here
// can only have come from the chain comparison.
let wrongArtifact = null;
if (tamperedWork !== '') {
  const alternate = Object.fromEntries(ROLES.map((role) => [role, base64(join(tamperedWork, 'evidence', role, 'checked.bin'))]));
  const alternateMultiprogram = base64(join(tamperedWork, 'set', 'multiprogram.checked'));
  const saved = multiprogramRef.value;
  multiprogramRef.value = alternateMultiprogram;
  wrongArtifact = await attempt(page, 'wrong-artifact', alternate);
  multiprogramRef.value = saved;
}

await browser.close();

const report = {
  schema: 'dclutch-frontend-ungate-attempt-v1',
  endpoint,
  run,
  payer,
  registryProgram: plan.registry.program_id,
  honest: {
    status: honest.status,
    signButtons: honest.signButtons,
    opened: honest.signButtons.length > 0 && honest.signButtons.every((button) => !button.disabled),
  },
  tampered: {
    status: tampered.status,
    signButtons: tampered.signButtons,
    opened: tampered.signButtons.length > 0 && tampered.signButtons.every((button) => !button.disabled),
  },
  wrongArtifact: wrongArtifact === null ? null : {
    status: wrongArtifact.status,
    signButtons: wrongArtifact.signButtons,
    opened: wrongArtifact.signButtons.length > 0 && wrongArtifact.signButtons.every((button) => !button.disabled),
  },
};
writeFileSync(join(outDir, 'ungate.json'), `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
