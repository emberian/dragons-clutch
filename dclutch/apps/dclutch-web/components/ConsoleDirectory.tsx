import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { Card, CardContent } from '@/components/ui/card';
import {
  CAPABILITY_ACTIONS_V1,
  capabilityActContractV1,
  type CapabilityActionV1,
} from '@/lib/capabilityModel';
import { docsHrefV1 } from '@/lib/flags';

/**
 * `/console` — one directory generated from the executable capability truth.
 *
 * A linked workspace may still perform its own chain-specific preflight, but
 * this directory never keeps a second description of what the workspace can
 * do. Capability labels and act contracts come directly from the catalogue
 * used by `/operate`; only lifecycle grouping and two evidence-only support
 * tools are local presentation facts.
 */

type ActionConsoleDefinitionV1 = Readonly<{
  workspace: string;
  href: string;
  name: string;
}>;

type SupportConsoleV1 = Readonly<{
  href: string;
  name: string;
  outcome: string;
  contract: string;
}>;

type ConsoleEntryV1 = Readonly<{
  href: string;
  name: string;
  actions: ReadonlyArray<CapabilityActionV1>;
  support: SupportConsoleV1 | null;
}>;

type ConsoleBandV1 = Readonly<{
  title: string;
  deck: string;
  actionConsoles?: ReadonlyArray<ActionConsoleDefinitionV1>;
  supportConsoles?: ReadonlyArray<SupportConsoleV1>;
}>;

const BANDS_V1: ReadonlyArray<ConsoleBandV1> = Object.freeze([
  Object.freeze({
    title: 'Author and open',
    deck: 'Compile the product, then open the market from authenticated inputs.',
    actionConsoles: Object.freeze([
      Object.freeze({ workspace: '/product-v2', href: '/product-v2#spline-product', name: 'Product compiler' }),
      Object.freeze({ workspace: '/found', href: '/found#current-founding', name: 'Founding' }),
    ]),
  }),
  Object.freeze({
    title: 'Trade and resolve',
    deck: 'Construct the market’s live trading, clearing, and resolution acts.',
    actionConsoles: Object.freeze([
      Object.freeze({ workspace: '/liquidity', href: '/liquidity', name: 'Dealer liquidity' }),
      Object.freeze({ workspace: '/general', href: '/general', name: 'General clearing' }),
      Object.freeze({ workspace: '/resolution', href: '/resolution', name: 'Resolution' }),
    ]),
  }),
  Object.freeze({
    title: 'Run the deployment',
    deck: 'Activate checked releases and produce current operator artifacts.',
    actionConsoles: Object.freeze([
      Object.freeze({ workspace: '/release', href: '/release', name: 'Release activation' }),
      Object.freeze({ workspace: '/operate', href: '/operate', name: 'Operations' }),
    ]),
  }),
  Object.freeze({
    title: 'Verify the record',
    deck: 'Reacquire lifecycle readiness and compare durable evidence with finalized state.',
    supportConsoles: Object.freeze([
      Object.freeze({
        href: '/workbench',
        name: 'Lifecycle workbench',
        outcome: 'Read the remaining lifecycle work for one authenticated Market.',
        contract: 'Finalized reads only. Produces a readiness map; no transaction.',
      }),
      Object.freeze({
        href: '/local',
        name: 'Local successor',
        outcome: 'Rejoin a checkpointed local validator to its published evidence.',
        contract: 'Local files and finalized reads only. Produces a byte-for-byte comparison.',
      }),
    ]),
  }),
]);

function workspacePathV1(workspace: CapabilityActionV1['workspace']): string | null {
  if (workspace === null || workspace === 'market-detail') return null;
  return workspace.split('#', 1)[0] ?? null;
}

const EXECUTABLE_ACTIONS_V1 = Object.freeze(CAPABILITY_ACTIONS_V1.filter(
  (candidate) => candidate.implementation !== 'awaiting-production',
));

function entriesForBandV1(band: ConsoleBandV1): ReadonlyArray<ConsoleEntryV1> {
  const actionEntries = (band.actionConsoles ?? []).map((definition) => Object.freeze({
    href: definition.href,
    name: definition.name,
    actions: Object.freeze(EXECUTABLE_ACTIONS_V1.filter(
      (candidate) => workspacePathV1(candidate.workspace) === definition.workspace,
    )),
    support: null,
  })).filter((entry) => entry.actions.length > 0);
  const supportEntries = (band.supportConsoles ?? []).map((support) => Object.freeze({
    href: support.href,
    name: support.name,
    actions: Object.freeze([]) as ReadonlyArray<CapabilityActionV1>,
    support,
  }));
  return Object.freeze([...actionEntries, ...supportEntries]);
}

function contractsForActionsV1(actions: ReadonlyArray<CapabilityActionV1>): ReadonlyArray<string> {
  return Object.freeze(Array.from(new Set(actions.map((candidate) => {
    const contract = capabilityActContractV1(candidate);
    return `${contract.result} ${contract.authority}`;
  }))));
}

const DIRECTORY_ACTION_COUNT_V1 = BANDS_V1.reduce(
  (total, band) => total + entriesForBandV1(band).reduce((subtotal, entry) => subtotal + entry.actions.length, 0),
  0,
);

export default function ConsoleDirectory() {
  return <main className="product-shell trade-v3-shell">
    <Nav current="/console" status="operator tools" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Tools for running and building on dClutch</p>
        <h1>Choose the<br /><em>outcome.</em></h1>
        <p>{DIRECTORY_ACTION_COUNT_V1} executable protocol acts are routed below. Each
        entry says what works, what it produces, and whose authority it asks for.
        Market-participant acts stay on the selected <Anchor href="/markets">Market</Anchor>.</p>
        <p>These claims come from the same capability catalogue used for chain
        preflight. Artifact inputs name their producer; the complete provenance
        table is <a href={docsHrefV1('readme.html', 'README.md')}>“The artifacts,
        and where they come from”</a>.</p>
      </div>
    </section>

    <section aria-label="Operator consoles">
      {BANDS_V1.map((band, index) => {
        const entries = entriesForBandV1(band);
        return <Card className="trade-v3-card" key={band.title}>
          <header><span>{String(index + 1).padStart(2, '0')}</span><div><h2>{band.title}</h2><p>{band.deck}</p></div></header>
          <CardContent className="console-index p-0">
            {entries.map((entry) => {
              const outcomes = entry.support === null
                ? entry.actions.map((candidate) => candidate.action)
                : [entry.support.outcome];
              const contracts = entry.support === null
                ? contractsForActionsV1(entry.actions)
                : [entry.support.contract];
              return <Anchor key={entry.href} className="console-entry" href={entry.href}>
                <strong>{entry.name}</strong>
                <span className="console-entry-copy">
                  <b>{outcomes.join(' · ')}</b>
                  {contracts.map((contract) => <small key={contract}>{contract}</small>)}
                </span>
                <em aria-hidden="true">→</em>
              </Anchor>;
            })}
          </CardContent>
        </Card>;
      })}
    </section>
  </main>;
}
