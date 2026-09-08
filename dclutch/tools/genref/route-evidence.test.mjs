import assert from "node:assert/strict";
import test from "node:test";

import { classifyRouteEvidence } from "./route-evidence.mjs";

const binding = (substrate, outcome) => ({ substrate, outcome, campaign: "fixture" });
const native = (substrate, outcome) => ({ substrate, outcome, campaign: "fixture" });

test("a refused local-validator binding is never accepted Agave evidence", () => {
  const held = classifyRouteEvidence({
    bindings: [binding("local-validator", "refused")],
  });
  assert.equal(held.kind, "refusal-claim");
  assert.equal(held.substrate, "local-validator");
  assert.equal(held.acceptedNative.length, 0);
});

test("a successful local-validator binding remains a claim without a checked ledger", () => {
  const held = classifyRouteEvidence({
    bindings: [binding("local-validator", "executed")],
  });
  assert.equal(held.kind, "success-claim");
  assert.equal(held.substrate, "local-validator");
});

test("ProgramTest success is distinct from accepted Agave execution", () => {
  const held = classifyRouteEvidence({
    bindings: [binding("program-test", "executed")],
  });
  assert.equal(held.kind, "success-claim");
  assert.equal(held.substrate, "program-test");
});

test("a checked refusal stays refusal-only even beside a successful binding claim", () => {
  const held = classifyRouteEvidence({
    bindings: [binding("local-validator", "executed")],
    native: [native("local-validator", "refused")],
  });
  assert.equal(held.kind, "refused-only");
  assert.equal(held.successfulClaims.length, 1);
  assert.equal(held.acceptedNative.length, 0);
});

test("only a checked successful native row establishes accepted Agave execution", () => {
  const held = classifyRouteEvidence({
    bindings: [binding("program-test", "refused")],
    native: [native("local-validator", "executed")],
  });
  assert.equal(held.kind, "accepted-agave");
  assert.equal(held.substrate, "local-validator");
});

test("a refused devnet record cannot outrank accepted local evidence", () => {
  const held = classifyRouteEvidence({
    native: [native("devnet", "refused"), native("local-validator", "executed")],
  });
  assert.equal(held.kind, "accepted-agave");
  assert.equal(held.substrate, "local-validator");
});

test("a route with no evidence, claim or blocker is unrecorded", () => {
  const held = classifyRouteEvidence({});
  assert.equal(held.kind, "unrecorded");
});

test("all evidence classes form a complete tally including unrecorded", () => {
  const fixtures = [
    { native: [native("local-validator", "executed")] },
    { native: [native("program-test", "executed")] },
    { native: [native("devnet", "refused")] },
    { bindings: [binding("local-validator", "executed")] },
    { bindings: [binding("program-test", "refused")] },
    { rule: { route: "fixture/*" } },
    {},
  ];
  const tally = {
    "accepted-agave": 0,
    "accepted-program-test": 0,
    "refused-only": 0,
    "success-claim": 0,
    "refusal-claim": 0,
    blocked: 0,
    unrecorded: 0,
  };

  for (const fixture of fixtures) {
    const held = classifyRouteEvidence(fixture);
    assert.ok(Object.hasOwn(tally, held.kind), `unknown partition ${held.kind}`);
    tally[held.kind] += 1;
  }

  assert.deepEqual(tally, {
    "accepted-agave": 1,
    "accepted-program-test": 1,
    "refused-only": 1,
    "success-claim": 1,
    "refusal-claim": 1,
    blocked: 1,
    unrecorded: 1,
  });
});
