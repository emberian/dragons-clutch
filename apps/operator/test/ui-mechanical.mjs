import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { encodeIntent, INTEGER_TRANSPORT } from "../action.js";
import { eventHasSafeNumbers } from "../stream.js";
import { campaignIsPresentable } from "../pyth.js";
import {
  decimalCents,
  decimalDifference,
  decimalMax,
  decimalPercent,
  exactInteger,
  numeric,
} from "../dom.js";

const here = new URL("../", import.meta.url);
const source = async (name) => readFile(new URL(name, here), "utf8");

test("exact integer strings never pass through Number", () => {
  for (const [raw, formatted] of [
    ["9007199254740992", "9,007,199,254,740,992"],
    ["9007199254740993", "9,007,199,254,740,993"],
    ["18446744073709551615", "18,446,744,073,709,551,615"],
  ]) {
    assert.equal(JSON.parse(JSON.stringify({ amount: raw })).amount, raw);
    assert.equal(numeric(raw), formatted);
    assert.equal(exactInteger(raw), BigInt(raw));
  }
  assert.equal(numeric(18446744073709551615n), "18,446,744,073,709,551,615");
  assert.equal(numeric(9007199254740992), "INVALID UNSAFE NUMBER");
  assert.equal(numeric(Number.MAX_SAFE_INTEGER), "9,007,199,254,740,991");
  assert.equal(numeric(null), "—");
});

test("wide decimal arithmetic stays exact until a bounded display coordinate", () => {
  assert.equal(decimalDifference("18446744073709551615", "9007199254740993"), "18437736874454810622");
  assert.equal(decimalMax(["9007199254740992", "9007199254740993"]), "9007199254740993");
  assert.equal(decimalPercent("9007199254740993", "18446744073709551615"), 0.04);
  assert.equal(decimalCents("24001"), "$240.01");
  for (const invalid of ["", "00", "01", "+1", "1.0", "1e3", " 1", "١"]) {
    assert.equal(exactInteger(invalid), null);
  }
});

test("trade intents preserve full-width decimal strings and refuse unsafe Numbers", () => {
  for (const amount of ["9007199254740992", "9007199254740993", "18446744073709551615"]) {
    const body = encodeIntent({
      action: "endow",
      amount,
      integer_transport: "caller-cannot-downgrade-this",
    });
    assert.equal(JSON.parse(JSON.stringify(body)).amount, amount);
    assert.equal(body.integer_transport, INTEGER_TRANSPORT);
  }
  for (const amount of [9007199254740992, Number("9007199254740993"), Number("18446744073709551615")]) {
    assert.throws(
      () => encodeIntent({ action: "endow", amount }),
      /unsafe JSON number/
    );
  }
});

test("the untrusted event projection refuses unsafe JSON numbers", () => {
  for (const amount of ["9007199254740992", "9007199254740993", "18446744073709551615"]) {
    assert.equal(eventHasSafeNumbers({ type: "state", decoded: { amount } }), true);
  }
  assert.equal(
    eventHasSafeNumbers({ type: "state", decoded: { amount: 9007199254740992 } }),
    false
  );
  assert.equal(
    eventHasSafeNumbers({ type: "state", decoded: { internal: ["1", 9007199254740992, "3"] } }),
    false
  );
});

test("Pyth presentation keeps retained history and refuses transitional joined-v3", () => {
  const base = {
    claim: "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE",
    retained_transcript: true,
    provider: [],
    steps: [],
    rollbacks: [],
    source: {},
  };
  assert.equal(campaignIsPresentable({
    ...base,
    schema: "dragons-clutch/operator/local-real-pyth-transcript/v1",
    campaign_mode: "source-only-v1",
  }), true);
  assert.equal(campaignIsPresentable({
    ...base,
    schema: "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v2",
    campaign_mode: "joined-user-lifecycle-v1",
    lifecycle: {},
  }), true);

  const currentSource = {
    registered_source_plane_count: "1",
    wrong_feed_verified_vaa_account: "wrong-feed-vaa",
    wrong_feed_observation: {},
    freshness: {
      scope: "append-time freshness; final Clock informational",
      append_age_seconds: "240",
      final_age_seconds: "1240",
    },
  };
  assert.equal(campaignIsPresentable({
    ...base,
    schema: "dragons-clutch/operator/local-real-pyth-transcript/v2",
    campaign_mode: "source-only-v1",
    source: currentSource,
  }), true);
  assert.equal(campaignIsPresentable({
    ...base,
    schema: "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v4",
    campaign_mode: "joined-user-lifecycle-v1",
    source: currentSource,
    lifecycle: {},
  }), true);
  assert.equal(campaignIsPresentable({
    ...base,
    schema: "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v3",
    campaign_mode: "joined-user-lifecycle-v1",
    source: currentSource,
    lifecycle: {},
  }), false);
});

