/* Retained local-real Pyth campaign.
 *
 * This screen receives a versioned, daemon-validated projection of the three
 * public transcript files. It has no action controls and no transaction, RPC,
 * wallet, signer, provider API, or key-file code. */

import { digest, el, fields, numeric, row } from "./dom.js";
import { chip } from "./evidence.js";

const SOURCE_V1_SCHEMA = "dragons-clutch/operator/local-real-pyth-transcript/v1";
const SOURCE_V2_SCHEMA = "dragons-clutch/operator/local-real-pyth-transcript/v2";
const JOINED_V2_SCHEMA = "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v2";
const JOINED_V4_SCHEMA = "dragons-clutch/operator/local-real-pyth-joined-lifecycle/v4";
const SOURCE_ONLY_MODE = "source-only-v1";
const JOINED_LIFECYCLE_MODE = "joined-user-lifecycle-v1";
const TRADE_BLOCKER = "missing-sealed-price-grid-and-epoch-plane";
const CLAIM = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";

const currentSourceIsPresentable = (source) => Boolean(
  source
  && source.registered_source_plane_count === "1"
  && typeof source.wrong_feed_verified_vaa_account === "string"
  && source.wrong_feed_verified_vaa_account.length > 0
  && source.wrong_feed_observation
  && typeof source.wrong_feed_observation === "object"
  && source.freshness
  && typeof source.freshness === "object"
  && typeof source.freshness.scope === "string"
  && typeof source.freshness.append_age_seconds === "string"
  && typeof source.freshness.final_age_seconds === "string"
);

