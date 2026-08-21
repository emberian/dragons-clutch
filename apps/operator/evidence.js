/* The claim vocabulary, copied VERBATIM from apps/static-client/app.js.
 *
 * This file must stay a copy, not a variation.  The Operator Bench is a new
 * surface for evidence that already exists; inventing a chip here would mean
 * inventing a claim, and the point of a frozen vocabulary is that a new
 * screen cannot quietly widen what the project says about itself.
 *
 * If a kind is ever added, it is added there first and copied here second. */
export const EVIDENCE = Object.freeze({
  LOCAL_FIXTURE: Object.freeze({ label: "LOCAL FIXTURE", className: "local-chip" }),
  PROVED_MODEL: Object.freeze({ label: "PROVED-MODEL", className: "evidence-chip" }),
  CHECKED_RUST_SUBSET: Object.freeze({ label: "CHECKED-RUST-SUBSET", className: "evidence-chip" }),
  CHECKED_FINITE: Object.freeze({ label: "CHECKED-FINITE", className: "evidence-chip" }),
  HOST_TESTED: Object.freeze({ label: "HOST-TESTED", className: "evidence-chip" }),
  SBF_EXECUTED: Object.freeze({ label: "SBF-EXECUTED", className: "evidence-chip" }),
  MODEL_ONLY: Object.freeze({ label: "MODEL-ONLY", className: "proposed-chip" }),
  PROPOSED: Object.freeze({ label: "PROPOSED", className: "proposed-chip" }),
  IN_FLIGHT: Object.freeze({ label: "IN-FLIGHT", className: "proposed-chip" }),
  STOP: Object.freeze({ label: "STOP", className: "stop-chip" }),
  UNAVAILABLE: Object.freeze({ label: "UNAVAILABLE", className: "stop-chip" })
});

export const EVIDENCE_KINDS = new Set(Object.keys(EVIDENCE));

/* A chip for one frozen kind.  An unknown kind renders as UNAVAILABLE rather
 * than as its own name: a claim this page cannot vouch for must not get to
 * print itself. */
export const chip = (kind) => {
  const formatted = EVIDENCE_KINDS.has(kind) ? EVIDENCE[kind] : EVIDENCE.UNAVAILABLE;
  const element = document.createElement("span");
  element.className = `evidence-chip ${formatted.className}`;
  element.textContent = formatted.label;
  return element;
};
