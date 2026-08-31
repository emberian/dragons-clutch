import { type StepRefusalV1 } from '@/lib/tradeFlowRefusals';

/**
 * One named refusal, rendered where it can be acted on.
 *
 * Two parts, in this order and never the other one: what to DO, then what the
 * protocol actually said. A refusal a reader cannot act on is a mystery, and a
 * refusal whose own words are paraphrased away is a rumour -- so the remedy
 * leads and the detail survives verbatim.
 *
 * `detail` is one text node inside one element on purpose. These strings are
 * pinned by `toContain` guards across the suite, and a sentence wrapped half
 * in a `<span>` for emphasis stops being findable: the guard then passes or
 * fails for reasons that have nothing to do with whether the site is honest.
 * Style the element, never the sentence inside it.
 */
export default function StepRefusal({ refusal }: Readonly<{ refusal: StepRefusalV1 }>) {
  return <div className="flow-refusal" role="alert">
    <strong>{refusal.remedy}</strong>
    <p>{refusal.detail}</p>
  </div>;
}
