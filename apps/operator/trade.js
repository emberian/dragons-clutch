/* Trade mode: the Clutch, the Ticket, the Book, the Settlement.
 *
 * The rule that governs every number on these four screens is the same one
 * the honesty strip states: a value is either something the daemon *decoded
 * out of an account image the bank wrote*, or it is a proposal, and a proposal
 * carries a MODEL-ONLY chip. There is no third category. The painter, the
 * order preview, the implied forward and the resolution weights are all
 * proposals until a step row says the bank took them.
 *
 * The opponent is a fixed-belief automaton. It is called that here, in those
 * words, wherever it appears, because it is not a model and not an AI and the
 * demonstration is worthless if anyone thinks it is one. */

import { act } from "./action.js";
import { digest, el, fields, numeric, row } from "./dom.js";
import { chip } from "./evidence.js";

const KNOT_LABEL = Object.freeze(["$100", "$120", "$140", "$160", "$180", "$200", "$220", "$240"]);
const EPOCH_PHASE = Object.freeze(["open", "frozen", "cleared", "settled", "lapsed"]);
const RESERVATION_STATE = Object.freeze(["active", "released", "entitled", "consumed"]);

const decodedOf = (state, role) => {
  const entry = state.latest.get(role);
  return entry ? entry.decoded : null;
};

const pending = (what) =>
  row("callout", el("strong", null, "NOT YET OBSERVED"), el("span", null, what));

/* A table with a header row. Built as elements rather than as markup, like
 * everything else here, so a role name or a refusal can only ever be text. */
const table = (labels, lines) => {
  const grid = el("table");
  const head = el("thead");
  const header = el("tr");
  labels.forEach((label) => header.append(el("th", null, label)));
  head.append(header);
  const body = el("tbody");
  lines.forEach((line) => body.append(line));
  grid.append(head, body);
  return grid;
};

/* A proposal's label. Every caller that renders an unsubmitted number goes
 * through here, so "is this thing committed?" is one grep rather than a habit. */
const modelOnly = (what) => {
  const strip = el("div", "callout");
  strip.append(chip("MODEL_ONLY"), el("span", null, what));
  return strip;
};

const button = (label, className, handler) => {
  const node = el("button", className || "tab", label);
  node.type = "button";
  node.addEventListener("click", handler);
  return node;
};

/* Where the page keeps the two things that are genuinely local: which knot the
 * ticket is pointed at, and the eight numbers currently under the painter's
 * fingers. Neither is evidence and neither is ever sent anywhere until a
 * button is pressed. */
const ticket = {
  knot: 3,
  side: "buy",
  quantity: 500,
  limit: 6000,
  belief: [1250, 1250, 1250, 1250, 1250, 1250, 1250, 1250],
  tab: "single",
  proposal: null,
  notice: null
};

let repaint = () => {};
export const bindRepaint = (fn) => {
  repaint = fn;
};

const post = async (request, note) => {
  ticket.notice = { pending: true, text: note };
  repaint();
  const reply = await act(request);
  ticket.notice = {
    pending: false,
    ok: reply.ok === true,
    text: reply.ok === true ? `${note}: the bank accepted it` : String(reply.detail || "refused")
  };
  repaint();
  return reply;
};

/* ------------------------------------------------------------------ */
/* The hat row                                                         */
/* ------------------------------------------------------------------ */

const restingAt = (state, knot) => {
  const orders = state.session ? state.session.orders : [];
  return orders.filter((order) => order.outcome === knot && !order.retired);
};

/* One cell per knot: the automaton's resting quote, the person's own resting
 * orders, and — once the bank has selected a candidate — the cleared price.
 *
 * This is the degree-1 basis made clickable. Each knot is one hat, the ticket
 * points at exactly one of them, and the row is the same eight columns the
 * belief painter and the Book's overlay use, so the three screens are visibly
 * the same object seen three ways. */
