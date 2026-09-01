import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

import { JSDOM } from 'jsdom';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DirectPage from '@/app/direct/page';
import NotFound from '@/app/not-found';
import ActivityWorkspace from '@/components/ActivityWorkspace';
import BountyWalk from '@/components/BountyWalk';
import CampaignWorkspace from '@/components/CampaignWorkspace';
import ChainExplorer from '@/components/ChainExplorer';
import ConsoleDirectory from '@/components/ConsoleDirectory';
import CoreFoundWorkspace from '@/components/CoreFoundWorkspace';
import CreateMarketWizard from '@/components/CreateMarketWizard';
import DealerLiquidityWorkspace from '@/components/DealerLiquidityWorkspace';
import DirectTradeWorkspace from '@/components/DirectTradeWorkspace';
import GeneralWorkspace from '@/components/GeneralWorkspace';
import LaunchStory from '@/components/LaunchStory';
import LocalSuccessorWorkspace from '@/components/LocalSuccessorWorkspace';
import MarketAddressWorkspace from '@/components/MarketAddressWorkspace';
import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';
import MarketDiscoveryWorkspace from '@/components/MarketDiscoveryWorkspace';
import PopulationWorkspace from '@/components/PopulationWorkspace';
import PulseWorkspace from '@/components/PulseWorkspace';
import RationalRepresentationWorkspace from '@/components/RationalRepresentationWorkspace';
import ReleaseWorkspace from '@/components/ReleaseWorkspace';
import ResolutionWorkspace from '@/components/ResolutionWorkspace';
import SiteLanding from '@/components/SiteLanding';
import SmokeStory from '@/components/SmokeStory';



/**
 * The landmark gate, and the instrument it needed.
 *
 * THE MEASURED REASON THIS COULD NOT EXIST BEFORE. `vitest.config.ts` sets
 * `environment: 'node'`. There is no document, so across 171 test files there
 * were zero `getByRole` queries, zero `getByLabelText` queries and zero axe
 * runs, and every accessibility assertion in the repository was a substring
 * match on a rendered HTML string -- 1,012 of them. A substring match can see
 * that the word `<main` appears. It cannot see what encloses what. So "the
 * site header is nested inside `<main>` on 28 of 28 page shells" was true for
 * as long as it liked, in a suite that ran green the whole time.
 *
 * This file buys the missing instrument as a LIBRARY rather than as a vitest
 * environment, and the difference is load-bearing. Setting
 * `@vitest-environment jsdom` flips Vite's resolve conditions to `browser`,
 * which hands `@solana/web3.js` its browser build -- and the first run that
 * way did not fail an assertion, it failed to import: `findProgramAddressSync`
 * threw "Unable to find a viable program address nonce" while
 * `lib/operatorSurface.ts` was still evaluating. An instrument that changes
 * what the code under test resolves to is measuring a different application.
 * So the components render exactly as the rest of the suite renders them, in
 * the node environment, and jsdom only parses the HTML they produce. That
 * string is also precisely what the static export ships, which makes this the
 * tree a reader's browser actually receives.
 *
 * WHAT IT HOLDS.
 *   1. Exactly one `<main>` per page. Two main landmarks is the same defect as
 *      none: a reader jumping to "main" gets a coin toss.
 *   2. The site header is NOT inside `<main>`. `<header>` maps to the `banner`
 *      landmark only while it is not a descendant of `main`, `article`,
 *      `aside`, `nav` or `section` -- so nesting it inside `main` silently
 *      demotes it to a plain group, and the reader loses the one landmark that
 *      answers "where does this site's chrome start".
 *   3. The skip link's target is inside `<main>`. "Skip to main content" that
 *      lands anywhere else is a link that says one thing and does another.
 *
 * The roster below is checked against a source survey, so a new page shell
 * cannot be added without either joining the roster or being exempted in
 * writing. A gate that only covers the pages somebody remembered is the shape
 * of the problem, not the fix.
 */

