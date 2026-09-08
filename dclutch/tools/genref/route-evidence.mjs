// Pure route-evidence classification used by the generated reference and its
// focused controls. Bindings are claims; only finalized-instruction ledger
// rows and corroborated devnet records are checked native evidence.

export const SUBSTRATE_RANK = {
  none: 0,
  "program-test": 1,
  "local-validator": 2,
  devnet: 3,
};

const strongest = (entries) =>
  entries.reduce(
    (best, entry) =>
      SUBSTRATE_RANK[entry.substrate] > SUBSTRATE_RANK[best]
        ? entry.substrate
        : best,
    "none",
  );

export function classifyRouteEvidence({ bindings = [], native = [], rule = null }) {
  const acceptedNative = native.filter((entry) => entry.outcome === "executed");
  const refusedNative = native.filter((entry) => entry.outcome === "refused");
  const successfulClaims = bindings.filter((entry) => entry.outcome === "executed");
  const refusalClaims = bindings.filter((entry) => entry.outcome === "refused");
  const acceptedAgave = acceptedNative.filter(
    (entry) => entry.substrate === "devnet" || entry.substrate === "local-validator",
  );
  const acceptedProgramTest = acceptedNative.filter(
    (entry) => entry.substrate === "program-test",
  );

  if (acceptedAgave.length > 0) {
    return {
      kind: "accepted-agave",
      substrate: strongest(acceptedAgave),
      bindings,
      native,
      acceptedNative,
      refusedNative,
      successfulClaims,
      refusalClaims,
      rule,
    };
  }
  if (acceptedProgramTest.length > 0) {
    return {
      kind: "accepted-program-test",
      substrate: "program-test",
      bindings,
      native,
      acceptedNative,
      refusedNative,
      successfulClaims,
      refusalClaims,
      rule,
    };
  }
  if (refusedNative.length > 0) {
    return {
      kind: "refused-only",
      substrate: strongest(refusedNative),
      bindings,
      native,
      acceptedNative,
      refusedNative,
      successfulClaims,
      refusalClaims,
      rule,
    };
  }
  if (successfulClaims.length > 0) {
    return {
      kind: "success-claim",
      substrate: strongest(successfulClaims),
      bindings,
      native,
      acceptedNative,
      refusedNative,
      successfulClaims,
      refusalClaims,
      rule,
    };
  }
  if (refusalClaims.length > 0) {
    return {
      kind: "refusal-claim",
      substrate: strongest(refusalClaims),
      bindings,
      native,
      acceptedNative,
      refusedNative,
      successfulClaims,
      refusalClaims,
      rule,
    };
  }
  return {
    kind: rule ? "blocked" : "unrecorded",
    substrate: "none",
    bindings,
    native,
    acceptedNative,
    refusedNative,
    successfulClaims,
    refusalClaims,
    rule,
  };
}
