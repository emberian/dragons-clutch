import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { numeric } from "../dom.js";

const here = new URL("../", import.meta.url);
const source = async (name) => readFile(new URL(name, here), "utf8");

test("exact integer strings never pass through Number", () => {
  assert.equal(numeric("18446744073709551615"), "18,446,744,073,709,551,615");
  assert.equal(numeric(18446744073709551615n), "18,446,744,073,709,551,615");
  assert.equal(numeric(null), "—");
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

test("the retained Pyth surface is truth-labelled and has no campaign action", async () => {
  const [app, pyth, stream] = await Promise.all([
    source("app.js"),
    source("pyth.js"),
    source("stream.js"),
  ]);
  assert.match(app, /identity\.mode === "pyth-local"/);
  assert.match(stream, /case "pyth-campaign"/);
  for (const phrase of [
    "NON-PRODUCTION",
    "SYNTHETIC OBSERVATION",
    "LOCAL VALIDATOR ONLY",
    "NO VALUE",
    "not devnet price evidence",
    "joined-user-lifecycle-v1",
    "TRADE BLOCKED / NOT SUBSTITUTED",
    "missing-sealed-price-grid-and-epoch-plane",
  ]) {
    assert.match(pyth, new RegExp(phrase));
  }
  assert.doesNotMatch(pyth, /\bact\s*\(|fetch\s*\(|EventSource|signTransaction|sendTransaction/);
});
