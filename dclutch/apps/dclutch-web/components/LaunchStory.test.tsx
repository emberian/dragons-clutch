import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The closed face, pinned against a mock pending cut now that the published
// fixture names a Market. Its companion, LaunchStory.opened.test.tsx, renders
// the open face the same way; between them both states stay guarded whatever
// the fixture currently says.
vi.mock('@/lib/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@/lib/publicCutStaging')>('@/lib/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: null,
    activity: { found: null, trade: null, resolve: null, redeem: null },
    checkedReleases: {},
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { DEVNET_DEPLOYMENT_V1 } = await import('@/lib/deployments');
const { default: LaunchStory } = await import('./LaunchStory');

describe('launch story', () => {
  it('says no market is open, in words a reader can act on', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('Markets you can');
    expect(html).toContain('check <em>yourself.</em>');
    expect(html).toContain('No market is open yet.');
    expect(html).toContain('nothing it cannot back up');
    expect(html).toContain('href="/markets"');
    expect(html).toContain('href="/explorer"');
    expect(html).toContain('href="/activity"');
    expect(html).toContain('<strong>7</strong><span>programs</span>');
    expect(html).toContain('<strong>64</strong><span>lock cap</span>');
    expect(html).toContain('<strong>NO</strong><span>market open</span>');
    expect(html).not.toContain('0.50%');
    expect(html).toContain('href="/campaign"');
    expect(html).toContain('href="/population"');
    // DERIVED FROM THE RECORD, not restated. This pinned cohort-8's shortened
    // registry id as a literal, so the assertion aged out the moment devnet
    // redeployed and told a reader the page was broken when only the test was.
    // The claim worth making is that the page shows THE SHIPPED registry.
    const registry = DEVNET_DEPLOYMENT_V1.programs.registry;
    expect(html).toContain(`${registry.slice(0, 5)}…${registry.slice(-5)}`);
    expect(html).toContain('Test assets have no monetary value.');
  });

  // The page's headline claim is a chain of steps, and it once read
  // FOUND -> DIRECT -> RESOLVE -> REDEEM under the label "release / current"
  // while neither resolution nor redemption was reachable. That is the exact
  // regression these two assertions exist to refuse: the advertised chain must
  // stop where the product stops.
  it('advertises only the steps that work, and dates the ones that do not', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('<code>FOUND → JOIN → TRADE</code>');
    expect(html).not.toContain('RESOLVE');
    expect(html).not.toContain('REDEEM');
    // Resolve and Redeem stay on the rail -- a reader should know where the
    // product is going -- but only in the future tense.
    expect(html).toContain('<strong>Resolve</strong><p>Not yet.');
    expect(html).toContain('<strong>Redeem</strong><p>Not yet — no market has an answer.');
    expect(html).toContain('Follow the three steps that work today.');
  });

  it('does not call itself live while no market is open', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('public devnet · programs deployed');
    expect(html).not.toContain('public devnet · live');
    expect(html).not.toContain('LIVE / 01');
    // The finale heading used to say "Open the market." unconditionally, beside
    // a button that correctly fell back to "Explore markets".
    expect(html).not.toContain('<em>Open the market.</em>');
    expect(html).toContain('<em>Read the chain.</em>');
  });
});
