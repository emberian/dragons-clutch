/* The only thing this page can do: read the daemon's event log.
 *
 * There is no RPC client here, no wallet, no transaction serializer, and no
 * key material.  The bench learns exactly what the daemon published and
 * nothing else, which is what makes "the browser never builds a transaction"
 * a property of the code rather than a promise in a README.
 *
 * The daemon replays its whole log to every subscriber, so a reload or a late
 * open produces the same screen as one that was watching from the start. */

const EMPTY = Object.freeze({
  identity: null,
  roster: [],
  plan: null,
  steps: new Map(),
  states: new Map(),
  latest: new Map(),
  clock: null,
  crank: null,
  conservation: null,
  /* Trade mode only: the founded market, the automaton's own disclosure, the
   * session's phase and book, the last painted belief, and the cleared
   * vector.  All null in watch mode, which is how the page knows which set of
   * screens it is looking at. */
  market: null,
  bot: null,
  session: null,
  belief: null,
  clearing: null,
  pyth: null,
  boot: [],
  fault: null,
  done: null,
  events: 0,
  connected: false
});

export const eventHasSafeNumbers = (value) => {
  if (typeof value === "number") return Number.isSafeInteger(value);
  if (Array.isArray(value)) return value.every(eventHasSafeNumbers);
  if (value && typeof value === "object") {
    return Object.values(value).every(eventHasSafeNumbers);
  }
  return true;
};

export const createStore = () => {
  const state = {
    ...EMPTY,
    steps: new Map(),
    states: new Map(),
    latest: new Map(),
    boot: [],
    listeners: new Set()
  };

  const notify = () => state.listeners.forEach((listener) => listener(state));

  const apply = (event) => {
    state.events += 1;
    switch (event.type) {
      case "identity":
        state.identity = event;
        break;
      case "roster":
        state.roster = Array.isArray(event.actors) ? event.actors : [];
        break;
      case "plan":
        state.plan = event;
        break;
      case "boot":
        state.boot.push(event);
        break;
      case "step": {
        const previous = state.steps.get(event.ordinal) || {};
        state.steps.set(event.ordinal, { ...previous, ...event });
        break;
      }
      case "state": {
        const bucket = state.states.get(event.ordinal) || [];
        bucket.push(event);
        state.states.set(event.ordinal, bucket);
        /* The most recent image of each role, which is what the market
         * screens read.  Roles are the plan's own vocabulary, so this map is
         * keyed the same way the conservation table is. */
        state.latest.set(event.role, event);
        break;
      }
      case "clock":
        state.clock = event;
        break;
      case "crank":
        state.crank = event;
        break;
      case "conservation":
        state.conservation = event;
        break;
      case "market":
        state.market = event.identity;
        break;
      case "bot":
        state.bot = event.disclosure;
        break;
      case "session":
        state.session = event;
        break;
      case "belief":
        state.belief = event;
        break;
      case "clearing":
        state.clearing = event;
        break;
      case "pyth-campaign":
        state.pyth = event;
        break;
      case "fault":
        state.fault = event;
        break;
      case "done":
        state.done = event;
        state.clock = null;
        break;
      default:
        break;
    }
  };

  return {
    state,
    subscribe(listener) {
      state.listeners.add(listener);
      listener(state);
      return () => state.listeners.delete(listener);
    },
    connect() {
      const source = new EventSource("/api/events");
      source.addEventListener("open", () => {
        state.connected = true;
        notify();
      });
      source.addEventListener("error", () => {
        state.connected = false;
        notify();
      });
      source.addEventListener("message", (message) => {
        let event = null;
        try {
          event = JSON.parse(message.data);
        } catch (error) {
          return;
        }
        if (!event || typeof event.type !== "string") return;
        if (!eventHasSafeNumbers(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: event contained an unsafe JSON number instead of a canonical decimal string"
          };
          state.connected = false;
          source.close();
          notify();
          return;
        }
        apply(event);
        notify();
      });
      return source;
    }
  };
};
