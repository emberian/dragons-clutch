'use client';

import PageShell from '@/components/PageShell';
import Nav from '@/components/Nav';
import PublicDeploymentEvidence from '@/components/PublicDeploymentEvidence';
import { FormEvent, ReactNode, useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';

import { deploymentProgramLabelsV1, type DeploymentV1 } from '@/lib/deployments';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { inspectAccount, type ExplorerAccountResult } from '@/lib/explorer/account';
import type { DecodedField, DecodedRecord } from '@/lib/explorer/accountRecords';
import type { Derivation } from '@/lib/explorer/derivations';
import { unselectedEntryRoutes } from '@/lib/explorer/instructions';
import { inspectMarketLens, type LensNode, type MarketLens } from '@/lib/explorer/marketLens';
import {
  classifySearchV1,
  inspectProtocolHomeV1,
  type ProtocolActivityRowV1,
  type ProtocolHomeV1,
  type ProtocolProgramCardV1,
} from '@/lib/explorer/protocolHome';
import { SBF_DEFAULT_HEAP_BYTES_V1, SBF_RUNTIME_VERSIONS_V1 } from '@/lib/generated/sbfRuntimeV1';
import { attributionTitle, describeAttribution, hexCode } from '@/lib/explorer/refusals';
import {
  inspectTransaction,
  type ExplorerInstruction,
  type ExplorerTransactionResult,
} from '@/lib/explorer/transaction';
import { inspectFinalizedRecord, type RecordObservation } from '@/lib/records';
import { scanProgram, SolanaRpcClient, type ConnectionFacts, type ProgramSnapshot } from '@/lib/rpc';

type View = 'account' | 'transaction' | 'market' | 'scan' | 'record';

const VIEWS: ReadonlyArray<Readonly<{ id: View; label: string; hint: string }>> = Object.freeze([
  { id: 'account', label: 'Account', hint: 'Any address, decoded against its own record magic' },
  { id: 'transaction', label: 'Transaction', hint: 'A signature, decoded per program, refusals named' },
  { id: 'market', label: 'Market lens', hint: 'One Market’s record graph, every id navigable' },
  { id: 'scan', label: 'Program scan', hint: 'Bounded sweep of a program’s own accounts' },
  { id: 'record', label: 'Record pair', hint: 'One finalized record and its staging cursor' },
]);

type Query = Readonly<{ view: View; q: string }>;

type Async<T> =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'loading'; message: string }>
  | Readonly<{ kind: 'error'; message: string }>
  | Readonly<{ kind: 'ready'; value: T }>;

const IDLE = Object.freeze({ kind: 'idle' } as const);

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'The request failed without a usable error message.';
}

function compact(value: string, edge = 6): string {
  return value.length <= edge * 2 + 1 ? value : `${value.slice(0, edge)}…${value.slice(-edge)}`;
}

function isView(value: string | null): value is View {
  return VIEWS.some((entry) => entry.id === value);
}

// ----------------------------------------------------- the location, as state

function subscribeToLocation(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange);
  return () => window.removeEventListener('popstate', onChange);
}

function readLocationSearch(): string {
  return window.location.search;
}

/** The server has no location; the client re-reads on hydration. */
function readServerSearch(): string {
  return '';
}

function parseSearch(search: string): Query {
  const params = new URLSearchParams(search);
  const view = params.get('view');
  return Object.freeze({ view: isView(view) ? view : 'account', q: params.get('q') ?? '' });
}

// ---------------------------------------------------------------- small parts

/** A link into another view of this explorer. A real href, so it can be copied. */
function Jump({
  view,
  value,
  children,
  title,
}: Readonly<{ view: View; value: string; children: ReactNode; title?: string }>) {
  return (
    <a
      className="xp-jump"
      href={`?view=${view}&q=${encodeURIComponent(value)}`}
      title={title ?? value}
      data-xp-view={view}
      data-xp-q={value}
    >
      {children}
    </a>
  );
}

function Chip({ tone, children }: Readonly<{ tone: 'pass' | 'caution' | 'fail' | 'muted'; children: ReactNode }>) {
  return <span className={`status-chip ${tone}`}>{children}</span>;
}

function Notice({ kind, title, message }: Readonly<{ kind: 'loading' | 'error' | 'quiet'; title: string; message: string }>) {
  return (
    <div className={`notice ${kind === 'quiet' ? '' : kind}`}>
      <span className="notice-mark" aria-hidden="true">{kind === 'loading' ? '…' : kind === 'error' ? '!' : '·'}</span>
      <div>
        <p className="eyebrow">{kind === 'loading' ? 'Bounded request in progress' : kind === 'error' ? 'No state accepted' : 'Nothing queried yet'}</p>
        <h2>{title}</h2>
        <p>{message}</p>
      </div>
    </div>
  );
}

function Honest({ children }: Readonly<{ children: ReactNode }>) {
  return <p className="xp-honest">{children}</p>;
}

// --------------------------------------------------------------- field values

function FieldValue({ field }: Readonly<{ field: DecodedField }>) {
  const value = field.value;
  switch (value.form) {
    case 'scalar':
      return <span className="xp-num">{value.text}</span>;
    case 'address':
      return (
        <Jump view="account" value={value.base58}>
          {compact(value.base58, 8)}
        </Jump>
      );
    case 'identity':
      return <span className="xp-hex" title={value.hex}>{compact(value.hex, 10)}</span>;
    case 'both':
      return (
        <span className="xp-both">
          {value.base58 === null ? null : (
            <Jump view="account" value={value.base58}>{compact(value.base58, 8)}</Jump>
          )}
          <em className="xp-hex" title={value.hex}>{compact(value.hex, 10)}</em>
        </span>
      );
    case 'enum':
      return value.name === null
        ? <span className="xp-unknown">tag {value.tag} · unnamed by the schema</span>
        : <span className="xp-enum">{value.name}<em>tag {value.tag}</em></span>;
    case 'reserved':
      return value.zero
        ? <span className="xp-ok">zero, as required</span>
        : <span className="xp-bad" title={value.hex}>NONZERO · {compact(value.hex, 8)}</span>;
    case 'span':
      return (
        <span className="xp-span">
          {value.bytes} bytes<em title={value.hex}>{value.hex}…</em>
          {value.note === null ? null : <small>{value.note}</small>}
        </span>
      );
    case 'scale':
      // The number and its reading together. The exponent alone is the byte;
      // the reading alone would hide which value produced it.
      return (
        <span className="xp-scale">
          <span className="xp-num">{value.exponent}</span>
          <small>{value.reading}</small>
        </span>
      );
    case 'refused':
      return <span className="xp-bad">{value.reason}</span>;
  }
}

