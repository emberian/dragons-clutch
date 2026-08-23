/* Live local-real Pyth supervision.
 *
 * This is intentionally not the retained campaign reader and not the Friday
 * ticket. The daemon started a clean-HEAD loopback child; this screen renders
 * its versioned live events and has no action capable of starting, signing,
 * reordering, or extending the campaign. */

import { digest, el, fields, numeric, row } from "./dom.js";
import { chip } from "./evidence.js";

const phaseCard = (state) => {
  const run = state.liveRun;
  const section = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Live real-Pyth loopback campaign"), chip(run?.phase === "passed" ? "SBF_EXECUTED" : "IN_FLIGHT"));
  section.append(heading);
  if (!run) {
    section.append(el("p", "muted", "Waiting for the supervised child boundary."));
    return section;
  }
  section.append(
    row(
      "callout callout-warn",
      el("strong", null, "LIVE, NOT RETAINED"),
      el("span", null, "The daemon is supervising the real captured router/receiver laboratory now. This is not a transcript replay, provider availability, devnet, mainnet, or a wallet.")
    ),
    fields("", [
      ["Phase", run.phase],
      ["Campaign", run.campaign_mode],
      ["Loopback RPC", run.rpc_url],
      ["Loopback WebSocket", run.websocket_url],
      ["Loopback faucet", run.faucet],
      ["Loopback gossip", run.gossip],
      ["Dynamic port range", run.dynamic_port_range],
      ["Retained transcript", run.retained_transcript ? "yes" : "no"],
      ["Browser authority", run.authority]
    ])
  );
  return section;
};

const identityCard = (state) => {
  const identity = state.identity;
  const manifest = state.liveManifest;
  const section = el("section", "card");
  section.append(el("h2", null, "Live build and provider identity"));
  if (!identity || !manifest) {
    section.append(el("p", "muted", "The clean-source gate and build are still running. No ELF identity has been admitted yet."));
    return section;
  }
  section.append(
    fields("", [
      ["Source profile", identity.source_profile],
      ["Repository HEAD", digest(identity.repository_head)],
      ["Clutch ELF sha256", digest(identity.elf_sha256)],
      ["Validator sha256", digest(identity.validator_binary_sha256)],
      ["Source-profile snapshot sha256", digest(identity.source_profile_snapshot_sha256)],
      ["Program id", digest(identity.program_id)],
      ["Genesis prerequisites", numeric(String(manifest.genesis_prerequisite_roles.length))],
      ["Provider loader accounts", numeric(String(manifest.provider.length))],
      ["Evidence scope", identity.evidence_scope]
    ]),
    el("p", "muted", "The browser receives public identities only. Ephemeral payer and owner keys remain inside the supervised runner's private temporary directory and are removed by that runner.")
  );
  return section;
};

const sourceCard = (result, evidenceScope) => {
  const archive = result.source_archive;
  const rollback = result.out_of_order_boundary_rollback;
  const section = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Authenticated two-boundary source"), chip(evidenceScope));
  section.append(
    heading,
    fields("", [
      ["Boundaries", numeric(result.boundary_count)],
      ["Signed transactions", numeric(result.step_count)],
      ["Archive", digest(archive.key)],
      ["Archive owner", digest(archive.owner)],
      ["Exact account bytes", numeric(archive.data_len)],
      ["Complete body sha256", digest(archive.body_sha256)],
      ["Page commitment", digest(archive.page_commitment)],
      ["Resolved payout", numeric(result.resolved_payout)],
      ["Trade", result.trade_status]
    ])
  );

  const table = el("table", "roster");
  const head = el("thead");
  const headRow = el("tr");
  for (const label of ["index", "bucket", "lower", "upper", "sequence", "write slot", "publish time"]) {
    const cell = el("th", null, label);
    cell.scope = "col";
    headRow.append(cell);
  }
  head.append(headRow);
  const body = el("tbody");
  result.archive_records.forEach((record) => {
    const line = el("tr");
    for (const value of [record.index, record.bucket, record.lower, record.upper, record.sequence, record.write_slot, record.publish_time]) {
      line.append(el("td", null, numeric(value)));
    }
    body.append(line);
  });
  table.append(head, body);
  section.append(
    table,
    row(
      "callout",
      el("strong", null, "OUT-OF-ORDER REFUSAL CLOSED"),
      el("span", null, `Skipped receiver account ${rollback.skipped_update_account} remained absent; archive and treasury full-state hashes are equal at ${rollback.before_snapshot_sha256}.`)
    )
  );
  return section;
};

const terminalCard = (result) => {
  const terminal = result.terminal;
  const section = el("section", "card");
  section.append(
    el("h2", null, "Terminal conservation"),
    fields("", [
      ["Collateral atoms", numeric(result.collateral_atoms)],
      ["Buyer returned", numeric(terminal.buyer_token_atoms)],
      ["Seller returned", numeric(terminal.seller_token_atoms)],
      ["Hoard token residue", numeric(terminal.hoard_token_atoms)],
      ["Hoard liability residue", numeric(terminal.hoard_collateral_atoms)],
      ["Buyer cash residue", numeric(terminal.buyer_position_cash_atoms)],
      ["Seller cash residue", numeric(terminal.seller_position_cash_atoms)]
    ]),
    el("p", "muted", "These are live producer-validated terminal account facts from the same child process. The page does not independently query RPC and does not promote them into retained evidence.")
  );
  return section;
};

const outputCard = (state) => {
  const section = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Allowlisted campaign progress"), el("span", "count", `${state.liveOutput.length} lines`));
  const output = el("ol", "live-log");
  state.liveOutput.forEach((entry) => {
    output.append(el("li", null, `${entry.sequence} · ${entry.text}`));
  });
  section.append(
    heading,
    el("p", "muted", "Only structurally allowlisted milestones, waits, and transaction results cross this boundary. Stderr, filesystem paths, arbitrary child text, and retained result JSON remain process-local."),
    output
  );
  return section;
};

export const renderLivePyth = (state) => {
  const cards = [phaseCard(state), identityCard(state)];
  if (state.liveResult) {
    const scope = state.identity?.evidence_scope === "SBF_EXECUTED" ? "SBF_EXECUTED" : "IN_FLIGHT";
    cards.push(sourceCard(state.liveResult, scope), terminalCard(state.liveResult));
  }
  cards.push(outputCard(state));
  if (state.fault) {
    cards.unshift(row("callout callout-stop", el("strong", null, "STOP"), el("span", null, state.fault.text)));
  }
  return cards;
};
