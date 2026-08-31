import { type FlowStepV1, type StepStatusV1 } from '@/lib/tradeFlowSteps';

/**
 * The whole shape of the trade, before you start it.
 *
 * Seven steps, always all seven, in one of five states. That is the entire
 * point of the rail: the flat panel this replaces could only ever tell a
 * reader where they were by what happened to be rendered, so a person one
 * refusal deep had no way to see how much was left or what it would ask of
 * them. Here the map is above the territory and it never changes size.
 *
 * The rail is ANCHORS, not buttons, for two reasons that turn out to be the
 * same reason. A disabled button is the flat-console failure in miniature --
 * it says no and cannot say why -- and this surface's tests forbid the word
 * `greyed-out` precisely because that pattern kept coming back. Anchors to
 * step ids are always live, always keyboard-reachable, work with no script at
 * all, and land the reader ON the step whose body carries the reason.
 */

/** What the rail calls each state. Short, because the body carries the detail. */
const STATUS_WORDS_V1: Readonly<Record<StepStatusV1, string>> = Object.freeze({
  done: 'done',
  current: 'you are here',
  available: 'open',
  blocked: 'blocked',
  upcoming: 'later',
});

/** The id one step's body carries, and the rail links to. */
export function tradeStepIdV1(index: number): string {
  return `trade-step-${index}`;
}

export default function FlowRail({ steps }: Readonly<{ steps: ReadonlyArray<FlowStepV1> }>) {
  return <nav className="flow-rail" aria-label="The seven steps of this trade">
    <ol>
      {steps.map((step) => <li key={step.key} className={`flow-rail-${step.status}`}>
        <a href={`#${tradeStepIdV1(step.index)}`} aria-current={step.status === 'current' ? 'step' : undefined}>
          <span aria-hidden="true">{step.index}</span>
          <strong>{step.title}</strong>
          <small>{STATUS_WORDS_V1[step.status]}</small>
        </a>
      </li>)}
    </ol>
  </nav>;
}
