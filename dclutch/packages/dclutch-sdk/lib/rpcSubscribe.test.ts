import { describe, expect, it } from 'vitest';

import {
  AccountWatchV1,
  interpretSocketMessageV1,
  subscribeRequestV1,
  watchSentenceV1,
  websocketEndpointV1,
  type AccountChangeV1,
  type SocketLikeV1,
  type WatchStateV1,
} from './rpcSubscribe';

/**
 * The socket is the one place in this app where the chain talks first, so the
 * things pinned here are the ones that would let it talk the page into
 * something: a notification treated as data, a fan-out of connections, or a
 * failure that reads as a fact.
 */

/** A socket a test drives by hand. */
function fakeSocket() {
  const handlers = new Map<string, Array<(event: unknown) => void>>();
  const sent: Array<string> = [];
  let closed = false;
  const socket: SocketLikeV1 = {
    send: (data) => { sent.push(data); },
    close: () => { closed = true; },
    addEventListener: (type, handler) => {
      const list = handlers.get(type) ?? [];
      list.push(handler);
      handlers.set(type, list);
    },
  };
  const fire = (type: string, event?: unknown) => {
    for (const handler of handlers.get(type) ?? []) handler(event);
  };
  return { socket, sent, fire, isClosed: () => closed };
}

function watcher(addresses: ReadonlyArray<string>) {
  const states: Array<WatchStateV1> = [];
  const changes: Array<AccountChangeV1> = [];
  const opened: Array<string> = [];
  const fake = fakeSocket();
  const watch = new AccountWatchV1('https://api.devnet.solana.com', addresses, {
    onChange: (change) => { changes.push(change); },
    onState: (state) => { states.push(state); },
    socketFactory: (url) => { opened.push(url); return fake.socket; },
  });
  return { watch, states, changes, opened, fake };
}

const MARKET = '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq';
const CLAIMS = '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC';

describe('the subscription endpoint', () => {
  it('is derived from the read endpoint, never configured', () => {
    expect(websocketEndpointV1('https://api.devnet.solana.com')).toBe('wss://api.devnet.solana.com/');
    expect(websocketEndpointV1('http://127.0.0.1:8899')).toBe('ws://127.0.0.1:8899/');
  });

  it('refuses to guess at anything that is not an http endpoint', () => {
    expect(websocketEndpointV1('ftp://example.test')).toBeNull();
    expect(websocketEndpointV1('not a url')).toBeNull();
  });
});

describe('reading a frame off the socket', () => {
  it('recognises a subscription confirmation', () => {
    expect(interpretSocketMessageV1('{"jsonrpc":"2.0","result":42,"id":1}'))
      .toEqual({ kind: 'subscribed', id: 1, subscription: 42 });
  });

  it('recognises a change, and carries the slot the node saw it at', () => {
    const frame = JSON.stringify({
      jsonrpc: '2.0',
      method: 'accountNotification',
      params: { subscription: 42, result: { context: { slot: 490502336 }, value: { lamports: 1 } } },
    });
    expect(interpretSocketMessageV1(frame)).toEqual({ kind: 'changed', subscription: 42, slot: '490502336' });
  });

  it('recognises a refusal and keeps the node’s own reason', () => {
    const frame = JSON.stringify({ jsonrpc: '2.0', id: 3, error: { code: -32601, message: 'Method not found' } });
    expect(interpretSocketMessageV1(frame)).toEqual({ kind: 'refused', id: 3, reason: 'Method not found' });
  });

  /**
   * A node may send anything. Nothing off this socket is allowed to throw,
   * because a frame this app does not consume is not an error and must not
   * become one on a page that is only reading.
   */
  it('ignores every frame it does not consume, and never throws on one', () => {
    for (const frame of ['', 'not json', '[]', 'null', '{"method":"somethingElse"}', '{"id":1}', '{"jsonrpc":"2.0","method":"accountNotification","params":{}}']) {
      expect(interpretSocketMessageV1(frame)).toEqual({ kind: 'ignored' });
    }
    expect(interpretSocketMessageV1(undefined)).toEqual({ kind: 'ignored' });
    expect(interpretSocketMessageV1({ data: 'x' })).toEqual({ kind: 'ignored' });
  });
});