const hatRow = (state, onPick) => {
  const grid = el("div", "hat-row");
  const cleared = state.clearing ? state.clearing.prices : null;
  for (let knot = 0; knot < 8; knot += 1) {
    const cell = el("button", knot === ticket.knot ? "hat hat-on" : "hat");
    cell.type = "button";
    cell.append(el("span", "hat-knot", KNOT_LABEL[knot]));
    const mine = restingAt(state, knot).filter((order) => order.owner === "human");
    const theirs = restingAt(state, knot).filter((order) => order.owner === "bot");
    const quote = theirs[0];
    cell.append(
      el(
        "span",
        "hat-quote",
        quote ? `${quote.side} ${numeric(quote.limit)}` : "no automaton quote"
      )
    );
    cell.append(el("span", "hat-mine", mine.length ? `${mine.length} of yours` : "—"));
    if (cleared) cell.append(el("span", "hat-cleared", `cleared ${numeric(cleared[knot])}`));
    cell.addEventListener("click", () => onPick(knot));
    grid.append(cell);
  }
  return grid;
};

/* ------------------------------------------------------------------ */
/* The Clutch                                                          */
/* ------------------------------------------------------------------ */

export const renderClutch = (state) => {
  const cards = [];
  const market = state.market;
  const epoch = decodedOf(state, "friday.epoch");
  const window = decodedOf(state, "friday.window");

  const head = el("section", "card");
  head.append(el("h2", null, "The Friday clutch"));
  head.append(
    el(
      "p",
      "muted",
      "Eight hats on a $100–$240 knot grid, degree 1, cleared under the frozen general-clearing policy. The market was founded here by a signed CreateMarket against a fresh local ledger; nothing about it is injected bank state except the Realm prerequisites the banner enumerates."
    )
  );
  if (!market) {
    head.append(pending("the founded market"));
  } else {
    head.append(
      fields("", [
        ["market", digest(market.market)],
        ["terms", digest(market.terms)],
        ["basis degree", numeric(market.basis_degree)],
        ["knots", market.knots_cents.map((cents) => `$${cents / 100}`).join(" · ")],
        ["outcomes", numeric(market.outcome_count)],
        ["price scale", numeric(market.price_scale)],
        ["limit ladder step", numeric(market.ladder_step)],
        ["statistic", `STAT-TERMINAL-0${market.statistic_id}`],
        ["edge policy", `EDGE-CLAMP-0${market.edge_policy_id}`]
      ])
    );
  }
  cards.push(head);

  const who = el("section", "card");
  who.append(el("h2", null, "Who is at the table"));
  if (!market) {
    who.append(pending("the actor roster"));
  } else {
    who.append(
      table(
        ["role", "who", "wallet", "position", "cash", "reserved"],
        market.actors.map((actor) => {
          const position = decodedOf(state, `${actor.role}.position`);
          const line = el("tr");
          line.append(
            el("td", "role", actor.role),
            el("td", null, actor.label),
            el("td", null, digest(actor.pubkey)),
            el("td", null, digest(actor.position)),
            el("td", null, numeric(position ? position.cash_atoms : null)),
            el("td", null, numeric(position ? position.reserved_cash_atoms : null))
          );
          return line;
        })
      )
    );
  }
  cards.push(who);

  const phase = el("section", "card");
  phase.append(el("h2", null, "Epoch"));
  phase.append(
    fields("", [
      ["session phase", state.session ? state.session.phase : "—"],
      ["epoch phase", epoch ? EPOCH_PHASE[epoch.phase] || `phase ${epoch.phase}` : "—"],
      ["live orders", numeric(epoch ? epoch.order_count : null)],
      ["distinct owners", numeric(epoch ? epoch.owner_count : null)],
      ["freeze deadline slot", numeric(window ? window.freeze_deadline_slot : (state.session ? state.session.freeze_deadline_slot : null))],
      ["selection deadline slot", numeric(window ? window.selection_deadline_slot : null)],
      ["bank clock", state.clock ? `${numeric(state.clock.slot)} → ${numeric(state.clock.target)} (${state.clock.reason})` : "—"]
    ])
  );
  const controls = el("div", "controls");
  const open = state.session && state.session.phase === "open";
  controls.append(
    button("Freeze the book, then settle", open ? "tab tab-on" : "tab", () => {
      post({ action: "freeze" }, "freeze");
    })
  );
  if (!open) {
    controls.append(
      el(
        "span",
        "muted",
        "Freeze is available while the book is open. The program gates the freeze on its own clock, so this closes at the deadline slot rather than immediately."
      )
    );
  }
  phase.append(controls);
  cards.push(phase);
  return cards;
};

