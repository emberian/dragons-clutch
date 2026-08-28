/**
 * The SDK owns current/historical Market classification and all discovery
 * joins. The app re-exports it so browser and external clients cannot drift.
 */
export * from '../../../packages/dclutch-sdk/lib/marketDiscovery';
