import Anchor from '@/components/Anchor';
import { type MarketGateV1 } from '@/lib/tradeFlowSteps';

/**
 * The wall that stands before the stepper, and instead of it.
 *
 * `phase` and `activation` are facts about the Market. No step moves them, no
 * wallet changes them, and nothing a reader does on this page can help. So the
 * seven steps do not render at all: showing them greyed under "this market can
 * never trade" would be the flat console in a new costume -- a screen full of
 * controls whose only real content is a refusal buried somewhere in it.
 *
 * The detail renders whole. `activation` ends "Activation is the operator’s
 * move, not yours", which is the remedy: it tells a reader the thing they were
 * about to go looking for does not exist and the wait is not theirs to end.
 * That clause is the reason this card is a card and not a shrug.
 */
export default function MarketGateCard({
  gate,
}: Readonly<{ gate: Extract<MarketGateV1, { kind: 'closed' }> }>) {
  return <div className="flow-gate" role="status">
    <span>Trading closed · {gate.wall}</span>
    <strong>{gate.heading}</strong>
    <p>{gate.detail}</p>
    <Anchor className="secondary-action" href="/markets">See the markets that are open →</Anchor>
  </div>;
}