/* ------------------------------------------------------------------ */
/* The Ticket                                                          */
/* ------------------------------------------------------------------ */

const noticeRow = () => {
  if (!ticket.notice) return null;
  if (ticket.notice.pending) {
    const strip = el("div", "callout");
    strip.append(chip("IN_FLIGHT"), el("span", null, `${ticket.notice.text} — submitted, waiting on the bank`));
    return strip;
  }
  return row(
    ticket.notice.ok ? "callout" : "refusal",
    el("strong", null, ticket.notice.ok ? "ACCEPTED" : "REFUSED"),
    el("span", null, ticket.notice.text)
  );
};

const singleTicket = (state) => {
  const card = el("section", "card");
  card.append(el("h2", null, "Limit ticket"));
  card.append(
    el(
      "p",
      "muted",
      "Pick a hat, take a side, name a size and a limit. The limit must be an exact member of the frozen tick ladder — the program refuses anything else, and so does this form, with the same reason."
    )
  );
  card.append(hatRow(state, (knot) => {
    ticket.knot = knot;
    repaint();
  }));

  const form = el("div", "ticket-form");
  const sides = el("div", "controls");
  ["buy", "sell"].forEach((side) => {
    sides.append(
      button(side, ticket.side === side ? "tab tab-on" : "tab", () => {
        ticket.side = side;
        repaint();
      })
    );
  });
  form.append(el("label", "ticket-label", "side"), sides);

  const quantity = el("input");
  quantity.type = "number";
  quantity.min = "1";
  quantity.step = "1";
  quantity.value = String(ticket.quantity);
  quantity.addEventListener("change", () => {
    ticket.quantity = Number(quantity.value) || 0;
  });
  form.append(el("label", "ticket-label", "eggs"), quantity);

  const step = state.market ? state.market.ladder_step : 200;
  const limit = el("input");
  limit.type = "number";
  limit.min = "0";
  limit.step = String(step);
  limit.value = String(ticket.limit);
  limit.addEventListener("change", () => {
    ticket.limit = Number(limit.value) || 0;
  });
  form.append(el("label", "ticket-label", `limit (multiple of ${step})`), limit);
  card.append(form);

  const controls = el("div", "controls");
  controls.append(
    button("Place this order", "tab tab-on", () => {
      post(
        {
          action: "place",
          outcome: ticket.knot,
          side: ticket.side,
          quantity: ticket.quantity,
          limit: ticket.limit
        },
        `place ${ticket.side} ${ticket.quantity} at ${KNOT_LABEL[ticket.knot]}`
      );
    })
  );
  card.append(controls);
  const notice = noticeRow();
  if (notice) card.append(notice);
  return card;
};

/* The density painter.
 *
 * Eight numbers, dragged. The quantizer is the daemon's — a port of
 * `docs/site-plan/friday_clutch_check.py`'s largest-remainder rule with
 * lowest-index ties, pinned by a unit test against that script's own vectors —
 * so the browser drags and the daemon rounds. Everything the preview shows is
 * MODEL-ONLY until a step row says otherwise. */
