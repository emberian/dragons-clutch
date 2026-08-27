import { type RegistryActivationPlanV1 } from './releaseRegistry';

/// The wallet un-gate contract, stated in both directions.
///
/// Specified by `docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md`. The
/// gate is a refusal, so it lives beside the other refusals rather than inside
/// a component: nothing about a loaded manifest, a connected wallet, or a
/// hopeful address opens signing on its own.
///
/// Everything the gate needs was already demanded by `prepareRegistryActivation`
/// before it would return a plan at all — contract items 2 through 4, at
/// finalized commitment: the checked manifests decode and rebuild each other;
/// the release-set and five artifact records exist at their derived PDAs, are
/// Registry-owned, rent-reserved, byte-identical to the evidence, with vacant
/// staging cursors; and every role's Loader Program/ProgramData accounts match
/// the checked release's geometry and digests exactly. A plan in hand IS that
/// conjunction. The only thing left for this gate to decide is whether the
/// wallet now connected is the one the plan already declared it would pay with.
export const UNGATE_SHUT_V1 = 'Signing stays closed. It opens only when one activation plan is green against this chain and the connected wallet is exactly that plan’s declared fee payer.';

/// The single sentence a green plan licenses, and its explicit limits.
///
/// Quoting the contract: a green `prepareRegistryActivation` "does not make the
/// addresses official, does not make the frontend official, and does not
/// transfer to devnet or mainnet." That is rendered next to the buttons it
/// unlocks, so the limit travels with the capability instead of living in a
/// document nobody opens while clicking.
export const UNGATE_LICENCE_V1 = 'This browser observed a chain whose finalized Registry records and Loader accounts match a named checked release set built from a named commit. That is the whole claim. It does not make these addresses official, does not make this frontend official, and does not transfer to devnet or mainnet.';

export type ReleaseUngateV1 = Readonly<{ open: boolean; reason: string }>;

/// Decide whether wallet signing is open for one observed activation plan.
///
/// Never returns `open` without a plan AND a connected wallet AND exact
/// equality between that wallet and the plan's declared fee payer. A closed
/// gate never carries the licence sentence, so no caller can render an
/// authorization the chain did not support.
export function releaseUngateV1(plan: RegistryActivationPlanV1 | null, connectedWallet: string | null): ReleaseUngateV1 {
  if (plan === null) return Object.freeze({ open: false, reason: `No activation plan is green against this chain. ${UNGATE_SHUT_V1}` });
  if (connectedWallet === null || connectedWallet.length === 0) return Object.freeze({ open: false, reason: `No browser wallet is connected. ${UNGATE_SHUT_V1}` });
  if (connectedWallet !== plan.payer) return Object.freeze({ open: false, reason: `Connected wallet ${connectedWallet} is not the plan’s declared fee payer ${plan.payer}. ${UNGATE_SHUT_V1}` });
  return Object.freeze({ open: true, reason: UNGATE_LICENCE_V1 });
}