function RecordFields({ decoded }: Readonly<{ decoded: DecodedRecord }>) {
  if (decoded.fields.length === 0) {
    return <Honest>{decoded.spec.note}</Honest>;
  }
  return (
    <>
      <table className="xp-fields">
        <thead>
          <tr><th>offset</th><th>field</th><th>value</th></tr>
        </thead>
        <tbody>
          {decoded.fields.map((field) => (
            <tr key={`${field.label}-${field.offset}`}>
              <td className="xp-off">{field.offset}<em>+{field.bytes}</em></td>
              <td className="xp-label">{field.label}</td>
              <td className="xp-value"><FieldValue field={field} /></td>
            </tr>
          ))}
        </tbody>
      </table>
      {decoded.rows === null ? null : (
        <div className="xp-rows">
          <p className="eyebrow">{decoded.rows.count} × {decoded.rows.label} · {decoded.rows.strideBytes} bytes each, from offset {decoded.rows.offset}</p>
          {decoded.rows.scalars === null
            ? <Honest>The rows are wider than one scalar, so they are counted rather than expanded.</Honest>
            : <ol className="xp-scalars">{decoded.rows.scalars.map((entry, index) => <li key={index}><em>{index}</em>{entry}</li>)}</ol>}
        </div>
      )}
      {decoded.spec.note === null ? null : <Honest>{decoded.spec.note}</Honest>}
    </>
  );
}

function Derivations({ derivations }: Readonly<{ derivations: ReadonlyArray<Derivation> }>) {
  if (derivations.length === 0) return null;
  return (
    <ul className="checks">
      {derivations.map((derivation) => (
        <li key={derivation.name} className={derivation.matches ? 'check-pass' : 'check-fail'}>
          <span aria-hidden="true">{derivation.matches ? '✓' : '×'}</span>
          <div>
            <strong>{derivation.name}{derivation.matches ? '' : ' — DOES NOT REPRODUCE THIS ADDRESS'}</strong>
            <small>
              [{derivation.seeds.join(' · ')}] under {compact(derivation.program, 6)} → {compact(derivation.derived, 8)}
              {derivation.matches ? `, bump ${derivation.bump}` : ''}
            </small>
          </div>
        </li>
      ))}
    </ul>
  );
}

// ------------------------------------------------------------- account view

function AccountView({ state }: Readonly<{ state: Async<ExplorerAccountResult> }>) {
  if (state.kind === 'idle') {
    return <Notice kind="quiet" title="Paste an address." message="Any account: a Market, a Position, a finalized record, a token account, a program. Its layout comes from its own leading magic, and its PDA derivations are run under its actual owner." />;
  }
  if (state.kind === 'loading') return <Notice kind="loading" title="Reading the account" message={state.message} />;
  if (state.kind === 'error') return <Notice kind="error" title="Account read refused" message={state.message} />;
  if (state.value.status === 'empty') {
    return (
      <article className="account-card refused">
        <div className="card-topline"><p className="account-kind">Empty</p><Chip tone="fail">absent</Chip></div>
        <h3 title={state.value.address}>{compact(state.value.address, 12)}</h3>
        <p className="refusal-reason">{state.value.reason}</p>
      </article>
    );
  }

  const account = state.value.account;
  const decoded = account.decoded;
  return (
    <div className="xp-stack">
      <article className="account-card">
        <div className="card-topline">
          <p className="account-kind">
            {decoded === null ? 'Unrecognized record' : `${decoded.spec.family} · ${decoded.spec.name}`}
            {account.header === null ? null : <em className="xp-magic">{account.header}</em>}
          </p>
          {decoded === null
            ? <Chip tone="caution">no layout</Chip>
            : <Chip tone={decoded.widthCheck.ok ? 'pass' : 'caution'}>{decoded.widthCheck.ok ? 'width agrees' : 'width disagrees'}</Chip>}
        </div>
        <h3 title={account.address}>{compact(account.address, 12)}</h3>
        {decoded === null ? null : <p className="observation">{decoded.spec.summary}</p>}
        <dl className="fact-list">
          <div><dt>Owner</dt><dd><Jump view="account" value={account.owner}>{compact(account.owner, 8)}</Jump>{account.ownerLabel === null ? null : ` · ${account.ownerLabel}`}</dd></div>
          <div><dt>Executable</dt><dd>{account.executable ? 'yes' : 'no'}</dd></div>
          <div><dt>Data</dt><dd>{account.dataBytes} bytes{decoded === null ? '' : ` · schema expects ${decoded.widthCheck.expected}`}</dd></div>
          <div><dt>Lamports</dt><dd>{account.rent.lamports}{account.rent.exemptionMinimum === null ? '' : ` · rent minimum ${account.rent.exemptionMinimum}`}</dd></div>
          <div><dt>Rent</dt><dd>{account.rent.exempt === null ? 'unknown' : account.rent.exempt ? 'exempt' : 'BELOW MINIMUM'}</dd></div>
          <div><dt>Finalized at</dt><dd>slot {account.observedSlot} (floor {account.floorSlot})</dd></div>
        </dl>
        <p className="xp-quiet">{account.rent.note}</p>
        {account.note === null ? null : <Honest>{account.note}</Honest>}
        {decoded === null ? <p className="xp-raw">{account.headHex}…</p> : null}
      </article>

      {decoded === null ? null : (
        <section className="xp-panel">
          <p className="eyebrow">Fields</p>
          <RecordFields decoded={decoded} />
        </section>
      )}

      {account.derivations.length === 0 ? null : (
        <section className="xp-panel">
          <p className="eyebrow">Address derivation</p>
          <Derivations derivations={account.derivations} />
        </section>
      )}

      {account.record === null ? null : (
        <section className="xp-panel">
          <p className="eyebrow">Content-addressed record</p>
          <dl className="fact-list">
            <div><dt>Content identity</dt><dd className="xp-hex">{account.record.contentDigest}</dd></div>
            <div><dt>Schema</dt><dd>{account.record.schema ?? 'no emitted schema reproduces this address under this owner'}</dd></div>
            {account.record.stagingAddress === null ? null : (
              <div><dt>Staging cursor</dt><dd><Jump view="account" value={account.record.stagingAddress}>{compact(account.record.stagingAddress, 8)}</Jump></dd></div>
            )}
          </dl>
          {account.record.schema === null
            ? <Honest>sha256 of these exact bytes is shown because any finalized record is addressed by it. That no schema reproduces this address is not a claim the account is invalid — only that it is not one of the record schemas this browser was emitted with.</Honest>
            : <Honest>The schema was not read out of the content. It is the one whose raw-record PDA lands on this exact address, under this account&rsquo;s actual owner.</Honest>}
        </section>
      )}
    </div>
  );
}