const painter = (state) => {
  const card = el("section", "card");
  card.append(el("h2", null, "Belief"));
  card.append(
    el(
      "p",
      "muted",
      "Drag a density over the eight hats. The daemon quantizes it onto the limit ladder with the canonical largest-remainder rule, then inverts the automaton's own book-former against its resting quotes to propose the orders that belief implies."
    )
  );

  const sliders = el("div", "painter");
  ticket.belief.forEach((value, index) => {
    const column = el("div", "painter-knot");
    const input = el("input");
    input.type = "range";
    input.min = "0";
    input.max = "5000";
    input.step = "50";
    input.value = String(value);
    input.className = "painter-slider";
    input.addEventListener("input", () => {
      ticket.belief[index] = Number(input.value) || 0;
      ticket.proposal = null;
      repaint();
    });
    column.append(input);
    column.append(el("span", "painter-value", numeric(value)));
    column.append(el("span", "painter-knot-label", KNOT_LABEL[index]));
    sliders.append(column);
  });
  card.append(sliders);

  const total = ticket.belief.reduce((sum, value) => sum + value, 0);
  card.append(modelOnly(`raw drag total ${numeric(total)}; the daemon renormalizes it onto the price scale before anything is proposed`));

  const controls = el("div", "controls");
  controls.append(
    button("Preview the orders this implies", "tab", async () => {
      const reply = await act({ action: "propose", belief: ticket.belief });
      ticket.proposal = reply;
      repaint();
    })
  );
  controls.append(
    button("Place them all", "tab tab-on", () => {
      ticket.proposal = null;
      post({ action: "paint", belief: ticket.belief }, "place the painted belief");
    })
  );
  card.append(controls);

  if (ticket.proposal) {
    if (!ticket.proposal.ok) {
      card.append(row("refusal", el("strong", null, "REFUSED"), el("span", null, String(ticket.proposal.detail))));
    } else {
      card.append(modelOnly("quantized belief, and the orders it implies — none of this has been submitted"));
      card.append(
        el(
          "p",
          "mono",
          ticket.proposal.belief.map((value, index) => `${KNOT_LABEL[index]} ${value}`).join("   ")
        )
      );
      card.append(
        table(
          ["hat", "side", "eggs", "your limit", "crosses", "their limit"],
          ticket.proposal.proposed.map((entry) => {
            const line = el("tr");
            line.append(
              el("td", "role", KNOT_LABEL[entry.outcome]),
              el("td", null, entry.side),
              el("td", null, numeric(entry.quantity)),
              el("td", null, numeric(entry.limit)),
              el("td", null, `rank ${entry.crosses_rank}`),
              el("td", null, numeric(entry.their_limit))
            );
            return line;
          })
        )
      );
      if (!ticket.proposal.proposed.length) {
        card.append(
          row(
            "callout",
            el("strong", null, "NO CROSSING"),
            el("span", null, "this belief agrees with the automaton everywhere it quotes, so it implies no orders")
          )
        );
      }
    }
  }
  return card;
};

const portfolioTicket = () => {
  const card = el("section", "card");
  card.append(el("h2", null, "Portfolio ticket"));
  card.append(
    el(
      "p",
      "muted",
      "One order over a coefficient vector: lots of a fixed bundle rather than eggs on a single hat. The per-lot collateral bound is not tick-checked — only single-Egg limits live on the ladder."
    )
  );
  const form = el("div", "ticket-form");
  const coefficients = el("input");
  coefficients.type = "text";
  coefficients.value = "0,0,1,2,1,0,0,0";
  form.append(el("label", "ticket-label", "coefficients"), coefficients);
  const lots = el("input");
  lots.type = "number";
  lots.min = "1";
  lots.value = "50";
  form.append(el("label", "ticket-label", "lots"), lots);
  const perLot = el("input");
  perLot.type = "number";
  perLot.min = "0";
  perLot.value = "88";
  form.append(el("label", "ticket-label", "collateral per lot"), perLot);
  const sides = el("div", "controls");
  let side = "buy";
  ["buy", "sell"].forEach((option) => {
    const node = button(option, option === side ? "tab tab-on" : "tab", () => {
      side = option;
      [...sides.children].forEach((child) => {
        child.className = child.textContent === side ? "tab tab-on" : "tab";
      });
    });
    sides.append(node);
  });
  form.append(el("label", "ticket-label", "side"), sides);
  card.append(form);
  card.append(
    row(
      "controls",
      button("Place the portfolio order", "tab tab-on", () => {
        post(
          {
            action: "place-portfolio",
            coefficients: coefficients.value
              .split(",")
              .map((part) => Number(part.trim()) || 0),
            side,
            lots: Number(lots.value) || 0,
            limit_per_lot: Number(perLot.value) || 0
          },
          "place the portfolio ticket"
        );
      })
    )
  );
  return card;
};

