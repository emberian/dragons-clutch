#!/usr/bin/env node
// Does the site NAVIGATE? Click every link on every page of the assembled
// Pages artifact, in a real browser, against a server shaped like the host.
//
//   node tools/gauntlet/frontend/pages-nav-check.mjs --site _site
//
// Why this exists, when `tools/genref/render-site.mjs` already link-checks the
// artifact: that check proves an href RESOLVES to a file the host can serve.
// It cannot see whether clicking it does anything. On 2026-08-27 every page of
// the live site link-checked green while every link in it was dead — vinext
// 1.0.0-beta.3's `next/link` shim calls `preventDefault()` and then throws on
// an `import("./navigation.js")` that yields an export-less namespace in the
// production bundle, so the click was swallowed and no navigation followed.
// A reader could only move around the site by typing URLs.
//
// The two questions here are the two that check cannot answer:
//
//   1. Every root-relative link on every page: does clicking it land on the
//      path it names? (`apps/dclutch-web/components/Anchor.tsx` is the fix
//      that keeps this green; this is what would catch its regression.)
//   2. `/markets/<address>` is dynamic, so the export writes no file for it
//      and the host answers 404.html. Does that document RENDER the Market
//      detail surface (app/not-found.tsx resolving the path client-side), and
//      does a path that is genuinely no route still say so?
//
// The server below answers like GitHub Pages: `/x` from `x.html` or
// `x/index.html`, and anything else from 404.html with a 404 status. It does
// NOT rewrite unknown paths to index.html — a fallback the host does not have
// would hide exactly the failure this looks for.
//
// Playwright is deliberately not a repository dependency (see README.md):
// pass `--playwright /abs/path/to/node_modules/playwright/index.mjs` or set
// PLAYWRIGHT_MODULE. Nothing here imports from `apps/`.

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

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

const site = path.resolve(argument('site'));
// An address that is certainly not a Market on any chain. The point is the
// SURFACE, not the read: the detail page renders its form and its own address
// before any RPC call, and refuses by name afterwards.
const FAKE_MARKET = argument('market', 'So11111111111111111111111111111111111111112');
const UNKNOWN_PATH = argument('unknown-path', '/no-such-page-at-all');

if (!fs.existsSync(path.join(site, 'index.html'))) {
  console.error(`pages-nav-check: ${site} has no index.html; assemble the artifact first`);
  process.exit(2);
}

// ------------------------------------------------------ the host, as it is

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.rsc': 'text/x-component; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
};

const isFile = (p) => fs.existsSync(p) && fs.statSync(p).isFile();

function servedBy(urlPath) {
  let clean;
  try {
    clean = decodeURIComponent(urlPath.split('?')[0].split('#')[0]);
  } catch {
    return null;
  }
  if (clean.includes('..')) return null;
  const abs = path.join(site, clean);
  if (isFile(abs)) return abs;
  if (isFile(`${abs}.html`)) return `${abs}.html`;
  if (isFile(path.join(abs, 'index.html'))) return path.join(abs, 'index.html');
  return null;
}

const server = http.createServer((req, res) => {
  const found = servedBy(req.url ?? '/');
  if (found) {
    res.writeHead(200, { 'content-type': TYPES[path.extname(found)] ?? 'application/octet-stream' });
    fs.createReadStream(found).pipe(res);
    return;
  }
  const notFound = path.join(site, '404.html');
  if (isFile(notFound)) {
    res.writeHead(404, { 'content-type': 'text/html; charset=utf-8' });
    fs.createReadStream(notFound).pipe(res);
    return;
  }
  res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
  res.end('404');
});

const baseUrl = await new Promise((resolve) => {
  server.listen(0, '127.0.0.1', () => resolve(`http://127.0.0.1:${server.address().port}`));
});

// The app's routes are the .html files the export wrote at the artifact root.
// 404.html is not a route; it is checked separately, as the thing it is.
const routes = fs
  .readdirSync(site)
  .filter((e) => e.endsWith('.html') && e !== '404.html')
  .map((e) => (e === 'index.html' ? '/' : `/${e.slice(0, -'.html'.length)}`))
  .sort();

