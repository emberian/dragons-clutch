import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { encodeIntent, INTEGER_TRANSPORT } from "../action.js";
import {
  createStore,
  eventHasSafeNumbers,
  liveResultIsPresentable,
  liveRunIsPresentable,
} from "../stream.js";
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

test("a graph snapshot is promoted atomically and a partial successor preserves the old image", () => {
  const store = createStore();
  const account = (role, slot, ordinal = 1) => ({
    type: "state",
    snapshot_schema: "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2",
    context_slot: slot,
    ordinal,
    role,
    address: `${role}-address`,
    address_binding: "test-derived",
    present: true,
    owner: "program-owner",
    executable: false,
    account_schema: { name: "market", bytes: "1", tag: 1, version: 1 },
    decoded: { kind: "market" },
  });
  const snapshot = (slot, states, count = String(states.length)) => ({
    type: "account-snapshot-v2",
    schema: "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2",
    context_slot: slot,
    ordinal: 1,
    root_role: "friday.market",
    root_address: "friday.market-address",
    account_count: count,
    states,
  });

  store.ingest(snapshot("10", [
    account("friday.market", "10"),
    account("optional.collateral", "10"),
  ]));
  assert.equal(store.state.snapshot.context_slot, "10");
  assert.deepEqual([...store.state.latest.keys()], ["friday.market", "optional.collateral"]);

  store.ingest(snapshot("11", [account("friday.market", "11")], "2"));
  assert.equal(store.state.snapshot.context_slot, "10");
  assert.equal(store.state.latest.get("friday.market").context_slot, "10");
  assert.match(store.state.fault.text, /incomplete or malformed/);

  const absent = {
    type: "state",
    snapshot_schema: "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2",
    context_slot: "12",
    ordinal: 1,
    role: "optional.collateral",
    address: "optional.collateral-address",
    address_binding: "test-derived",
    present: false,
    decoded: null,
  };
  store.ingest(snapshot("12", [account("friday.market", "12"), absent]));
  assert.equal(store.state.snapshot.context_slot, "12");
  assert.deepEqual([...store.state.latest.keys()], ["friday.market"]);
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
  const [session, stream, trade] = await Promise.all([
    readFile(new URL("../../programs/clutch-sbf/operatord/src/session.rs", here), "utf8"),
    source("stream.js"),
    source("trade.js"),
  ]);
  for (const phrase of [
    "DAEMON FIXTURE DECLARATION",
    "DAEMON SESSION MEMORY",
    "VALIDATED DAEMON SAME-CONTEXT SNAPSHOT V2",
    "DAEMON RPC OBSERVATION",
    "DAEMON-REPORTED TRANSACTION RECEIPTS",
    "MIXED PROJECTION",
    "candidate-plan coordinates",
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
  assert.match(session, /"type": "candidate-plan"/);
  assert.match(session, /dragons-clutch\/operator\/candidate-plan\/v1/);
  assert.match(session, /"type": "candidate-trial"/);
  assert.doesNotMatch(session, /"type": "clearing(?:-attempt)?"/);
  assert.match(stream, /case "candidate-plan"/);
  assert.match(stream, /case "candidate-trial"/);
  assert.match(stream, /legacy pre-submit clearing schema/);
  assert.match(stream, /case "account-snapshot-v2"/);
  assert.match(stream, /graph-root-bracketed-account-snapshot\/v2/);
  assert.match(stream, /state\.latest = nextLatest/);
  assert.match(session, /"present": false/);
  assert.match(session, /complete snapshot is admitted/);
  assert.doesNotMatch(trade, /state\.clearing/);
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

const liveResult = () => ({
  type: "live-real-pyth-result",
  schema: "dragons-clutch/operator/live-real-pyth-result/v1",
  claim: "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE",
  campaign_mode: "joined-multiboundary-v1",
  transcript_schema: "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1",
  retained_transcript: false,
  genesis_hash: "local-genesis",
  boundary_count: "2",
  step_count: "56",
  sealed: true,
  resolved_payout: "1",
  archive_records: [
    { index: "0", bucket: "10", lower: "999", upper: "1001", sequence: "120", write_slot: "20", publish_time: "120" },
    { index: "1", bucket: "11", lower: "998", upper: "1002", sequence: "180", write_slot: "21", publish_time: "180" },
  ],
  source_archive: {
    key: "archive-address",
    owner: "clutch-program",
    executable: false,
    data_len: "2560",
    body_sha256: "a".repeat(64),
    page_commitment: "b".repeat(64),
    feed_id: "c".repeat(64),
    window_id: "d".repeat(64),
    record_count: "2",
  },
  out_of_order_boundary_rollback: {
    ok: true,
    skipped_update_account: "skipped-receiver-account",
    skipped_update_absent_after_refusal: true,
    instruction_error: { instruction_index: "2", custom_code: "122", custom_code_hex: "0x7a" },
    watched_accounts: [],
    before_snapshot_sha256: "e".repeat(64),
    after_snapshot_sha256: "e".repeat(64),
    snapshots_equal: true,
  },
  trade_status: "settled",
  collateral_atoms: "128",
  terminal: {
    buyer_position_cash_atoms: "0",
    buyer_position_internal: ["0", "0", "0", "0"],
    seller_position_cash_atoms: "0",
    seller_position_internal: ["0", "0", "0", "0"],
    supply_internal: ["0", "0", "0", "0"],
    hoard_collateral_atoms: "0",
    hoard_token_atoms: "0",
    buyer_token_atoms: "76",
    seller_token_atoms: "52",
  },
});

test("live Pyth projection admits only exact unretained terminal closure", () => {
  const run = {
    type: "live-real-pyth-run",
    schema: "dragons-clutch/operator/live-real-pyth-run/v1",
    mode: "pyth-live",
    phase: "running",
    campaign_mode: "joined-multiboundary-v1",
    retained_transcript: false,
    rpc_url: "http://127.0.0.1:9137",
    websocket_url: "ws://127.0.0.1:9138",
    faucet: "127.0.0.1:9139",
    gossip: "127.0.0.1:9200",
    dynamic_port_range: "9201-9250",
    authority: "read-only live child telemetry; no retained transcript; no browser key material",
  };
  assert.equal(liveRunIsPresentable(run), true);
  assert.equal(liveRunIsPresentable({ ...run, retained_transcript: true }), false);
  assert.equal(liveRunIsPresentable({ ...run, rpc_port: 9137 }), false);

  assert.equal(liveResultIsPresentable(liveResult()), true);
  const numericSubstitution = liveResult();
  numericSubstitution.out_of_order_boundary_rollback.watched_accounts.push({ lamports: 1 });
  assert.equal(liveResultIsPresentable(numericSubstitution), false);
  const wrongRefusal = liveResult();
  wrongRefusal.out_of_order_boundary_rollback.instruction_error.custom_code = "121";
  assert.equal(liveResultIsPresentable(wrongRefusal), false);
  const skippedBucket = liveResult();
  skippedBucket.archive_records[1].bucket = "12";
  assert.equal(liveResultIsPresentable(skippedBucket), false);
  const residue = liveResult();
  residue.terminal.supply_internal[2] = "1";
  assert.equal(liveResultIsPresentable(residue), false);
});

test("the live Pyth page is read-only and the launcher disables transcript retention", async () => {
  const [app, live, launcher, daemon] = await Promise.all([
    source("app.js"),
    source("live-pyth.js"),
    readFile(new URL("../../scripts/run_operator_real_pyth_live.sh", here), "utf8"),
    readFile(new URL("../../programs/clutch-sbf/operatord/src/pyth_live.rs", here), "utf8"),
  ]);
  assert.match(app, /LIVE CHILD \/ NOT RETAINED \/ BROWSER READ-ONLY/);
  assert.match(app, /identity\.mode === "pyth-live"/);
  for (const phrase of [
    "LIVE, NOT RETAINED",
    "real captured router/receiver laboratory",
    "Ephemeral payer and owner keys remain inside",
    "does not independently query RPC",
  ]) {
    assert.match(live, new RegExp(phrase));
  }
  assert.doesNotMatch(live, /\bact\s*\(|fetch\s*\(|EventSource|signTransaction|sendTransaction|privateKey|secretKey/);
  assert.match(launcher, /unset CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR/);
  assert.doesNotMatch(launcher, /--transcript/);
  assert.match(daemon, /env_remove\("CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR"\)/);
  assert.match(daemon, /admitted_progress_line/);
  assert.match(daemon, /stream != "stdout"/);
  assert.match(live, /filesystem paths, arbitrary child text, and retained result JSON remain process-local/);
});
