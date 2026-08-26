'use client';

import Link from 'next/link';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { LOCAL_SUCCESSOR_CHECKPOINT, discoverLocalSuccessor, type LocalSuccessorSnapshot, type SuccessorAccountObservation, type SuccessorOrigin } from '@/lib/localSuccessor';
import { SolanaRpcClient } from '@/lib/rpc';

type Discovery = Readonly<{ kind: 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: LocalSuccessorSnapshot }>;

const ORIGINS: Readonly<Record<SuccessorOrigin, Readonly<{ label: string; explanation: string }>>> = Object.freeze({
  'genesis-prepared': Object.freeze({ label: 'prepared input', explanation: 'Present at genesis; no creation transaction is claimed.' }),
  'genesis-prepared-then-transaction-mutated': Object.freeze({ label: 'prepared → mutated', explanation: 'Created at genesis, then changed by real SBF transitions.' }),
  'transaction-created': Object.freeze({ label: 'transaction output', explanation: 'Allocated, assigned, and written by a recorded protocol transaction.' }),
  'genesis-prepared-refusal-sentinel': Object.freeze({ label: 'hostile prepared input', explanation: 'Deliberately occupied at genesis to force the late transaction to roll back.' }),
});

function message(error: unknown): string { return error instanceof Error ? error.message : 'local successor discovery failed without a usable refusal reason'; }
function compact(value: string): string { return value.length > 24 ? `${value.slice(0, 9)}…${value.slice(-7)}` : value; }
function accountTitle(account: SuccessorAccountObservation): string { return account.name.split('.').map((part) => part[0].toUpperCase() + part.slice(1)).join(' · '); }

function AccountCard({ account }: Readonly<{ account: SuccessorAccountObservation }>) {
  const origin = ORIGINS[account.expected.origin];
  return <article className={`local-account-card ${account.matches ? '' : 'refused'}`}>
    <div className="local-account-top"><span className={`status-chip ${account.matches ? 'pass' : 'fail'}`}>{account.matches ? 'exact' : 'refused'}</span><span className="local-origin">{origin.label}</span></div>
    <h3>{accountTitle(account)}</h3><p className="local-kind">{account.parsed.kind} · {account.parsed.headline}</p><code>{account.expected.address}</code>
    <dl>{account.parsed.facts.slice(0, 5).map((entry) => <div key={entry.label}><dt>{entry.label}</dt><dd>{entry.value}</dd></div>)}</dl>
    <p className="local-lineage">{origin.explanation}</p>{account.refusal && <p className="direct-refusal">{account.refusal}</p>}
  </article>;
}

