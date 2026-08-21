/* The Walk: one row per submitted transaction, and what the bank did with it.
 *
 * Refusals are rendered as first-class results, not as errors.  "The program
 * refused this, exactly here, with exactly this code" is the evidence; a bench
 * that greyed those rows out or hid them behind an error style would be
 * throwing away the half of the walk that shows the boundaries hold. */

import { el, fields, digest, numeric, row } from "./dom.js";

/* The per-transaction compute ceiling the plan measures against.  Overridden
 * by the daemon's own value as soon as the plan event arrives. */
const DEFAULT_CEILING = 1400000;

const clockBanner = (state) => {
  if (!state.clock || state.done) return null;
  const clock = state.clock;
  const banner = el("section", "card clock");
  banner.append(
    el("h2", null, "Waiting on the bank's real clock"),
    el(
      "p",
      "muted",
      "The deadline transitions of the general plane are clock-gated by the program, so this walk waits on the validator rather than warping it."
    ),
    fields("", [
      ["reason", clock.reason],
      ["current slot", numeric(clock.slot)],
      ["target slot", numeric(clock.target)],
      ["slots remaining", numeric(clock.remaining)]
    ])
  );
  return banner;
};

const computeBar = (units, ceiling) => {
  const cell = el("div", "cu");
  if (units === null || units === undefined) {
    cell.append(el("span", "cu-value muted", "not reported"));
    return cell;
  }
  const share = Math.max(0, Math.min(1, units / ceiling));
  const track = el("div", "cu-track");
  const bar = el("div", "cu-bar");
  bar.style.width = `${(share * 100).toFixed(2)}%`;
  if (share > 0.85) bar.classList.add("cu-hot");
  track.append(bar);
  cell.append(track, el("span", "cu-value", `${numeric(units)} / ${numeric(ceiling)} CU`));
  return cell;
};

const decodedList = (entries) => {
  const list = el("ul", "decoded");
  entries.forEach((entry) => {
    const item = el("li");
    item.append(el("span", "decoded-role", entry.role));
    const decoded = entry.decoded || {};
    const parts = Object.keys(decoded)
      .filter((key) => key !== "kind")
      .map((key) => `${key}=${JSON.stringify(decoded[key])}`);
    item.append(el("span", "decoded-kind", decoded.kind || "opaque"));
    item.append(el("span", "decoded-body", parts.join("  ")));
    list.append(item);
  });
  return list;
};

const stepRow = (state, step, ceiling) => {
  const record = state.steps.get(step.ordinal) || {};
  const status = record.state && record.state !== "inflight" ? record.state : record.state ? "inflight" : "pending";
  const item = el("article", `step step-${status} kind-${step.kind}`);

  const head = el("header", "step-head");
  head.append(
    el("span", "step-ordinal", String(step.ordinal).padStart(2, "0")),
    el("span", "step-name", step.name),
    el("span", `step-badge badge-${step.kind}`, step.kind === "accept" ? "accept" : step.kind),
    el("span", "step-status", status)
  );
  item.append(head);
  item.append(el("p", "step-note", step.note));

  if (step.kind === "refuse") {
    item.append(
      row(
        "refusal",
        el("strong", null, `expects Custom(${step.expect_code_hex || "?"})`),
        el("span", null, step.reference || "")
      )
    );
  }

  const facts = [
    ["oracle", step.oracle],
    ["instructions", numeric(step.instructions)],
    ["transaction bytes", numeric(step.bytes)],
    ["declared reloads", numeric(step.reloads)]
  ];
  if (typeof record.slot === "number") facts.push(["confirmed slot", numeric(record.slot)]);
  if (record.confirmation) facts.push(["commitment", record.confirmation]);
  if (step.wait_slot) facts.push(["waits for slot", numeric(step.wait_slot)]);
  if (step.wait_after) facts.push(["waits for", `${step.wait_after.step} + ${step.wait_after.delta}`]);
  item.append(fields("step-fields", facts));

  if (record.state) item.append(computeBar(record.cu === undefined ? null : record.cu, ceiling));
  if (record.signature) {
    const line = el("div", "signature");
    line.append(el("span", "muted", "signature"), digest(record.signature));
    item.append(line);
  }
  const observed = state.states.get(step.ordinal);
  if (observed && observed.length) item.append(decodedList(observed));
  return item;
};

