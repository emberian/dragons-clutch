/* Operator Bench — wiring.
 *
 * Zero dependencies, no build step, no bundler. Hand-authored ES modules
 * loaded directly by the browser, which is the point: a page that exists to
 * make a trust boundary legible should not ask you to trust a dependency tree
 * to read it. */

import { el, fill } from "./dom.js";
import { chip } from "./evidence.js";
import { createStore } from "./stream.js";
import { renderBench } from "./bench.js";
import { renderWalk } from "./walk.js";

const SCREENS = Object.freeze([
  { id: "bench", label: "Bench", render: renderBench },
  { id: "walk", label: "Walk", render: renderWalk }
]);

const store = createStore();
let current = "bench";

/* The honesty strip. Permanent, non-dismissible, and rendered before any
 * state can arrive — there is no code path on this page that removes it, and
 * the digest it shows is the one the daemon hashed out of the file the
 * validator loaded. */
const renderBanner = (state) => {
  const strip = document.getElementById("honesty");
  const identity = state.identity;
  const parts = [
    el("span", "honesty-flag", "NON-PRODUCTION"),
    el("span", null, "mock-source ELF"),
    el("span", "honesty-sep", "·"),
    el("code", "digest", identity ? identity.elf_sha256 : "sha256 pending"),
    el("span", "honesty-sep", "·"),
    el("span", null, "LOCAL 127.0.0.1 ONLY"),
    el("span", "honesty-sep", "·"),
    el("span", null, "no value"),
    el("span", "honesty-sep", "·"),
    el("span", null, "evidence scope"),
    chip("SBF_EXECUTED"),
    el("span", "honesty-note", "unpromoted")
  ];
  fill(strip, ...parts);
};

const renderNav = () => {
  const nav = document.getElementById("nav");
  fill(
    nav,
    ...SCREENS.map((screen) => {
      const button = el("button", screen.id === current ? "tab tab-on" : "tab", screen.label);
      button.type = "button";
      button.addEventListener("click", () => {
        current = screen.id;
        render(store.state);
      });
      return button;
    })
  );
};

const render = (state) => {
  renderBanner(state);
  renderNav();
  const screen = SCREENS.find((candidate) => candidate.id === current) || SCREENS[0];
  fill(document.getElementById("screen"), ...screen.render(state));
  const status = document.getElementById("status");
  const label = state.done
    ? `walk ${state.done.verdict}`
    : state.fault
      ? "faulted"
      : state.plan
        ? "walking"
        : "starting";
  fill(status, el("span", state.connected ? "live" : "dead", state.connected ? "stream attached" : "stream detached"), el("span", null, label));
};

store.subscribe(render);
store.connect();