export default function LocalSuccessorWorkspace() {
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'loading', message: 'Reading the immutable localhost successor checkpoint from finalized RPC…' });
  const load = useCallback(() => discoverLocalSuccessor(new SolanaRpcClient(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url)), []);
  const refresh = useCallback(async () => {
    setDiscovery({ kind: 'loading', message: 'Reacquiring 33 exact accounts, both program-owned account sets, Loader facts, and transaction retention…' });
    try { const snapshot = await load(); setDiscovery({ kind: 'ready', snapshot }); }
    catch (error) { setDiscovery({ kind: 'error', message: `Refused: ${message(error)}` }); }
  }, [load]);
  useEffect(() => { let current = true; void load().then((snapshot) => { if (current) setDiscovery({ kind: 'ready', snapshot }); }, (error: unknown) => { if (current) setDiscovery({ kind: 'error', message: `Refused: ${message(error)}` }); }); return () => { current = false; }; }, [load]);
  const snapshot = discovery.kind === 'ready' ? discovery.snapshot : null;
  const scenarios = useMemo(() => snapshot === null ? [] : (['primary', 'lifecycle', 'rollback'] as const).map((name) => ({ name, accounts: snapshot.accounts.filter((account) => account.name.startsWith(`${name}.`)) })), [snapshot]);
  const prepared = snapshot?.accounts.filter((account) => account.expected.origin.startsWith('genesis-prepared')).length ?? 0;
  const pruned = snapshot?.transactions.filter((transaction) => transaction.rpcStatus === 'pruned').length ?? 0;
  const statusMessage = discovery.kind === 'ready' ? '' : discovery.message;

  return <main className="product-shell direct-workspace local-successor-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/direct">Direct</Link><Link href="/economic">Economic</Link><Link href="/product-v2">Product V2</Link><Link href="/release">Release</Link><Link className="active" href="/local">Local</Link></nav><span className="preview-control"><i className="preview-dot" />localhost RPC</span></header>
    <section className="market-heading local-heading"><div><div className="market-kicker"><span>real SBF</span><span>real Pyth packet</span><span>finalized RPC</span></div><h1>The local chain, with its evidence boundaries left intact.</h1><p>This profile reads the live immutable Registry and Resolution programs, their present accounts, and the checkpointed transaction lineage. Genesis-prepared records remain labeled as prepared inputs. Only the activation cache and valid Resolution certificates are described as transaction-created.</p></div><aside className="local-endpoint"><span>Fixed profile</span><strong>{LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url}</strong><code>{LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash}</code><button type="button" onClick={() => void refresh()} disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Refresh exact state'}</button></aside></section>
    <section className="local-status-strip" aria-live="polite"><i className={snapshot ? 'online' : discovery.kind === 'error' ? 'offline' : ''} /><strong>{snapshot ? 'Immutable successor validator reacquired' : statusMessage}</strong>{snapshot && <span>slot {snapshot.observedSlot} · Agave {snapshot.facts.solanaCore} · feature {snapshot.facts.featureSet}</span>}</section>
    {snapshot && <>
      <section className="local-metrics" aria-label="Evidence summary"><article><span>Current exact state</span><strong>{snapshot.exactAccounts}/{snapshot.accounts.length}</strong><p>account bodies match the hash-pinned checkpoint</p></article><article><span>Transaction outputs</span><strong>{snapshot.transactionCreatedAccounts}</strong><p>activation and certificates physically created by SBF</p></article><article><span>Prepared lineage</span><strong>{prepared}</strong><p>records, Markets, states, funding, and hostile input</p></article><article><span>RPC transaction retention</span><strong>{snapshot.queryableTransactions}/{snapshot.transactions.length}</strong><p>{pruned} transaction bodies have aged out of this ledger</p></article></section>
      <section className="direct-card local-evidence"><div className="direct-card-heading"><span>01</span><div><h2>What this run proves—and what it does not</h2><p>Evidence is tiered by what can still be re-read from the validator versus what the hash-pinned runner captured at execution time.</p></div></div><div className="local-evidence-grid"><article><b>LIVE</b><h3>Finalized account state</h3><p>{snapshot.exactAccounts} exact bodies, owners, widths, balances, and hashes reacquired now. {snapshot.unexpectedProgramAccounts.length + snapshot.missingProgramAccounts.length === 0 ? 'No unexpected or missing Registry/Resolution-owned accounts.' : 'The program-owned account inventory differs.'}</p></article><article><b>LIVE + PINNED</b><h3>Immutable Loader boundary</h3><p>Registry and Resolution remain slot-zero, authority-none Loader V3 programs whose complete bodies match the checkpoint.</p></article><article><b>RUNNER CAPTURE</b><h3>Transaction execution</h3><p>{snapshot.transactions.length} protocol transactions carry signatures, slots, fees, CU, logs, and outcomes. {pruned === 0 ? 'Every body remains queryable now.' : `${pruned} bodies are now pruned, so the UI does not pretend RPC independently retains those execution records.`}</p></article><article><b>{snapshot.rollbackCurrent ? 'CAPTURE + LIVE OUTPUT' : 'REFUSED'}</b><h3>Atomic rollback</h3><p>{snapshot.rollbackCurrent ? 'Before/after hashes were equal for the failed final action, and the hostile certificate still matches that unchanged output checkpoint.' : 'Current rollback state no longer joins the captured before/after proof.'}</p></article></div><div className="local-boundary"><strong>Not claimed</strong><span>No checked production release. No captured Pyth deployment identity. No on-chain creation for genesis-prepared semantic records, Markets, Source states, or funding.</span></div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>02</span><div><h2>Three actual Source lifecycles</h2><p>Cards are decoded from present program-owned account bytes, not reconstructed from a frontend market model.</p></div></div><div className="local-scenarios">{scenarios.map((scenario) => <article key={scenario.name}><header><span>{scenario.name}</span><strong>{scenario.accounts.every((account) => account.matches) ? 'exact checkpoint' : 'refused'}</strong></header><div>{scenario.accounts.filter((account) => account.name.includes('.state') || account.name.includes('.certificate.')).map((account) => <AccountCard key={account.name} account={account} />)}</div></article>)}</div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>03</span><div><h2>Protocol transaction lineage</h2><p>“Pruned” means the immutable runner evidence records the transaction, but this validator no longer returns its body or signature index. That is a weaker current evidence level, and it is shown as such.</p></div></div><div className="local-transactions">{snapshot.transactions.map((transaction, index) => <article key={transaction.signature}><span>{String(index + 1).padStart(2, '0')}</span><div><h3>{transaction.label.replaceAll('_', ' ')}</h3><code>{transaction.signature}</code><p>{transaction.detail}</p></div><dl><div><dt>slot</dt><dd>{transaction.slot}</dd></div><div><dt>compute</dt><dd>{transaction.computeUnits} CU</dd></div><div><dt>result</dt><dd>{transaction.outcome}</dd></div><div><dt>RPC now</dt><dd className={transaction.rpcStatus}>{transaction.rpcStatus}</dd></div></dl></article>)}</div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>04</span><div><h2>Exact account inventory</h2><p>Every account is tied to one explicit origin class. Expanding the list does not silently promote prepared inputs into transaction outputs.</p></div></div><details className="local-inventory"><summary>Inspect all {snapshot.accounts.length} checkpoint accounts</summary><div className="local-account-grid">{snapshot.accounts.map((account) => <AccountCard key={account.name} account={account} />)}</div></details></section>
      <section className="direct-card"><div className="direct-card-heading"><span>05</span><div><h2>Genesis boundary, verbatim from the run plan</h2><p>The status page treats these as constraints, not footnotes.</p></div></div><ol className="local-genesis-list">{LOCAL_SUCCESSOR_CHECKPOINT.evidence.genesis_fixture_boundary.map((boundary) => <li key={boundary}>{boundary}</li>)}</ol><div className="local-provenance"><span>tool {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.tool_commit)}</span><span>source {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.exact_source_commit)}</span><span>plan {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.plan_sha256)}</span><span>evidence {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.evidence_sha256)}</span></div></section>
    </>}
    <footer className="product-footer"><span>Read-only localhost profile · no wallet · no signing · no submission</span><span>Static clients remain untrusted projections</span></footer>
  </main>;
}