describe('a watch over one socket', () => {
  it('opens exactly one connection for the whole address set', () => {
    const { watch, opened, fake } = watcher([MARKET, CLAIMS]);
    watch.open();
    fake.fire('open');
    expect(opened).toEqual(['wss://api.devnet.solana.com/']);
    expect(fake.sent).toHaveLength(2);
    expect(fake.sent[0]).toBe(subscribeRequestV1(1, MARKET));
    expect(fake.sent[1]).toBe(subscribeRequestV1(2, CLAIMS));
    watch.close();
  });

  it('subscribes at the same commitment the rest of the app reads at', () => {
    expect(subscribeRequestV1(1, MARKET)).toContain('"commitment":"finalized"');
  });

  it('goes live once the node confirms, and reports each change against its own address', () => {
    const { watch, states, changes, fake } = watcher([MARKET, CLAIMS]);
    watch.open();
    fake.fire('open');
    fake.fire('message', { data: '{"jsonrpc":"2.0","result":11,"id":1}' });
    fake.fire('message', { data: '{"jsonrpc":"2.0","result":22,"id":2}' });
    expect(states).toEqual(['connecting', 'live', 'live']);

    fake.fire('message', { data: JSON.stringify({ method: 'accountNotification', params: { subscription: 22, result: { context: { slot: 7 } } } }) });
    expect(changes).toEqual([{ address: CLAIMS, slot: '7' }]);
    watch.close();
  });

  /**
   * The attribution rule, in the protocol. Subscription ids are the node's,
   * not ours; reporting a change under the wrong address would tell a reader
   * the wrong account moved.
   */
  it('never reports a change for a subscription it does not recognise', () => {
    const { watch, changes, fake } = watcher([MARKET]);
    watch.open();
    fake.fire('open');
    fake.fire('message', { data: '{"jsonrpc":"2.0","result":11,"id":1}' });
    fake.fire('message', { data: JSON.stringify({ method: 'accountNotification', params: { subscription: 999, result: { context: { slot: 7 } } } }) });
    expect(changes).toEqual([]);
    watch.close();
  });

  it('reports unavailable when the endpoint cannot carry a subscription at all', () => {
    const states: Array<WatchStateV1> = [];
    const watch = new AccountWatchV1('ftp://example.test', [MARKET], {
      onChange: () => {},
      onState: (state) => { states.push(state); },
      socketFactory: () => { throw new Error('should never be reached'); },
    });
    watch.open();
    expect(states).toEqual(['unavailable']);
  });

  it('reports unavailable when there is no socket to be had, without throwing', () => {
    const states: Array<WatchStateV1> = [];
    const watch = new AccountWatchV1('https://api.devnet.solana.com', [MARKET], {
      onChange: () => {},
      onState: (state) => { states.push(state); },
      socketFactory: () => { throw new Error('blocked'); },
    });
    expect(() => watch.open()).not.toThrow();
    expect(states).toEqual(['connecting', 'unavailable']);
  });

  it('treats a dropped connection as unavailable rather than as silence', () => {
    const { watch, states, fake } = watcher([MARKET]);
    watch.open();
    fake.fire('open');
    fake.fire('message', { data: '{"jsonrpc":"2.0","result":11,"id":1}' });
    fake.fire('close');
    expect(states[states.length - 1]).toBe('unavailable');
    watch.close();
  });

  it('says nothing more once closed, however loudly the socket carries on', () => {
    const { watch, changes, states, fake } = watcher([MARKET]);
    watch.open();
    fake.fire('open');
    fake.fire('message', { data: '{"jsonrpc":"2.0","result":11,"id":1}' });
    const seen = states.length;
    watch.close();
    expect(fake.isClosed()).toBe(true);
    fake.fire('message', { data: JSON.stringify({ method: 'accountNotification', params: { subscription: 11, result: { context: { slot: 9 } } } }) });
    fake.fire('close');
    expect(changes).toHaveLength(0);
    expect(states).toHaveLength(seen);
  });

  it('does not open a socket for an empty address set', () => {
    const { watch, states, opened } = watcher([]);
    watch.open();
    expect(opened).toEqual([]);
    expect(states).toEqual(['unavailable']);
  });
});

describe('what the reader is told', () => {
  it('never presents an unavailable watch as a fact about the market', () => {
    const sentence = watchSentenceV1('unavailable', 'devnet');
    // Renegotiated 2026-08-31: this sentence used to reassure the reader that
    // "everything on the page is still the read it says it is". Deleted. It
    // still says what is off and what to do instead, and still never blames
    // the market.
    expect(sentence).toContain('Live updates are off');
    expect(sentence).toContain('re-read button');
    expect(sentence).not.toContain('error');
  });

  it('says what a live watch will actually do', () => {
    expect(watchSentenceV1('live', 'devnet')).toContain('re-reads itself when the market changes');
  });
});