const yourOrders = (state) => {
  const card = el("section", "card");
  card.append(el("h2", null, "Your resting orders"));
  const orders = (state.session ? state.session.orders : []).filter(
    (order) => order.owner === "human"
  );
  if (!orders.length) {
    card.append(pending("any order of yours"));
    return card;
  }
  orders.forEach((order) => {
    const line = el("div", "owner-bar");
    const reservation = decodedOf(state, `friday.reservation-${order.rank}`);
    line.append(
      el("span", "decoded-role", `rank ${order.rank}`),
      el(
        "span",
        null,
        order.kind === "portfolio"
          ? `${order.side} ${numeric(order.quantity)} lots at ${numeric(order.limit)}/lot`
          : `${order.side} ${numeric(order.quantity)} at ${KNOT_LABEL[order.outcome]} limit ${numeric(order.limit)}`
      ),
      el(
        "span",
        "step-status",
        reservation
          ? `${RESERVATION_STATE[reservation.state] || `state ${reservation.state}`} · entitled ${numeric(reservation.entitled_units)} · consumed ${numeric(reservation.consumed_units)}`
          : "reservation not yet reloaded"
      )
    );
    if (!order.retired && state.session && state.session.phase === "open") {
      line.append(
        button("retire", "tab", () => {
          post({ action: "cancel", rank: order.rank }, `retire rank ${order.rank}`);
        })
      );
    }
    if (order.retired) line.append(el("span", "step-badge badge-refuse", "retired"));
    card.append(line);
  });
  return card;
};

export const renderTicket = (state) => {
  const tabs = el("div", "controls");
  [
    ["single", "Single hat"],
    ["belief", "Belief"],
    ["portfolio", "Portfolio"]
  ].forEach(([id, label]) => {
    tabs.append(
      button(label, ticket.tab === id ? "tab tab-on" : "tab", () => {
        ticket.tab = id;
        repaint();
      })
    );
  });
  const header = el("section", "card");
  header.append(el("h2", null, "Ticket"));
  header.append(tabs);
  const body =
    ticket.tab === "belief" ? painter(state) : ticket.tab === "portfolio" ? portfolioTicket() : singleTicket(state);
  return [header, body, yourOrders(state)];
};

/* ------------------------------------------------------------------ */
/* The Book                                                            */
/* ------------------------------------------------------------------ */

/* Two beliefs and one price, drawn the way the disagreement exhibit draws
 * them: the two stated vectors as outlines, the cleared vector as the solid
 * bars between them. Nothing is drawn for the cleared vector until the bank
 * has verified a candidate carrying it. */
/* The SVG namespace, read off a real node in the document rather than written
 * here as a URL. Nothing under apps/operator/ names an address, on or off this
 * machine, and the grep that gates that is exact. */
const SVG_NS = document.getElementById("svg-namespace").namespaceURI;

