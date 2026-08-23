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
  snapshot: null,
  clock: null,
  crank: null,
  conservation: null,
  /* Trade mode only: the founded market, the automaton's own disclosure, the
   * session's phase and book, the last painted belief, and the pre-submit
   * candidate plan. All null in watch mode, which is how the page knows which
   * set of screens it is looking at. */
  market: null,
  bot: null,
  session: null,
  belief: null,
  candidatePlan: null,
  candidateTrials: [],
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

const SNAPSHOT_V2_SCHEMA = "dragons-clutch/operator/graph-root-bracketed-account-snapshot/v2";
const CANONICAL_INTEGER = /^(0|[1-9][0-9]*)$/;

const validatedSnapshotStates = (event) => {
  if (
    event.schema !== SNAPSHOT_V2_SCHEMA ||
    typeof event.root_role !== "string" ||
    typeof event.root_address !== "string" ||
    typeof event.context_slot !== "string" ||
    !CANONICAL_INTEGER.test(event.context_slot) ||
    typeof event.account_count !== "string" ||
    !CANONICAL_INTEGER.test(event.account_count) ||
    !Array.isArray(event.states) ||
    BigInt(event.account_count) !== BigInt(event.states.length)
  ) return null;

  const roles = new Set();
  for (const account of event.states) {
    if (!account || typeof account !== "object") return null;
    const explicitAbsence = account.present === false && account.decoded === null;
    const validatedPresence =
      account.present === true &&
      typeof account.owner === "string" &&
      account.executable === false &&
      account.account_schema &&
      typeof account.address_binding === "string";
    if (
      account.type !== "state" ||
      typeof account.role !== "string" ||
      typeof account.address !== "string" ||
      typeof account.address_binding !== "string" ||
      roles.has(account.role) ||
      account.snapshot_schema !== SNAPSHOT_V2_SCHEMA ||
      account.context_slot !== event.context_slot ||
      account.ordinal !== event.ordinal ||
      (!explicitAbsence && !validatedPresence)
    ) return null;
    roles.add(account.role);
  }
  const root = event.states.find((account) => account.role === event.root_role);
  if (!root || root.present !== true || root.address !== event.root_address) return null;
  return event.states.slice();
};

export const createStore = () => {
  const state = {
    ...EMPTY,
    steps: new Map(),
    states: new Map(),
    latest: new Map(),
    boot: [],
    candidateTrials: [],
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
        if (event.snapshot_schema !== undefined) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: sequential snapshot state cannot be promoted atomically"
          };
          break;
        }
        const bucket = state.states.get(event.ordinal) || [];
        bucket.push(event);
        state.states.set(event.ordinal, bucket);
        /* The most recent image of each role, which is what the market
         * screens read.  Roles are the plan's own vocabulary, so this map is
         * keyed the same way the conservation table is. */
        if (event.present === false) state.latest.delete(event.role);
        else state.latest.set(event.role, event);
        break;
      }
      case "account-snapshot-v2": {
        const accounts = validatedSnapshotStates(event);
        if (!accounts) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: incomplete or malformed graph snapshot V2"
          };
          break;
        }
        const nextLatest = new Map();
        for (const account of accounts) {
          if (account.present !== false) nextLatest.set(account.role, account);
        }
        state.states.set(event.ordinal, accounts);
        state.latest = nextLatest;
        state.snapshot = event;
        state.conservation = null;
        break;
      }
      case "snapshot-v2":
        state.fault = {
          type: "fault",
          text: "UNTRUSTED PROJECTION REFUSED: legacy sequential snapshot schema"
        };
        break;
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
      case "candidate-trial":
        if (event.schema !== "dragons-clutch/operator/candidate-trial/v1") {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: unknown candidate-trial schema"
          };
          break;
        }
        state.candidateTrials.push(event);
        break;
      case "candidate-plan":
        if (event.schema !== "dragons-clutch/operator/candidate-plan/v1") {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: unknown candidate-plan schema"
          };
          break;
        }
        state.candidatePlan = event;
        break;
      case "clearing":
      case "clearing-attempt":
        state.fault = {
          type: "fault",
          text: "UNTRUSTED PROJECTION REFUSED: legacy pre-submit clearing schema"
        };
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

  const ingest = (event) => {
    if (!event || typeof event.type !== "string") return;
    if (!eventHasSafeNumbers(event)) {
      state.fault = {
        type: "fault",
        text: "UNTRUSTED PROJECTION REFUSED: event contained an unsafe JSON number instead of a canonical decimal string"
      };
      notify();
      return;
    }
    apply(event);
    notify();
  };

  return {
    state,
    ingest,
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
          state.connected = false;
          source.close();
        }
        ingest(event);
      });
      return source;
    }
  };
};
