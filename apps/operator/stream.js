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
  liveOwner: null,
  liveBuilder: null,
  liveManifest: null,
  liveChain: null,
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
const LIVE_CHAIN_SCHEMA = "dragons-clutch/operator/live-real-pyth-chain-discovery/v1";
const LIVE_BUILDER_SIGNING_SCHEMA = "dragons-clutch/operator/local-real-builder-signing/v1";
const LIVE_CLAIM = "NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE";
const LIVE_CAMPAIGN_MODE = "joined-multiboundary-v1";
const LIVE_TRANSCRIPT_SCHEMA = "dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1";
const LOWER_HEX_64 = /^[0-9a-f]{64}$/;
const SOLANA_ADDRESS = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const SOLANA_SIGNATURE = /^[1-9A-HJ-NP-Za-km-z]{64,88}$/;
const ROLLBACK_DOMAIN = "dragons-clutch/local-real-pyth/rollback-snapshot/v1";
const ROLLBACK_ENCODING = "domain || target_count:u64-le || repeated(key:32 || present:u8 || if-present(lamports:u64-le || owner:32 || executable:u8 || data_len:u64-le || data))";
const hasExactKeys = (value, keys) => Boolean(
  value
  && typeof value === "object"
  && !Array.isArray(value)
  && Object.keys(value).sort().join("\0") === [...keys].sort().join("\0")
);
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
  && ["starting", "running", "session-ready", "passed", "failed"].includes(event.phase)
  && event.campaign_mode === LIVE_CAMPAIGN_MODE
  && event.retained_transcript === false
  && /^http:\/\/127\.0\.0\.1:[0-9]+$/.test(event.rpc_url)
  && /^ws:\/\/127\.0\.0\.1:[0-9]+$/.test(event.websocket_url)
  && /^127\.0\.0\.1:[0-9]+$/.test(event.faucet)
  && /^127\.0\.0\.1:[0-9]+$/.test(event.gossip)
  && /^[0-9]+-[0-9]+$/.test(event.dynamic_port_range)
  && event.authority === "read-only live child telemetry; no retained transcript; no browser key material"
);

const liveOwnerIsPresentable = (event) => Boolean(
  eventUsesExactDecimalTransport(event)
  && hasExactKeys(event, [
    "type", "schema", "session_id", "lifecycle", "actors", "private_paths_exported",
    "private_key_material_exported", "browser_signing", "daemon_signing_seam",
  ])
  && event.type === "live-local-session-owner"
  && event.schema === "dragons-clutch/operator/local-session-owner/v1"
  && /^[0-9]+-[0-9]+$/.test(event.session_id)
  && event.lifecycle === "daemon-owned child, validator, work directory, and ephemeral signer roster"
  && Array.isArray(event.actors)
  && event.actors.length === 2
  && event.actors.every((actor, index) => (
    hasExactKeys(actor, ["role", "public_key"])
    && actor.role === ["payer", "second_owner"][index]
    && SOLANA_ADDRESS.test(actor.public_key)
  ))
  && event.actors[0].public_key !== event.actors[1].public_key
  && event.private_paths_exported === false
  && event.private_key_material_exported === false
  && event.browser_signing === false
  && event.daemon_signing_seam === "owner-scoped local signers; typed result-bound plan is signed only after the terminal chain check and is never submitted"
);

