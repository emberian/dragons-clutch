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