// --------------------------------------------------------- transaction view

function InstructionCard({ instruction }: Readonly<{ instruction: ExplorerInstruction }>) {
  const decoded = instruction.decoded;
  const inner = instruction.innerIndex !== null;
  return (
    <article className={`xp-ix${inner ? ' xp-ix-inner' : ''}`}>
      <div className="xp-ix-head">
        <span className="xp-ix-index">
          {inner ? `${instruction.outerIndex}.${instruction.innerIndex}` : instruction.outerIndex}
          {instruction.stackHeight === null ? null : <em>depth {instruction.stackHeight}</em>}
        </span>
        <div>
          <strong>
            {instruction.programAddress === null
              ? `account index ${instruction.programIndex}, outside this transaction’s address list`
              : <Jump view="account" value={instruction.programAddress}>{compact(instruction.programAddress, 8)}</Jump>}
            {instruction.programLabel === null ? null : <span className="xp-prog-label">{instruction.programLabel}</span>}
          </strong>
          <small>{decoded.bytes} bytes{decoded.magic === null ? '' : ` · magic ${decoded.magic}`}</small>
        </div>
      </div>

      {decoded.routes.length === 0 ? null : (
        <ul className="xp-routes">
          {decoded.routes.map((route) => (
            <li key={route.routeId}>
              <code>{route.routeId}</code>
              {route.summary === null ? null : <p>{route.summary}</p>}
              <small>{route.handler} · {route.provenance}</small>
            </li>
          ))}
        </ul>
      )}
      {decoded.routes.length > 1 ? (
        <Honest>
          The census enumerates {decoded.routes.length} routes behind this one magic. The leading bytes alone do not
          choose between them, so all of them are shown.
        </Honest>
      ) : null}
      {decoded.note === null ? null : <Honest>{decoded.note}</Honest>}

      {decoded.body === null ? null : (
        <details className="xp-body">
          <summary>{decoded.body.spec.name} · {decoded.body.widthCheck.expected}</summary>
          <RecordFields decoded={decoded.body} />
        </details>
      )}
      {decoded.inner === null ? null : (
        <details className="xp-body">
          <summary>Wrapped family request · {decoded.inner.bytes} bytes{decoded.inner.magic === null ? '' : ` · ${decoded.inner.magic}`}</summary>
          {decoded.inner.routes.length === 0 ? <Honest>{decoded.inner.note ?? 'No route is selected by the wrapped magic.'}</Honest> : (
            <ul className="xp-routes">
              {decoded.inner.routes.map((route) => <li key={route.routeId}><code>{route.routeId}</code>{route.summary === null ? null : <p>{route.summary}</p>}</li>)}
            </ul>
          )}
          {decoded.inner.body === null ? null : <RecordFields decoded={decoded.inner.body} />}
        </details>
      )}

      {instruction.accounts.length === 0 ? null : (
        <details className="xp-body">
          <summary>{instruction.accounts.length} accounts</summary>
          <ol className="xp-accounts">
            {instruction.accounts.map((account, index) => (
              <li key={`${account.index}-${index}`}>
                <em>{account.index}</em>
                {account.address === null
                  ? <span className="xp-unknown">outside the address list</span>
                  : <Jump view="account" value={account.address}>{compact(account.address, 8)}</Jump>}
                {account.label === null ? null : <small>{account.label}</small>}
              </li>
            ))}
          </ol>
        </details>
      )}
    </article>
  );
}

