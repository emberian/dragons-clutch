import { describe, expect, it } from 'vitest';

import example from '@/fixtures/simulator-status.example.json';
import {
  NO_SIMULATOR_SENTENCE_V1,
  SIMULATOR_STATUS_URL_V1,
  STALE_AFTER_MS_V1,
  parseSimulatorStatusV1,
  readSimulatorStatusV1,
  simulatorBeatV1,
} from './simulatorStatus';

// The adversarial cases mutate a copy of the real example, one field at a
// time, so each refusal is exactly one step from a document the decoder
// accepts — a hand-built forgery could fail for an unrelated reason.
function mutated(change: (copy: Record<string, unknown>) => void): unknown {
  const copy = JSON.parse(JSON.stringify(example)) as Record<string, unknown>;
  change(copy);
  return copy;
}

describe('the simulator status decoder', () => {
  it('accepts the example artifact completely', () => {
    const status = parseSimulatorStatusV1(example);
    expect(status.clusterLabel).toBe('local');
    expect(status.mode).toBe('sustain');
    expect(status.cyclesRun).toBe(12);
    expect(status.cyclesTarget).toBeNull();
    expect(status.tradesLanded).toBe(24);
    expect(status.signatures).toHaveLength(2);
    expect(status.wallets).toHaveLength(2);
    expect(status.wallets[1]?.solLamports).toBeNull();
    expect(status.lastReconciliation?.ok).toBe(true);
    // The artifact records an absolute path inside the writer's work
    // directory. That is right for the simulator and wrong for a public page:
    // it prints an operator's local filesystem layout and tells a reader
    // nothing. The file's NAME does tell them something — which cycle's census
    // this verdict came from — so that is the part that survives the decode,
    // and the rest is dropped rather than rendered.
    expect(status.lastReconciliation?.detail).toBe('from census file cycle-000012.json');
    expect(status.lastReconciliation?.detail).not.toContain('/');
    expect(status.halted).toBe(false);
  });

  it('tolerates extra keys, because the writer appends them freely', () => {
    const status = parseSimulatorStatusV1(mutated((copy) => { copy.extra_note = 'anything'; }));
    expect(status.cyclesRun).toBe(12);
  });

  it('refuses another schema', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => { copy.schema = 'dclutch-load-simulator-status-v2'; })))
      .toThrow('another schema');
  });

  it('refuses a cluster label it does not know', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => { (copy.cluster as Record<string, unknown>).label = 'mainnet'; })))
      .toThrow('local or devnet');
  });

  it('refuses a non-canonical market address', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => { (copy.market as Record<string, unknown>).address = 'not-an-address'; })))
      .toThrow('canonical Solana address');
  });

  it('refuses a negative or fractional count', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => { (copy.cycles as Record<string, unknown>).run = -1; })))
      .toThrow('non-negative integer');
    expect(() => parseSimulatorStatusV1(mutated((copy) => { (copy.trades as Record<string, unknown>).landed = 1.5; })))
      .toThrow('non-negative integer');
  });

  it('refuses a wallet without a canonical address', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => {
      (copy.wallets as Array<Record<string, unknown>>)[0].address = '';
    }))).toThrow('wallet 0');
  });

  it('refuses an unparseable timestamp', () => {
    expect(() => parseSimulatorStatusV1(mutated((copy) => { copy.updated_at = 'yesterday-ish'; })))
      .toThrow('parseable timestamp');
  });

  it('accepts a null reconciliation and a null halt reason', () => {
    const status = parseSimulatorStatusV1(mutated((copy) => { copy.last_reconciliation = null; }));
    expect(status.lastReconciliation).toBeNull();
    expect(status.haltReason).toBeNull();
  });
});

describe('the heartbeat judgement', () => {
  const fresh = parseSimulatorStatusV1(example);
  const justAfter = Date.parse(fresh.updatedAt) + 1_000;

  it('reads a fresh write as running', () => {
    expect(simulatorBeatV1(fresh, justAfter).state).toBe('running');
  });

  it('reads an old write as stale, at the named bound', () => {
    expect(simulatorBeatV1(fresh, justAfter + STALE_AFTER_MS_V1).state).toBe('stale');
  });

  it('reads a halt as halted, whatever the timestamps say', () => {
    const halted = parseSimulatorStatusV1(mutated((copy) => { copy.halted = true; copy.halt_reason = 'conservation violated at cycle 12'; }));
    const beat = simulatorBeatV1(halted, justAfter);
    expect(beat.state).toBe('halted');
    expect(beat.sentence).toContain('conservation violated at cycle 12');
  });

  it('reads a stop request as stopping', () => {
    const stopping = parseSimulatorStatusV1(mutated((copy) => { copy.stopping = true; }));
    expect(simulatorBeatV1(stopping, justAfter).state).toBe('stopping');
  });
});

describe('the guarded reader', () => {
  it('pins the one URL the surface reads, because the link checker cannot see it', () => {
    expect(SIMULATOR_STATUS_URL_V1).toBe('/simulator-status.json');
  });

  it('reads a missing artifact as absent, not as an error', async () => {
    const read = await readSimulatorStatusV1(async () => ({ ok: false, text: async () => 'not found' }));
    expect(read.kind).toBe('absent');
  });

  it('reads a host fallback page as absent — a static host answers a missing path with HTML', async () => {
    const read = await readSimulatorStatusV1(async () => ({ ok: true, text: async () => '<!doctype html><html>…</html>' }));
    expect(read.kind).toBe('absent');
  });

  it('reads a network refusal as absent', async () => {
    const read = await readSimulatorStatusV1(async () => { throw new Error('connection refused'); });
    expect(read.kind).toBe('absent');
  });

  it('reads a real JSON document that fails the decoder as refused, with the field named', async () => {
    const read = await readSimulatorStatusV1(async () => ({ ok: true, text: async () => JSON.stringify({ schema: 'something-else' }) }));
    expect(read.kind).toBe('refused');
    if (read.kind === 'refused') expect(read.reason).toContain('another schema');
  });

  it('reads the example artifact as loaded', async () => {
    const read = await readSimulatorStatusV1(async () => ({ ok: true, text: async () => JSON.stringify(example) }));
    expect(read.kind).toBe('loaded');
    if (read.kind === 'loaded') expect(read.status.tradesLanded).toBe(24);
  });

  it('keeps the honest-empty sentence explicit about zeros', () => {
    expect(NO_SIMULATOR_SENTENCE_V1).toContain('nothing below is a zero');
  });
});
