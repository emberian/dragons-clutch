/* Retained local-real Pyth campaign.
 *
 * This screen receives a versioned, daemon-validated projection of the three
 * public transcript files. It has no action controls and no transaction, RPC,
 * wallet, signer, provider API, or key-file code. */

import { digest, el, fields, numeric, row } from "./dom.js";
import { chip } from "./evidence.js";

const SCHEMA = "dragons-clutch/operator/local-real-pyth-transcript/v1";
const CLAIM = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";

const boundary = () => {
  const card = el("section", "card pyth-boundary");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Permanent evidence boundary"), chip("SBF_EXECUTED"));
  card.append(
    heading,
    row(
      "callout callout-warn",
      el("strong", null, "NON-PRODUCTION"),
      el("span", null, "SYNTHETIC OBSERVATION · LOCAL VALIDATOR ONLY · NO VALUE")
    ),
    el(
      "p",
      null,
      "Captured deployed Pyth receiver/router program bytes executed locally with freshly initialized laboratory guardian and Config state. This is not devnet price evidence, provider availability, current deployment evidence, a production source release, a wallet, or browser transaction construction/submission."
    ),
    el(
      "p",
      "muted",
      "The display is an untrusted projection of retained campaign.json, result.json, and probe-evidence.json. The daemon requires their exact truth label, signed step order, well-formed retained identities, rollback closures, seal, and resolution before publishing this screen."
    )
  );
  return card;
};

const pending = () => {
  const card = el("section", "card");
  card.append(
    el("h2", null, "Retained campaign transcript"),
    el("p", "muted", "Waiting for a validated local-real Pyth transcript from the loopback daemon.")
  );
  return card;
};

const valid = (campaign) => Boolean(
  campaign
  && campaign.schema === SCHEMA
  && campaign.claim === CLAIM
  && campaign.retained_transcript === true
  && Array.isArray(campaign.provider)
  && Array.isArray(campaign.steps)
  && Array.isArray(campaign.rollbacks)
);

const unavailable = (campaign) => {
  const card = el("section", "card callout callout-bad");
  card.append(
    el("strong", null, "UNAVAILABLE"),
    el("span", null, campaign ? "The campaign event did not match the recognized transcript schema." : "No campaign event received.")
  );
  return card;
};

const identityCard = (campaign) => {
  const identity = campaign.identity;
  const listener = campaign.listener_evidence;
  const card = el("section", "card");
  card.append(
    el("h2", null, "Retained execution identity"),
    fields("", [
      ["Dragon's Clutch repository HEAD", digest(identity.repository_head)],
      ["Clutch ELF sha256", digest(identity.clutch_elf_sha256)],
      ["Selected validator sha256", digest(identity.validator_binary_sha256)],
      ["Validator build record sha256", digest(identity.validator_build_record_sha256)],
      ["Pyth upstream commit", digest(identity.upstream_pyth_crosschain_commit)],
      ["Source-profile snapshot sha256", digest(identity.source_profile_snapshot_sha256)],
      ["Synthetic signed VAA sha256", digest(identity.vaa_sha256)],
      ["PostUpdate data sha256", digest(identity.post_update_data_sha256)],
      ["Local genesis hash", digest(identity.genesis_hash)],
      ["Warp slot", numeric(identity.warp_slot)]
    ]),
    el("h3", null, "Loopback listener evidence"),
    fields("", [
      ["RPC", listener.rpc],
      ["WebSocket", listener.websocket],
      ["Faucet", listener.faucet],
      ["Gossip", listener.gossip],
      ["Dynamic range", listener.configured_dynamic_port_range],
      ["Probe before sha256", digest(listener.probe_before_sha256)],
      ["Probe after sha256", digest(listener.probe_after_sha256)]
    ]),
    el("p", "muted", listener.scope)
  );
  return card;
};

const providerCard = (campaign) => {
  const card = el("section", "card");
  card.append(
    el("h2", null, "Captured provider account identities"),
    el(
      "p",
      "muted",
      "Complete Upgradeable Loader Program and ProgramData account-body hashes reconstructed and checked by the campaign before any transaction was signed."
    )
  );
  const table = el("table", "provider-table");
  const head = el("thead");
  const headRow = el("tr");
  ["role", "address", "complete account-body sha256", "executable"].forEach((label) => {
    const cell = el("th", null, label);
    cell.scope = "col";
    headRow.append(cell);
  });
  head.append(headRow);
  const body = el("tbody");
  campaign.provider.forEach((provider) => {
    const line = el("tr");
    const address = el("td");
    address.append(digest(provider.address));
    const hash = el("td");
    hash.append(digest(provider.complete_account_body_sha256));
    line.append(
      el("td", "role", provider.role),
      address,
      hash,
      el("td", null, provider.executable ? "yes" : "no")
    );
    body.append(line);
  });
  table.append(head, body);
  card.append(table);
  return card;
};

