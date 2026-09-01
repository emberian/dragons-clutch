'use client';

import PageShell from '@/components/PageShell';
import ConsoleHeader from '@/components/ConsoleHeader';
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

/**
 * The checkpoint names each transaction with the runner's own identifier
 * (`registry_activate_release_set`). Those are keys, not headlines: a reader
 * gets the action in words, and an identifier the checkpoint adds later is
 * humanized rather than printed raw.
 */
const TRANSACTION_TITLES: Readonly<Record<string, string>> = Object.freeze({
  registry_activate_release_set: 'Activate the release set',
  registry_reauthenticate_resolution: 'Reauthenticate Resolution',
  resolution_accept_real_pyth_primary: 'Accept the Pyth primary source',
  resolution_lifecycle_recovery: 'Lifecycle: recovery',
  resolution_lifecycle_exhaustion: 'Lifecycle: exhaustion',
  resolution_lifecycle_failure: 'Lifecycle: failure',
  resolution_rollback_recovery: 'Rollback: recovery',
  resolution_rollback_exhaustion: 'Rollback: exhaustion',
  resolution_rollback_failure: 'Rollback: failure',
});

function message(error: unknown): string { return error instanceof Error ? error.message : 'local successor discovery failed without a usable refusal reason'; }
function compact(value: string): string { return value.length > 24 ? `${value.slice(0, 9)}…${value.slice(-7)}` : value; }
function accountTitle(account: SuccessorAccountObservation): string { return account.name.split('.').map((part) => part[0].toUpperCase() + part.slice(1)).join(' · '); }
export function transactionTitle(label: string): string {
  const known = TRANSACTION_TITLES[label];
  if (known !== undefined) return known;
  const words = label.replaceAll('_', ' ').trim();
  return words.length === 0 ? label : words[0].toUpperCase() + words.slice(1);
}

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
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'loading', message: 'Reading finalized state…' });
  const load = useCallback(() => discoverLocalSuccessor(new SolanaRpcClient(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url)), []);
  const refresh = useCallback(async () => {
    setDiscovery({ kind: 'loading', message: 'Reading finalized state…' });
    try { const snapshot = await load(); setDiscovery({ kind: 'ready', snapshot }); }
    catch (error) { setDiscovery({ kind: 'error', message: `Refused: ${message(error)}` }); }
  }, [load]);
  useEffect(() => { let current = true; void load().then((snapshot) => { if (current) setDiscovery({ kind: 'ready', snapshot }); }, (error: unknown) => { if (current) setDiscovery({ kind: 'error', message: `Refused: ${message(error)}` }); }); return () => { current = false; }; }, [load]);
  const snapshot = discovery.kind === 'ready' ? discovery.snapshot : null;
  const scenarios = useMemo(() => snapshot === null ? [] : (['primary', 'lifecycle', 'rollback'] as const).map((name) => ({ name, accounts: snapshot.accounts.filter((account) => account.name.startsWith(`${name}.`)) })), [snapshot]);
  const prepared = snapshot?.accounts.filter((account) => account.expected.origin.startsWith('genesis-prepared')).length ?? 0;
  const pruned = snapshot?.transactions.filter((transaction) => transaction.rpcStatus === 'pruned').length ?? 0;
  const statusMessage = discovery.kind === 'ready' ? '' : discovery.message;

  return <PageShell className="product-shell direct-workspace local-successor-shell" header={<ConsoleHeader path="/local" title="Local successor" purpose="Compare the checkpointed validator's finalized state against the published evidence." />}>
    <section className="market-heading local-heading"><div><h1>The local chain.</h1><p>Every account on the fixed localhost validator, compared byte for byte against the hash-pinned checkpoint.</p></div><aside className="local-endpoint"><span>Fixed profile</span><strong>{LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url}</strong><code>{LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash}</code><button type="button" onClick={() => void refresh()} disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Refresh exact state'}</button></aside></section>
    <section className="local-status-strip" aria-live="polite"><i className={snapshot ? 'online' : discovery.kind === 'error' ? 'offline' : ''} /><strong>{snapshot ? 'Finalized state read' : statusMessage}</strong>{snapshot && <span>slot {snapshot.observedSlot} · Agave {snapshot.facts.solanaCore} · feature {snapshot.facts.featureSet}</span>}</section>
    {snapshot && <>
      <section className="local-metrics" aria-label="Evidence summary"><article><span>Current exact state</span><strong>{snapshot.exactAccounts}/{snapshot.accounts.length}</strong><p>account bodies match the hash-pinned checkpoint</p></article><article><span>Transaction outputs</span><strong>{snapshot.transactionCreatedAccounts}</strong><p>activation and certificates created by a transaction</p></article><article><span>Prepared lineage</span><strong>{prepared}</strong><p>records, Markets, states, funding, and hostile input</p></article><article><span>RPC transaction retention</span><strong>{snapshot.queryableTransactions}/{snapshot.transactions.length}</strong><p>{pruned} transaction bodies have aged out of this ledger</p></article></section>
      <section className="direct-card local-evidence"><div className="direct-card-heading"><span>01</span><div><h2>Evidence levels</h2><p>LIVE is re-read from the validator now. RUNNER CAPTURE was recorded when the transactions ran and cannot be re-read.</p></div></div><div className="local-evidence-grid"><article><b>LIVE</b><h3>Finalized account state</h3><p>{snapshot.exactAccounts} exact bodies, owners, widths, balances, and hashes reacquired now. {snapshot.unexpectedProgramAccounts.length + snapshot.missingProgramAccounts.length === 0 ? 'No unexpected or missing Registry/Resolution-owned accounts.' : 'The program-owned account inventory differs.'}</p></article><article><b>LIVE + PINNED</b><h3>Immutable Loader boundary</h3><p>Registry and Resolution remain slot-zero, authority-none Loader V3 programs whose complete bodies match the checkpoint.</p></article><article><b>RUNNER CAPTURE</b><h3>Transaction execution</h3><p>{snapshot.transactions.length} protocol transactions carry signatures, slots, fees, CU, logs, and outcomes. {pruned === 0 ? 'Every body remains queryable now.' : `${pruned} bodies are pruned and no longer returned by this validator.`}</p></article><article><b>{snapshot.rollbackCurrent ? 'CAPTURE + LIVE OUTPUT' : 'REFUSED'}</b><h3>Atomic rollback</h3><p>{snapshot.rollbackCurrent ? 'Before/after hashes were equal for the failed final action, and the hostile certificate still matches that unchanged output checkpoint.' : 'Current rollback state no longer joins the captured before/after proof.'}</p></article></div><div className="local-boundary"><strong>Not claimed</strong><span>No checked production release. No captured Pyth deployment identity. No on-chain creation for genesis-prepared semantic records, Markets, Source states, or funding.</span></div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>02</span><div><h2>Three Source lifecycles</h2></div></div><div className="local-scenarios">{scenarios.map((scenario) => <article key={scenario.name}><header><span>{scenario.name}</span><strong>{scenario.accounts.every((account) => account.matches) ? 'exact checkpoint' : 'refused'}</strong></header><div>{scenario.accounts.filter((account) => account.name.includes('.state') || account.name.includes('.certificate.')).map((account) => <AccountCard key={account.name} account={account} />)}</div></article>)}</div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>03</span><div><h2>Protocol transactions</h2><p>“Pruned” means the runner recorded the transaction but this validator no longer returns its body or signature index.</p></div></div><div className="local-transactions">{snapshot.transactions.map((transaction, index) => <article key={transaction.signature}><span>{String(index + 1).padStart(2, '0')}</span><div><h3>{transactionTitle(transaction.label)}</h3><code>{transaction.signature}</code><p>{transaction.detail}</p></div><dl><div><dt>slot</dt><dd>{transaction.slot}</dd></div><div><dt>compute</dt><dd>{transaction.computeUnits} CU</dd></div><div><dt>result</dt><dd>{transaction.outcome}</dd></div><div><dt>RPC now</dt><dd className={transaction.rpcStatus}>{transaction.rpcStatus}</dd></div></dl></article>)}</div></section>
      <section className="direct-card"><div className="direct-card-heading"><span>04</span><div><h2>Account inventory</h2></div></div><details className="local-inventory"><summary>Inspect all {snapshot.accounts.length} checkpoint accounts</summary><div className="local-account-grid">{snapshot.accounts.map((account) => <AccountCard key={account.name} account={account} />)}</div></details></section>
      <section className="direct-card"><div className="direct-card-heading"><span>05</span><div><h2>Genesis boundary</h2></div></div><ol className="local-genesis-list">{LOCAL_SUCCESSOR_CHECKPOINT.evidence.genesis_fixture_boundary.map((boundary) => <li key={boundary}>{boundary}</li>)}</ol><div className="local-provenance"><span>tool {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.tool_commit)}</span><span>source {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.exact_source_commit)}</span><span>plan {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.plan_sha256)}</span><span>evidence {compact(LOCAL_SUCCESSOR_CHECKPOINT.provenance.evidence_sha256)}</span></div></section>
    </>}
    <footer className="product-footer"><span>Read-only localhost profile · no wallet · no signing · no submission</span></footer>
  </PageShell>;
}
