import { hex } from './bytes';
import * as DirectAbi from './generated/directInlineV3';

/**
 * What generation of the Direct bundle this build knows how to decode.
 *
 * Every entry is a REFERENCE to an already-generated constant, never a new
 * value. That is the whole discipline: this module is a VIEW over the canon,
 * so it cannot drift from it and cannot become one more hand-carried mirror of
 * the thing it describes. If the generator re-emits, this view moves with it.
 *
 * It exists so a refusal can say what this build is, instead of accusing the
 * chain of being wrong. A Market published under a release this build predates
 * is not a corrupt Market -- it is a newer one, and the honest sentence says
 * so and names both sides.
 */
export const DIRECT_DECODE_VINTAGE_V1 = Object.freeze({
  descriptor: DirectAbi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  accountProfile: DirectAbi.ACCOUNT_SCHEMA_RELEASE_ID,
  requestProfile: DirectAbi.REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID,
  lifecycle: DirectAbi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
  strategy: DirectAbi.EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
  transition: DirectAbi.TRANSITION_SCHEMA_RELEASE_ID,
  effect: DirectAbi.EFFECT_SCHEMA_RELEASE_ID_V4,
});

/** The first four bytes of an identity — enough to tell generations apart in prose. */
export function hex8(bytes: Uint8Array): string {
  return hex(bytes).slice(0, 8);
}

/**
 * One sentence naming every schema this build decodes, for the tail of a
 * vintage refusal. Prefixes only: the full identities belong in the field-level
 * message that named the disagreement, not in a wall of hex.
 */
export function describeDirectDecodeVintageV1(): string {
  const named = Object.entries(DIRECT_DECODE_VINTAGE_V1)
    .map(([field, id]) => `${field} ${hex8(id)}`)
    .join(', ');
  return `this build decodes ${named}`;
}