const sourceCard = (campaign) => {
  const source = campaign.source;
  const outcome = campaign.outcome;
  const card = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Receiver-written source, seal, and resolution"), chip("SBF_EXECUTED"));
  card.append(
    heading,
    fields("", [
      ["Synthetic provider feed id", digest(source.provider_feed_id_hex)],
      ["Price", numeric(source.price)],
      ["Confidence", numeric(source.confidence)],
      ["Exponent", numeric(source.exponent)],
      ["Publish time", numeric(source.publish_time)],
      ["Conservative interval lower", numeric(source.interval_lower)],
      ["Conservative interval upper", numeric(source.interval_upper)],
      ["Receiver-written update", digest(source.update_account)],
      ["Verified VAA account", digest(source.verified_vaa_account)],
      ["Archive sealed", outcome.sealed ? "yes" : "no"],
      ["Resolved categorical payout", numeric(outcome.resolved_payout)]
    ]),
    row(
      "callout callout-ok",
      el("strong", null, "INTERVAL SELECTS CELL 1"),
      el("span", null, `[${numeric(source.interval_lower)}, ${numeric(source.interval_upper)}] was sealed, then Resolve committed payout cell ${numeric(outcome.resolved_payout)}.`)
    ),
    fields("", [
      ["Atomic PostUpdate + Append signature", digest(outcome.joined_post_append_signature)],
      ["Seal signature", digest(outcome.seal_signature)],
      ["Resolve signature", digest(outcome.resolve_signature)]
    ])
  );
  return card;
};

const rollbackCard = (campaign) => {
  const card = el("section", "card");
  card.append(
    el("h2", null, "Atomic rollback negatives"),
    el(
      "p",
      "muted",
      "These are expected on-ledger refusals. A green row means both the provider-side write and the named Clutch/treasury state were checked unchanged after refusal."
    )
  );
  const list = el("ul", "identities");
  campaign.rollbacks.forEach((rollback) => {
    const item = el("li", rollback.ok ? "ok" : "bad");
    item.append(
      el("span", "identity-label", rollback.label),
      el("span", "identity-value", rollback.scope),
      el("span", "identity-verdict", rollback.ok ? "rolled back" : "FAIL")
    );
    list.append(item);
  });
  card.append(list);
  return card;
};

const transactionCard = (campaign) => {
  const card = el("section", "card log");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Exact signed transaction order"), el("span", "count", `${campaign.steps.length} confirmed`));
  card.append(
    heading,
    el(
      "p",
      "muted",
      "Every row is retained in submission order from getTransaction: signature, signed-wire hash, bank slot, compute units, fee, top-level program order, and exact error. Decimal integers remain strings across the daemon/browser boundary."
    )
  );
  campaign.steps.forEach((step) => {
    const refused = step.state === "refused-as-expected";
    const item = el("article", `step kind-${refused ? "refuse" : "accept"}`);
    const head = el("header", "step-head");
    head.append(
      el("span", "step-ordinal", String(step.ordinal).padStart(2, "0")),
      el("span", "step-name", step.label),
      el("span", `step-badge badge-${refused ? "refuse" : "accept"}`, refused ? "refuse" : "accept"),
      el("span", "step-status", step.state)
    );
    item.append(
      head,
      fields("step-fields", [
        ["slot", numeric(step.slot)],
        ["compute units", numeric(step.compute_units_consumed)],
        ["fee lamports", numeric(step.fee_lamports)],
        ["signature", digest(step.signature)],
        ["signed wire sha256", digest(step.signed_wire_sha256)],
        ["top-level program order", step.program_order.join(" → ")]
      ])
    );
    if (refused) {
      item.append(
        row(
          "refusal",
          el("strong", null, "expected SourceAdmissionFailed"),
          el("span", null, JSON.stringify(step.error))
        )
      );
    }
    card.append(item);
  });
  return card;
};

export const renderPyth = (state) => {
  const cards = [boundary()];
  if (!state.pyth) {
    cards.push(pending());
    return cards;
  }
  if (!valid(state.pyth)) {
    cards.push(unavailable(state.pyth));
    return cards;
  }
  cards.push(
    identityCard(state.pyth),
    providerCard(state.pyth),
    rollbackCard(state.pyth),
    sourceCard(state.pyth),
    transactionCard(state.pyth)
  );
  return cards;
};