/**
 * This application's root, taken from the working directory rather than from
 * `import.meta.url`.
 *
 * The sibling `.ts` surveys anchor themselves with
 * `fileURLToPath(new URL(..., import.meta.url))` and that is correct there. In
 * a `.tsx` file under this pipeline `import.meta.url` is not a `file:` URL at
 * all, and `fileURLToPath` throws while the module is still evaluating -- which
 * is what was actually wrong here for three runs, wearing a borrowed stack from
 * `@solana/web3.js` the whole time.
 *
 * Verified rather than assumed: a survey rooted at the wrong directory finds
 * nothing and reports every page shell as covered, which is the exact failure
 * this gate exists to refuse.
 */
const webRoot = process.cwd();
if (JSON.parse(readFileSync(join(webRoot, 'package.json'), 'utf8')).name !== 'dclutch-web') {
  throw new Error(`the landmark survey is rooted at ${webRoot}, which is not apps/dclutch-web`);
}

/** A market address the detail workspace can render before any chain read. */
const MARKET_V1 = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';

/** Every page shell this application serves, and how to render one. */
const PAGE_SHELLS_V1: ReadonlyArray<Readonly<{ file: string; render: (() => string) | null }>> = Object.freeze([
  { file: 'app/direct/page.tsx', render: () => renderToStaticMarkup(<DirectPage />) },
  { file: 'app/not-found.tsx', render: () => renderToStaticMarkup(<NotFound />) },
  { file: 'components/ActivityWorkspace.tsx', render: () => renderToStaticMarkup(<ActivityWorkspace />) },
  { file: 'components/BountyWalk.tsx', render: () => renderToStaticMarkup(<BountyWalk />) },
  { file: 'components/CampaignWorkspace.tsx', render: () => renderToStaticMarkup(<CampaignWorkspace />) },
  { file: 'components/ChainExplorer.tsx', render: () => renderToStaticMarkup(<ChainExplorer />) },
  { file: 'components/ConsoleDirectory.tsx', render: () => renderToStaticMarkup(<ConsoleDirectory />) },
  { file: 'components/CoreFoundWorkspace.tsx', render: () => renderToStaticMarkup(<CoreFoundWorkspace />) },
  { file: 'components/CreateMarketWizard.tsx', render: () => renderToStaticMarkup(<CreateMarketWizard />) },
  { file: 'components/DealerLiquidityWorkspace.tsx', render: () => renderToStaticMarkup(<DealerLiquidityWorkspace />) },
  { file: 'components/DirectTradeWorkspace.tsx', render: () => renderToStaticMarkup(<DirectTradeWorkspace />) },
  { file: 'components/GeneralWorkspace.tsx', render: () => renderToStaticMarkup(<GeneralWorkspace />) },
  { file: 'components/LaunchStory.tsx', render: () => renderToStaticMarkup(<LaunchStory />) },
  { file: 'components/LocalSuccessorWorkspace.tsx', render: () => renderToStaticMarkup(<LocalSuccessorWorkspace />) },
  { file: 'components/MarketAddressWorkspace.tsx', render: () => renderToStaticMarkup(<MarketAddressWorkspace />) },
  { file: 'components/MarketDetailWorkspace.tsx', render: () => renderToStaticMarkup(<MarketDetailWorkspace address={MARKET_V1} />) },
  { file: 'components/MarketDiscoveryWorkspace.tsx', render: () => renderToStaticMarkup(<MarketDiscoveryWorkspace />) },
  { file: 'components/MarketWorkbench.tsx', render: null },
  { file: 'components/OperatorSurface.tsx', render: null },
  { file: 'components/PopulationWorkspace.tsx', render: () => renderToStaticMarkup(<PopulationWorkspace />) },
  { file: 'components/PortfolioWorkspace.tsx', render: null },
  { file: 'components/ProductV2Studio.tsx', render: null },
  { file: 'components/PulseWorkspace.tsx', render: () => renderToStaticMarkup(<PulseWorkspace />) },
  { file: 'components/RationalRepresentationWorkspace.tsx', render: () => renderToStaticMarkup(<RationalRepresentationWorkspace />) },
  { file: 'components/ReleaseWorkspace.tsx', render: () => renderToStaticMarkup(<ReleaseWorkspace />) },
  { file: 'components/ResolutionWorkspace.tsx', render: () => renderToStaticMarkup(<ResolutionWorkspace />) },
  { file: 'components/SiteLanding.tsx', render: () => renderToStaticMarkup(<SiteLanding />) },
  { file: 'components/SmokeStory.tsx', render: () => renderToStaticMarkup(<SmokeStory />) },
]);

