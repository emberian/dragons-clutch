/**
 * The SDK owns the terminal certificate join and the redemption arithmetic.
 * The app re-exports it so the browser and every external client answer
 * "what did this market resolve ON" with one authority.
 */
export * from '@dclutch/sdk/marketResolution';
