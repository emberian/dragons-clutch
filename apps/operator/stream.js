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
  liveRun: null,
  liveManifest: null,
  liveResult: null,
  liveOutput: [],
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
const LIVE_RUN_SCHEMA = "dragons-clutch/operator/live-real-pyth-run/v1";
const LIVE_MANIFEST_SCHEMA = "dragons-clutch/operator/live-real-pyth-manifest/v1";
const LIVE_RESULT_SCHEMA = "dragons-clutch/operator/live-real-pyth-result/v1";
const LIVE_CLAIM = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const LIVE_CAMPAIGN_MODE = "joined-multiboundary-v1";
const LIVE_TRANSCRIPT_SCHEMA = "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1";
const LOWER_HEX_64 = /^[0-9a-f]{64}$/;
const eventUsesExactDecimalTransport = (value) => {
  if (typeof value === "number") return false;
  if (Array.isArray(value)) return value.every(eventUsesExactDecimalTransport);
  if (value && typeof value === "object") {
    return Object.values(value).every(eventUsesExactDecimalTransport);
  }
  return true;
};

export const liveRunIsPresentable = (event) => Boolean(
  event
  && eventUsesExactDecimalTransport(event)
  && event.type === "live-real-pyth-run"
  && event.schema === LIVE_RUN_SCHEMA
  && event.mode === "pyth-live"
  && ["starting", "running", "validating-exit", "passed", "failed"].includes(event.phase)
  && event.campaign_mode === LIVE_CAMPAIGN_MODE
  && event.retained_transcript === false
  && /^http:\/\/127\.0\.0\.1:[0-9]+$/.test(event.rpc_url)
  && /^ws:\/\/127\.0\.0\.1:[0-9]+$/.test(event.websocket_url)
  && /^127\.0\.0\.1:[0-9]+$/.test(event.faucet)
  && /^127\.0\.0\.1:[0-9]+$/.test(event.gossip)
  && /^[0-9]+-[0-9]+$/.test(event.dynamic_port_range)
  && event.authority === "read-only live child telemetry; no retained transcript; no browser key material"
);

const liveManifestIsPresentable = (event) => Boolean(
  event
  && eventUsesExactDecimalTransport(event)
  && event.type === "live-real-pyth-manifest"
  && event.schema === LIVE_MANIFEST_SCHEMA
  && event.claim === LIVE_CLAIM
  && event.campaign_mode === LIVE_CAMPAIGN_MODE
  && event.transcript_schema === LIVE_TRANSCRIPT_SCHEMA
  && event.retained_transcript === false
  && event.boundary_count === "2"
  && /^[0-9a-f]{40}$/.test(event.repository_head)
  && LOWER_HEX_64.test(event.clutch_elf_sha256)
  && LOWER_HEX_64.test(event.validator_binary_sha256)
  && LOWER_HEX_64.test(event.source_profile_snapshot_sha256)
  && Array.isArray(event.provider)
  && event.provider.length === 4
  && event.provider.every((row, index) => (
    row
    && row.role === ["receiver-program", "receiver-programdata", "router-program", "router-programdata"][index]
    && typeof row.address === "string"
    && row.address.length > 0
    && LOWER_HEX_64.test(row.complete_account_body_sha256)
    && row.executable === [true, false, true, false][index]
  ))
  && Array.isArray(event.genesis_prerequisite_roles)
  && event.genesis_prerequisite_roles.length > 0
  && event.genesis_prerequisite_roles.every((role) => typeof role === "string" && role.length > 0)
);