const overlay = (state) => {
  const width = 720;
  const height = 220;
  const pad = 28;
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.setAttribute("class", "overlay");
  svg.setAttribute("role", "img");
  const theirs = state.bot ? state.bot.quoted_belief : null;
  const mine = state.belief ? state.belief.belief : null;
  const cleared = state.clearing ? state.clearing.prices : null;
  const peak = Math.max(
    1,
    ...(theirs || [0]),
    ...(mine || [0]),
    ...(cleared || [0])
  );
  const x = (index) => pad + (index * (width - 2 * pad)) / 7;
  const y = (value) => height - pad - ((height - 2 * pad) * value) / peak;

  const make = (name, attributes) => {
    const node = document.createElementNS(SVG_NS, name);
    Object.entries(attributes).forEach(([key, value]) => node.setAttribute(key, String(value)));
    return node;
  };
  svg.append(make("line", { x1: pad, y1: height - pad, x2: width - pad, y2: height - pad, class: "axis" }));
  if (cleared) {
    cleared.forEach((value, index) => {
      const top = y(value);
      svg.append(
        make("rect", {
          x: x(index) - 12,
          y: top,
          width: 24,
          height: Math.max(0, height - pad - top),
          class: "cleared-bar"
        })
      );
    });
  }
  const line = (values, className) => {
    if (!values) return;
    svg.append(
      make("polyline", {
        points: values.map((value, index) => `${x(index)},${y(value)}`).join(" "),
        class: className
      })
    );
  };
  line(theirs, "belief-them");
  line(mine, "belief-you");
  KNOT_LABEL.forEach((label, index) => {
    const text = make("text", { x: x(index), y: height - 8, class: "axis-label" });
    text.textContent = label;
    svg.append(text);
  });
  return svg;
};

const automaton = (state) => {
  const card = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "The opponent"));
  heading.append(el("span", "count", "fixed-belief automaton"));
  card.append(heading);
  if (!state.bot) {
    card.append(pending("the automaton's disclosure"));
    return card;
  }
  const bot = state.bot;
  card.append(
    el(
      "p",
      "muted",
      `This is ${bot.kind} — ${bot.not}. It holds one integer vector that never changes and quotes by a published rule, so every order it will ever place can be worked out before it places one.`
    )
  );
  card.append(
    fields("", [
      ["published belief", bot.belief.join(" · ")],
      ["on the limit ladder", bot.quoted_belief.join(" · ")],
      ["reference it takes sides against", bot.reference.join(" · ")],
      ["eggs per quote", numeric(bot.quote_size)],
      ["opening rule", bot.opening_rule],
      ["response rule", bot.response_rule],
      ["where the belief comes from", bot.belief_source]
    ])
  );
  return card;
};

export const renderBook = (state) => {
  const cards = [automaton(state)];

  const shape = el("section", "card");
  shape.append(el("h2", null, "Two beliefs, one price"));
  shape.append(
    el(
      "p",
      "muted",
      "The outlines are the two stated beliefs on the eight hats. The bars are the cleared vector the bank stamped onto the selected candidate — absent until it has."
    )
  );
  shape.append(overlay(state));
  const legend = el("div", "controls");
  legend.append(
    el("span", "legend legend-them", "the automaton"),
    el("span", "legend legend-you", "your painted belief"),
    el("span", "legend legend-cleared", "cleared")
  );
  shape.append(legend);
  if (state.clearing) {
    shape.append(
      fields("", [
        ["cleared vector", state.clearing.prices.join(" · ")],
        ["taken as", state.clearing.price_basis],
        ["paired slices", numeric(state.clearing.slices)],
        ["virtual split / merge", `${numeric(state.clearing.virtual_split)} / ${numeric(state.clearing.virtual_merge)}`]
      ])
    );
  } else {
    shape.append(pending("a cleared vector"));
  }
  cards.push(shape);

  const page = decodedOf(state, "friday.page");
  const book = el("section", "card");
  const heading = el("div", "card-heading");
  heading.append(el("h2", null, "Order page"));
  if (page) heading.append(el("span", "count", `${page.order_count} live · ${page.tombstone_count} retired`));
  book.append(heading);
  if (!page) {
    book.append(pending("the order page"));
  } else {
    const lines = page.orders.map((order) => {
      const line = el("tr");
      const detail =
        order.kind === "single"
          ? `${KNOT_LABEL[order.outcome] || `outcome ${order.outcome}`} · qty ${numeric(order.quantity)} · limit ${numeric(order.limit)}`
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
    });
    book.append(table(["slot", "kind", "side", "owner", "order", "terms"], lines));
    if (page.frozen) {
      book.append(row("callout", el("strong", null, "FROZEN"), el("span", null, "the page no longer admits placements")));
    }
  }
  cards.push(book);
  return cards;
};

