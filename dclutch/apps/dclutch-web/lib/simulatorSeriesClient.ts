'use client';

import { useEffect, useState } from 'react';

import { readSimulatorSeriesV1, type SimulatorSeriesReadV1 } from '@/lib/simulatorSeries';

/**
 * One read of the published series per page load, however many charts want it.
 *
 * The discovery page draws a card per market and the detail page draws one
 * market; if each chart fetched for itself, opening a listing of twenty
 * markets would ask a static host for the same file twenty times. That is the
 * same mistake as a read fan-out against an RPC node, just cheaper, and the
 * fix is the same shape: do it once and share the result.
 *
 * The promise is cached, not the value, so simultaneous mounts join one
 * in-flight read rather than starting a second. Nothing here retries: the
 * reader already folds every transport failure into `absent`, which is the
 * honest answer for a file that is usually not published at all.
 */

let inFlight: Promise<SimulatorSeriesReadV1> | null = null;

/** The shared read. Exported so a test can observe how many times it fetches. */
export function loadSimulatorSeriesV1(): Promise<SimulatorSeriesReadV1> {
  inFlight ??= readSimulatorSeriesV1((url) => globalThis.fetch(url, {
    cache: 'no-store',
    redirect: 'error',
    credentials: 'omit',
  }));
  return inFlight;
}

/** Test seam: forget the shared read so the next call fetches again. */
export function resetSimulatorSeriesCacheV1(): void {
  inFlight = null;
}

/**
 * The series, or null until it settles. `preloaded` is the test seam every
 * other surface here uses: given one, the hook never fetches at all.
 */
export function useSimulatorSeriesV1(preloaded?: SimulatorSeriesReadV1): SimulatorSeriesReadV1 | null {
  const [read, setRead] = useState<SimulatorSeriesReadV1 | null>(preloaded ?? null);
  useEffect(() => {
    if (preloaded !== undefined) return undefined;
    let cancelled = false;
    void loadSimulatorSeriesV1().then((settled) => {
      if (!cancelled) setRead(settled);
    });
    return () => { cancelled = true; };
  }, [preloaded]);
  return read;
}