export const liveResultIsPresentable = (event) => {
  if (!(
    event
    && eventUsesExactDecimalTransport(event)
    && event.type === "live-real-pyth-result"
    && event.schema === LIVE_RESULT_SCHEMA
    && event.claim === LIVE_CLAIM
    && event.campaign_mode === LIVE_CAMPAIGN_MODE
    && event.transcript_schema === LIVE_TRANSCRIPT_SCHEMA
    && event.retained_transcript === false
    && event.boundary_count === "2"
    && event.step_count === "56"
    && event.sealed === true
    && event.resolved_payout === "1"
    && event.trade_status === "settled"
    && event.collateral_atoms === "128"
    && Array.isArray(event.archive_records)
    && event.archive_records.length === 2
    && event.source_archive
    && event.source_archive.executable === false
    && event.source_archive.data_len === "2560"
    && event.source_archive.record_count === "2"
    && typeof event.source_archive.key === "string"
    && event.source_archive.key.length > 0
    && typeof event.source_archive.owner === "string"
    && event.source_archive.owner.length > 0
    && LOWER_HEX_64.test(event.source_archive.body_sha256)
    && LOWER_HEX_64.test(event.source_archive.page_commitment)
    && LOWER_HEX_64.test(event.source_archive.feed_id)
    && LOWER_HEX_64.test(event.source_archive.window_id)
    && event.out_of_order_boundary_rollback
    && event.out_of_order_boundary_rollback.ok === true
    && event.out_of_order_boundary_rollback.skipped_update_absent_after_refusal === true
    && typeof event.out_of_order_boundary_rollback.skipped_update_account === "string"
    && event.out_of_order_boundary_rollback.skipped_update_account.length > 0
    && event.out_of_order_boundary_rollback.snapshots_equal === true
    && LOWER_HEX_64.test(event.out_of_order_boundary_rollback.before_snapshot_sha256)
    && event.out_of_order_boundary_rollback.before_snapshot_sha256
      === event.out_of_order_boundary_rollback.after_snapshot_sha256
    && event.out_of_order_boundary_rollback.instruction_error
    && event.out_of_order_boundary_rollback.instruction_error.instruction_index === "2"
    && event.out_of_order_boundary_rollback.instruction_error.custom_code === "122"
    && event.out_of_order_boundary_rollback.instruction_error.custom_code_hex === "0x7a"
    && event.terminal
    && event.terminal.buyer_token_atoms === "76"
    && event.terminal.seller_token_atoms === "52"
    && event.terminal.hoard_token_atoms === "0"
    && event.terminal.hoard_collateral_atoms === "0"
    && event.terminal.buyer_position_cash_atoms === "0"
    && event.terminal.seller_position_cash_atoms === "0"
    && [
      event.terminal.buyer_position_internal,
      event.terminal.seller_position_internal,
      event.terminal.supply_internal,
    ].every((vector) => Array.isArray(vector) && vector.length === 4 && vector.every((atom) => atom === "0"))
  )) return false;
  return event.archive_records.every((record, index, records) => {
    const shape = record.index === String(index)
      && CANONICAL_INTEGER.test(record.bucket)
      && CANONICAL_INTEGER.test(record.lower)
      && CANONICAL_INTEGER.test(record.upper)
      && CANONICAL_INTEGER.test(record.sequence)
      && CANONICAL_INTEGER.test(record.write_slot)
      && CANONICAL_INTEGER.test(record.publish_time)
      && BigInt(record.lower) <= BigInt(record.upper)
      && record.sequence === record.publish_time;
    if (!shape || index === 0) return shape;
    const previous = records[index - 1];
    return BigInt(record.bucket) === BigInt(previous.bucket) + 1n
      && BigInt(record.publish_time) === BigInt(previous.publish_time) + 60n;
  });
};

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
    liveOutput: [],
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
      case "live-real-pyth-run":
        if (!liveRunIsPresentable(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live real-Pyth run boundary"
          };
          break;
        }
        state.liveRun = event;
        break;
      case "live-real-pyth-manifest":
        if (!liveManifestIsPresentable(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live real-Pyth manifest"
          };
          break;
        }
        state.liveManifest = event;
        break;
      case "live-real-pyth-result":
        if (!liveResultIsPresentable(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live real-Pyth result"
          };
          break;
        }
        state.liveResult = event;
        break;
      case "live-output":
        if (
          event.schema !== LIVE_RUN_SCHEMA
          || !CANONICAL_INTEGER.test(event.sequence)
          || !["stdout", "stderr"].includes(event.stream)
          || typeof event.text !== "string"
        ) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live child output"
          };
          break;
        }
        state.liveOutput.push(event);
        if (state.liveOutput.length > 400) state.liveOutput.shift();
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