/* ------------------------------------------------------------------ */
/* Settlement                                                          */
/* ------------------------------------------------------------------ */

export const renderSettlement = (state) => {
  const cards = [];
  const strip = state.conservation;

  const positions = el("section", "card");
  positions.append(el("h2", null, "Positions"));
  positions.append(
    el(
      "p",
      "muted",
      "Every number below was decoded out of a Position image the bank wrote, reloaded after the transaction that changed it."
    )
  );
  if (!strip || !strip.rows.length) {
    positions.append(pending("any position"));
  } else {
    positions.append(
      table(
        ["who", "cash", "reserved"].concat(KNOT_LABEL),
        strip.rows.map((line) => {
          const record = el("tr");
          record.append(
            el("td", "role", line.role),
            el("td", null, numeric(line.cash)),
            el("td", null, numeric(line.reserved))
          );
          line.eggs.forEach((held) => record.append(el("td", null, numeric(held))));
          return record;
        })
      )
    );
  }
  cards.push(positions);

  const conservation = el("section", "card");
  conservation.append(el("h2", null, "The value plane"));
  conservation.append(
    el(
      "p",
      "muted",
      "Re-derived on every tick from the account images this session observed, never read from a plan's expected block."
    )
  );
  if (!strip) {
    conservation.append(pending("a conservation strip"));
  } else {
    conservation.append(
      fields("", [
        ["position cash total", numeric(strip.cash_total)],
        ["reserved against orders", numeric(strip.reserved_total)],
        ["locked backing", numeric(strip.locked)],
        ["pooled custody", numeric(strip.custody)],
        ["endowed", numeric(strip.endowed_total)],
        ["complete sets locked", numeric(strip.split_total)]
      ])
    );
    if (!strip.complete) {
      conservation.append(
        row("callout", el("strong", null, "PARTIAL"), el("span", null, `not yet observed: ${strip.pending.join(", ")}`))
      );
    }
    (strip.identities || []).forEach((entry) => {
      conservation.append(
        row(
          entry.ok ? "callout" : "refusal",
          el("strong", null, entry.ok ? "HOLDS" : "BROKEN"),
          el("span", null, `${entry.label}: observed ${numeric(entry.observed)}, expected ${numeric(entry.expected)}`)
        )
      );
    });
  }
  cards.push(conservation);

  const receipts = el("section", "card");
  receipts.append(el("h2", null, "Reservations at rest"));
  const roles = [...state.latest.keys()].filter((role) => role.includes("reservation-")).sort();
  if (!roles.length) {
    receipts.append(pending("any reservation"));
  } else {
    const lines = roles
      .map((role) => [role, decodedOf(state, role)])
      .filter(([, value]) => value && value.kind === "reservation")
      .map(([role, value]) => {
        const line = el("tr");
        line.append(
          el("td", "role", role.replace("friday.", "")),
          el("td", null, RESERVATION_STATE[value.state] || `state ${value.state}`),
          el("td", null, value.side),
          el("td", null, numeric(value.remaining_cash_atoms)),
          el("td", null, numeric(value.entitled_units)),
          el("td", null, numeric(value.consumed_units))
        );
        return line;
      });
    receipts.append(
      table(["reservation", "state", "side", "cash left", "entitled", "consumed"], lines)
    );
  }
  cards.push(receipts);
  return cards;
};