function TransactionView({ state }: Readonly<{ state: Async<ExplorerTransactionResult> }>) {
  if (state.kind === 'idle') {
    return <Notice kind="quiet" title="Paste a signature." message="Each instruction is decoded against the route the census says its magic selects, the CPI frames come from the chain’s own metadata, and a refusal is named rather than numbered." />;
  }
  if (state.kind === 'loading') return <Notice kind="loading" title="Reading the transaction" message={state.message} />;
  if (state.kind === 'error') return <Notice kind="error" title="Transaction read refused" message={state.message} />;
  if (state.value.status === 'absent') {
    return (
      <article className="account-card refused">
        <div className="card-topline"><p className="account-kind">Not served</p><Chip tone="fail">absent</Chip></div>
        <h3 title={state.value.signature}>{compact(state.value.signature, 12)}</h3>
        <p className="refusal-reason">{state.value.reason}</p>
      </article>
    );
  }

  const transaction = state.value.transaction;
  const refusal = transaction.refusal;
  return (
    <div className="xp-stack">
      <article className="account-card">
        <div className="card-topline">
          <p className="account-kind">Transaction</p>
          <Chip tone={transaction.succeeded ? 'pass' : 'fail'}>{transaction.succeeded ? 'executed' : 'refused'}</Chip>
        </div>
        <h3 title={transaction.signature}>{compact(transaction.signature, 12)}</h3>
        <dl className="fact-list">
          <div><dt>Slot</dt><dd>{transaction.slot}</dd></div>
          <div><dt>Fee</dt><dd>{transaction.feeLamports} lamports</dd></div>
          <div><dt>Compute units</dt><dd>{transaction.computeUnits ?? 'not reported'}{transaction.budget.computeUnitLimit === null ? '' : ` of ${transaction.budget.computeUnitLimit.toLocaleString('en-US')} requested`}</dd></div>
          <div><dt>Heap requested</dt><dd>{transaction.budget.heapFrameBytes === null ? `none · the ${SBF_DEFAULT_HEAP_BYTES_V1.toLocaleString('en-US')}-byte default` : `${transaction.budget.heapFrameBytes.toLocaleString('en-US')} bytes`}</dd></div>
          <div><dt>Instructions</dt><dd>{transaction.instructions.filter((entry) => entry.innerIndex === null).length} outer · {transaction.instructions.filter((entry) => entry.innerIndex !== null).length} inner</dd></div>
        </dl>
        {transaction.note === null ? null : <Honest>{transaction.note}</Honest>}
      </article>

      {refusal === null ? null : (
        <article className={`account-card ${refusal.attribution.disposition === 'named' ? 'xp-refusal' : 'refused'}`}>
          <div className="card-topline">
            <p className="account-kind">Refusal · {hexCode(refusal.code)}</p>
            <Chip tone={refusal.attribution.disposition === 'named' ? 'caution' : 'fail'}>
              {refusal.attribution.disposition}
            </Chip>
          </div>
          <h3>{attributionTitle(refusal.attribution)}</h3>
          <p className="observation">{describeAttribution(refusal.attribution)}</p>
          <dl className="fact-list">
            {refusal.attribution.disposition === 'named' || refusal.attribution.disposition === 'banded' ? (
              <div><dt>Band</dt><dd>{refusal.attribution.band.label} · {refusal.attribution.band.package} · {refusal.attribution.band.tier}</dd></div>
            ) : null}
            <div><dt>Raised by</dt><dd>{refusal.program === null ? 'the logs name no frame for this code' : <Jump view="account" value={refusal.program}>{compact(refusal.program, 8)}</Jump>}</dd></div>
            <div><dt>Read from</dt><dd>{refusal.source}</dd></div>
            {refusal.attribution.disposition === 'named' ? (
              <div><dt>Declared at</dt><dd className="xp-hex">{refusal.attribution.refusal.provenance}</dd></div>
            ) : null}
          </dl>
          <Honest>
            The code is the last custom error in the log, because a frame that catches a child&rsquo;s refusal and raises its
            own has the last word. The program is the first frame to report that code, because a propagated refusal is
            re-reported by every frame it unwinds through and only the innermost one originated it.
          </Honest>
        </article>
      )}

      {transaction.abort === null || transaction.abortDiagnosis === null ? null : (
        <article className="account-card refused">
          <div className="card-topline">
            <p className="account-kind">Runtime abort · no custom code</p>
            <Chip tone="fail">
              {transaction.abort.named === null
                ? 'unnamed by the pinned runtime'
                : `${transaction.abort.named.origin === 'vm' ? 'EbpfError' : 'SyscallError'}::${transaction.abort.named.variant}`}
            </Chip>
          </div>
          <h3>{transaction.abortDiagnosis.title}</h3>
          <p className="observation">{transaction.abortDiagnosis.finding}</p>
          <dl className="fact-list">
            <div><dt>The runtime said</dt><dd className="xp-hex">{transaction.abort.sentence}</dd></div>
            <div>
              <dt>Faulted in</dt>
              <dd>{transaction.abort.program === null
                ? 'the logs name no frame for this abort'
                : <Jump view="account" value={transaction.abort.program}>{compact(transaction.abort.program, 8)}</Jump>}</dd>
            </div>
            {transaction.abort.fault === null ? null : (
              <div>
                <dt>Address</dt>
                <dd className="xp-hex">
                  0x{transaction.abort.fault.address.toString(16)}
                  {' · '}
                  {transaction.abort.fault.region === null
                    ? 'in no region the virtual machine declares'
                    : `${transaction.abort.fault.region} + ${transaction.abort.fault.offset.toLocaleString('en-US')}`}
                </dd>
              </div>
            )}
            {transaction.abort.meter === null ? null : (
              <div><dt>Last meter</dt><dd>{transaction.abort.meter.consumed.toLocaleString('en-US')} of {transaction.abort.meter.limit.toLocaleString('en-US')} units</dd></div>
            )}
            {transaction.runtimeError === null ? null : (
              <div><dt>Reported by the node as</dt><dd>{transaction.runtimeError}</dd></div>
            )}
          </dl>
          {transaction.abortDiagnosis.remedy === null ? null : (
            <p className="observation"><strong>What can be done:</strong> {transaction.abortDiagnosis.remedy}</p>
          )}
          <Honest>
            An abort is not a refusal: the program never returned a code, so there is no band and no dClutch name to give.
            What there is instead is the virtual machine&rsquo;s own sentence, matched against the `#[error]` format strings
            of the pinned runtime (solana-sbpf {SBF_RUNTIME_VERSIONS_V1.sbpf}, solana-syscalls {SBF_RUNTIME_VERSIONS_V1.syscalls}),
            and the fault address placed in the memory map those same crates declare.
            {transaction.abortDiagnosis.confidence === 'named'
              ? ' The runtime’s vocabulary names this one.'
              : transaction.abortDiagnosis.confidence === 'placed'
                ? ' No pinned format string prints this exact sentence — the reading rests on the address, which is read separately for that reason.'
                : ' Nothing in the pinned vocabulary prints this sentence, so it is shown verbatim and nothing is inferred from it.'}
          </Honest>
        </article>
      )}

      {transaction.runtimeError === null || transaction.abort !== null ? null : (
        <article className="account-card refused">
          <div className="card-topline"><p className="account-kind">Runtime refusal</p><Chip tone="fail">no custom code</Chip></div>
          <h3>{transaction.runtimeError}</h3>
          <Honest>This refusal came from the Solana runtime, not from a program&rsquo;s own refusal enum, so it has no band and no dClutch name. It is shown in the runtime&rsquo;s own words.</Honest>
        </article>
      )}

      <section className="xp-panel">
        <p className="eyebrow">Instructions · outer frames with their CPI children</p>
        <div className="xp-ix-list">
          {transaction.instructions.map((instruction, index) => (
            <InstructionCard key={`${instruction.outerIndex}-${instruction.innerIndex ?? 'outer'}-${index}`} instruction={instruction} />
          ))}
        </div>
      </section>

      {transaction.invoked.length === 0 ? null : (
        <section className="xp-panel">
          <p className="eyebrow">Programs the chain&rsquo;s logs report as invoked</p>
          <ol className="xp-accounts">
            {transaction.invoked.map((frame, index) => (
              <li key={`${frame.program}-${index}`}>
                <em>[{frame.depth}]</em>
                <Jump view="account" value={frame.program}>{compact(frame.program, 8)}</Jump>
              </li>
            ))}
          </ol>
        </section>
      )}

      {transaction.logMessages.length === 0 ? null : (
        <details className="xp-panel xp-logs">
          <summary>{transaction.logMessages.length} log messages</summary>
          <pre>{transaction.logMessages.join('\n')}</pre>
        </details>
      )}
    </div>
  );
}

