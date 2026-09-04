/**
 * ONE table of web-tree file -> twin class, read by both of its readers.
 *
 * `apps/dclutch-web` and `packages/dclutch-sdk` carry copies of the same
 * client modules and the same generators, and two instruments have opinions
 * about every one of those files:
 *
 *   * `packages/dclutch-sdk/scripts/sync-from-web.mjs` decides whether to
 *     ABSORB a web file into the package.
 *   * `apps/dclutch-web/lib/twinIdentity.test.ts` decides whether the pair
 *     must be BYTE-IDENTICAL.
 *
 * They used to hold two maps, and the maps disagreed. Measured 2026-09-04, at
 * `2e3ec462b`, over the 32 pairs that differ:
 *
 *   * `lib/deployments.ts` and `lib/walletHandoff.ts` were REEXPORT to the
 *     test and ABSENT from the script — so `--copy` would have overwritten a
 *     286-line deployment manifest and a 446-line conformance module with the
 *     two-line browser shims that re-export them.
 *   * `lib/directMakerReplay.ts` was SDK-owned to the script and BACKLOG to
 *     the test, which is the same file described as "never absorb this" and
 *     "absorb this" at once.
 *   * and the disagreement was wider than the three that were noticed:
 *     `lib/directParticipant.ts`, `lib/resolutionCertificateV2.ts` and
 *     `lib/supplyShares.ts` were BACKLOG to the test while being one-line
 *     re-exports of 673, 164 and 79 lines of SDK, and
 *     `lib/directMakerReplay.test.ts` was BACKLOG in both directions while
 *     the SDK copy was 59 lines AHEAD.
 *
 * The fix is not a third map. A file has one class; the two readers ask
 * different questions of it. So the class lives here, once, and each reader
 * asks its own question through {@link absorbsFromWeb} and
 * {@link twinsMustDiffer}.
 *
 * ## The classes
 *
 * | class | sync-from-web | twin identity |
 * | --- | --- | --- |
 * | `TWIN` | absorbs on drift | pair must be byte-identical |
 * | `BACKLOG` | absorbs on drift | pair must still differ |
 * | `DELIBERATE-DIVERGENCE` | reports, never copies | pair must still differ |
 * | `REEXPORT` | skips | pair must still differ |
 * | `SHIM` | skips | pair must still differ |
 * | `SDK-OWNED` | skips | pair must still differ, or has no web copy |
 * | `WEB-ONLY` | skips | must have NO SDK copy |
 *
 * `TWIN`, and the unabsorbed half of `BACKLOG`, are DERIVED rather than
 * listed: a pair with no entry here is a twin, and a web file with no SDK copy
 * and no entry here is absorption backlog. Everything else is named below with
 * its reason, which is why this table shrinks the same way the old ones did —
 * absorb a file, delete its line.
 */

/** The seven classes, exhaustive and disjoint. */
export const TWIN_CLASSES = Object.freeze([
  'TWIN',
  'REEXPORT',
  'SHIM',
  'SDK-OWNED',
  'BACKLOG',
  'DELIBERATE-DIVERGENCE',
  'WEB-ONLY',
]);

const REEXPORT_REASON = 'the web file is a compatibility shim re-exporting its SDK semantic owner';

/**
 * Every web-tree path whose class is not the derived default, keyed by its
 * path relative to `apps/dclutch-web`.
 *
 * Keep this at the size of its justifications. A line here is a place where
 * two trees answer differently and only this file says which answer is meant.
 */
