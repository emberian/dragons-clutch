import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeSession } from '../src/context';
import { nameRefusals } from '../src/output';
import { decodeWalkBook } from '../src/commands/walk';
import { intentFromJson } from '../src/commands/trade';

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

describe('trade wire formats', () => {
  it('round-trips a signed intent through its JSON file shape', () => {
    const decoded = intentFromJson({
      schema: 'dclutch-direct-intent-v1',
      maker: key(3),
      signature: 'ab'.repeat(64),
      intent: {
        side: 0, lifecycle: 0, outcome: 1, market: key(4),
        generation: '1', nonce: '2', validFrom: '10', validThrough: '160',
        maximumFill: '5', limitPrice: '400000', feeBasisPoints: 30, collateralAccount: key(5),
      },
    });
    expect(decoded.intent.side).toBe(0);
    expect(decoded.intent.maximumFill).toBe(5n);
    expect(decoded.signature.length).toBe(64);
  });

  it('refuses an intent whose numbers are not exact unsigned decimals', () => {
    expect(() => intentFromJson({
      schema: 'dclutch-direct-intent-v1',
      maker: key(3),
      signature: 'ab'.repeat(64),
      intent: { side: 0, lifecycle: 0, outcome: 1, market: key(4), generation: '-1', nonce: '2', validFrom: '10', validThrough: '160', maximumFill: '5', limitPrice: '4', feeBasisPoints: 30, collateralAccount: key(5) },
    })).toThrow(/generation/);
  });
});
