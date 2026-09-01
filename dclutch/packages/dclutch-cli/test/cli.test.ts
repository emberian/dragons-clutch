import { decodeDirectIntentTicketV1 } from '@dclutch/sdk/directTicket';
import { encodeCompactIntentSigningMessageV2 } from '@dclutch/sdk/directInlineV3';
import { inspectDirectMakerNonceV1 } from '@dclutch/sdk/directMakerReplay';
import { Keypair, PublicKey } from '@solana/web3.js';
import nacl from 'tweetnacl';
import { describe, expect, it } from 'vitest';

import { decodeSession } from '../src/context';
import { nameRefusals } from '../src/output';
import { decodeWalkBook, FAILURE_WALK_MUTATION_REFUSAL_V1 } from '../src/commands/walk';
import {
  DIRECT_TRADE_MUTATION_REFUSAL_V1,
  offerCommand,
  parseOfferLifecycleV1,
  tradeCommand,
} from '../src/commands/trade';
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

  it('refuses submission before reading a book or key until a durable walk journal exists', async () => {
    const out: string[] = [];
    const err: string[] = [];
    const missing = '/this/failure-walk-file-must-not-be-read.json';
    const code = await run([
      '--book', missing,
      '--keypair', missing,
      '--rpc', 'http://127.0.0.1:1/',
      'walk',
    ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
    expect(code).toBe(1);
    expect(out).toEqual([]);
    expect(err).toEqual([`refused: ${FAILURE_WALK_MUTATION_REFUSAL_V1}`]);
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
    expect(out[0]).toContain('offer sell                       derive seller state + nonce and sign one portable sell ticket (--out; never submits)');
    expect(out[0]).toContain('intent sell|buy                  low-level: sign one fully explicit portable Direct intent (--out; never submits)');
    expect(out[0]).toContain('route release-set|direct         produce pinned checked release/Direct route evidence (read-only devnet; no keys)');
    expect(out[0]).toContain('walk                             preview the funded failure walk (--dry-run required; submission disabled)');
    expect(out[0]).not.toContain('cross a sell intent');
    expect(out[0]).not.toContain('cross a buy intent');
  });

  it('names both offer lifecycle choices and refuses a guessed default', () => {
    expect(parseOfferLifecycleV1('fok')).toBe(0);
    expect(parseOfferLifecycleV1('ioc')).toBe(1);
    expect(() => parseOfferLifecycleV1(undefined)).toThrow(/pass --lifecycle fok\|ioc/);
    expect(() => parseOfferLifecycleV1('1')).toThrow(/fok\|ioc/);
  });

  it('authenticates the route before it can read the explicitly named maker key', async () => {
    const out: string[] = [];
    const err: string[] = [];
    const route = '/this/offer-route-must-be-read-first.json';
    const keypair = '/this/offer-keypair-must-not-be-read-yet.json';
    const code = await run([
      '--route', route, '--keypair', keypair, '--maker', key(1), '--out', '/this/no-ticket.json',
      '--outcome', '0', '--fill', '1', '--price', '1', '--duration-slots', '1', '--lifecycle', 'ioc',
      'offer', 'sell',
    ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
    expect(code).toBe(1);
    expect(out).toEqual([]);
    expect(err.join('\n')).toContain(route);
    expect(err.join('\n')).not.toContain(keypair);
  });

  it('derives, signs, and emits the same canonical ticket consumed by the SDK', async () => {
    const signer = Keypair.fromSeed(new Uint8Array(32).fill(17));
    const maker = signer.publicKey.toBase58();
    const trading = key(21);
    const market = key(22);
    const collateral = key(23);
    const replay = await inspectDirectMakerNonceV1({
      finalizedSlot: async () => '80',
      accountInfo: async () => Object.freeze({ slot: '80', account: null }),
    }, { tradingProgram: trading, market, generation: 9n, maker });
    const events: string[] = [];
    let writtenPath = '';
    let writtenText = '';
    const context = Object.freeze({
      rpcUrl: 'http://127.0.0.1:1/',
      session: Object.freeze({ rpcUrl: null, programs: Object.freeze({}), markets: Object.freeze([]) }),
      json: false,
      deployment: null,
      flags: Object.freeze({
        maker,
        out: '/captured/portable-ticket.json',
        outcome: '1',
        fill: '400',
        price: '350000',
        'duration-slots': '25',
        lifecycle: 'ioc',
      }),
    });
    const code = await offerCommand(context, { out: () => undefined, err: () => undefined }, 'sell', {}, {
      observe: async (_context, observedMaker) => {
        events.push('observe');
        expect(observedMaker).toBe(maker);
        return Object.freeze({
          route: Object.freeze({
            market, generation: 9n, outcomeCount: 2, priceScale: 1_000_000n,
            feeBasisPoints: 25, tradingProgram: trading,
          }),
          seller: Object.freeze({
            status: 'ready' as const, observedSlot: '79', market, generation: 9n, owner: maker,
            coordinates: Object.freeze({
              aggregate: key(24), position: key(25), collateral, custodyAuthority: key(26),
            }),
            collateralMint: key(27), tokenProgram: key(28), positionRevision: 3n,
            positionBalances: Object.freeze([500n, 900n]), collateralPrestate: 'initialized' as const,
            reason: 'ready',
          }),
          replay,
        });
      },
      loadMaker: () => {
        events.push('load-key');
        expect(events).toEqual(['observe', 'load-key']);
        return signer;
      },
      writeTicket: (path, text) => {
        events.push('write');
        writtenPath = path;
        writtenText = text;
      },
    });

    expect(code).toBe(0);
    expect(events).toEqual(['observe', 'load-key', 'write']);
    expect(writtenPath).toBe('/captured/portable-ticket.json');
    const ticket = decodeDirectIntentTicketV1(writtenText);
    expect(ticket).toMatchObject({
      maker,
      intent: {
        side: 0, lifecycle: 1, outcome: 1, market, generation: 9n, nonce: 0n,
        validFrom: 80n, validThrough: 105n, maximumFill: 400n, limitPrice: 350_000n,
        feeBasisPoints: 25, collateralAccount: collateral,
      },
    });
    expect(nacl.sign.detached.verify(
      encodeCompactIntentSigningMessageV2(ticket.intent),
      ticket.signature,
      signer.publicKey.toBytes(),
    )).toBe(true);
  });
});
