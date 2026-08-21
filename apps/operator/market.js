/* Funding, Ticket, and Book: the walk read as a market rather than as a log.
 *
 * Every cell on these three screens is the latest account image the daemon
 * reloaded, decoded through the frozen layout codecs.  Nothing is projected,
 * inferred, or animated forward: a value appears when an account carrying it
 * has actually been read back from the bank, and a role that has not been
 * touched yet says so.
 *
 * These are read-only views of the committed walk.  Interactive Endow, Split,
 * and PlaceOrder builders remain unbuilt; the daemon's action endpoint refuses
 * every write, so nothing here can pretend to be a control it is not. */

import { el, fields, numeric, row } from "./dom.js";

const OWNERS = Object.freeze([
  { label: "founding actor", position: "general-market.position", token: "actor-collateral" },
  { label: "owner B", position: "owner-b.position", token: "owner-b.collateral" },
  { label: "owner C", position: "owner-c.position", token: "owner-c.collateral" },
  { label: "owner D", position: "owner-d.position", token: "owner-d.collateral" }
]);

const RESERVATION_STATE = Object.freeze(["active", "released", "entitled", "consumed"]);
const EPOCH_PHASE = Object.freeze(["open", "frozen", "cleared", "settled", "lapsed"]);
const CANDIDATE_STATUS = Object.freeze({ 0: "submitted", 1: "verified", 2: "selected" });

const decodedOf = (state, role) => {
  const entry = state.latest.get(role);
  return entry ? entry.decoded : null;
};

const pending = (what) => row("callout", el("strong", null, "NOT YET OBSERVED"), el("span", null, what));

/* ------------------------------------------------------------------ */
/* Funding                                                             */
/* ------------------------------------------------------------------ */

const cashBar = (cash, reserved) => {
  const cell = el("div", "cu");
  const total = Math.max(cash, 1);
  const track = el("div", "cu-track");
  const held = el("div", "cu-bar");
  held.style.width = `${((Math.min(reserved, cash) / total) * 100).toFixed(2)}%`;
  held.classList.add("cu-hot");
  track.append(held);
  cell.append(track, el("span", "cu-value", `${numeric(reserved)} reserved of ${numeric(cash)} cash`));
  return cell;
};

export const renderFunding = (state) => {
  const cards = [];
  const custody = decodedOf(state, "general-market.hoard-token");
  const hoard = decodedOf(state, "general-market.hoard");
  const supply = decodedOf(state, "general-market.supply");

  const pool = el("section", "card");
  pool.append(el("h2", null, "Pooled custody"));
  pool.append(
    el(
      "p",
      "muted",
      "One Token-2022 account holds every endowed atom. Locked backing is the Hoard's own ledger of what is currently minted against; the difference is trading cash still inside custody."
    )
  );
  if (custody || hoard) {
    pool.append(
      fields("", [
        ["custody token amount", numeric(custody ? custody.amount : null)],
        ["locked backing", numeric(hoard ? hoard.collateral_atoms : null)],
        [
          "free cash in custody",
          numeric(custody && hoard ? custody.amount - hoard.collateral_atoms : null)
        ],
        ["internal supply", supply ? supply.internal_supply.join(" / ") : "—"],
        ["external supply", supply ? supply.external_supply.join(" / ") : "—"]
      ])
    );
  } else {
    pool.append(pending("the Hoard and its custody token"));
  }
  cards.push(pool);

  const table = el("section", "card");
  table.append(el("h2", null, "Owner funding"));
  const grid = el("table");
  const head = el("thead");
  const headRow = el("tr");
  ["owner", "collateral wallet", "position cash", "reserved", "free", "eggs[0]", "eggs[1]"].forEach((label) =>
    headRow.append(el("th", null, label))
  );
  head.append(headRow);
  const body = el("tbody");
  OWNERS.forEach((owner) => {
    const position = decodedOf(state, owner.position);
    const wallet = decodedOf(state, owner.token);
    const line = el("tr");
    line.append(
      el("td", "role", owner.label),
      el("td", null, numeric(wallet ? wallet.amount : null)),
      el("td", null, numeric(position ? position.cash_atoms : null)),
      el("td", null, numeric(position ? position.reserved_cash_atoms : null)),
      el("td", null, numeric(position ? position.free_cash_atoms : null)),
      el("td", null, numeric(position ? position.internal[0] : null)),
      el("td", null, numeric(position ? position.internal[1] : null))
    );
    body.append(line);
  });
  grid.append(head, body);
  table.append(grid);
  OWNERS.forEach((owner) => {
    const position = decodedOf(state, owner.position);
    if (!position) return;
    const strip = el("div", "owner-bar");
    strip.append(el("span", "decoded-role", owner.label), cashBar(position.cash_atoms, position.reserved_cash_atoms));
    table.append(strip);
  });
  cards.push(table);
  return cards;
};

/* ------------------------------------------------------------------ */
/* Ticket                                                              */
/* ------------------------------------------------------------------ */

const bookOrders = (state) => {
  const page = decodedOf(state, "general.page");
  return page && Array.isArray(page.orders) ? page.orders : [];
};

