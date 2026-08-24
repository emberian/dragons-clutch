'use client';

import { FormEvent, useMemo, useState } from 'react';

import type { AccountProjection, BindingCheck, DecodedProjection } from '@/lib/decoders';
import { inspectFinalizedRecord, type RecordObservation } from '@/lib/records';
import { scanProgram, SolanaRpcClient, type ConnectionFacts, type ProgramSnapshot } from '@/lib/rpc';

type ConnectionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'loading'; message: string }>
  | Readonly<{ kind: 'error'; message: string }>
  | Readonly<{ kind: 'ready'; facts: ConnectionFacts; snapshot: ProgramSnapshot }>;

type RecordState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'loading' }>
  | Readonly<{ kind: 'error'; message: string }>
  | Readonly<{ kind: 'ready'; observation: RecordObservation }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'The request failed without a usable error message.';
}

function compact(value: string, edge = 7): string {
  return value.length <= edge * 2 + 1 ? value : `${value.slice(0, edge)}…${value.slice(-edge)}`;
}

function CheckList({ checks }: Readonly<{ checks: ReadonlyArray<BindingCheck> }>) {
  if (checks.length === 0) return <p className="quiet">No local binding is defined for this projection.</p>;
  return (
    <ul className="checks">
      {checks.map((check) => (
        <li key={check.label} className={check.ok ? 'check-pass' : 'check-fail'}>
          <span aria-hidden="true">{check.ok ? '✓' : '×'}</span>
          <div><strong>{check.label}</strong><small>{check.detail}</small></div>
        </li>
      ))}
    </ul>
  );
}

function DecodedCard({ projection }: Readonly<{ projection: DecodedProjection }>) {
  const checksPass = projection.bindings.length > 0 && projection.bindings.every((check) => check.ok);
  return (
    <article className="account-card">
      <div className="card-topline">
        <p className="account-kind">{projection.kind} · schema {projection.schema}</p>
        <span className={`status-chip ${checksPass ? 'pass' : 'caution'}`}>
          {checksPass ? 'local checks pass' : 'untrusted projection'}
        </span>
      </div>
      <h3 title={projection.address}>{compact(projection.address, 10)}</h3>
      <p className="observation">Finalized observation slot {projection.observedSlot} · {projection.lamports} lamports</p>
      <dl className="fact-list">
        {projection.details.map((fact) => (
          <div key={fact.label}><dt>{fact.label}</dt><dd title={fact.value}>{fact.value}</dd></div>
        ))}
      </dl>
      <CheckList checks={projection.bindings} />
    </article>
  );
}

function RefusedCard({ projection }: Readonly<{ projection: Extract<AccountProjection, { status: 'refused' }> }>) {
  return (
    <article className="account-card refused">
      <div className="card-topline">
        <p className="account-kind">{projection.kind}</p>
        <span className="status-chip fail">refused</span>
      </div>
      <h3 title={projection.address}>{compact(projection.address, 10)}</h3>
      <p className="observation">Finalized observation slot {projection.observedSlot} · header {projection.header || 'empty'}</p>
      <p className="refusal-reason">{projection.reason}</p>
    </article>
  );
}

function AccountCard({ projection }: Readonly<{ projection: AccountProjection }>) {
  return projection.status === 'decoded'
    ? <DecodedCard projection={projection} />
    : <RefusedCard projection={projection} />;
}

