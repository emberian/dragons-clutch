/**
 * The three public smoke markets, as deployed facts.
 *
 * One author for the /smoke story page and the /bounty walk page: each
 * market's address is either the REAL devnet Market account DEPLOY-1 founded,
 * or null — and null renders as "not live yet", never as a placeholder
 * address. Flipping a market live is filling in its record here; nothing else
 * on either page needs editing.
 */

export interface SmokeMarketV1 {
  /** The story card's number and title, stable across launch. */
  readonly title: string;
  /** The devnet Market account (Core-owned CoreState), or null pre-launch. */
  readonly address: string | null;
  /** One sentence of deployed fact shown when live (window, band, source). */
  readonly liveNote: string | null;
}

/** Devnet cluster identity every smoke market lives on. */
export const SMOKE_CLUSTER_GENESIS_V1 = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';

export const SMOKE_MARKETS_V1: Readonly<Record<'price' | 'graduation' | 'abandoned', SmokeMarketV1>> =
  Object.freeze({
    price: Object.freeze({
      title: 'Oracle truth · the price market',
      address: null,
      liveNote: null,
    }),
    graduation: Object.freeze({
      title: 'Cross-chain truth · the graduation market',
      address: null,
      liveNote: null,
    }),
    abandoned: Object.freeze({
      title: 'Adversarial truth · the abandoned market',
      address: null,
      liveNote: null,
    }),
  });

/** The abandoned market's posted bounty, in lamports (the walk pays exactly this). */
export const SMOKE_WALK_BOUNTY_LAMPORTS_V1 = 250_000;

/** True once any smoke market is live on devnet. */
export function smokeIsLiveV1(): boolean {
  return Object.values(SMOKE_MARKETS_V1).some((market) => market.address !== null);
}