export const TWIN_CLASSIFICATION = Object.freeze({
  // --- WEB-ONLY: no SDK copy, by design. ---
  'lib/walletStandard.ts': ['WEB-ONLY', 'Wallet Standard discovery is browser coupling: it reads `window`'],
  'lib/walletStandard.test.ts': ['WEB-ONLY', 'the browser-coupled half tests only in the browser tree'],
  'lib/sbomVerify.test.ts': ['WEB-ONLY', 'a repo-wide gate over the published SBOM, not client logic'],
  'lib/twinIdentity.test.ts': ['WEB-ONLY',
    'the twin gate itself: it reads BOTH trees, so an absorbed copy would compare a package against itself and pass on anything'],

  // --- REEXPORT: the web file is `export * from` its SDK owner and nothing else. ---
  // The check below is not a promise — `isPureReExport` reads the file, so a
  // line here that stops being a re-export takes the gate red.
  'lib/capabilityModel.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/deployments.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/directMakerReplay.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/directOfferAuthoring.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/directParticipant.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/founding/principalCapacity.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/marketDiscovery.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/marketResolution.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/operatorSurface.ts': ['REEXPORT',
    'was a 377-line fork that authenticated the deployment more weakly than its owner; deleted rather than deepened'],
  'lib/rationalOpenChainV4.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/rationalOpenHotV3.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/rationalOpenWasmV1.testSupport.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/rationalRetireReceiptV4.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/rationalTerminalChainV4.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/resolutionCertificateV2.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/supplyShares.ts': ['REEXPORT', REEXPORT_REASON],
  'lib/walletHandoff.ts': ['REEXPORT', REEXPORT_REASON],

  // --- SHIM: delegates the semantics and owns one browser-only half. ---
  'lib/slotClock.ts': ['SHIM',
    're-exports the SDK arithmetic and adds the one chain-touching step, reading two cluster block times'],
  'lib/slotClock.test.ts': ['SHIM', 'tests only the app half the shim adds'],
  'lib/ticketBoard.ts': ['SHIM',
    'the SDK transport takes its board URL as an argument; this is where the deployment is decided, from the one NEXT_PUBLIC_* variable a static export keeps'],

  // --- SDK-OWNED: the package copy is AHEAD; absorbing upward would regress it. ---
  'lib/directMakerReplay.test.ts': ['SDK-OWNED',
    'the module beside it is a re-export, and the SDK test has grown the nonce-PAIR cases this copy never got: absorbing web-side would delete them'],

  // --- DELIBERATE-DIVERGENCE: SDK-side edits, merged by hand. ---
  'lib/founding/principalCapacity.test.ts': ['DELIBERATE-DIVERGENCE', 'deliberate SDK-side edit; merge by hand'],
  'lib/localSuccessor.ts': ['DELIBERATE-DIVERGENCE', 'deliberate SDK-side edit; merge by hand'],
  'lib/rpc.ts': ['DELIBERATE-DIVERGENCE', 'deliberate SDK-side edit; merge by hand'],
  'lib/rpc.test.ts': ['DELIBERATE-DIVERGENCE', 'deliberate SDK-side edit; merge by hand'],
  'scripts/abi-coverage.mjs': ['DELIBERATE-DIVERGENCE',
    'each package censuses its own module inventory against its own baseline'],
  'scripts/abi-coverage.baseline.json': ['DELIBERATE-DIVERGENCE', 'the baseline of the census above, per package'],

  // --- BACKLOG: web-side work the package has not absorbed, named as debt. ---
  'lib/activity.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/activity.test.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/claimsCustodyReplay.test.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/directTicket.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/directTicket.test.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/directTradeSpine.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
  'lib/founding/lookupTable.ts': ['BACKLOG', 'web-side change awaiting SDK absorption'],
});

/**
 * The class of one web-tree path, and why.
 *
 * `hasSdkTwin` decides the two derived answers: a listed path keeps its class,
 * an unlisted path with a package copy is a `TWIN`, and an unlisted path
 * without one is `BACKLOG` the package has never absorbed at all.
 */
export function classifyWebPath(path, hasSdkTwin) {
  const entry = TWIN_CLASSIFICATION[path];
  if (entry !== undefined) return Object.freeze({ class: entry[0], reason: entry[1], listed: true });
  if (hasSdkTwin) return Object.freeze({ class: 'TWIN', reason: 'hand-maintained copies of one file', listed: false });
  return Object.freeze({ class: 'BACKLOG', reason: 'not absorbed into the package at all', listed: false });
}

/** Whether `sync-from-web.mjs` may copy this class over the package's copy. */
export function absorbsFromWeb(twinClass) {
  return twinClass === 'TWIN' || twinClass === 'BACKLOG';
}

/** Whether `sync-from-web.mjs` reports the drift without copying it. */
export function reportsWithoutCopying(twinClass) {
  return twinClass === 'DELIBERATE-DIVERGENCE';
}

/** Whether the twin gate requires the pair to DIFFER rather than to match. */
export function twinsMustDiffer(twinClass) {
  return twinClass !== 'TWIN';
}

/**
 * Whether a file's text is a re-export of one module and nothing else.
 *
 * The `REEXPORT` class is the one that costs 974 lines when it is wrong, so it
 * is checked against the file rather than asserted: strip comments, and what
 * remains must be a single `export * from '…';`.
 */
export function isPureReExport(text) {
  const bare = text
    .replaceAll(/\/\*[\s\S]*?\*\//g, '')
    .replaceAll(/^\s*\/\/.*$/gm, '')
    .trim();
  return /^export \* from '[^']+';$/.test(bare);
}