const { chromium } = await playwright();
// A bundled chromium if playwright has one, otherwise the installed Chrome.
const browser = await chromium
  .launch()
  .catch(() => chromium.launch({ channel: argument('channel', 'chrome') }));

let failures = 0;
const fail = (line) => {
  failures++;
  console.error(`  FAIL ${line}`);
};

console.log(`pages-nav-check: ${site} on ${baseUrl}`);
console.log(`pages-nav-check: ${routes.length} routes\n`);

// ------------------------------------------------------ 1. does it navigate

for (const route of routes) {
  const page = await browser.newPage();
  const response = await page.goto(`${baseUrl}${route}`, { waitUntil: 'networkidle' });
  await page.waitForTimeout(300);
  const hrefs = (
    await page.$$eval('a[href^="/"]', (as) =>
      Array.from(new Set(as.map((a) => a.getAttribute('href')))),
    )
  ).filter((href) => href && href !== route);
  await page.close();

  let inert = 0;
  for (const href of hrefs) {
    // A fresh page per click: a click that DID navigate must not change what
    // the next click starts from.
    const clicker = await browser.newPage();
    await clicker.goto(`${baseUrl}${route}`, { waitUntil: 'networkidle' });
    await clicker.waitForTimeout(250);
    let landed = new URL(clicker.url()).pathname;
    let reason = '';
    try {
      await clicker.locator(`a[href="${href}"]`).first().click({ timeout: 5000 });
      await clicker.waitForTimeout(800);
      landed = new URL(clicker.url()).pathname;
    } catch (error) {
      reason = ` (${String(error.message).split('\n')[0].slice(0, 100)})`;
    }
    await clicker.close();
    const want = href.split('?')[0].split('#')[0];
    if (landed !== want && landed !== `${want}/` && `${landed}/` !== want) {
      inert++;
      fail(`${route} -> ${href} left the browser at ${landed}${reason}`);
    }
  }
  console.log(
    `${response?.status()} ${route.padEnd(14)} ${String(hrefs.length).padStart(2)} links, ${inert} inert`,
  );
}

// -------------------------------------------- 2. does 404.html carry the app

console.log('');
const perma = await browser.newPage();
const permaResponse = await perma.goto(`${baseUrl}/markets/${FAKE_MARKET}`, {
  waitUntil: 'networkidle',
});
await perma.waitForTimeout(1500);
const permaMain = (await perma.textContent('main').catch(() => null)) ?? '';
const permaForms = (await perma.$$('form')).length;
console.log(`/markets/<address> hard load: HTTP ${permaResponse?.status()}`);
// The host says 404 -- correct, it has no file -- and the document renders the
// Market detail surface anyway. Both halves are the fix.
if (permaResponse?.status() !== 404) {
  fail(`a static host must answer an unwritten path with 404, got ${permaResponse?.status()}`);
}
if (!permaMain.includes(FAKE_MARKET.slice(0, 8))) {
  fail('the Market detail surface did not render its own address');
}
if (permaForms === 0) {
  fail('the Market detail surface rendered no read form');
}
if (/could not be found/i.test(permaMain)) {
  fail('the permalink still lands on a dead not-found page');
}
await perma.close();

const bogus = await browser.newPage();
const bogusResponse = await bogus.goto(`${baseUrl}${UNKNOWN_PATH}`, { waitUntil: 'networkidle' });
await bogus.waitForTimeout(1500);
const bogusMain = (await bogus.textContent('main').catch(() => null)) ?? '';
console.log(`${UNKNOWN_PATH} hard load: HTTP ${bogusResponse?.status()}`);
if (bogusResponse?.status() !== 404) {
  fail(`an unknown path must answer 404, got ${bogusResponse?.status()}`);
}
if (!bogusMain.includes(UNKNOWN_PATH)) {
  fail('the not-found surface did not name the path that was asked for');
}
if (bogusMain.includes('decoded field by field')) {
  fail('an unknown path rendered the Market detail surface; the fallback invents routes');
}
await bogus.close();

await browser.close();
server.close();

console.log('');
if (failures > 0) {
  console.error(`pages-nav-check: ${failures} failure(s).`);
  process.exit(1);
}
console.log(`pages-nav-check: ${routes.length} routes navigate, permalinks resolve, unknown paths refuse.`);
