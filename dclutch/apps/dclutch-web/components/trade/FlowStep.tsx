import { type ReactNode } from 'react';

import StepRefusal from '@/components/trade/StepRefusal';
import { tradeStepIdV1 } from '@/components/trade/FlowRail';
import { assignRefusalV1 } from '@/lib/tradeFlowRefusals';
import { type FlowStepV1 } from '@/lib/tradeFlowSteps';

/**
 * One step's shell: its number, its name, the question it answers, and its
 * state.
 *
 * A blocked step shows its reason and not its controls. That is the honest
 * pairing -- a live input that cannot do anything is a worse lie than an
 * absent one, and the reason names the thing that would bring the controls
 * back rather than saying "finish the previous step", which tells a reader
 * only that they are not where they wanted to be.
 *
 * The blocked reason is routed through the refusal table on its way out. Most
 * reasons are already one remedy-first sentence and route to nothing, so they
 * render as a sentence. The `packet` wall is the exception that makes this
 * worth doing: it arrives as the protocol's own two-part message, and routing
 * puts its remedy above its measurement without editing the wall's text.
 */
export default function FlowStep({
  step,
  children,
}: Readonly<{ step: FlowStepV1; children?: ReactNode }>) {
  const blocked = step.blockedReason;
  const routed = blocked === null ? null : assignRefusalV1(blocked, step.index);
  return <section id={tradeStepIdV1(step.index)} className={`flow-step flow-step-${step.status}`}>
    <header>
      <span aria-hidden="true">{step.index}</span>
      <div>
        <h3>{step.title}</h3>
        <p>{step.question}</p>
      </div>
    </header>
    {routed !== null && (routed.routed
      ? <StepRefusal refusal={routed} />
      : <p className="flow-blocked">{blocked}</p>)}
    {blocked === null && children}
  </section>;
}
