/* The Bench: what is running, what it was built from, who is signing, and
 * where the walk has got to.
 *
 * Every number on this screen is something the daemon observed.  The ELF
 * digest is hashed from the file the validator loaded, not read from a
 * manifest; the precreated-account list is the plan's own enumeration of what
 * it could not create permissionlessly. */

import { decimalMax, el, fields, digest, numeric, row } from "./dom.js";
import { chip } from "./evidence.js";

const STATE_LABEL = Object.freeze({
  pending: "pending",
  inflight: "in flight",
  accepted: "accepted",
  refused: "refused as expected",
  waiting: "waiting on the bank clock"
});

const stepState = (state, ordinal) => {
  const record = state.steps.get(ordinal);
  if (record && record.state && record.state !== "inflight") return record.state;
  if (record && record.state === "inflight") {
    return state.clock ? "waiting" : "inflight";
  }
  return "pending";
};

const health = (state) => {
  const identity = state.identity;
  const section = el("section", "card");
  section.append(el("h2", null, "Local validator"));
  if (!identity) {
    section.append(el("p", "muted", "No bank yet. The daemon publishes this once the ledger is up and the program account is executable."));
    const stages = el("ul", "boot");
    state.boot.forEach((entry) => stages.append(el("li", null, `${entry.stage} — ${entry.text}`)));
    section.append(stages);
    return section;
  }
  const lastSlot = decimalMax(
    [...state.steps.values()].map((record) => record.slot),
    state.clock ? state.clock.slot : "0"
  );
  section.append(
    fields("", [
      ["Loopback RPC", identity.rpc_url],
      ["Ledger", identity.ledger],
      ["Program id", digest(identity.program_id)],
      ["Integer transport", identity.integer_transport || "legacy safe-number compatibility"],
      ["Latest observed slot", numeric(lastSlot)],
      ["Stream", state.connected ? "attached" : "detached"],
      ["Events received", numeric(state.events)]
    ])
  );
  return section;
};

const artifact = (state) => {
  const identity = state.identity;
  const section = el("section", "card");
  section.append(el("h2", null, "Program artifact"));
  if (!identity) {
    section.append(el("p", "muted", "The ELF is built and hashed before the bank starts."));
    return section;
  }
  section.append(
    row(
      "callout callout-warn",
      el("strong", null, "NON-PRODUCTION"),
      el(
        "span",
        null,
        "Built with --features non-production-mock-source. The default production-inert ELF refuses this unregistered V1 Endow with 0x0079 and must never be credited with this walk."
      )
    ),
    fields("", [
      ["Source profile", identity.source_profile],
      ["Path", identity.elf_path],
      ["Bytes", numeric(identity.elf_bytes)],
      ["sha256 (hashed here, from the loaded file)", digest(identity.elf_sha256)]
    ])
  );
  return section;
};

const scope = (state) => {
  const identity = state.identity;
  const section = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Evidence scope"), chip("SBF_EXECUTED"));
  section.append(heading);
  section.append(
    el(
      "p",
      null,
      "Signed, confirmed, committed sequential execution on a local validator from a genesis-assisted prestate. Unpromoted. Not a deployment, not devnet, not mainnet, not a wallet, not an operatorless venue."
    )
  );
  if (identity && identity.genesis_assisted) {
    section.append(
      row(
        "callout",
        el("strong", null, "NOT END TO END"),
        el(
          "span",
          null,
          `${identity.precreated.length} program-owned prerequisite(s) were precreated by local validator genesis. An ordinary wallet could not have created them.`
        )
      )
    );
    const list = el("ul", "precreated");
    identity.precreated.forEach((entry) => list.append(el("li", null, entry)));
    section.append(list);
  }
  return section;
};

const roster = (state) => {
  const section = el("section", "card");
  section.append(el("h2", null, "Signing roster"));
  section.append(
    el(
      "p",
      "muted",
      "Fresh test-only keys, minted into a private temporary directory and unlinked when the daemon exits. Only public keys ever leave the daemon."
    )
  );
  const table = el("table", "roster");
  const head = el("thead");
  const headRow = el("tr");
  headRow.append(el("th", null, "role"), el("th", null, "public key"));
  head.append(headRow);
  const body = el("tbody");
  state.roster.forEach((actor) => {
    const line = el("tr");
    const key = el("td");
    key.append(digest(actor.pubkey));
    line.append(el("td", "role", actor.role), key);
    body.append(line);
  });
  table.append(head, body);
  section.append(table);
  return section;
};

const rail = (state) => {
  const section = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Lifecycle rail"));
  if (state.plan) heading.append(el("span", "count", `${state.plan.steps.length} steps`));
  section.append(heading);
  /* A trade session has no plan and never will: its book is authored at the
   * keyboard.  `fill` drops falsy children, so returning nothing here is how
   * the rail simply is not on that screen, rather than being a card that
   * explains its own absence. */
  if (!state.plan) return null;
  const spine = el("ol", "rail");
  state.plan.steps.forEach((step) => {
    const current = stepState(state, step.ordinal);
    const item = el("li", `rail-step rail-${current}`);
    item.append(
      el("span", "rail-dot", ""),
      el("span", "rail-ordinal", String(step.ordinal).padStart(2, "0")),
      el("span", "rail-name", step.name),
      el("span", "rail-state", STATE_LABEL[current] || current)
    );
    if (step.kind === "refuse") item.append(el("span", "rail-code", step.expect_code_hex || "refusal"));
    spine.append(item);
  });
  section.append(spine);
  return section;
};

export const renderBench = (state) => [health(state), artifact(state), scope(state), roster(state), rail(state)];