export const liveBuilderSigningIsPresentable = (event, owner = null, chain = null) => {
  const sourceWindow = event?.source_window;
  return Boolean(
    eventUsesExactDecimalTransport(event)
    && hasExactKeys(event, [
      "type", "schema", "session_id", "boundary", "plan_schema", "family",
      "source_archive", "market", "source_window", "required_signers",
      "unsigned_transaction_sha256", "signed_transaction_sha256",
      "signed_transaction_bytes", "blockhash_source", "submitted",
      "submission_signature", "signed_bytes_exported", "private_key_material_exported",
      "browser_signing", "transaction_admission",
    ])
    && event.type === "live-local-builder-signing"
    && event.schema === LIVE_BUILDER_SIGNING_SCHEMA
    && /^[0-9]+-[0-9]+$/.test(event.session_id)
    && event.boundary === "DAEMON-OWNED LOCAL SIGNING / NOT SUBMITTED / NO BROWSER KEY MATERIAL"
    && event.plan_schema === "dragons-clutch/operator/local-real-transaction-plan/v1"
    && event.family === "freeze-epoch"
    && SOLANA_ADDRESS.test(event.source_archive)
    && SOLANA_ADDRESS.test(event.market)
    && hasExactKeys(sourceWindow, ["start_bucket", "end_bucket_exclusive"])
    && CANONICAL_INTEGER.test(sourceWindow.start_bucket)
    && CANONICAL_INTEGER.test(sourceWindow.end_bucket_exclusive)
    && BigInt(sourceWindow.end_bucket_exclusive) === BigInt(sourceWindow.start_bucket) + 2n
    && Array.isArray(event.required_signers)
    && event.required_signers.length === 1
    && event.required_signers[0] === "payer"
    && LOWER_HEX_64.test(event.unsigned_transaction_sha256)
    && LOWER_HEX_64.test(event.signed_transaction_sha256)
    && event.unsigned_transaction_sha256 !== event.signed_transaction_sha256
    && CANONICAL_INTEGER.test(event.signed_transaction_bytes)
    && BigInt(event.signed_transaction_bytes) > 0n
    && event.blockhash_source === "confirmed loopback getLatestBlockhash"
    && event.submitted === false
    && event.submission_signature === null
    && event.signed_bytes_exported === false
    && event.private_key_material_exported === false
    && event.browser_signing === false
    && event.transaction_admission === "not exposed; this terminal-state plan proves only builder and signer continuity"
    && (!owner || event.session_id === owner.session_id)
    && (!chain || event.source_archive === chain.root_address)
  );
};

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

const liveRollbackIsPresentable = (rollback, kind, label) => {
  if (!(
    hasExactKeys(rollback, [
      "ok", "attempt_kind", "attempt_identity", "ephemeral_update_account",
      "ephemeral_update_absent_after_refusal", "refusal_step_label", "refusal_signature",
      "instruction_error", "snapshot_encoding", "snapshot_domain", "watched_accounts",
      "before_snapshot_sha256", "after_snapshot_sha256", "snapshots_equal",
    ])
    && rollback.ok === true
    && rollback.attempt_kind === kind
    && SOLANA_ADDRESS.test(rollback.ephemeral_update_account)
    && rollback.ephemeral_update_absent_after_refusal === true
    && rollback.refusal_step_label === label
    && SOLANA_SIGNATURE.test(rollback.refusal_signature)
    && hasExactKeys(rollback.instruction_error, ["instruction_index", "custom_code", "custom_code_hex"])
    && rollback.instruction_error.instruction_index === "2"
    && rollback.instruction_error.custom_code === "122"
    && rollback.instruction_error.custom_code_hex === "0x7a"
    && rollback.snapshot_encoding === ROLLBACK_ENCODING
    && rollback.snapshot_domain === ROLLBACK_DOMAIN
    && Array.isArray(rollback.watched_accounts)
    && rollback.watched_accounts.length === 2
    && rollback.watched_accounts.every((row, index) => (
      hasExactKeys(row, ["role", "address"])
      && row.role === ["source_archive", "receiver_treasury"][index]
      && SOLANA_ADDRESS.test(row.address)
    ))
    && rollback.watched_accounts[0].address !== rollback.watched_accounts[1].address
    && LOWER_HEX_64.test(rollback.before_snapshot_sha256)
    && rollback.before_snapshot_sha256 === rollback.after_snapshot_sha256
    && rollback.snapshots_equal === true
  )) return false;
  const identity = rollback.attempt_identity;
  if (kind === "wrong_config") {
    return hasExactKeys(identity, ["attempted_config_account", "registered_config_account"])
      && SOLANA_ADDRESS.test(identity.attempted_config_account)
      && SOLANA_ADDRESS.test(identity.registered_config_account)
      && identity.attempted_config_account !== identity.registered_config_account;
  }
  if (kind === "wrong_feed") {
    return hasExactKeys(identity, ["attempted_provider_feed_id", "registered_provider_feed_id", "verified_vaa_account"])
      && LOWER_HEX_64.test(identity.attempted_provider_feed_id)
      && LOWER_HEX_64.test(identity.registered_provider_feed_id)
      && identity.attempted_provider_feed_id !== identity.registered_provider_feed_id
      && SOLANA_ADDRESS.test(identity.verified_vaa_account);
  }
  return kind === "out_of_order_boundary"
    && hasExactKeys(identity, ["attempted_boundary_index", "expected_next_boundary_index", "attempted_publish_time", "expected_next_publish_time"])
    && identity.attempted_boundary_index === "1"
    && identity.expected_next_boundary_index === "0"
    && CANONICAL_INTEGER.test(identity.attempted_publish_time)
    && CANONICAL_INTEGER.test(identity.expected_next_publish_time)
    && BigInt(identity.attempted_publish_time) === BigInt(identity.expected_next_publish_time) + 60n;
};