// ----------------------------------------------------------- market lens view

const BANDS: ReadonlyArray<Readonly<{ id: LensNode['band']; label: string }>> = Object.freeze([
  { id: 'market', label: 'The Market' },
  { id: 'identity', label: 'Immutable identities · what the nine seeds name' },
  { id: 'collateral', label: 'Collateral' },
  { id: 'liability', label: 'Liability and its backing' },
  { id: 'capability', label: 'Capabilities' },
  { id: 'settlement', label: 'Settlement' },
]);

function LensCard({ node }: Readonly<{ node: LensNode }>) {
  return (
    <article className="xp-node">
      <div className="xp-node-head">
        <strong>{node.title}</strong>
        <Chip tone={node.provenance.kind === 'observed' ? 'pass' : node.provenance.kind === 'unavailable' ? 'fail' : 'caution'}>
          {node.provenance.kind}
        </Chip>
      </div>
      <p>{node.summary}</p>
      {node.address === null ? null : (
        <p className="xp-node-address"><Jump view="account" value={node.address}>{compact(node.address, 10)}</Jump></p>
      )}
      {node.contentId === null ? null : <p className="xp-hex" title={node.contentId}>{compact(node.contentId, 12)}</p>}
      {node.facts.length === 0 ? null : (
        <dl className="fact-list">
          {node.facts.map((held) => <div key={held.label}><dt>{held.label}</dt><dd title={held.value}>{held.value.length > 24 ? compact(held.value, 9) : held.value}</dd></div>)}
        </dl>
      )}
      {node.provenance.kind === 'observed'
        ? <small className="xp-quiet">observed at finalized slot {node.provenance.slot}</small>
        : node.provenance.kind === 'unavailable'
          ? <small className="xp-quiet">{node.provenance.reason}</small>
          : <small className="xp-quiet">{node.provenance.how}</small>}
    </article>
  );
}

function MarketView({ state }: Readonly<{ state: Async<MarketLens> }>) {
  if (state.kind === 'idle') {
    return <Notice kind="quiet" title="Paste a Market address." message="A Market is not one account. This joins the Core state, the Realm it is collateralized in, the Claims aggregate, the Hoard behind it and the capability manifest, and makes every identity openable." />;
  }
  if (state.kind === 'loading') return <Notice kind="loading" title="Joining the record graph" message={state.message} />;
  if (state.kind === 'error') return <Notice kind="error" title="Market read refused" message={state.message} />;

  const lens = state.value;
  return (
    <div className="xp-stack">
      <article className="account-card">
        <div className="card-topline">
          <p className="account-kind">Market lens</p>
          <Chip tone={lens.gaps.length === 0 ? 'pass' : 'caution'}>{lens.nodes.length} nodes · {lens.gaps.length} gaps</Chip>
        </div>
        <h3 title={lens.address}>{compact(lens.address, 12)}</h3>
        <p className="observation">Finalized floor slot {lens.floorSlot}</p>
        {lens.bindings.length === 0 ? null : (
          <ul className="checks">
            {lens.bindings.map((check) => (
              <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
                <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
                <div><strong>{check.label}</strong><small>{check.detail}</small></div>
              </li>
            ))}
          </ul>
        )}
        {lens.gaps.length === 0 ? null : (
          <ul className="xp-gaps">{lens.gaps.map((gap) => <li key={gap}>{gap}</li>)}</ul>
        )}
      </article>

      {BANDS.map((band) => {
        const nodes = lens.nodes.filter((node) => node.band === band.id);
        if (nodes.length === 0) return null;
        return (
          <section className="xp-panel" key={band.id}>
            <p className="eyebrow">{band.label}</p>
            <div className="xp-node-grid">{nodes.map((node) => <LensCard key={node.id} node={node} />)}</div>
          </section>
        );
      })}
    </div>
  );
}

// ------------------------------------------------------- the protocol, on load

/** Wall-clock age of a block time, for the activity list. Client-only. */
function ageText(blockTime: string | null): string | null {
  if (blockTime === null) return null;
  const seconds = Math.floor(Date.now() / 1000) - Number(blockTime);
  if (!Number.isFinite(seconds) || seconds < 0) return null;
  if (seconds < 90) return `${seconds}s ago`;
  if (seconds < 5_400) return `${Math.round(seconds / 60)}m ago`;
  if (seconds < 129_600) return `${Math.round(seconds / 3_600)}h ago`;
  return `${Math.round(seconds / 86_400)}d ago`;
}

function roleTitle(role: string): string {
  return role[0].toUpperCase() + role.slice(1);
}

