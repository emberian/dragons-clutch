'use client';

import { useEffect, useRef, useState } from 'react';

import {
  AccountWatchV1,
  type AccountChangeV1,
  type SocketFactoryV1,
  type WatchStateV1,
} from '@/lib/rpcSubscribe';

/**
 * One watch per surface, torn down with it.
 *
 * The address list arrives as an array, which React re-creates on every render,
 * so the effect is keyed on the JOINED addresses rather than on the array's
 * identity. Without that, every render would close a socket and open another
 * one — a reconnect storm produced entirely by the renderer, against a public
 * endpoint, which is exactly the class of thing this app caps its concurrent
 * reads to avoid.
 *
 * `onChange` is held in a ref for the same reason: a caller's closure changes
 * on every render and must not be able to reopen the connection.
 *
 * Effects do not run under `renderToStaticMarkup`, which is how this suite
 * renders components, so no test render opens a socket. The protocol itself is
 * tested against an injected socket in lib/rpcSubscribe.test.ts, and this hook
 * takes the same injection point so a surface can be driven by hand.
 */
export function useAccountWatchV1(
  endpoint: string,
  addresses: ReadonlyArray<string>,
  onChange: (change: AccountChangeV1) => void,
  socketFactory?: SocketFactoryV1,
): WatchStateV1 {
  const [state, setState] = useState<WatchStateV1>('idle');
  const handler = useRef(onChange);
  // Kept current in an effect, not during render: the point of the ref is that
  // a caller's closure changing must not be able to reopen the connection, and
  // writing it while rendering is its own hazard.
  useEffect(() => { handler.current = onChange; }, [onChange]);
  const key = addresses.join(',');

  useEffect(() => {
    if (key.length === 0) return undefined;
    const watch = new AccountWatchV1(endpoint, key.split(','), {
      onChange: (change) => handler.current(change),
      onState: setState,
      socketFactory,
    });
    // Opening reports `connecting` (or `unavailable`) straight away, so the
    // state below is never the previous market's for even one paint.
    watch.open();
    return () => { watch.close(); };
  }, [endpoint, key, socketFactory]);

  // With nothing to watch there is nothing to report, and that is derived
  // rather than stored: a state that only ever means "no addresses" should not
  // be something an effect has to remember to write.
  return key.length === 0 ? 'idle' : state;
}