const liveLiabilitiesArePresentable = (terminal) => {
  const liabilities = terminal?.liabilities;
  const ledger = liabilities?.supply_ledger;
  const zeroVector = (vector) => Array.isArray(vector)
    && vector.length === 4
    && vector.every((atom) => atom === "0");
  return Boolean(
    hasExactKeys(liabilities, ["all_zero", "supply_ledger", "outcome_mints"])
    && liabilities.all_zero === true
    && hasExactKeys(ledger, ["address", "outcome_count", "internal_supply", "external_supply", "aggregate_supply"])
    && SOLANA_ADDRESS.test(ledger.address)
    && ledger.outcome_count === "4"
    && zeroVector(ledger.internal_supply)
    && zeroVector(ledger.external_supply)
    && zeroVector(ledger.aggregate_supply)
    && Array.isArray(liabilities.outcome_mints)
    && liabilities.outcome_mints.length === 4
    && new Set(liabilities.outcome_mints.map((mint) => mint.address)).size === 4
    && liabilities.outcome_mints.every((mint, index) => (
      hasExactKeys(mint, ["outcome_index", "address", "supply"])
      && mint.outcome_index === String(index)
      && SOLANA_ADDRESS.test(mint.address)
      && mint.supply === "0"
    ))
  );
};

export const liveChainIsPresentable = (event) => {
  if (!(
    eventUsesExactDecimalTransport(event)
    && hasExactKeys(event, [
      "type", "schema", "mode", "authority", "context_slot", "attempts",
      "root_role", "root_address", "program_id", "token_program", "accounts",
      "restart_descriptor",
    ])
    && event.type === "live-real-pyth-chain-discovery"
    && event.schema === LIVE_CHAIN_SCHEMA
    && event.mode === "pyth-live"
    && event.authority === "loopback RPC graph-root-bracketed same-context account envelopes"
    && CANONICAL_INTEGER.test(event.context_slot)
    && /^(1|2|3)$/.test(event.attempts)
    && event.root_role === "source_archive"
    && SOLANA_ADDRESS.test(event.root_address)
    && SOLANA_ADDRESS.test(event.program_id)
    && SOLANA_ADDRESS.test(event.token_program)
    && Array.isArray(event.accounts)
    && event.accounts.length === 6
  )) return false;
  const expectedRoles = [
    "source_archive", "supply_ledger", "outcome_mint.0", "outcome_mint.1",
    "outcome_mint.2", "outcome_mint.3",
  ];
  const addresses = new Set();
  const accountsValid = event.accounts.every((account, index) => (
    hasExactKeys(account, [
      "role", "address", "address_source", "owner", "executable", "lamports",
      "data_len", "body_sha256", "account_schema",
    ])
    && account.role === expectedRoles[index]
    && SOLANA_ADDRESS.test(account.address)
    && !addresses.has(account.address)
    && (addresses.add(account.address) || true)
    && account.address_source === "admitted-live-result"
    && account.owner === (index < 2 ? event.program_id : event.token_program)
    && account.executable === false
    && CANONICAL_INTEGER.test(account.lamports)
    && account.data_len === (index === 0 ? "2560" : index === 1 ? "333" : "82")
    && LOWER_HEX_64.test(account.body_sha256)
    && account.account_schema === (index === 0
      ? "source-archive-v2/exact-2560"
      : index === 1
        ? "supply-ledger/v2-exact"
        : "token-2022-base-mint/exact-82")
  ));
  const restart = event.restart_descriptor;
  return accountsValid
    && event.accounts[0].address === event.root_address
    && hasExactKeys(restart, [
      "schema", "session_id", "genesis_hash", "repository_head", "rpc_url", "program_id",
      "source_archive", "supply_ledger", "outcome_mints", "public_only",
      "signer_material", "restart_capability",
    ])
    && restart.schema === "dragons-clutch/operator/local-session-restart-descriptor/v1"
    && /^[0-9]+-[0-9]+$/.test(restart.session_id)
    && typeof restart.genesis_hash === "string"
    && restart.genesis_hash.length > 0
    && /^[0-9a-f]{40}$/.test(restart.repository_head)
    && /^http:\/\/127\.0\.0\.1:[0-9]+$/.test(restart.rpc_url)
    && restart.program_id === event.program_id
    && restart.source_archive === event.accounts[0].address
    && restart.supply_ledger === event.accounts[1].address
    && Array.isArray(restart.outcome_mints)
    && restart.outcome_mints.length === 4
    && restart.outcome_mints.every((address, index) => address === event.accounts[index + 2].address)
    && restart.public_only === true
    && restart.signer_material === "not exported"
    && restart.restart_capability === "read-only rediscovery while the daemon-owned child is live; local signer continuity is owner-scoped but transaction admission is not yet exposed";
};