function ProgramCard({ card }: Readonly<{ card: ProtocolProgramCardV1 }>) {
  return (
    <article className="xp-node">
      <div className="xp-node-head">
        <strong>{roleTitle(card.role)}</strong>
        <Chip tone={card.status === 'live' ? 'pass' : 'fail'}>
          {card.status === 'live' ? 'live · executable' : card.status === 'absent' ? 'ABSENT' : 'NOT EXECUTABLE'}
        </Chip>
      </div>
      <p>{card.meaning}</p>
      <p className="xp-node-address"><Jump view="account" value={card.address}>{compact(card.address, 10)}</Jump></p>
      <dl className="fact-list">
        {card.ownerLabel === null ? null : <div><dt>Loader</dt><dd>{card.ownerLabel}</dd></div>}
        {card.deploymentSlot === null ? null : <div><dt>First deployed at slot</dt><dd>{card.deploymentSlot}</dd></div>}
      </dl>
      <small className="xp-quiet"><Jump view="scan" value={card.address} title={`Scan the accounts ${roleTitle(card.role)} owns`}>scan its accounts →</Jump></small>
    </article>
  );
}

function ActivityRows({ rows }: Readonly<{ rows: ReadonlyArray<ProtocolActivityRowV1> }>) {
  return (
    <ol className="xp-accounts xp-activity">
      {rows.map((row) => {
        const age = ageText(row.blockTime);
        return (
          <li key={row.signature}>
            <Chip tone={row.succeeded ? 'pass' : 'fail'}>{row.succeeded ? 'executed' : 'refused'}</Chip>
            <Jump view="transaction" value={row.signature}>{compact(row.signature, 10)}</Jump>
            <small>
              {row.roles.map(roleTitle).join(' · ')} · slot {row.slot}{age === null ? '' : ` · ${age}`}
            </small>
          </li>
        );
      })}
    </ol>
  );
}

function ProtocolHomeView({ state, deployment }: Readonly<{ state: Async<ProtocolHomeV1>; deployment: DeploymentV1 }>) {
  if (state.kind === 'idle' || state.kind === 'loading') {
    return <Notice kind="loading" title={`Reading the ${deployment.label} deployment`} message="Probing the endpoint, then reading the seven role programs at one finalized observation and the node’s recent signature history for them…" />;
  }
  if (state.kind === 'error') {
    return <Notice kind="error" title="The deployment read refused" message={`${state.message} — the seven baked addresses are still shown in the cluster picker’s deployment; nothing about the protocol is inferred from a failed read.`} />;
  }
  const home = state.value;
  return (
    <div className="xp-stack">
      {home.clusterCheck !== 'mismatch' ? null : (
        <article className="account-card refused">
          <div className="card-topline"><p className="account-kind">Wrong chain</p><Chip tone="fail">genesis mismatch</Chip></div>
          <h3>This endpoint serves {home.clusterName}, not the cluster this deployment’s addresses live on.</h3>
          <p className="refusal-reason">
            The chain’s own genesis hash is {compact(home.facts.genesisHash, 8)}; the manifest expects a different one. The
            program cards below are reads of THIS chain and say what it holds at those addresses — which may be nothing.
          </p>
        </article>
      )}

      <section className="xp-panel">
        <p className="eyebrow">
          The seven role programs · read live at finalized slot {home.observedSlot} · {home.clusterName} · solana {home.facts.solanaCore}
        </p>
        <div className="xp-node-grid">
          {home.cards.map((card) => <ProgramCard key={card.role} card={card} />)}
        </div>
        <Honest>{deployment.provenance}</Honest>
      </section>

      <section className="xp-panel">
        <p className="eyebrow">Recent protocol transactions · decoded by name, newest first</p>
        {home.activity.length === 0 ? null : <ActivityRows rows={home.activity} />}
        <Honest>{home.activityNote}</Honest>
      </section>
    </div>
  );
}

// ----------------------------------------------------------------- the shell

