import Anchor from '@/components/Anchor';

/**
 * The whole page a refused Market address gets: the chain's verdict, told
 * once, with somewhere to go — instead of a stack of sections each explaining
 * their own emptiness.
 *
 * The refusal text is the SDK's named verdict (a historical-generation story,
 * a wrong owner, an absent account …) and is rendered verbatim: this page
 * adds navigation, never interpretation.
 */
export default function RefusedMarketStory({
  refusal,
  observedSlot,
  address,
}: Readonly<{
  refusal: string;
  observedSlot: string;
  address: string;
}>) {
  return <>
    <p className="market-refusal">{refusal}</p>
    <dl className="detail-facts">
      <div><dt>Address read</dt><dd><code>{address}</code></dd></div>
      <div><dt>Finalized observation slot</dt><dd>{observedSlot}</dd></div>
    </dl>
    <p className="direct-status">That refusal is the whole truth this page has. Nothing below it — phase, balances, collateral, capabilities — can be asserted for an account the reader could not authenticate, so this page does not pretend to a structure it could not read.</p>
    <div className="direct-actions">
      <Anchor className="secondary-action" href="/markets">Browse the current markets →</Anchor>
      <Anchor className="secondary-action" href={`/explorer?view=market&q=${encodeURIComponent(address)}`}>See the raw account in the explorer →</Anchor>
    </div>
  </>;
}
