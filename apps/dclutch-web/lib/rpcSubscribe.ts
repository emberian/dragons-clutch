/**
 * Watching a small, named set of accounts for change — over one socket.
 *
 * Every surface in this app reads once, on mount, and then goes still. That is
 * a fair thing for a page that promises finalized reads, and it is also why a
 * market whose claims changed while you were looking at it says nothing. A
 * Solana node will tell you: `accountSubscribe` is free, and it is the only
 * part of this stack where the chain talks first.
 *
 * WHAT THIS DELIBERATELY DOES NOT DO. It does not decode the account data the
 * node pushes. A notification says only "this account is not what you read",
 * and that is all this file reports. Everything on screen still comes from the
 * app's one bounded, finalized read path — so a live update re-runs that read
 * rather than opening a second, unaudited decoder that could disagree with it.
 * The socket makes the page ask again; it never becomes a source of truth.
 *
 * WHY ONE SOCKET. `SolanaRpcClient` caps concurrent reads per endpoint at two
 * because a fan-out of small reads is what earns a 429. A socket per card
 * would be the same mistake in a different protocol: twenty cards, twenty
 * connections, one node. So a watch takes the whole address set at once and
 * multiplexes it, and the caller is expected to have exactly one.
 *
 * WHY IT IS ALLOWED TO FAIL QUIETLY. A static host, a blocked port, a browser
 * with no WebSocket, an endpoint that speaks http only — all of these are
 * ordinary, none of them are errors, and none of them may take the page down.
 * The watch reports `unavailable`, the surface says live updates are not
 * running and points at the button that always worked, and nothing else
 * changes. `SolanaRpcClient` REFUSES a ws:// endpoint on purpose
 * (lib/rpc.ts), so the socket URL is derived here and never configured.
 */

/** What a surface tells the reader about its own liveness. */
export type WatchStateV1 = 'idle' | 'connecting' | 'live' | 'unavailable';

export type AccountChangeV1 = Readonly<{
  /** The watched address the node says has changed. */
  address: string;
  /** The slot the node observed that change at. */
  slot: string;
}>;

/** The part of a WebSocket this file uses; a test supplies its own. */
export type SocketLikeV1 = Readonly<{
  send(data: string): void;
  close(): void;
  addEventListener(type: string, handler: (event: unknown) => void): void;
}>;

export type SocketFactoryV1 = (url: string) => SocketLikeV1;

/**
 * The subscription endpoint for a read endpoint, or null when there cannot be
 * one. Solana nodes serve JSON-RPC over http(s) and pubsub over the same host
 * with the ws(s) scheme; anything else is not a thing to guess at.
 */
export function websocketEndpointV1(httpEndpoint: string): string | null {
  let parsed: URL;
  try { parsed = new URL(httpEndpoint); } catch { return null; }
  if (parsed.protocol === 'https:') parsed.protocol = 'wss:';
  else if (parsed.protocol === 'http:') parsed.protocol = 'ws:';
  else return null;
  return parsed.toString();
}

/** One `accountSubscribe` call, at the commitment the rest of the app reads. */
export function subscribeRequestV1(id: number, address: string): string {
  return JSON.stringify({
    jsonrpc: '2.0',
    id,
    method: 'accountSubscribe',
    params: [address, { encoding: 'base64', commitment: 'finalized' }],
  });
}

export type SocketMessageV1 =
  /** The node confirmed request `id` as subscription `subscription`. */
  | Readonly<{ kind: 'subscribed'; id: number; subscription: number }>
  /** The node says the account behind `subscription` changed at `slot`. */
  | Readonly<{ kind: 'changed'; subscription: number; slot: string }>
  /** The node refused a request outright. */
  | Readonly<{ kind: 'refused'; id: number; reason: string }>
  /** Anything else, including every message shape this app does not use. */
  | Readonly<{ kind: 'ignored' }>;

const IGNORED: SocketMessageV1 = Object.freeze({ kind: 'ignored' as const });

/**
 * Read one frame off the socket. Pure, so the protocol is testable without a
 * network: a malformed or unexpected frame is `ignored`, never a throw, because
 * a node is entitled to send things this app does not consume.
 */
export function interpretSocketMessageV1(raw: unknown): SocketMessageV1 {
  if (typeof raw !== 'string') return IGNORED;
  let body: Record<string, unknown>;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) return IGNORED;
    body = parsed as Record<string, unknown>;
  } catch { return IGNORED; }

  if (body.method === 'accountNotification') {
    const params = body.params;
    if (params === null || typeof params !== 'object') return IGNORED;
    const shaped = params as Record<string, unknown>;
    const subscription = shaped.subscription;
    const result = shaped.result;
    if (typeof subscription !== 'number' || result === null || typeof result !== 'object') return IGNORED;
    const context = (result as Record<string, unknown>).context;
    const slot = context === null || typeof context !== 'object'
      ? undefined
      : (context as Record<string, unknown>).slot;
    if (typeof slot !== 'number' || !Number.isSafeInteger(slot)) return IGNORED;
    return Object.freeze({ kind: 'changed' as const, subscription, slot: String(slot) });
  }

  if (typeof body.id === 'number') {
    if (typeof body.result === 'number') {
      return Object.freeze({ kind: 'subscribed' as const, id: body.id, subscription: body.result });
    }
    const error = body.error;
    if (error !== null && typeof error === 'object') {
      const message = (error as Record<string, unknown>).message;
      return Object.freeze({
        kind: 'refused' as const,
        id: body.id,
        reason: typeof message === 'string' ? message : 'the node refused the subscription without a reason',
      });
    }
  }
  return IGNORED;
}

