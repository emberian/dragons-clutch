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
  clock: null,
  conservation: null,
  boot: [],
  fault: null,
  done: null,
  events: 0,
  connected: false
});

export const createStore = () => {
  const state = {
    ...EMPTY,
    steps: new Map(),
    states: new Map(),
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
        break;
      }
      case "clock":
        state.clock = event;
        break;
      case "conservation":
        state.conservation = event;
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
        apply(event);
        notify();
      });
      return source;
    }
  };
};