test("the document and generated controls expose the accessibility contract", async () => {
  const [html, app, trade] = await Promise.all([
    source("index.html"),
    source("app.js"),
    source("trade.js"),
  ]);
  assert.match(html, /<h1 class="brand-name">Operator Bench<\/h1>/);
  assert.match(html, /id="status"[^>]*aria-live="polite"/);
  assert.match(app, /setAttribute\("aria-current"/);
  assert.match(trade, /cell\.scope = "col"/);
  assert.match(trade, /setAttribute\("aria-label", `Belief weight at/);
  assert.match(trade, /setAttribute\("aria-label", "Automaton belief/);
  assert.match(trade, /setAttribute\("role", ticket\.notice\.ok \? "status" : "alert"/);
});

test("belief dragging updates in place and freeze is phase-disabled", async () => {
  const trade = await source("trade.js");
  const inputHandler = trade.match(
    /input\.addEventListener\("input", \(\) => \{([\s\S]*?)\n    \}\);/,
  );
  assert.ok(inputHandler, "belief input handler is present");
  assert.doesNotMatch(inputHandler[1], /repaint\(/);
  assert.match(inputHandler[1], /valueNode\.textContent/);
  assert.match(trade, /freeze\.disabled = !open/);
});

test("trade fields preserve source boundaries and pre-submit coordinates stay model-only", async () => {
  const trade = await source("trade.js");
  for (const phrase of [
    "DAEMON FIXTURE DECLARATION",
    "DAEMON SESSION MEMORY",
    "ROLE-DECODED RPC DATA",
    "DAEMON RPC OBSERVATION",
    "DAEMON-REPORTED TRANSACTION RECEIPTS",
    "MIXED PROJECTION",
    "candidate-trial coordinates",
    "pre-submit model output",
    "does not establish that the bank accepted, verified, or selected",
  ]) {
    assert.match(trade, new RegExp(phrase));
  }
  for (const falsePromotion of [
    "bank-stamped cleared prices",
    "cleared vector the bank stamped",
    "once the bank has selected a candidate",
  ]) {
    assert.doesNotMatch(trade, new RegExp(falsePromotion));
  }
});

test("the retained Pyth surface is truth-labelled and has no campaign action", async () => {
  const [app, pyth, stream] = await Promise.all([
    source("app.js"),
    source("pyth.js"),
    source("stream.js"),
  ]);
  assert.match(app, /identity\.mode === "pyth-local"/);
  assert.match(app, /READ-ONLY RETAINED TRANSCRIPT/);
  assert.match(stream, /case "pyth-campaign"/);
  for (const phrase of [
    "NON-PRODUCTION",
    "SYNTHETIC OBSERVATION",
    "LOCAL VALIDATOR ONLY",
    "NO VALUE",
    "not devnet price evidence",
    "joined-user-lifecycle-v1",
    "local-real-pyth-transcript/v2",
    "local-real-pyth-joined-lifecycle/v2",
    "local-real-pyth-joined-lifecycle/v4",
    "TRADE BLOCKED / NOT SUBSTITUTED",
    "missing-sealed-price-grid-and-epoch-plane",
    "TRADE SETTLED / NOT SUBSTITUTED",
    "best valid submitted candidate",
    "not a claim of optimal clearing",
    "Terminal exact two-owner conservation",
    "READ-ONLY RETAINED TRANSCRIPT",
    "does not re-read the chain",
  ]) {
    assert.match(pyth, new RegExp(phrase));
  }
  assert.doesNotMatch(pyth, /\bact\s*\(|fetch\s*\(|EventSource|signTransaction|sendTransaction/);
});