export default function ChainExplorer() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [programId, setProgramId] = useState('');
  const [connection, setConnection] = useState<ConnectionState>({ kind: 'idle' });
  const [schemaReleaseId, setSchemaReleaseId] = useState('');
  const [contentDigest, setContentDigest] = useState('');
  const [record, setRecord] = useState<RecordState>({ kind: 'idle' });

  const rpc = useMemo(() => {
    try { return new SolanaRpcClient(endpoint); } catch { return null; }
  }, [endpoint]);

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setConnection({ kind: 'loading', message: 'Probing RPC identity…' });
    setRecord({ kind: 'idle' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const facts = await client.probe();
      setConnection({ kind: 'loading', message: 'Reading finalized program-account headers and reacquiring recognized accounts…' });
      const snapshot = await scanProgram(client, programId);
      setConnection({ kind: 'ready', facts, snapshot });
    } catch (error) {
      setConnection({ kind: 'error', message: errorMessage(error) });
    }
  }

  async function inspectRecord(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (rpc === null) {
      setRecord({ kind: 'error', message: 'Enter an http or https RPC endpoint first.' });
      return;
    }
    setRecord({ kind: 'loading' });
    try {
      setRecord({ kind: 'ready', observation: await inspectFinalizedRecord(rpc, programId, schemaReleaseId, contentDigest) });
    } catch (error) {
      setRecord({ kind: 'error', message: errorMessage(error) });
    }
  }

  const ready = connection.kind === 'ready' ? connection : null;

  return (
    <main className="shell">
      <header className="masthead">
        <a className="brand" href="#top" aria-label="dClutch chain explorer home">
          <span className="brand-mark" aria-hidden="true">dC</span><span>dClutch</span>
        </a>
        <div className="header-boundaries">
          <p className="read-only-pill"><span aria-hidden="true" /> Read-only projection</p>
          <p className="not-official">Not an official deployment</p>
        </div>
      </header>

      <section className="hero" id="top">
        <p className="eyebrow">Real infrastructure · no wallet required</p>
        <h1>Inspect what the program <em>actually</em> owns.</h1>
        <p className="lede">Point this client at a local validator or devnet endpoint. It reads finalized program accounts, decodes only recognized canonical layouts, and visibly refuses everything else.</p>
      </section>

      <section className="workspace" aria-labelledby="connection-title">
        <form className="connection-card" onSubmit={connect}>
          <div className="section-heading">
            <p className="step">01</p>
            <div><h2 id="connection-title">Choose the chain</h2><p>Nothing is contacted until you ask to connect.</p></div>
          </div>
          <label><span>JSON-RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value)} spellCheck={false} /></label>
          <label><span>dClutch program ID</span><input required value={programId} onChange={(event) => setProgramId(event.target.value.trim())} placeholder="Enter a deployed program public key" spellCheck={false} /></label>
          <button type="submit" disabled={connection.kind === 'loading'}>{connection.kind === 'loading' ? 'Reading finalized state…' : 'Connect & scan'}</button>
          <p className="boundary">No wallet adapter. No transaction construction, signing, or submission.</p>
        </form>

        <div className="state-panel" aria-live="polite">
          {connection.kind === 'idle' && <EmptyState />}
          {connection.kind === 'loading' && <Notice kind="loading" title="Acquiring chain state" message={connection.message} />}
          {connection.kind === 'error' && <Notice kind="error" title="RPC observation refused" message={connection.message} />}
          {ready && (
            <div className="connection-summary">
              <p className="eyebrow">Finalized RPC observation</p>
              <h2>{ready.snapshot.totalAccounts === '0' ? 'No program accounts found.' : `${ready.snapshot.totalAccounts} program accounts observed.`}</h2>
              <p>These are browser projections, not protocol authority. A passing local check only proves the named byte/PDA relationship.</p>
              <dl className="summary-grid">
                <div><dt>Solana core</dt><dd>{ready.facts.solanaCore}</dd></div>
                <div><dt>Genesis hash</dt><dd title={ready.facts.genesisHash}>{compact(ready.facts.genesisHash, 8)}</dd></div>
                <div><dt>Finalized scan slot</dt><dd>{ready.snapshot.scanSlot}</dd></div>
                <div><dt>Decoded / refused</dt><dd>{ready.snapshot.decodedAccounts} / {ready.snapshot.refusedAccounts}</dd></div>
              </dl>
            </div>
          )}
        </div>
      </section>

      {ready && ready.snapshot.projections.length > 0 && (
        <section className="accounts-section" aria-labelledby="accounts-title">
          <div className="section-title-row"><p className="step">02</p><div><h2 id="accounts-title">Bounded account projection</h2><p>At most 256 headers and 128 recognized accounts are acquired per scan.</p></div></div>
          <div className="account-grid">{ready.snapshot.projections.map((projection) => <AccountCard key={projection.address} projection={projection} />)}</div>
        </section>
      )}

      <section className="record-section" aria-labelledby="record-title">
        <div className="record-intro">
          <p className="step">03</p>
          <h2 id="record-title">Inspect one finalized record</h2>
          <p>Records are headerless content. Supply its authenticated schema/release ID and content digest; this client derives both PDAs, fetches both at one finalized floor, and requires the staging cursor to be absent.</p>
        </div>
        <form className="record-form" onSubmit={inspectRecord}>
          <label><span>Schema / release ID · 32-byte lowercase hex</span><input required pattern="[0-9a-f]{64}" minLength={64} maxLength={64} value={schemaReleaseId} onChange={(event) => setSchemaReleaseId(event.target.value.trim())} /></label>
          <label><span>Content digest · SHA-256 lowercase hex</span><input required pattern="[0-9a-f]{64}" minLength={64} maxLength={64} value={contentDigest} onChange={(event) => setContentDigest(event.target.value.trim())} /></label>
          <button type="submit" disabled={record.kind === 'loading' || programId.length === 0}>{record.kind === 'loading' ? 'Reading record pair…' : 'Inspect finalized pair'}</button>
        </form>
        <div className="record-output" aria-live="polite">
          {record.kind === 'idle' && <p className="quiet">No record has been queried. Program-wide scans never guess headerless record schemas.</p>}
          {record.kind === 'error' && <Notice kind="error" title="Record observation refused" message={record.message} />}
          {record.kind === 'loading' && <Notice kind="loading" title="Reading exact record pair" message="Acquiring the raw record and staging-cursor accounts at a shared finalized floor…" />}
          {record.kind === 'ready' && (
            <div className="record-result">
              <div className="card-topline"><p className="account-kind">Structural record evidence</p><span className={`status-chip ${record.observation.status === 'structurally-final' ? 'pass' : 'fail'}`}>{record.observation.status}</span></div>
              <p className="observation">Finalized floor {record.observation.floorSlot} · content bytes {record.observation.contentBytes ?? 'unavailable'}</p>
              <CheckList checks={record.observation.checks} />
              <p className="refusal-note">A schema-specific validator is not present in this browser. Structural finality is not a claim that the content has valid protocol semantics.</p>
            </div>
          )}
        </div>
      </section>

      <footer><p>Untrusted static projection of user-selected infrastructure.</p><p>Transaction workflows are deliberately unavailable.</p></footer>
    </main>
  );
}

function EmptyState() {
  return <div className="empty-panel"><div className="radar" aria-hidden="true"><span /></div><p className="eyebrow">Waiting for a real endpoint</p><h2>Chain state will appear here.</h2><p>Market, Realm, Position, RentCredit, and finalized-record observations remain visibly untrusted until every named local check passes.</p></div>;
}

function Notice({ kind, title, message }: Readonly<{ kind: 'loading' | 'error'; title: string; message: string }>) {
  return <div className={`notice ${kind}`}><span className="notice-mark" aria-hidden="true">{kind === 'loading' ? '…' : '!'}</span><div><p className="eyebrow">{kind === 'loading' ? 'Bounded request in progress' : 'No state accepted'}</p><h2>{title}</h2><p>{message}</p></div></div>;
}
