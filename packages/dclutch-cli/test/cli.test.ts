import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeSession } from '../src/context';
import { nameRefusals } from '../src/output';
import { decodeWalkBook } from '../src/commands/walk';
import { DIRECT_TRADE_MUTATION_REFUSAL_V1, tradeCommand } from '../src/commands/trade';
import { run } from '../src/main';

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

describe('session decoding', () => {
  it('reads program ids and rpc url out of a successor run spec', () => {
    const session = decodeSession({
      schema: 'dclutch-local-successor-run-spec-v2',
      rpc_url: 'http://127.0.0.1:21000/',
      registry: { program_id: key(1) },
      core: { program_id: key(2) },
      claims: { program_id: key(3) },
      rent_credit: { program_id: key(4) },
    });
    expect(session.rpcUrl).toBe('http://127.0.0.1:21000/');
    expect(session.programs.core).toBe(key(2));
    expect(session.programs.rentCredit).toBe(key(4));
    expect(session.programs.trading).toBeUndefined();
  });

  it('reads founded markets out of a run evidence document, founding market first', () => {
    const session = decodeSession({
      schema: 'dclutch-local-successor-run-evidence-v2',
      accounts: { market: { address: key(9) }, founding_market: { address: key(8) } },
    });
    expect(session.markets).toEqual([key(8), key(9)]);
  });

  it('round-trips its own compact session shape', () => {
    const session = decodeSession({
      schema: 'dclutch-cli-session-v1',
      rpcUrl: 'http://127.0.0.1:20890/',
      programs: { core: key(2), claims: key(3) },
      markets: [key(7)],
    });
    expect(session.programs.claims).toBe(key(3));
    expect(session.markets).toEqual([key(7)]);
  });
});

describe('refusal naming in error text', () => {
  it('names the code inside a validator log line', () => {
    const named = nameRefusals('Transaction simulation failed: custom program error: 0x5000');
    expect(named).toContain('ClaimsSbfError::Instruction');
  });

  it('names the code inside an embedded JSON-RPC InstructionError', () => {
    const named = nameRefusals('sendTransaction refused: {"InstructionError":[0,{"Custom":32768}]}');
    expect(named).toContain('ResolutionError::AccountFrame');
  });

  it('leaves a foreign code named as foreign, never guessed', () => {
    const named = nameRefusals('custom program error: 0x1');
    expect(named).toContain('not a dClutch refusal');
  });
});

describe('walk book', () => {
  it('refuses a book missing any frame slot, naming the slot', () => {
    expect(() => decodeWalkBook({ market: key(1) })).toThrow(/resolutionProgram/);
  });

  it('accepts a complete book', () => {
    const book: Record<string, string> = {};
    for (const field of [
      'resolutionProgram', 'market', 'coreProgram', 'registryActivation', 'sourceResolutionState',
      'resolutionCertificate', 'sourceMaterial', 'sourceMaterialStagingVacancy', 'windowSpec',
      'windowSpecStagingVacancy', 'productRecord', 'productRecordStagingVacancy', 'resultDomain',
      'resultDomainStagingVacancy', 'portfolioRecord', 'portfolioRecordStagingVacancy',
      'capabilityManifest', 'capabilityManifestStagingVacancy', 'failureFunding',
    ]) book[field] = key(2);
    expect(decodeWalkBook(book).market).toBe(key(2));
  });
});

describe('public Direct mutation boundary', () => {
  it.each(['buy', 'sell'] as const)('%s has no context, key, signing, or RPC capability', async (action) => {
    await expect(tradeCommand(action)).rejects.toThrow(`${action} refused: ${DIRECT_TRADE_MUTATION_REFUSAL_V1}`);
  });

  it.each(['buy', 'sell'] as const)('%s refuses before caller-named session, route, or key files are read', async (action) => {
    const out: string[] = [];
    const err: string[] = [];
    const missing = `/this/${action}-caller-file-must-not-be-read.json`;
    const code = await run([
      '--session', missing,
      '--route', missing,
      '--keypair', missing,
      '--counter-keypair', missing,
      '--rpc', 'http://127.0.0.1:1/',
      action,
    ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
    expect(code).toBe(1);
    expect(out).toEqual([]);
    expect(err).toEqual([`refused: ${action} refused: ${DIRECT_TRADE_MUTATION_REFUSAL_V1}`]);
  });

  it('help states the closed mutation boundary and never advertises submission', async () => {
    const out: string[] = [];
    const code = await run(['--help'], {}, { out: (line) => out.push(line), err: () => undefined });
    expect(code).toBe(0);
    expect(out).toHaveLength(1);
    expect(out[0]).toContain('buy                              disabled: refuses before context, keys, signing, or RPC access');
    expect(out[0]).toContain('intent sell|buy                  authenticate a route and sign one off-chain Direct intent (--out; never submits)');
    expect(out[0]).not.toContain('cross a sell intent');
    expect(out[0]).not.toContain('cross a buy intent');
  });
});