const ticketCard = (role, reservation, order) => {
  const card = el("article", `step kind-${reservation.side === "buy" ? "accept" : "refuse"}`);
  const head = el("header", "step-head");
  head.append(
    el("span", "step-name", role),
    el("span", `step-badge badge-${reservation.side === "buy" ? "accept" : "refuse"}`, reservation.side),
    el("span", "step-status", RESERVATION_STATE[reservation.state] || `state ${reservation.state}`)
  );
  card.append(head);

  const facts = [
    ["order id", reservation.order_id],
    ["owner", reservation.owner],
    ["reserved cash", numeric(reservation.initial_cash_atoms)],
    ["remaining cash", numeric(reservation.remaining_cash_atoms)],
    ["reserved eggs", reservation.initial_internal.join(" / ")],
    ["remaining eggs", reservation.remaining_internal.join(" / ")]
  ];
  if (order && order.kind === "single") {
    facts.push(
      ["limit", numeric(order.limit)],
      ["quantity", numeric(order.quantity)],
      ["outcome", String(order.outcome)],
      ["minimum fill", numeric(order.minimum_fill)]
    );
  }
  if (order && order.kind === "portfolio") {
    facts.push(
      ["coefficients", order.coefficients.join(" / ")],
      ["lots", numeric(order.lots)],
      ["limit collateral per lot", numeric(order.limit_collateral_per_lot)],
      ["minimum fill lots", numeric(order.minimum_fill_lots)]
    );
  }
  card.append(fields("step-fields", facts));
  if (!order) {
    card.append(
      row("refusal", el("strong", null, "no live book slot"), el("span", null, "the order was retired, or the page has not been reloaded since"))
    );
  }
  return card;
};

export const renderTicket = (state) => {
  const orders = bookOrders(state);
  const section = el("section", "card");
  section.append(el("h2", null, "Limit tickets"));
  section.append(
    el(
      "p",
      "muted",
      "One card per reservation the walk created, joined to its live book slot by order id. A ticket is what the venue actually holds against an order: the cash or eggs escrowed, and what remains of them."
    )
  );
  const roles = [...state.latest.keys()].filter((role) => role.includes("reservation-")).sort();
  if (!roles.length) {
    section.append(pending("any reservation"));
    return [section];
  }
  roles.forEach((role) => {
    const reservation = decodedOf(state, role);
    if (!reservation || reservation.kind !== "reservation") return;
    const order = orders.find((entry) => entry.order_id === reservation.order_id) || null;
    section.append(ticketCard(role, reservation, order));
  });
  return [section];
};

/* ------------------------------------------------------------------ */
/* Book                                                                */
/* ------------------------------------------------------------------ */

const orderRow = (order) => {
  const line = el("tr");
  const detail =
    order.kind === "single"
      ? `outcome ${order.outcome} · qty ${numeric(order.quantity)} · limit ${numeric(order.limit)}`
      : order.kind === "portfolio"
        ? `coeff [${order.coefficients.join(", ")}] · lots ${numeric(order.lots)} · limit/lot ${numeric(order.limit_collateral_per_lot)}`
        : `retired at generation ${numeric(order.retired_generation)}`;
  line.append(
    el("td", null, String(order.slot)),
    el("td", "role", order.kind),
    el("td", null, order.side || "—"),
    el("td", null, order.owner),
    el("td", null, order.order_id),
    el("td", null, detail)
  );
  return line;
};

export const renderBook = (state) => {
  const cards = [];
  const epoch = decodedOf(state, "general.epoch");
  const window = decodedOf(state, "general.window");
  const candidate = decodedOf(state, "general.candidate");
  const page = decodedOf(state, "general.page");

  const header = el("section", "card");
  header.append(el("h2", null, "Epoch"));
  if (epoch || window) {
    header.append(
      fields("", [
        ["epoch index", numeric(epoch ? epoch.epoch_index : null)],
        ["phase", epoch ? EPOCH_PHASE[epoch.phase] || `phase ${epoch.phase}` : "—"],
        ["orders", numeric(epoch ? epoch.order_count : null)],
        ["distinct owners", numeric(epoch ? epoch.owner_count : null)],
        ["price scale", numeric(epoch ? epoch.price_scale : null)],
        ["freeze deadline slot", numeric(window ? window.freeze_deadline_slot : null)],
        ["selection deadline slot", numeric(window ? window.selection_deadline_slot : null)],
        ["selected slot", numeric(window ? window.selected_slot : null)]
      ])
    );
  } else {
    header.append(pending("the epoch and its window"));
  }
  cards.push(header);

  const book = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Order page"));
  if (page) heading.append(el("span", "count", `${page.order_count} live · ${page.tombstone_count} retired`));
  book.append(heading);
  if (!page) {
    book.append(pending("the order page"));
  } else {
    const grid = el("table");
    const head = el("thead");
    const headRow = el("tr");
    ["slot", "kind", "side", "owner", "order id", "terms"].forEach((label) => headRow.append(el("th", null, label)));
    head.append(headRow);
    const body = el("tbody");
    page.orders.forEach((order) => body.append(orderRow(order)));
    grid.append(head, body);
    book.append(grid);
    if (page.frozen) {
      book.append(row("callout", el("strong", null, "FROZEN"), el("span", null, "the page no longer admits placements")));
    }
  }
  cards.push(book);

  const selected = el("section", "card");
  selected.append(el("h2", null, "Candidate"));
  if (!candidate) {
    selected.append(pending("a candidate record"));
  } else {
    selected.append(
      fields("", [
        ["status", CANDIDATE_STATUS[candidate.status] || `status ${candidate.status}`],
        ["prices", candidate.prices.join(" / ")],
        ["virtual split", numeric(candidate.virtual_split)],
        ["virtual merge", numeric(candidate.virtual_merge)],
        ["distinct owners", numeric(candidate.distinct_owners)],
        ["orders in candidate", numeric(candidate.order_len)],
        ["submitted slot", numeric(candidate.submitted_slot)]
      ])
    );
  }
  cards.push(selected);
  return cards;
};