const conservationStrip = (state) => {
  const strip = state.conservation;
  const section = el("section", "card conservation");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Conservation, re-derived from observed bytes"));
  if (strip) heading.append(el("span", "count", strip.live ? "live" : "terminal"));
  section.append(heading);
  section.append(
    el(
      "p",
      "muted",
      "Every number below is read out of an account image this walk actually reloaded, at the offsets the plan's conservation table names. Nothing here is copied from the plan's expected block; the expected block is what is being checked."
    )
  );
  if (!strip) {
    section.append(el("p", "muted", "No reloads yet."));
    return section;
  }
  const table = el("table", "ledger");
  const head = el("thead");
  const headRow = el("tr");
  ["role", "cash", "reserved", "eggs[0]", "eggs[1]"].forEach((label) => headRow.append(el("th", null, label)));
  head.append(headRow);
  const body = el("tbody");
  (strip.rows || []).forEach((entry) => {
    const line = el("tr");
    line.append(
      el("td", "role", entry.role),
      el("td", null, numeric(entry.cash)),
      el("td", null, numeric(entry.reserved)),
      el("td", null, numeric(entry.eggs[0])),
      el("td", null, numeric(entry.eggs[1]))
    );
    body.append(line);
  });
  table.append(head, body);
  section.append(table);
  section.append(
    fields("", [
      ["position cash total", numeric(strip.cash_total)],
      ["eggs outcome 0", numeric(strip.eggs ? strip.eggs[0] : null)],
      ["eggs outcome 1", numeric(strip.eggs ? strip.eggs[1] : null)],
      ["locked backing", numeric(strip.locked)],
      ["pooled custody token", numeric(strip.custody)],
      ["endowed total", numeric(strip.endowed_total)],
      ["split total", numeric(strip.split_total)]
    ])
  );
  if (!strip.complete && strip.pending && strip.pending.length) {
    section.append(
      row(
        "callout",
        el("strong", null, "PARTIAL"),
        el("span", null, `not yet observed: ${strip.pending.join(", ")}`)
      )
    );
  }
  const verdicts = [...(strip.checks || []), ...(strip.identities || [])];
  if (verdicts.length) {
    const list = el("ul", "identities");
    verdicts.forEach((entry) => {
      const item = el("li", entry.ok ? "ok" : "bad");
      item.append(
        el("span", "identity-label", entry.label),
        el("span", "identity-value", `observed ${numeric(entry.observed)} · expected ${numeric(entry.expected)}`),
        el("span", "identity-verdict", entry.ok ? "ok" : "FAIL")
      );
      list.append(item);
    });
    section.append(list);
  }
  return section;
};

const verdictCard = (state) => {
  if (state.fault) {
    return row("card callout callout-bad", el("strong", null, "FAULT"), el("span", null, state.fault.text));
  }
  if (!state.done) return null;
  const passed = state.done.verdict === "PASS";
  return row(
    `card callout ${passed ? "callout-ok" : "callout-bad"}`,
    el("strong", null, state.done.verdict),
    el("span", null, `work dir ${state.done.work_dir}`)
  );
};

export const renderWalk = (state) => {
  const ceiling = state.plan && state.plan.compute_unit_ceiling ? state.plan.compute_unit_ceiling : DEFAULT_CEILING;
  const cards = [];
  const verdict = verdictCard(state);
  if (verdict) cards.push(verdict);
  const clock = clockBanner(state);
  if (clock) cards.push(clock);
  cards.push(conservationStrip(state));
  const log = el("section", "card log");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Step log"));
  if (state.plan) heading.append(el("span", "count", `ceiling ${numeric(ceiling)} CU`));
  log.append(heading);
  if (!state.plan) {
    log.append(el("p", "muted", "Waiting for the plan."));
  } else {
    state.plan.steps.forEach((step) => log.append(stepRow(state, step, ceiling)));
  }
  cards.push(log);
  return cards;
};
