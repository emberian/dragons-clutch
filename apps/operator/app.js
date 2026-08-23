/* Operator Bench — wiring.
 *
 * Zero dependencies, no build step, no bundler. Hand-authored ES modules
 * loaded directly by the browser, which is the point: a page that exists to
 * make a trust boundary legible should not ask you to trust a dependency tree
 * to read it.
 *
 * Three modes share this page. In *watch* mode the daemon replays the sealed
 * lane's forty-four-step plan and the screens are read-only. In *trade* mode
 * it founds the Friday clutch and the screens become a ticket. The page does
 * not choose: it renders whichever screen set the daemon's own stream implies,
 * because a `market` event only ever arrives from a trade session. The
 * retained local-real Pyth mode is selected only by its explicit identity. */

import { el, fill } from "./dom.js";
import { chip } from "./evidence.js";
import { createStore } from "./stream.js";
import { renderBench } from "./bench.js";
import { renderWalk } from "./walk.js";
import { renderFunding, renderTicket, renderBook } from "./market.js";
import { renderPyth } from "./pyth.js";
import {
  bindRepaint,
  renderBook as renderTradeBook,
  renderClutch,
  renderSettlement,
  renderSteps,
  renderTicket as renderTradeTicket
} from "./trade.js";

const WATCH_SCREENS = Object.freeze([
  { id: "bench", label: "Bench", render: renderBench },
  { id: "walk", label: "Walk", render: renderWalk },
  { id: "funding", label: "Funding", render: renderFunding },
  { id: "ticket", label: "Ticket", render: renderTicket },
  { id: "book", label: "Book", render: renderBook }
]);

const TRADE_SCREENS = Object.freeze([
  { id: "clutch", label: "Clutch", render: renderClutch },
  { id: "ticket", label: "Ticket", render: renderTradeTicket },
  { id: "book", label: "Book", render: renderTradeBook },
  { id: "settlement", label: "Settlement", render: renderSettlement },
  { id: "steps", label: "Steps", render: renderSteps },
  { id: "bench", label: "Bench", render: renderBench }
]);

const PYTH_SCREENS = Object.freeze([
  { id: "pyth", label: "Retained campaign", render: renderPyth }
]);

const store = createStore();
let current = null;

const screensFor = (state) => (
  state.identity && state.identity.mode === "pyth-local"
    ? PYTH_SCREENS
    : state.market
      ? TRADE_SCREENS
      : WATCH_SCREENS
);

/* The honesty strip. Permanent, non-dismissible, and rendered before any
 * state can arrive — there is no code path on this page that removes it, and
 * the digest it shows is the one the daemon hashed out of the file the
 * validator loaded. */
const renderBanner = (state) => {
  const strip = document.getElementById("honesty");
  const identity = state.identity;
  const pyth = identity && identity.mode === "pyth-local";
  const parts = [
    el("span", "honesty-flag", "NON-PRODUCTION"),
    el("span", null, pyth ? "SYNTHETIC OBSERVATION" : "mock-source ELF"),
    el("span", "honesty-sep", "·"),
    el("code", "digest", identity ? identity.elf_sha256 : "sha256 pending"),
    el("span", "honesty-sep", "·"),
    el("span", null, pyth ? "LOCAL VALIDATOR ONLY" : "LOCAL 127.0.0.1 ONLY"),
    ...(pyth ? [el("span", "honesty-sep", "·"), el("span", null, "READ-ONLY RETAINED TRANSCRIPT")] : []),
    el("span", "honesty-sep", "·"),
    el("span", null, "no value"),
    el("span", "honesty-sep", "·"),
    el("span", null, "evidence scope"),
    chip(identity && identity.evidence_scope ? identity.evidence_scope : "SBF_EXECUTED"),
    el("span", "honesty-note", "unpromoted")
  ];
  fill(strip, ...parts);
};

const renderNav = (state) => {
  const nav = document.getElementById("nav");
  fill(
    nav,
    ...screensFor(state).map((screen) => {
      const button = el("button", screen.id === current ? "tab tab-on" : "tab", screen.label);
      button.type = "button";
      button.setAttribute("aria-current", screen.id === current ? "page" : "false");
      button.addEventListener("click", () => {
        current = screen.id;
        render(store.state);
      });
      return button;
    })
  );
};

const render = (state) => {
  const screens = screensFor(state);
  if (!current || !screens.some((screen) => screen.id === current)) {
    current = screens[0].id;
  }
  renderBanner(state);
  renderNav(state);
  const screen = screens.find((candidate) => candidate.id === current) || screens[0];
  fill(document.getElementById("screen"), ...screen.render(state));

  const brand = document.getElementById("brand-sub");
  if (brand) {
    brand.textContent = state.identity && state.identity.mode === "pyth-local"
      ? "read-only retained campaign — synthetic, local"
      : state.market
        ? "Friday clutch — trade mode, committed, local"
        : "general clearing, committed, local";
  }

  const status = document.getElementById("status");
  const label = state.fault
    ? "faulted"
    : state.pyth
      ? state.done ? `retained campaign ${state.done.verdict}` : "retained campaign loaded"
    : state.session
      ? `clutch ${state.session.phase}`
      : state.done
        ? `walk ${state.done.verdict}`
        : state.plan
          ? "walking"
          : "starting";
  fill(
    status,
    el("span", state.connected ? "live" : "dead", state.connected ? "stream attached" : "stream detached"),
    el("span", null, label)
  );
};

/* The ticket keeps a little local state — which knot is selected, what the
 * painter's sliders read — and needs to redraw when a button changes it, not
 * only when the daemon speaks. */
bindRepaint(() => render(store.state));

store.subscribe(render);
store.connect();