export default function ChainExplorer() {
  // The active deployment is the whole chain selection: endpoint and the seven
  // program addresses come from the baked manifest (or the picker's Custom
  // slot), so this page asks the visitor for NOTHING before showing the
  // protocol.
  const deployment = useDeploymentV1();

  // The location is an external system, so it is read through the primitive
  // React provides for reading external systems. That keeps the whole view
  // URL-addressable — a Market page can link straight into it, and a reader can
  // share exactly what they are looking at — without hydrating state inside an
  // effect, which cascades renders and mismatches what the server rendered.
  const search = useSyncExternalStore(subscribeToLocation, readLocationSearch, readServerSearch);
  const fromUrl = useMemo(() => parseSearch(search), [search]);

  // A field the reader has edited overrides the URL until the next navigation.
  const [queryOverride, setQueryOverride] = useState<Query | null>(null);
  const [inputOverride, setInputOverride] = useState<string | null>(null);
  const query = queryOverride ?? fromUrl;
  const input = inputOverride ?? fromUrl.q;

  const [home, setHome] = useState<Async<ProtocolHomeV1>>(IDLE);
  const [account, setAccount] = useState<Async<ExplorerAccountResult>>(IDLE);
  const [transaction, setTransaction] = useState<Async<ExplorerTransactionResult>>(IDLE);
  const [market, setMarket] = useState<Async<MarketLens>>(IDLE);
  const [scan, setScan] = useState<Async<Readonly<{ facts: ConnectionFacts; snapshot: ProgramSnapshot }>>>(IDLE);
  const [record, setRecord] = useState<Async<RecordObservation>>(IDLE);
  const [schemaReleaseId, setSchemaReleaseId] = useState('');
  const [contentDigest, setContentDigest] = useState('');
  const [searchProblem, setSearchProblem] = useState<string | null>(null);

  const labels = useMemo(() => deploymentProgramLabelsV1(deployment), [deployment]);

  const syncUrl = useCallback((next: Query) => {
    const params = new URLSearchParams();
    params.set('view', next.view);
    if (next.q !== '') params.set('q', next.q);
    window.history.replaceState(null, '', `?${params.toString()}`);
  }, []);

  const run = useCallback(
    async (next: Query) => {
      if (next.q === '' || next.view === 'record') return;
      let client: SolanaRpcClient;
      try {
        client = new SolanaRpcClient(deployment.endpoint);
      } catch (error) {
        const message = errorMessage(error);
        setAccount({ kind: 'error', message });
        setTransaction({ kind: 'error', message });
        setMarket({ kind: 'error', message });
        setScan({ kind: 'error', message });
        return;
      }
      if (next.view === 'account') {
        setAccount({ kind: 'loading', message: 'Acquiring the account at a finalized floor…' });
        try {
          setAccount({ kind: 'ready', value: await inspectAccount(client, { address: next.q, programLabels: labels }) });
        } catch (error) {
          setAccount({ kind: 'error', message: errorMessage(error) });
        }
        return;
      }
      if (next.view === 'transaction') {
        setTransaction({ kind: 'loading', message: 'Reading the finalized transaction, its logs and its CPI frames…' });
        try {
          setTransaction({ kind: 'ready', value: await inspectTransaction(client, { signature: next.q, programLabels: labels }) });
        } catch (error) {
          setTransaction({ kind: 'error', message: errorMessage(error) });
        }
        return;
      }
      if (next.view === 'scan') {
        setScan({ kind: 'loading', message: 'Probing RPC identity, then reading finalized program-account headers…' });
        try {
          const facts = await client.probe();
          const snapshot = await scanProgram(client, next.q);
          setScan({ kind: 'ready', value: { facts, snapshot } });
        } catch (error) {
          setScan({ kind: 'error', message: errorMessage(error) });
        }
        return;
      }
      if (next.view === 'market') {
        setMarket({ kind: 'loading', message: 'Joining Core state, Realm, Claims aggregate, Hoard and capability manifest at one floor…' });
        try {
          setMarket({
            kind: 'ready',
            value: await inspectMarketLens(client, {
              coreProgramId: deployment.programs.core,
              registryProgramId: deployment.programs.registry,
              claimsProgramId: deployment.programs.claims,
              custodyProgramId: deployment.programs.custody,
              address: next.q,
            }),
          });
        } catch (error) {
          setMarket({ kind: 'error', message: errorMessage(error) });
        }
      }
    },
    [deployment, labels],
  );

  // THE PROTOCOL, on load: no typing, no submit. Re-read when the picker
  // changes the deployment (its snapshot identity is stable otherwise).
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (cancelled) return;
      setHome({ kind: 'loading', message: 'Reading the seven role programs…' });
      void (async () => {
        try {
          const client = new SolanaRpcClient(deployment.endpoint);
          const value = await inspectProtocolHomeV1(client, deployment);
          if (!cancelled) setHome({ kind: 'ready', value });
        } catch (error) {
          if (!cancelled) setHome({ kind: 'error', message: errorMessage(error) });
        }
      })();
    });
    return () => {
      cancelled = true;
    };
  }, [deployment]);

  const goto = useCallback(
    (view: View, q: string) => {
      const next: Query = Object.freeze({ view, q });
      setQueryOverride(next);
      setInputOverride(q);
      syncUrl(next);
      void run(next);
    },
    [run, syncUrl],
  );

  // One delegated handler for every in-page jump, so each link keeps a real,
  // copyable href while still resolving without a page load.
  const onJump = useCallback(
    (event: React.MouseEvent<HTMLElement>) => {
      const target = (event.target as HTMLElement).closest('a[data-xp-view]');
      if (target === null) return;
      const view = target.getAttribute('data-xp-view');
      const value = target.getAttribute('data-xp-q');
      if (!isView(view) || value === null) return;
      event.preventDefault();
      goto(view, value);
    },
    [goto],
  );

  // A link into this page carries its own query, so opening one resolves it
  // rather than showing an empty search. The read is started on a microtask so
  // nothing is set during the effect's synchronous phase, and `startedRef`
  // keeps a re-render from re-reading the same query.
  const startedRef = useRef<string | null>(null);
  useEffect(() => {
    const key = `${query.view}\0${query.q}\0${deployment.endpoint}`;
    if (query.q === '' || startedRef.current === key) return;
    startedRef.current = key;
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void run(query);
    });
    return () => {
      cancelled = true;
    };
  }, [deployment.endpoint, query, run]);

  function submitSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const classified = classifySearchV1(input);
    if (classified.kind === 'refused') {
      setSearchProblem(classified.reason);
      return;
    }
    setSearchProblem(null);
    if (classified.kind === 'transaction') {
      goto('transaction', classified.signature);
      return;
    }
    // An address keeps an address-shaped view the reader chose; otherwise the
    // account view decodes anything by its own magic.
    goto(query.view === 'market' || query.view === 'scan' ? query.view : 'account', classified.address);
  }

  function clearSearch() {
    setSearchProblem(null);
    setInputOverride('');
    const next: Query = Object.freeze({ view: 'account', q: '' });
    setQueryOverride(next);
    syncUrl(next);
  }

  async function runRecord(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setRecord({ kind: 'loading', message: 'Acquiring the record and its staging cursor at one finalized floor…' });
    try {
      const client = new SolanaRpcClient(deployment.endpoint);
      setRecord({ kind: 'ready', value: await inspectFinalizedRecord(client, input.trim(), schemaReleaseId, contentDigest) });
    } catch (error) {
      setRecord({ kind: 'error', message: errorMessage(error) });
    }
  }

  const placeholder =
    query.view === 'scan' || query.view === 'record' ? 'Program ID'
      : query.view === 'market' ? 'Market address'
        : 'Search — an address or a transaction signature';

  const atHome = query.q === '' && query.view !== 'scan' && query.view !== 'record';

  return (
    <PageShell className="shell xp" header={<Nav current="/explorer" status="read-only projection" />} onClick={onJump}>

      <section className="xp-hero">
        <p className="eyebrow">Seven devnet programs, live · no wallet, no setup</p>
        <h1>Every record the protocol writes, decoded by its own schema.</h1>
        <p className="lede">
          Paste an address, a signature, or a program ID. The seven {deployment.label} programs are below.
        </p>
        <PublicDeploymentEvidence deployment={deployment} />
      </section>

      <form className="xp-chain" onSubmit={query.view === 'record' ? (event) => void runRecord(event) : submitSearch}>
        <div className="xp-query">
          <input
            value={input}
            onChange={(event) => setInputOverride(event.target.value)}
            placeholder={placeholder}
            spellCheck={false}
            aria-label="Search the chain"
          />
          <button type="submit">{query.view === 'record' ? 'Inspect the finalized pair' : query.view === 'scan' ? 'Scan program accounts' : 'Search'}</button>
          {atHome ? null : (
            <button type="button" className="xp-clear" onClick={clearSearch}>← the protocol</button>
          )}
        </div>
        {searchProblem === null ? null : <p className="xp-problem">{searchProblem}</p>}

        <div className="xp-tabs" role="tablist">
          {VIEWS.map((view) => (
            <button
              key={view.id}
              type="button"
              role="tab"
              aria-selected={query.view === view.id}
              className={query.view === view.id ? 'active' : ''}
              onClick={() => { const next = Object.freeze({ view: view.id, q: input.trim() }); setQueryOverride(next); syncUrl(next); }}
              title={view.hint}
            >
              {view.label}
            </button>
          ))}
        </div>

        {query.view === 'record' ? (
          <div className="xp-chain-row">
            <label><span>Schema / release ID · 32-byte lowercase hex</span><input pattern="[0-9a-f]{64}" minLength={64} maxLength={64} value={schemaReleaseId} onChange={(event) => setSchemaReleaseId(event.target.value.trim())} /></label>
            <label><span>Content digest · SHA-256 lowercase hex</span><input pattern="[0-9a-f]{64}" minLength={64} maxLength={64} value={contentDigest} onChange={(event) => setContentDigest(event.target.value.trim())} /></label>
          </div>
        ) : null}
      </form>

      <section className="xp-output" aria-live="polite">
        {atHome ? <ProtocolHomeView state={home} deployment={deployment} /> : (
          <>
            {query.view === 'account' ? <AccountView state={account} /> : null}
            {query.view === 'transaction' ? <TransactionView state={transaction} /> : null}
            {query.view === 'market' ? <MarketView state={market} /> : null}
            {query.view === 'scan' ? <ScanView state={scan} /> : null}
            {query.view === 'record' ? <RecordView state={record} /> : null}
          </>
        )}
      </section>

      <details className="xp-panel xp-census">
        <summary>Developer note · the routes no leading magic selects</summary>
        <p className="xp-quiet">
          Seven entry routes are selected by a predicate, a decoded action tag, or an exact instruction length rather
          than by a leading magic. An instruction to one of them is shown with its bytes and its program, and no route
          is guessed for it.
        </p>
        <ul className="xp-unselected">
          {unselectedEntryRoutes().map((route) => (
            <li key={route.routeId}><code>{route.routeId}</code><small>{route.selectors.join(' · ') || 'no selector the census could classify'}</small></li>
          ))}
        </ul>
      </details>

      <footer>
        <p>Untrusted static projection of the active deployment&rsquo;s infrastructure.</p>
        <p>No wallet adapter. No transaction construction, signing, or submission.</p>
      </footer>
    </PageShell>
  );
}