/** Every source file that opens a `<main>` and puts a site header in it. */
function surveyPageShells(): ReadonlyArray<string> {
  const found: string[] = [];
  const walk = (absolute: string) => {
    for (const entry of readdirSync(absolute).sort()) {
      if (entry === 'node_modules' || entry === 'dist' || entry.startsWith('.')) continue;
      const child = join(absolute, entry);
      if (statSync(child).isDirectory()) { walk(child); continue; }
      if (!entry.endsWith('.tsx') || entry.endsWith('.test.tsx')) continue;
      const source = readFileSync(child, 'utf8');
      // A page shell is either the canonical `PageShell`, or a hand-rolled
      // `<main>` that carries site chrome. The second clause is the one that
      // matters: it catches a page that opens its own main and puts a header
      // in it, which is precisely the shape this gate exists to refuse, and it
      // would be invisible to a survey that only looked for `PageShell`.
      const shell = /<PageShell[\s/>]/.test(source);
      const handRolled = source.includes('<main')
        && (/<Nav[\s/>]/.test(source) || /<ConsoleHeader[\s/>]/.test(source));
      if (!shell && !handRolled) continue;
      found.push(relative(webRoot, child).split('\\').join('/'));
    }
  };
  walk(join(webRoot, 'app'));
  walk(join(webRoot, 'components'));
  return found.sort();
}

function parse(html: string): Document {
  return new JSDOM(`<!doctype html><html><body>${html}</body></html>`).window.document;
}

describe('landmark structure', () => {
  it('renders every page shell the source survey finds', () => {
    // The roster is the thing that could silently shrink. If a new page shell
    // lands and nobody adds it here, this fails rather than the gate quietly
    // covering 27 of 28.
    expect(PAGE_SHELLS_V1.map((entry) => entry.file)).toEqual([...surveyPageShells()]);
    expect(PAGE_SHELLS_V1.length).toBeGreaterThanOrEqual(20);
  });

  it('renders all but the four this harness cannot load together', () => {
    // NOT a soft edge. Four page shells import `lib/operatorSurface.ts`, whose
    // module scope derives a ProgramData address for each of five roles. In
    // THIS file's module graph -- and only past a threshold, bisected to the
    // eighteenth component import -- that derivation throws "Unable to find a
    // viable program address nonce" during collection, while the same module
    // imported by itself, or by its own test file, evaluates fine.
    //
    // That is a real defect and it is routed as one: a module whose
    // module-scope work depends on how much of the graph loaded before it is a
    // latent bundle bug, not a test inconvenience. It is recorded here rather
    // than worked around silently, and the count is pinned so the list cannot
    // grow while nobody is looking.
    const unrendered = PAGE_SHELLS_V1.filter((shell) => shell.render === null).map((shell) => shell.file);
    expect(unrendered).toEqual([
      'components/MarketWorkbench.tsx',
      'components/OperatorSurface.tsx',
      'components/PortfolioWorkspace.tsx',
      'components/ProductV2Studio.tsx',
    ]);
  });

  for (const shell of PAGE_SHELLS_V1) {
    if (shell.render === null) continue;
    // Rendered inside the case, not at collection: a page that cannot render
    // must fail as itself rather than taking the whole file down with a stack
    // that names whichever module happened to be evaluating.
    it(`${shell.file} carries one main, a banner outside it, and the skip target inside it`, () => {
      // Rendered inside the case rather than at collection time: a page that
      // cannot render must fail as itself, not take the whole file down.
      const document = parse(shell.render!());

      expect(document.querySelectorAll('main').length, 'a page needs exactly one main landmark').toBe(1);

      const header = document.querySelector('header.product-nav');
      expect(header, 'this page shell renders no site header at all').not.toBeNull();
      expect(
        header?.closest('main'),
        'the site header is inside <main>, which demotes it out of the banner landmark',
      ).toBeNull();

      const target = document.querySelector('#main-content');
      expect(target, 'nothing on this page answers to #main-content').not.toBeNull();
      expect(
        target?.closest('main'),
        '"Skip to main content" lands outside the main it names',
      ).not.toBeNull();
    });
  }
});