export type AccountWatchOptionsV1 = Readonly<{
  onChange: (change: AccountChangeV1) => void;
  onState: (state: WatchStateV1) => void;
  /** Injected in tests; defaults to the platform WebSocket when there is one. */
  socketFactory?: SocketFactoryV1;
}>;

function platformSocketFactory(): SocketFactoryV1 | null {
  const constructor = (globalThis as { WebSocket?: new (url: string) => unknown }).WebSocket;
  if (typeof constructor !== 'function') return null;
  return (url) => new constructor(url) as unknown as SocketLikeV1;
}

/**
 * One socket, one address set, one lifetime.
 *
 * Deliberately not a reconnecting client. A dropped socket reports
 * `unavailable` and stops; the surface keeps its manual re-read, which is the
 * control that has always worked. A reconnect loop against a public endpoint
 * is a rate-limit incident waiting for a bad afternoon, and this is a devnet
 * preview, not a trading terminal.
 */
export class AccountWatchV1 {
  private socket: SocketLikeV1 | null = null;
  private readonly pending = new Map<number, string>();
  private readonly subscriptions = new Map<number, string>();
  private closed = false;

  constructor(
    private readonly endpoint: string,
    private readonly addresses: ReadonlyArray<string>,
    private readonly options: AccountWatchOptionsV1,
  ) {}

  /** Open the socket and subscribe to every address. Never throws. */
  open(): void {
    if (this.closed || this.socket !== null) return;
    const url = websocketEndpointV1(this.endpoint);
    const factory = this.options.socketFactory ?? platformSocketFactory();
    if (url === null || factory === null || this.addresses.length === 0) {
      this.options.onState('unavailable');
      return;
    }
    this.options.onState('connecting');
    let socket: SocketLikeV1;
    try {
      socket = factory(url);
    } catch {
      this.options.onState('unavailable');
      return;
    }
    this.socket = socket;

    socket.addEventListener('open', () => {
      if (this.closed) return;
      this.addresses.forEach((address, index) => {
        const id = index + 1;
        this.pending.set(id, address);
        try { socket.send(subscribeRequestV1(id, address)); } catch { /* reported by the close handler */ }
      });
    });

    socket.addEventListener('message', (event) => {
      if (this.closed) return;
      const data = (event as { data?: unknown }).data;
      const message = interpretSocketMessageV1(data);
      if (message.kind === 'subscribed') {
        const address = this.pending.get(message.id);
        if (address !== undefined) {
          this.pending.delete(message.id);
          this.subscriptions.set(message.subscription, address);
          this.options.onState('live');
        }
        return;
      }
      if (message.kind === 'changed') {
        const address = this.subscriptions.get(message.subscription);
        if (address !== undefined) this.options.onChange({ address, slot: message.slot });
        return;
      }
      if (message.kind === 'refused') {
        this.pending.delete(message.id);
        // One refused address does not end the watch; a watch with nothing
        // subscribed has nothing to report and says so.
        if (this.pending.size === 0 && this.subscriptions.size === 0) this.options.onState('unavailable');
      }
    });

    const fail = () => {
      if (this.closed) return;
      this.options.onState('unavailable');
    };
    socket.addEventListener('error', fail);
    socket.addEventListener('close', fail);
  }

  /** Stop watching. Safe to call more than once, and after a failure. */
  close(): void {
    this.closed = true;
    const socket = this.socket;
    this.socket = null;
    this.pending.clear();
    this.subscriptions.clear();
    if (socket !== null) {
      try { socket.close(); } catch { /* a socket that cannot close is already gone */ }
    }
  }
}

/** One plain sentence per state, for a surface to render as-is. */
export function watchSentenceV1(state: WatchStateV1, endpointLabel: string): string {
  switch (state) {
    case 'live':
      return `Watching ${endpointLabel} for changes to this market. If it changes, this page re-reads it by itself.`;
    case 'connecting':
      return `Opening a connection to ${endpointLabel} to watch this market for changes…`;
    case 'unavailable':
      return `Live updates are not running — this endpoint did not accept a subscription. Everything on the page is still the read it says it is, and the re-read button always works.`;
    default:
      return 'Not watching for changes.';
  }
}