export const liveResultIsPresentable = (event, chain = null) => {
  if (!(
    event
    && eventUsesExactDecimalTransport(event)
    && hasExactKeys(event, [
      "type", "schema", "claim", "campaign_mode", "transcript_schema",
      "retained_transcript", "genesis_hash", "boundary_count", "step_count",
      "sealed", "resolved_payout", "archive_records", "source_archive",
      "wrong_config_rollback", "wrong_feed_rollback", "out_of_order_boundary_rollback",
      "trade_status", "collateral_atoms", "terminal",
    ])
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
    && SOLANA_ADDRESS.test(event.source_archive.key)
    && SOLANA_ADDRESS.test(event.source_archive.owner)
    && LOWER_HEX_64.test(event.source_archive.body_sha256)
    && LOWER_HEX_64.test(event.source_archive.page_commitment)
    && LOWER_HEX_64.test(event.source_archive.feed_id)
    && LOWER_HEX_64.test(event.source_archive.window_id)
    && liveRollbackIsPresentable(event.wrong_config_rollback, "wrong_config", "wrong-config-post-update-plus-append-rollback")
    && liveRollbackIsPresentable(event.wrong_feed_rollback, "wrong_feed", "wrong-feed-post-update-plus-append-rollback")
    && liveRollbackIsPresentable(event.out_of_order_boundary_rollback, "out_of_order_boundary", "out-of-order-boundary-post-update-plus-append-rollback")
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
    && liveLiabilitiesArePresentable(event.terminal)
  )) return false;
  const recordsPresentable = event.archive_records.every((record, index, records) => {
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
  if (!recordsPresentable || chain === null) return recordsPresentable;
  return liveChainIsPresentable(chain)
    && chain.restart_descriptor.genesis_hash === event.genesis_hash
    && chain.accounts[0].address === event.source_archive.key
    && chain.accounts[0].body_sha256 === event.source_archive.body_sha256
    && chain.accounts[1].address === event.terminal.liabilities.supply_ledger.address
    && chain.accounts.slice(2).every((account, index) => (
      account.address === event.terminal.liabilities.outcome_mints[index].address
    ));
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
      case "live-local-session-owner":
        if (!liveOwnerIsPresentable(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed local session owner boundary"
          };
          break;
        }
        state.liveOwner = event;
        break;
      case "live-local-builder-signing":
        if (!state.liveOwner || !state.liveChain || !liveBuilderSigningIsPresentable(event, state.liveOwner, state.liveChain)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed local builder/signing boundary"
          };
          break;
        }
        state.liveBuilder = event;
        break;
      case "live-real-pyth-result":
        if (!state.liveChain || !liveResultIsPresentable(event, state.liveChain)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live real-Pyth result"
          };
          break;
        }
        state.liveResult = event;
        break;
      case "live-real-pyth-chain-discovery":
        if (!liveChainIsPresentable(event)) {
          state.fault = {
            type: "fault",
            text: "UNTRUSTED PROJECTION REFUSED: malformed live chain discovery"
          };
          break;
        }
        state.liveChain = event;
        break;
      case "live-output":
        if (
          event.schema !== LIVE_RUN_SCHEMA
          || !CANONICAL_INTEGER.test(event.sequence)
          || event.stream !== "stdout"
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