const boundary = () => {
  const card = el("section", "card pyth-boundary");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Permanent evidence boundary"), chip("SBF_EXECUTED"));
  card.append(
    heading,
    row(
      "callout",
      el("strong", null, "READ-ONLY RETAINED TRANSCRIPT"),
      el("span", null, "No live validator or RPC is attached. This screen cannot trade, extend, replay, refresh, or re-read the recorded campaign.")
    ),
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
      "The display is an untrusted, read-only projection of retained campaign.json, result.json, and probe-evidence.json. The daemon requires their exact truth label, signed step order, well-formed retained identities, rollback closures, seal, and resolution before publishing this screen. It does not re-read the chain."
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

export const campaignIsPresentable = (campaign) => Boolean(
  campaign
  && (
    (
      campaign.schema === SOURCE_V1_SCHEMA
      && campaign.campaign_mode === SOURCE_ONLY_MODE
    )
    || (
      campaign.schema === SOURCE_V2_SCHEMA
      && campaign.campaign_mode === SOURCE_ONLY_MODE
      && currentSourceIsPresentable(campaign.source)
    )
    || (
      campaign.schema === JOINED_V2_SCHEMA
      && campaign.campaign_mode === JOINED_LIFECYCLE_MODE
      && campaign.lifecycle
      && typeof campaign.lifecycle === "object"
    )
    || (
      campaign.schema === JOINED_V4_SCHEMA
      && campaign.campaign_mode === JOINED_LIFECYCLE_MODE
      && campaign.lifecycle
      && typeof campaign.lifecycle === "object"
      && currentSourceIsPresentable(campaign.source)
    )
  )
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
  const wrongFeed = source.wrong_feed_observation;
  const freshness = source.freshness;
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
      ...(source.registered_source_plane_count === "1" ? [
        ["Registered source planes", numeric(source.registered_source_plane_count)],
        ["Wrong-feed Verified VAA account (producer-attested)", digest(source.wrong_feed_verified_vaa_account)],
        ["Wrong-feed provider id", digest(wrongFeed.provider_feed_id_hex)],
        ["Wrong-feed VAA sha256", digest(wrongFeed.vaa_sha256)],
        ["Wrong-feed PostUpdate sha256", digest(wrongFeed.post_update_data_sha256)],
        ["Wrong-feed Merkle update sha256", digest(wrongFeed.merkle_price_update_sha256)],
        ["Append-time Clock slot", numeric(freshness.append_clock.slot)],
        ["Append-time Clock timestamp", numeric(freshness.append_clock.unix_timestamp)],
        ["Append-time source age seconds", numeric(freshness.append_age_seconds)],
        ["Final Clock slot", numeric(freshness.final_clock.slot)],
        ["Final Clock timestamp", numeric(freshness.final_clock.unix_timestamp)],
        ["Final source age seconds (informational)", numeric(freshness.final_age_seconds)],
      ] : []),
      ["Archive sealed", outcome.sealed ? "yes" : "no"],
      ["Resolved categorical payout", numeric(outcome.resolved_payout)]
    ]),
    ...(freshness ? [el("p", "muted", freshness.scope)] : []),
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

const lifecycleCardV2 = (campaign) => {
  const lifecycle = campaign.lifecycle;
  if (!lifecycle) return null;
  const terminal = lifecycle.terminal;
  const trade = lifecycle.trade;
  const card = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Signed user lifecycle"), chip("SBF_EXECUTED"));
  card.append(
    heading,
    row(
      "callout callout-warn",
      el("strong", null, "NON-PRODUCTION · LOCAL ONLY"),
      el("span", null, "Ephemeral local signer and synthetic source; no wallet, browser transaction building, or value.")
    ),
    fields("", [
      ["Campaign mode", campaign.campaign_mode],
      ["Market genesis-assisted", lifecycle.market_genesis_assisted ? "yes" : "no — signed CreateMarket"],
      ["Market", digest(lifecycle.market)],
      ["Ephemeral user", digest(lifecycle.ephemeral_user)],
      ["User collateral token", digest(lifecycle.user_collateral_token)],
      ["Exact collateral atoms", numeric(lifecycle.collateral_atoms)],
      ["CreateMarket signature", digest(lifecycle.create_market_signature)],
      ["Endow signature", digest(lifecycle.endow_signature)],
      ["Split signature", digest(lifecycle.split_signature)],
      ["WithdrawCash signature", digest(lifecycle.withdraw_signature)]
    ]),
    el("h3", null, "RedeemInternal by outcome")
  );
  const redemptionTable = el("table", "provider-table");
  const redemptionHead = el("thead");
  const redemptionHeadRow = el("tr");
  ["outcome", "quantity", "payout atoms", "signature"].forEach((label) => {
    const cell = el("th", null, label);
    cell.scope = "col";
    redemptionHeadRow.append(cell);
  });
  redemptionHead.append(redemptionHeadRow);
  const redemptionBody = el("tbody");
  lifecycle.redeem_internal.forEach((redeem) => {
    const line = el("tr");
    const signature = el("td");
    signature.append(digest(redeem.signature));
    line.append(
      el("td", null, numeric(redeem.outcome)),
      el("td", null, numeric(redeem.quantity)),
      el("td", null, numeric(redeem.payout_atoms)),
      signature
    );
    redemptionBody.append(line);
  });
  redemptionTable.append(redemptionHead, redemptionBody);
  card.append(
    redemptionTable,
    el("h3", null, "Terminal exact conservation"),
    fields("", [
      ["Position cash atoms", numeric(terminal.position_cash_atoms)],
      ["Position internal outcomes", terminal.position_internal.map(numeric).join(" · ")],
      ["Supply internal outcomes", terminal.supply_internal.map(numeric).join(" · ")],
      ["Hoard collateral obligation atoms", numeric(terminal.hoard_collateral_atoms)],
      ["Hoard token atoms", numeric(terminal.hoard_token_atoms)],
      ["User token atoms returned", numeric(terminal.user_token_atoms)]
    ]),
    row(
      "callout callout-warn",
      el("strong", null, "TRADE BLOCKED / NOT SUBSTITUTED"),
      el("span", null, `${TRADE_BLOCKER}: ${trade.detail}`)
    )
  );
  return card;
};

const lifecycleCardV4 = (campaign) => {
  const lifecycle = campaign.lifecycle;
  const terminal = lifecycle.terminal;
  const trade = lifecycle.trade;
  const funding = trade.second_owner_account_creation_funding;
  const orders = trade.orders;
  const post = trade.post_settlement;
  const card = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Signed two-owner lifecycle and settled trade"), chip("SBF_EXECUTED"));
  card.append(
    heading,
    row(
      "callout callout-warn",
      el("strong", null, "NON-PRODUCTION · LOCAL ONLY"),
      el("span", null, "Two ephemeral local signers and a synthetic source; no wallet, browser transaction building, or value.")
    ),
    row(
      "callout callout-ok",
      el("strong", null, "TRADE SETTLED / NOT SUBSTITUTED"),
      el("span", null, "The exact retained relation admitted the best valid submitted candidate; this is not a claim of optimal clearing.")
    ),
    fields("", [
      ["Campaign mode", campaign.campaign_mode],
      ["Market genesis-assisted", lifecycle.market_genesis_assisted ? "yes" : "no — signed CreateMarket"],
      ["Market", digest(lifecycle.market)],
      ["Buyer", digest(lifecycle.ephemeral_users[0])],
      ["Seller", digest(lifecycle.ephemeral_users[1])],
      ["Buyer collateral token", digest(lifecycle.user_collateral_tokens[0])],
      ["Seller collateral token", digest(lifecycle.user_collateral_tokens[1])],
      ["Exact combined collateral atoms", numeric(lifecycle.collateral_atoms)],
      ["CreateMarket signature", digest(lifecycle.create_market_signature)],
      ["Buyer Endow signature", digest(lifecycle.buyer_endow_signature)],
      ["Seller Endow signature", digest(lifecycle.seller_endow_signature)],
      ["Seller Split signature", digest(lifecycle.split_signature)],
      ["Buyer WithdrawCash signature", digest(lifecycle.buyer_withdraw_signature)],
      ["Seller WithdrawCash signature", digest(lifecycle.seller_withdraw_signature)]
    ]),
    el("h3", null, "Signed artifact, epoch, and candidate plane"),
    fields("", [
      ["PriceGrid", digest(trade.price_grid)],
      ["PriceGrid digest", digest(trade.price_grid_digest)],
      ["PriceGrid upload signatures", numeric(String(trade.grid_upload_signatures.length))],
      ["General policy upload signatures", numeric(String(trade.policy_upload_signatures.length))],
      ["Grid genesis-assisted", trade.grid_genesis_assisted ? "yes" : "no"],
      ["Epoch genesis-assisted", trade.epoch_genesis_assisted ? "yes" : "no"],
      ["Order genesis-assisted", trade.order_genesis_assisted ? "yes" : "no"],
      ["Candidate genesis-assisted", trade.candidate_genesis_assisted ? "yes" : "no"],
      ["Second-owner creation funding lamports", numeric(funding.lamports)],
      ["Second-owner funding signature", digest(funding.signature)],
      ["Second-owner funding genesis-assisted", funding.genesis_assisted ? "yes" : "no"],
      ["Epoch", digest(trade.epoch)],
      ["Epoch id", digest(trade.epoch_id)],
      ["InitEpoch signature", digest(trade.init_epoch_signature)],
      ["FreezeEpoch signature", digest(trade.freeze_epoch_signature)],
      ["Candidate", digest(trade.candidate)],
      ["Exact simplex prices", trade.prices.map(numeric).join(" · ")],
      ["Exact fills", trade.fills.map(numeric).join(" · ")],
      ["Witness slices", numeric(trade.witness_slices)],
      ["SubmitCandidate signature", digest(trade.submit_signature)],
      ["CompleteClearWork signature", digest(trade.complete_verification_signature)],
      ["FinalizeSelection signature", digest(trade.selection_signature)],
      ["FreezeEntitlement signature", digest(trade.freeze_entitlement_signature)],
      ["Entitle signature", digest(trade.entitle_signature)],
      ["Settle signature", digest(trade.settlement_signature)]
    ]),
    el("h3", null, "Exact funded book")
  );

  const orderTable = el("table", "provider-table");
  const orderHead = el("thead");
  const orderHeadRow = el("tr");
  ["owner", "side", "outcome", "quantity", "limit", "signature"].forEach((label) => {
    const cell = el("th", null, label);
    cell.scope = "col";
    orderHeadRow.append(cell);
  });
  orderHead.append(orderHeadRow);
  const orderBody = el("tbody");
  [
    [trade.owners[0], orders.buyer, orders.buyer_signature],
    [trade.owners[1], orders.seller, orders.seller_signature]
  ].forEach(([owner, order, signatureValue]) => {
    const line = el("tr");
    const ownerCell = el("td");
    const signatureCell = el("td");
    ownerCell.append(digest(owner));
    signatureCell.append(digest(signatureValue));
    line.append(
      ownerCell,
      el("td", null, order.side),
      el("td", null, numeric(order.outcome)),
      el("td", null, numeric(order.quantity)),
      el("td", null, numeric(order.limit)),
      signatureCell
    );
    orderBody.append(line);
  });
  orderTable.append(orderHead, orderBody);
  card.append(
    orderTable,
    el("h3", null, "Post-settlement state"),
    fields("", [
      ["Buyer cash", numeric(post.buyer_cash)],
      ["Buyer internal outcomes", post.buyer_internal.map(numeric).join(" · ")],
      ["Seller cash", numeric(post.seller_cash)],
      ["Seller internal outcomes", post.seller_internal.map(numeric).join(" · ")],
      ["Locked collateral", numeric(post.locked_collateral)],
      ["Pooled custody", numeric(post.pooled_custody)]
    ]),
    el("h3", null, "Owner-bound RedeemInternal rows")
  );

  const redemptionTable = el("table", "provider-table");
  const redemptionHead = el("thead");
  const redemptionHeadRow = el("tr");
  ["owner", "outcome", "quantity", "payout atoms", "signature"].forEach((label) => {
    const cell = el("th", null, label);
    cell.scope = "col";
    redemptionHeadRow.append(cell);
  });
  redemptionHead.append(redemptionHeadRow);
  const redemptionBody = el("tbody");
  lifecycle.redeem_internal.forEach((redeem) => {
    const line = el("tr");
    const owner = el("td");
    const signature = el("td");
    owner.append(digest(redeem.owner));
    signature.append(digest(redeem.signature));
    line.append(
      owner,
      el("td", null, numeric(redeem.outcome)),
      el("td", null, numeric(redeem.quantity)),
      el("td", null, numeric(redeem.payout_atoms)),
      signature
    );
    redemptionBody.append(line);
  });
  redemptionTable.append(redemptionHead, redemptionBody);
  card.append(
    redemptionTable,
    el("h3", null, "Terminal exact two-owner conservation"),
    fields("", [
      ["Buyer position cash atoms", numeric(terminal.buyer_position_cash_atoms)],
      ["Buyer position internal outcomes", terminal.buyer_position_internal.map(numeric).join(" · ")],
      ["Seller position cash atoms", numeric(terminal.seller_position_cash_atoms)],
      ["Seller position internal outcomes", terminal.seller_position_internal.map(numeric).join(" · ")],
      ["Supply internal outcomes", terminal.supply_internal.map(numeric).join(" · ")],
      ["Hoard collateral obligation atoms", numeric(terminal.hoard_collateral_atoms)],
      ["Hoard token atoms", numeric(terminal.hoard_token_atoms)],
      ["Buyer token atoms returned", numeric(terminal.buyer_token_atoms)],
      ["Seller token atoms returned", numeric(terminal.seller_token_atoms)]
    ])
  );
  return card;
};

const lifecycleCard = (campaign) => {
  if (!campaign.lifecycle) return null;
  return campaign.schema === JOINED_V4_SCHEMA
    ? lifecycleCardV4(campaign)
    : lifecycleCardV2(campaign);
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
  if (!campaignIsPresentable(state.pyth)) {
    cards.push(unavailable(state.pyth));
    return cards;
  }
  cards.push(identityCard(state.pyth), providerCard(state.pyth));
  const lifecycle = lifecycleCard(state.pyth);
  if (lifecycle) cards.push(lifecycle);
  cards.push(rollbackCard(state.pyth), sourceCard(state.pyth), transactionCard(state.pyth));
  return cards;
};