// -------------------------------------------------- the two preserved surfaces

function ScanView({ state }: Readonly<{ state: Async<Readonly<{ facts: ConnectionFacts; snapshot: ProgramSnapshot }>> }>) {
  if (state.kind === 'idle') return <Notice kind="quiet" title="Enter a program ID." message="At most 256 account headers and 128 recognized accounts are acquired per scan." />;
  if (state.kind === 'loading') return <Notice kind="loading" title="Acquiring chain state" message={state.message} />;
  if (state.kind === 'error') return <Notice kind="error" title="RPC observation refused" message={state.message} />;
  const { facts, snapshot } = state.value;
  return (
    <div className="xp-stack">
      <article className="account-card">
        <div className="card-topline"><p className="account-kind">Program scan</p><Chip tone="muted">{snapshot.totalAccounts} accounts</Chip></div>
        <h3>{snapshot.totalAccounts === '0' ? 'No program accounts found.' : `${snapshot.decodedAccounts} decoded · ${snapshot.refusedAccounts} refused`}</h3>
        <dl className="fact-list">
          <div><dt>Solana core</dt><dd>{facts.solanaCore}</dd></div>
          <div><dt>Genesis hash</dt><dd title={facts.genesisHash}>{compact(facts.genesisHash, 8)}</dd></div>
          <div><dt>Scan slot</dt><dd>{snapshot.scanSlot}</dd></div>
        </dl>
        <Honest>
          This sweep uses the older projection in <code>lib/decoders.ts</code>, which recognizes two record shapes. Open
          any address in the Account view for the full schema decode.
        </Honest>
      </article>
      <section className="xp-panel">
        <p className="eyebrow">Accounts observed</p>
        <ol className="xp-accounts">
          {snapshot.projections.map((projection) => (
            <li key={projection.address}>
              <em>{projection.kind}</em>
              <Jump view="account" value={projection.address}>{compact(projection.address, 8)}</Jump>
              <small>{projection.status === 'decoded' ? `${projection.lamports} lamports` : projection.reason}</small>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}

function RecordView({ state }: Readonly<{ state: Async<RecordObservation> }>) {
  if (state.kind === 'idle') return <Notice kind="quiet" title="Records are headerless content." message="Supply the authenticated schema/release ID and content digest; both PDAs are derived, both are fetched at one finalized floor, and the staging cursor is required to be absent." />;
  if (state.kind === 'loading') return <Notice kind="loading" title="Reading the exact record pair" message={state.message} />;
  if (state.kind === 'error') return <Notice kind="error" title="Record observation refused" message={state.message} />;
  const observation = state.value;
  return (
    <article className="account-card">
      <div className="card-topline">
        <p className="account-kind">Structural record evidence</p>
        <Chip tone={observation.status === 'structurally-final' ? 'pass' : 'fail'}>{observation.status}</Chip>
      </div>
      <p className="observation">Finalized floor {observation.floorSlot} · content bytes {observation.contentBytes ?? 'unavailable'}</p>
      <p className="xp-node-address"><Jump view="account" value={observation.rawAddress}>{compact(observation.rawAddress, 10)}</Jump></p>
      <ul className="checks">
        {observation.checks.map((check) => (
          <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
            <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
            <div><strong>{check.label}</strong><small>{check.detail}</small></div>
          </li>
        ))}
      </ul>
      <Honest>Structural finality is not a claim that the content has valid protocol semantics. Open the record in the Account view to decode it against its own magic.</Honest>
    </article>
  );
}
