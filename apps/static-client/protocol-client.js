/*
 * Offline protocol-control surface. No network, wallet, signing, submission,
 * storage, or background work is reachable from this file.
 */
(function (root) {
  "use strict";

  const CONTRACTS = root.GlassProtocolContracts;
  const $ = (id) => document.getElementById(id);
  const ID32 = /^[0-9a-f]{64}$/;
  const SHA256 = /^[0-9a-f]{64}$/;
  const COMMIT = /^[0-9a-f]{40}$/;
  const UINT = /^(0|[1-9][0-9]*)$/;
  const BASE58 = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
  const U64_MAX = (1n << 64n) - 1n;
  const U128_MAX = (1n << 128n) - 1n;
  const LIVENESS_ACTIONS = Object.freeze({ "observe-donation": 0, "spend-work": 1, "close-success": 2, "close-failure": 3 });
  const LIVENESS_KINDS = Object.freeze(Object.fromEntries(CONTRACTS.orders.livenessCompartments.map((name, index) => [name, index])));
  const SERIES_ACTIONS = Object.freeze({ "series-register": 1, "series-activate": 2, "series-advance": 3, "series-lapse": 4, "series-observe-donation": 5, "series-close": 6 });
  const SERIES_COMPONENTS = Object.freeze(Object.fromEntries(CONTRACTS.orders.seriesComponents.map((name, index) => [name, index])));
  const STRUCTURED_ACTIONS = Object.freeze({ "structured-create": 1, "structured-wrap-canonical": 2, "structured-wrap-full": 3, "structured-unwrap-canonical": 4, "structured-unwrap-full": 5, "structured-compact-donation": 6, "structured-redeem-terminal": 7, "structured-retire": 8 });
  const state = { configuration: null, projection: null, output: null };

  const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  const create = (name, className, text) => {
    const node = document.createElement(name);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const reset = (node) => node.replaceChildren();
  const text = (value, name, maximum) => {
    if (typeof value !== "string" || value.length === 0 || value.length > maximum) throw new Error(`${name} must be nonempty text no longer than ${maximum} characters.`);
    return value;
  };
  const identity = (value, name) => {
    if (typeof value !== "string" || !ID32.test(value)) throw new Error(`${name} must be a lowercase 32-byte hex identity.`);
    if (/^0+$/.test(value)) throw new Error(`${name} must not be zero.`);
    return value;
  };
  const optionalIdentity = (value, name) => value === null || value === undefined || value === "" ? null : identity(value, name);
  const uint = (value, name, maximum) => {
    if (typeof value === "number" && !Number.isSafeInteger(value)) throw new Error(`${name} must be a decimal string when it exceeds JavaScript's safe-integer range.`);
    const normalized = typeof value === "number" ? String(value) : value;
    if (typeof normalized !== "string" || !UINT.test(normalized)) throw new Error(`${name} must be a canonical unsigned integer string.`);
    const parsed = BigInt(normalized);
    if (parsed > maximum) throw new Error(`${name} exceeds its exact integer width.`);
    return parsed;
  };
  const u64 = (value, name) => uint(value, name, U64_MAX);
  const u128 = (value, name) => uint(value, name, U128_MAX);
  const numberU32 = (value, name) => {
    const parsed = u64(value, name);
    if (parsed > 0xffffffffn) throw new Error(`${name} exceeds u32.`);
    return Number(parsed);
  };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const sum = (values) => values.reduce((total, value) => total + value, 0n);
  const ceilDiv = (numerator, denominator) => numerator === 0n ? 0n : ((numerator - 1n) / denominator) + 1n;
  const canonicalJson = (value) => {
    const visit = (item) => Array.isArray(item) ? item.map(visit) : plain(item)
      ? Object.keys(item).sort().reduce((out, key) => { out[key] = visit(item[key]); return out; }, {})
      : item;
    return JSON.stringify(visit(value));
  };

  const bytes = () => [];
  const putU8 = (out, value) => out.push(value & 0xff);
  const putLe = (out, value, width) => {
    let remaining = value;
    for (let index = 0; index < width; index += 1) {
      out.push(Number(remaining & 0xffn));
      remaining >>= 8n;
    }
    if (remaining !== 0n) throw new Error(`Value does not fit ${width} bytes.`);
  };
  const putHex = (out, value, name) => {
    identity(value, name);
    for (let index = 0; index < value.length; index += 2) out.push(Number.parseInt(value.slice(index, index + 2), 16));
  };
  const putAscii = (out, value) => { for (let index = 0; index < value.length; index += 1) out.push(value.charCodeAt(index)); };
  const putZeros = (out, count) => { for (let index = 0; index < count; index += 1) out.push(0); };
  const hex = (out) => out.map((value) => value.toString(16).padStart(2, "0")).join("");

  const releaseConfiguration = () => {
    const cluster = CONTRACTS.clusters.find((item) => item.id === $("cluster-target").value);
    const endpoint = $("rpc-target").value.trim();
    const programId = $("program-target").value.trim();
    const sourceCommit = $("release-source").value.trim();
    const elfSha256 = $("release-elf").value.trim();
    const manifestSha256 = $("release-manifest").value.trim();
    if (!cluster) throw new Error("Select a recognized cluster target.");
    if (!/^https?:\/\/[^\s]+$/.test(endpoint)) throw new Error("RPC target must be an explicit HTTP(S) URL. It will not be contacted.");
    if (programId && !BASE58.test(programId)) throw new Error("Program ID must be a canonical-looking base58 public key or blank.");
    if (sourceCommit && !COMMIT.test(sourceCommit)) throw new Error("Release source must be a 40-character lowercase commit hash or blank.");
    if (elfSha256 && !SHA256.test(elfSha256)) throw new Error("ELF SHA-256 must be 64 lowercase hexadecimal characters or blank.");
    if (manifestSha256 && !SHA256.test(manifestSha256)) throw new Error("Manifest SHA-256 must be 64 lowercase hexadecimal characters or blank.");
    const complete = Boolean(programId && sourceCommit && elfSha256 && manifestSha256);
    return Object.freeze({ schema: "dragons-clutch.local-release-target.v1", cluster: cluster.id, rpcEndpoint: endpoint, programId: programId || null, sourceCommit: sourceCommit || null, elfSha256: elfSha256 || null, releaseManifestSha256: manifestSha256 || null, complete, authority: "user-supplied-untrusted", networkRead: false, official: false });
  };

  const renderConfiguration = (configuration) => {
    state.configuration = configuration;
    const target = $("configuration-result");
    target.className = `configuration-result ${configuration.complete ? "ready" : "incomplete"}`;
    target.textContent = configuration.complete
      ? `Complete local target recorded for ${configuration.cluster}. It is still untrusted and non-official until the release and deployment are independently authenticated.`
      : `Target recorded for ${configuration.cluster}. Program/release binding is incomplete; transaction assembly and deployment claims remain disabled.`;
    $("configured-target-json").textContent = JSON.stringify(configuration, null, 2);
  };

  const validateAccountRead = (row, index, slot) => {
    assert(plain(row), `provenance.accountReads[${index}] must be an object.`);
    const address = text(row.address, `accountReads[${index}].address`, 96);
    const ownerProgram = text(row.ownerProgram, `accountReads[${index}].ownerProgram`, 96);
    assert(BASE58.test(address), `accountReads[${index}].address must be base58.`);
    assert(BASE58.test(ownerProgram), `accountReads[${index}].ownerProgram must be base58.`);
    assert(typeof row.bodySha256 === "string" && SHA256.test(row.bodySha256), `accountReads[${index}].bodySha256 must be lowercase SHA-256.`);
    const observedSlot = u64(row.slot, `accountReads[${index}].slot`);
    assert(observedSlot === slot, `accountReads[${index}].slot must equal observedAt.slot.`);
    return Object.freeze({ address, ownerProgram, bodySha256: row.bodySha256, slot: observedSlot.toString(), semanticKind: text(row.semanticKind, `accountReads[${index}].semanticKind`, 80) });
  };

  const validateOwnerRows = (general) => {
    assert(plain(general), "components.generalV2 must be an object.");
    const priceScale = u64(general.priceScale, "generalV2.priceScale");
    assert(priceScale > 0n, "generalV2.priceScale must be positive.");
    const selectedFeeAtoms = u128(general.selectedFeeAtoms, "generalV2.selectedFeeAtoms");
    assert(Array.isArray(general.owners) && general.owners.length > 0 && general.owners.length <= 64, "generalV2.owners must contain 1..64 owner rows.");
    assert(Number.isInteger(general.ownerCount) && general.ownerCount === general.owners.length, "generalV2.ownerCount must equal the owner row count.");
    let previous = null;
    const rows = general.owners.map((row, index) => {
      assert(plain(row), `generalV2.owners[${index}] must be an object.`);
      const owner = identity(row.owner, `owners[${index}].owner`);
      assert(previous === null || previous < owner, "General V2 owner rows must be unique and lexicographically sorted by raw identity bytes.");
      previous = owner;
      const buyOrderMask = u64(row.buyOrderMask, `owners[${index}].buyOrderMask`);
      const sellOrderMask = u64(row.sellOrderMask, `owners[${index}].sellOrderMask`);
      assert((buyOrderMask & sellOrderMask) === 0n && (buyOrderMask | sellOrderMask) !== 0n, `owners[${index}] masks must be disjoint and nonempty.`);
      const sliceCount = numberU32(row.sliceCount, `owners[${index}].sliceCount`);
      assert(sliceCount > 0 && sliceCount <= 65535, `owners[${index}].sliceCount must fit positive u16.`);
      const buyPriceUnits = u128(row.buyPriceUnits, `owners[${index}].buyPriceUnits`);
      const sellPriceUnits = u128(row.sellPriceUnits, `owners[${index}].sellPriceUnits`);
      const feeAtoms = u64(row.selectedFeeAtoms, `owners[${index}].selectedFeeAtoms`);
      const reservedCashAtoms = u64(row.reservedCashAtoms, `owners[${index}].reservedCashAtoms`);
      const positionCashAtoms = u64(row.positionCashAtoms, `owners[${index}].positionCashAtoms`);
      const positionReservedCashAtoms = u64(row.positionReservedCashAtoms, `owners[${index}].positionReservedCashAtoms`);
      assert(buyOrderMask === 0n ? buyPriceUnits === 0n : buyPriceUnits > 0n, `owners[${index}] buy total does not match its mask.`);
      assert(sellOrderMask === 0n ? sellPriceUnits === 0n : sellPriceUnits > 0n, `owners[${index}] sell total does not match its mask.`);
      assert(buyOrderMask !== 0n || (feeAtoms === 0n && reservedCashAtoms === 0n), `owners[${index}] seller-only row must explicitly carry zero fee and zero buy reservation.`);
      const considerationDebitAtoms = ceilDiv(buyPriceUnits, priceScale);
      const debitAtoms = considerationDebitAtoms + feeAtoms;
      const creditAtoms = sellPriceUnits / priceScale;
      assert(reservedCashAtoms >= debitAtoms, `owners[${index}] buy reservation cannot fund aggregate consideration plus selected fee.`);
      assert(positionCashAtoms >= reservedCashAtoms && positionReservedCashAtoms >= reservedCashAtoms && positionReservedCashAtoms <= positionCashAtoms, `owners[${index}] Position cash/reservation projection is inconsistent.`);
      const nextCash = positionCashAtoms - debitAtoms + creditAtoms;
      const nextReserved = positionReservedCashAtoms - reservedCashAtoms;
      assert(nextReserved <= nextCash, `owners[${index}] projected Position poststate violates reserved <= cash.`);
      return Object.freeze({ owner, buyOrderMask: buyOrderMask.toString(), sellOrderMask: sellOrderMask.toString(), sliceCount, buyPriceUnits: buyPriceUnits.toString(), sellPriceUnits: sellPriceUnits.toString(), selectedFeeAtoms: feeAtoms.toString(), reservedCashAtoms: reservedCashAtoms.toString(), considerationDebitAtoms: considerationDebitAtoms.toString(), debitAtoms: debitAtoms.toString(), creditAtoms: creditAtoms.toString(), positionCashAtoms: positionCashAtoms.toString(), positionReservedCashAtoms: positionReservedCashAtoms.toString(), nextCashAtoms: nextCash.toString(), nextReservedCashAtoms: nextReserved.toString(), state: text(row.state, `owners[${index}].state`, 40) });
    });
    assert(sum(rows.map((row) => BigInt(row.selectedFeeAtoms))) === selectedFeeAtoms, "Sum of owner fee rows must equal generalV2.selectedFeeAtoms exactly.");
    return Object.freeze({ market: identity(general.market, "generalV2.market"), epoch: identity(general.epoch, "generalV2.epoch"), selectedCandidate: identity(general.selectedCandidate, "generalV2.selectedCandidate"), priceScale: priceScale.toString(), selectedFeeAtoms: selectedFeeAtoms.toString(), ownerCount: rows.length, roundingPotPriceUnits: u128(general.roundingPotPriceUnits, "generalV2.roundingPotPriceUnits").toString(), owners: Object.freeze(rows) });
  };

  const validateFee = (fee, selectedCandidate, selectedFeeAtoms) => {
    assert(plain(fee), "components.fee must be an object.");
    assert(identity(fee.settlementCandidate, "fee.settlementCandidate") === selectedCandidate, "Fee record must bind the exact final settlement candidate.");
    assert(u128(fee.selectedFeeAtoms, "fee.selectedFeeAtoms").toString() === selectedFeeAtoms, "Fee record total must equal the owner settlement total.");
    assert(Array.isArray(fee.recipients) && fee.recipients.length > 0 && fee.recipients.length <= 64, "fee.recipients must contain 1..64 rows.");
    const recipients = fee.recipients.map((row, index) => Object.freeze({ recipient: identity(row.recipient, `fee.recipients[${index}].recipient`), feeAtoms: u64(row.feeAtoms, `fee.recipients[${index}].feeAtoms`).toString(), disposition: text(row.disposition, `fee.recipients[${index}].disposition`, 48) }));
    assert(sum(recipients.map((row) => BigInt(row.feeAtoms))) === BigInt(selectedFeeAtoms), "Recipient allocation must conserve the selected fee total exactly.");
    return Object.freeze({ feeRecord: identity(fee.feeRecord, "fee.feeRecord"), settlementCandidate: selectedCandidate, revenuePolicy: identity(fee.revenuePolicy, "fee.revenuePolicy"), treasuryPosition: identity(fee.treasuryPosition, "fee.treasuryPosition"), selectedFeeAtoms, recipients, treasuryCreditedAtoms: u64(fee.treasuryCreditedAtoms, "fee.treasuryCreditedAtoms").toString() });
  };

  const validateLiveness = (raw) => {
    assert(plain(raw), "components.liveness must be an object.");
    assert(Array.isArray(raw.compartments) && raw.compartments.length === CONTRACTS.orders.livenessCompartments.length, "Liveness projection must contain all seven compartments in canonical order.");
    const compartments = raw.compartments.map((row, index) => {
      const expected = CONTRACTS.orders.livenessCompartments[index];
      assert(row.kind === expected, `Liveness compartment ${index} must be ${expected}.`);
      const maximumCalls = numberU32(row.maximumCalls, `${expected}.maximumCalls`);
      const remainingCalls = numberU32(row.remainingCalls, `${expected}.remainingCalls`);
      assert(maximumCalls > 0 && remainingCalls <= maximumCalls, `${expected} call counters are inconsistent.`);
      const capitalized = u64(row.capitalizedWorkLamports, `${expected}.capitalizedWorkLamports`);
      const remaining = u64(row.remainingWorkLamports, `${expected}.remainingWorkLamports`);
      assert(remaining <= capitalized, `${expected} remaining work exceeds capitalized work.`);
      return Object.freeze({ kind: expected, phase: text(row.phase, `${expected}.phase`, 32), account: identity(row.account, `${expected}.account`), maximumCalls, remainingCalls, capitalizedWorkLamports: capitalized.toString(), remainingWorkLamports: remaining.toString(), rentPrincipalLamports: u64(row.rentPrincipalLamports, `${expected}.rentPrincipalLamports`).toString(), donationRemainingLamports: u64(row.donationRemainingLamports, `${expected}.donationRemainingLamports`).toString() });
    });
    return Object.freeze({ policyId: identity(raw.policyId, "liveness.policyId"), lifecycleId: identity(raw.lifecycleId, "liveness.lifecycleId"), neutralSink: identity(raw.neutralSink, "liveness.neutralSink"), compartments: Object.freeze(compartments) });
  };

  const validateSeries = (raw) => {
    assert(plain(raw), "components.series must be an object.");
    assert(Array.isArray(raw.components) && raw.components.length === CONTRACTS.orders.seriesComponents.length, "Series must contain all five funding components in canonical order.");
    const instanceCount = numberU32(raw.instanceCount, "series.instanceCount");
    const nextOrdinal = numberU32(raw.nextOrdinal, "series.nextOrdinal");
    const lapsedCount = numberU32(raw.lapsedCount, "series.lapsedCount");
    assert(instanceCount > 0 && nextOrdinal <= instanceCount && lapsedCount <= nextOrdinal, "Series ordinal/lapse counters are inconsistent.");
    const components = raw.components.map((row, index) => {
      const expected = CONTRACTS.orders.seriesComponents[index];
      assert(row.kind === expected, `Series funding component ${index} must be ${expected}.`);
      return Object.freeze({ kind: expected, remainingLamports: u64(row.remainingLamports, `${expected}.remainingLamports`).toString(), remainingCollateralAtoms: u64(row.remainingCollateralAtoms, `${expected}.remainingCollateralAtoms`).toString(), donationLamports: u64(row.donationLamports, `${expected}.donationLamports`).toString(), donationCollateralAtoms: u64(row.donationCollateralAtoms, `${expected}.donationCollateralAtoms`).toString(), consumedAllocations: numberU32(row.consumedAllocations, `${expected}.consumedAllocations`) });
    });
    return Object.freeze({ seriesPlanId: identity(raw.seriesPlanId, "series.seriesPlanId"), fundingTermsId: identity(raw.fundingTermsId, "series.fundingTermsId"), fundingQuoteId: identity(raw.fundingQuoteId, "series.fundingQuoteId"), phase: text(raw.phase, "series.phase", 40), instanceCount, nextOrdinal, lapsedCount, components: Object.freeze(components) });
  };

  const validateSource = (raw) => {
    assert(plain(raw), "components.sourcePlaneV3 must be an object.");
    return Object.freeze({ sourcePlaneId: identity(raw.sourcePlaneId, "sourcePlaneV3.sourcePlaneId"), sourceSpecId: identity(raw.sourceSpecId, "sourcePlaneV3.sourceSpecId"), parserReleaseId: identity(raw.parserReleaseId, "sourcePlaneV3.parserReleaseId"), summaryProgramId: identity(raw.summaryProgramId, "sourcePlaneV3.summaryProgramId"), repairGeneration: u64(raw.repairGeneration, "sourcePlaneV3.repairGeneration").toString(), requiredLastBucket: u64(raw.requiredLastBucket, "sourcePlaneV3.requiredLastBucket").toString(), lastIngestedBucket: u64(raw.lastIngestedBucket, "sourcePlaneV3.lastIngestedBucket").toString(), rawPageCount: numberU32(raw.rawPageCount, "sourcePlaneV3.rawPageCount"), headPhase: text(raw.headPhase, "sourcePlaneV3.headPhase", 40), windowPhase: text(raw.windowPhase, "sourcePlaneV3.windowPhase", 40), resultPhase: text(raw.resultPhase, "sourcePlaneV3.resultPhase", 40) });
  };

  const validateStructured = (rows) => {
    assert(Array.isArray(rows) && rows.length <= 64, "components.structuredClaims must be an array of at most 64 descriptors.");
    return Object.freeze(rows.map((row, index) => Object.freeze({ descriptor: identity(row.descriptor, `structuredClaims[${index}].descriptor`), wrapperProductId: identity(row.wrapperProductId, `structuredClaims[${index}].wrapperProductId`), market: identity(row.market, `structuredClaims[${index}].market`), state: text(row.state, `structuredClaims[${index}].state`, 24), mint: identity(row.mint, `structuredClaims[${index}].mint`), mintSupply: u64(row.mintSupply, `structuredClaims[${index}].mintSupply`).toString(), vaultBackingCashAtoms: u64(row.vaultBackingCashAtoms, `structuredClaims[${index}].vaultBackingCashAtoms`).toString(), beneficiaryFreeSurplusAtoms: u64(row.beneficiaryFreeSurplusAtoms, `structuredClaims[${index}].beneficiaryFreeSurplusAtoms`).toString() })));
  };

  const validateProjection = (raw) => {
    assert(plain(raw) && raw.schema === "dragons-clutch.account-projection.v1", "Projection schema must be dragons-clutch.account-projection.v1.");
    assert(plain(raw.observedAt), "observedAt is required.");
    const cluster = text(raw.observedAt.cluster, "observedAt.cluster", 32);
    assert(CONTRACTS.clusters.some((item) => item.id === cluster), "Projection cluster is not recognized.");
    const slot = u64(raw.observedAt.slot, "observedAt.slot");
    const commitment = text(raw.observedAt.commitment, "observedAt.commitment", 24);
    assert(["processed", "confirmed", "finalized", "local-bank"].includes(commitment), "Projection commitment is not recognized.");
    assert(plain(raw.release), "release binding is required.");
    assert(BASE58.test(raw.release.programId), "release.programId must be base58.");
    assert(BASE58.test(raw.release.programData), "release.programData must be base58.");
    assert(SHA256.test(raw.release.elfSha256), "release.elfSha256 must be lowercase SHA-256.");
    assert(SHA256.test(raw.release.manifestSha256), "release.manifestSha256 must be lowercase SHA-256.");
    assert(COMMIT.test(raw.release.sourceCommit), "release.sourceCommit must be a full lowercase commit hash.");
    const deploymentSlot = u64(raw.release.deploymentSlot, "release.deploymentSlot");
    assert(deploymentSlot <= slot, "Deployment slot cannot be later than the observation slot.");
    assert(plain(raw.provenance) && raw.provenance.method === "user-supplied-account-observation", "Provenance method must explicitly be user-supplied-account-observation.");
    assert(Array.isArray(raw.provenance.accountReads) && raw.provenance.accountReads.length > 0 && raw.provenance.accountReads.length <= 256, "provenance.accountReads must contain 1..256 exact observations.");
    const accountReads = Object.freeze(raw.provenance.accountReads.map((row, index) => validateAccountRead(row, index, slot)));
    assert(plain(raw.components), "components is required.");
    const generalV2 = raw.components.generalV2 ? validateOwnerRows(raw.components.generalV2) : null;
    const fee = raw.components.fee ? validateFee(raw.components.fee, generalV2 && generalV2.selectedCandidate, generalV2 && generalV2.selectedFeeAtoms) : null;
    assert(!fee || generalV2, "A fee projection requires its complete General V2 owner settlement book.");
    const components = Object.freeze({ generalV2, fee, sourcePlaneV3: raw.components.sourcePlaneV3 ? validateSource(raw.components.sourcePlaneV3) : null, liveness: raw.components.liveness ? validateLiveness(raw.components.liveness) : null, series: raw.components.series ? validateSeries(raw.components.series) : null, structuredClaims: raw.components.structuredClaims ? validateStructured(raw.components.structuredClaims) : Object.freeze([]) });
    assert(Object.values(components).some((value) => value && (!Array.isArray(value) || value.length > 0)), "Projection must contain at least one recognized protocol component.");
    const genesisHash = raw.observedAt.genesisHash === null || raw.observedAt.genesisHash === undefined ? null : text(raw.observedAt.genesisHash, "observedAt.genesisHash", 96);
    assert(genesisHash === null || BASE58.test(genesisHash), "observedAt.genesisHash must be a base58 hash or null.");
    return Object.freeze({ schema: raw.schema, trust: "untrusted-projection", observedAt: Object.freeze({ cluster, slot: slot.toString(), commitment, genesisHash }), release: Object.freeze({ programId: raw.release.programId, programData: raw.release.programData, deploymentSlot: deploymentSlot.toString(), elfSha256: raw.release.elfSha256, manifestSha256: raw.release.manifestSha256, sourceCommit: raw.release.sourceCommit, capabilityProfileId: optionalIdentity(raw.release.capabilityProfileId, "release.capabilityProfileId") }), provenance: Object.freeze({ method: raw.provenance.method, accountReads }), components });
  };

  const definition = (term, description) => {
    const row = create("div");
    row.append(create("dt", null, term), create("dd", null, description));
    return row;
  };
  const short = (value) => value.length > 18 ? `${value.slice(0, 9)}…${value.slice(-7)}` : value;
  const stateCard = (title, status) => {
    const card = create("article", "runtime-card");
    const heading = create("div", "card-heading");
    heading.append(create("h3", null, title), create("span", "evidence-chip local-chip", status));
    card.append(heading);
    return card;
  };
  const renderProjection = (projection) => {
    state.projection = projection;
    $("projection-status").className = "configuration-result ready";
    $("projection-status").textContent = `Loaded ${projection.provenance.accountReads.length} user-supplied account observations at ${projection.observedAt.cluster} slot ${projection.observedAt.slot}. Shape and exact-integer joins passed; chain authenticity was not established by this page.`;
    const target = $("protocol-state-grid");
    reset(target);
    const { generalV2, fee, sourcePlaneV3, liveness, series, structuredClaims } = projection.components;
    if (generalV2) {
      const card = stateCard("General V2 · owner settlement", `${generalV2.ownerCount} owners`);
      card.append(create("p", "runtime-summary", `Candidate ${short(generalV2.selectedCandidate)} · price scale ${generalV2.priceScale} · selected fee ${generalV2.selectedFeeAtoms} atoms · rounding pot ${generalV2.roundingPotPriceUnits} price units.`));
      const wrap = create("div", "runtime-table-wrap");
      const table = create("table", "runtime-table");
      table.innerHTML = "<thead><tr><th>Owner</th><th>Buy units</th><th>Sell units</th><th>Debit</th><th>Credit</th><th>Fee</th><th>Position after</th></tr></thead>";
      const body = create("tbody");
      generalV2.owners.forEach((row) => {
        const tr = create("tr");
        [short(row.owner), row.buyPriceUnits, row.sellPriceUnits, row.debitAtoms, row.creditAtoms, row.selectedFeeAtoms, `${row.nextCashAtoms} / ${row.nextReservedCashAtoms} reserved`].forEach((cell) => tr.append(create("td", null, cell)));
        body.append(tr);
      });
      table.append(body); wrap.append(table); card.append(wrap); target.append(card);
    }
    if (fee) {
      const card = stateCard("Fees · allocation and custody", `${fee.selectedFeeAtoms} atoms`);
      const list = create("dl", "runtime-facts");
      list.append(definition("Fee record", short(fee.feeRecord)), definition("Revenue policy", short(fee.revenuePolicy)), definition("Treasury Position", short(fee.treasuryPosition)), definition("Credited", `${fee.treasuryCreditedAtoms} atoms`), definition("Recipients", fee.recipients.map((row) => `${short(row.recipient)}: ${row.feeAtoms} (${row.disposition})`).join(" · ")));
      card.append(list, create("p", "runtime-boundary", "Future fees are revenue only after exact settlement. They are not liveness capitalization; treasury custody is an ordinary Position, not Hoard principal.")); target.append(card);
    }
    if (sourcePlaneV3) {
      const card = stateCard("SourcePlane V3", sourcePlaneV3.resultPhase);
      const list = create("dl", "runtime-facts");
      list.append(definition("Source contract", short(sourcePlaneV3.sourcePlaneId)), definition("Source spec", short(sourcePlaneV3.sourceSpecId)), definition("Parser release", short(sourcePlaneV3.parserReleaseId)), definition("Generation", sourcePlaneV3.repairGeneration), definition("Head / window", `${sourcePlaneV3.headPhase} / ${sourcePlaneV3.windowPhase}`), definition("Buckets", `${sourcePlaneV3.lastIngestedBucket} of required ${sourcePlaneV3.requiredLastBucket}`), definition("Raw pages", String(sourcePlaneV3.rawPageCount)));
      card.append(list, create("p", "runtime-boundary", "This is a projection of the authenticated lineage contract. The page did not invoke the parser, inspect ProgramData, read Clock, or establish account/PDA ownership.")); target.append(card);
    }
    if (liveness) {
      const card = stateCard("Prepaid liveness", "7 compartments");
      const wrap = create("div", "compartment-grid");
      liveness.compartments.forEach((row) => {
        const item = create("section", "compartment-item");
        item.append(create("h4", null, row.kind), create("p", null, `${row.phase} · ${row.remainingCalls}/${row.maximumCalls} calls`), create("code", null, `${row.remainingWorkLamports}/${row.capitalizedWorkLamports} work lamports`), create("small", null, `rent ${row.rentPrincipalLamports} · donation ${row.donationRemainingLamports}`)); wrap.append(item);
      });
      card.append(wrap, create("p", "runtime-boundary", "Every mandatory compartment is present. Collateral, Hoard principal, and future fee revenue are deliberately absent from this capital view.")); target.append(card);
    }
    if (series) {
      const card = stateCard("Series lifecycle", series.phase);
      card.append(create("p", "runtime-summary", `${short(series.seriesPlanId)} · next ordinal ${series.nextOrdinal}/${series.instanceCount} · ${series.lapsedCount} lapsed.`));
      const wrap = create("div", "compartment-grid five");
      series.components.forEach((row) => { const item = create("section", "compartment-item"); item.append(create("h4", null, row.kind), create("code", null, `${row.remainingLamports} lamports`), create("small", null, `${row.remainingCollateralAtoms} collateral atoms · ${row.consumedAllocations} allocations`)); wrap.append(item); });
      card.append(wrap, create("p", "runtime-boundary", "Principal and donations are displayed separately by component. Occurrence creation must authenticate SourcePlane, collateral, liveness, registry, and failure receipts atomically.")); target.append(card);
    }
    if (structuredClaims.length > 0) {
      const card = stateCard("Structured claims", `${structuredClaims.length} descriptors`);
      const wrap = create("div", "claim-list");
      structuredClaims.forEach((row) => { const item = create("section", "claim-item"); item.append(create("h4", null, short(row.wrapperProductId)), create("p", null, `${row.state} · supply ${row.mintSupply}`), create("small", null, `vault cash ${row.vaultBackingCashAtoms} · beneficiary-free surplus ${row.beneficiaryFreeSurplusAtoms}`)); wrap.append(item); });
      card.append(wrap, create("p", "runtime-boundary", "Wrapper supply is the authenticated Token-2022 mint supply, never a descriptor shadow. Surplus is beneficiary-free and cannot become fee or treasury revenue.")); target.append(card);
    }
  };

  const encodeSeriesPayload = (kind, fields) => {
    const out = bytes();
    const plan = identity(fields.seriesPlanId, "seriesPlanId");
    putHex(out, plan, "seriesPlanId");
    if (kind === "series-register") {
      putHex(out, identity(fields.fundingTermsId, "fundingTermsId"), "fundingTermsId");
      putHex(out, identity(fields.registryReleaseId, "registryReleaseId"), "registryReleaseId");
      putHex(out, identity(fields.capabilityProfileId, "capabilityProfileId"), "capabilityProfileId");
    } else if (kind === "series-advance") {
      putLe(out, BigInt(numberU32(fields.ordinal, "ordinal")), 4); putZeros(out, 4);
      putHex(out, identity(fields.sourceOccurrenceId, "sourceOccurrenceId"), "sourceOccurrenceId");
      putHex(out, identity(fields.marketInstanceId, "marketInstanceId"), "marketInstanceId");
    } else if (kind === "series-lapse") {
      putLe(out, BigInt(numberU32(fields.ordinal, "ordinal")), 4); putZeros(out, 4);
    } else if (kind === "series-observe-donation") {
      assert(Object.hasOwn(SERIES_COMPONENTS, fields.component), "component is not a canonical Series component.");
      assert(fields.asset === "lamports" || fields.asset === "collateral", "asset must be lamports or collateral.");
      putU8(out, SERIES_COMPONENTS[fields.component]); putU8(out, fields.asset === "lamports" ? 1 : 2); putZeros(out, 6);
    }
    return out;
  };

  const wrapSeriesRequest = (action, payload, sequence) => {
    const inner = [77, 2, action, ...payload];
    assert(inner.length <= 402, "Successor intent exceeds the frozen 402-byte inner ceiling.");
    const out = [0xd1, 1];
    putLe(out, sequence, 8); putU8(out, 0); putLe(out, BigInt(inner.length), 2); out.push(...inner);
    return out;
  };

  const encodeLiveness = (fields) => {
    assert(Object.hasOwn(LIVENESS_ACTIONS, fields.action), "action is not a liveness transition.");
    assert(Object.hasOwn(LIVENESS_KINDS, fields.kind), "kind is not a canonical liveness compartment.");
    const action = LIVENESS_ACTIONS[fields.action];
    const out = bytes(); putAscii(out, "DCLINT01"); putLe(out, 1n, 2); putU8(out, action); putU8(out, LIVENESS_KINDS[fields.kind]); putZeros(out, 4);
    ["policyId", "lifecycleId", "accountId", "semanticOwner", "quoteScheduleId"].forEach((name) => putHex(out, identity(fields[name], name), name));
    const receipt = action === 0 ? null : identity(fields.receiptId, "receiptId");
    const keeper = action === 1 ? identity(fields.keeper, "keeper") : null;
    if (receipt) putHex(out, receipt, "receiptId"); else putZeros(out, 32);
    if (keeper) putHex(out, keeper, "keeper"); else putZeros(out, 32);
    putLe(out, u64(fields.generation, "generation"), 8);
    const ordinal = action === 1 ? numberU32(fields.callOrdinal, "callOrdinal") : 0;
    assert(action !== 1 || ordinal > 0, "SpendWork callOrdinal must be positive.");
    putLe(out, BigInt(ordinal), 4); putZeros(out, 4);
    const ceiling = action === 1 ? u64(fields.callCeilingLamports, "callCeilingLamports") : 0n;
    const payment = action === 1 ? u64(fields.keeperPaymentLamports, "keeperPaymentLamports") : 0n;
    assert(action !== 1 || (ceiling > 0n && payment <= ceiling), "SpendWork payment must fit a positive authenticated call ceiling.");
    putLe(out, ceiling, 8); putLe(out, payment, 8); assert(out.length === 272, "Internal liveness encoder width mismatch."); return out;
  };

  const encodeStructured = (kind, fields) => {
    const out = bytes();
    if (kind === "structured-create") {
      putHex(out, identity(fields.nativeClaimId, "nativeClaimId"), "nativeClaimId");
      putHex(out, identity(fields.wrapperProductId, "wrapperProductId"), "wrapperProductId");
      assert(Array.isArray(fields.primitive) && fields.primitive.length === 16, "primitive must contain exactly 16 u64 coefficient strings, including canonical zero padding.");
      fields.primitive.forEach((value, index) => putLe(out, u64(value, `primitive[${index}]`), 8));
      assert(out.length === 192, "Structured descriptor payload width mismatch.");
      return out;
    }
    putHex(out, identity(fields.wrapperProductId, "wrapperProductId"), "wrapperProductId");
    if (["structured-compact-donation", "structured-retire"].includes(kind)) {
      putLe(out, u64(fields.vaultGeneration, "vaultGeneration"), 8); putLe(out, u64(fields.vaultReplaySequence, "vaultReplaySequence"), 8);
      assert(out.length === 48, "Structured vault payload width mismatch.");
      return out;
    }
    const quantity = u64(fields.quantity, "quantity"); assert(quantity > 0n, "quantity must be positive."); putLe(out, quantity, 8);
    putLe(out, u64(fields.userGeneration, "userGeneration"), 8); putLe(out, u64(fields.userReplaySequence, "userReplaySequence"), 8); putLe(out, u64(fields.vaultGeneration, "vaultGeneration"), 8); putLe(out, u64(fields.vaultReplaySequence, "vaultReplaySequence"), 8);
    assert(out.length === 72, "Structured quantity payload width mismatch."); return out;
  };

  const construct = (kind, fields, sequence) => {
    assert(plain(fields), "Intent fields must be a JSON object.");
    const target = state.configuration;
    if (Object.hasOwn(SERIES_ACTIONS, kind)) {
      const payload = encodeSeriesPayload(kind, fields);
      const data = wrapSeriesRequest(SERIES_ACTIONS[kind], payload, sequence);
      return Object.freeze({ schema: "dragons-clutch.unsigned-instruction.v1", contract: "source-series-v2", construction: "exact-successor-wire", sourceCommit: "cd51aba556a38fcba5326007f4f70e28cd825835", target, programId: target && target.programId, instructionDataEncoding: "hex", instructionData: hex(data), byteLength: data.length, accountMetas: null, executableCapability: false, expectedRuntimeResult: "UnsupportedInstruction", disabledReason: "The six action codecs are exact, but the central executable extension set is empty and the authenticated account-meta/runtime joins are not released.", authorization: { signer: null, signatures: [], submission: "absent" } });
    }
    if (kind === "liveness-transition") {
      const data = encodeLiveness(fields);
      return Object.freeze({ schema: "dragons-clutch.unsigned-inner-intent.v1", contract: "prepaid-liveness-v1", construction: "exact-inner-contract-only", sourceCommit: "d5d76c39327928be2aeeeb190de8a91734159580", target, innerDataEncoding: "hex", innerData: hex(data), byteLength: data.length, outerInstruction: null, disabledReason: "DCLINT01 is exact, but no central outer action, account-meta table, or SBF route owns it yet.", authorization: { signer: null, signatures: [], submission: "absent" } });
    }
    if (Object.hasOwn(STRUCTURED_ACTIONS, kind)) {
      const data = encodeStructured(kind, fields);
      return Object.freeze({ schema: "dragons-clutch.unsigned-inner-intent.v1", contract: "structured-claim-v1", localAction: STRUCTURED_ACTIONS[kind], construction: "exact-family-payload-only", sourceCommit: "8838df3b3e52d372b317a7e51df9fd3034e8bc43", target, innerDataEncoding: "hex", innerData: hex(data), byteLength: data.length, outerInstruction: null, disabledReason: "The family-local payload is exact, but the central registry allocates no structured local action and has not adopted descriptor account 0x88/1.", authorization: { signer: null, signatures: [], submission: "absent" } });
    }
    throw new Error("The selected construction contract is unknown.");
  };

  const renderInventory = () => {
    const target = $("protocol-capabilities"); reset(target);
    CONTRACTS.capabilities.forEach((capability) => {
      const card = create("article", `capability-card ${capability.enabled ? "enabled" : "disabled"}`);
      const header = create("div", "card-heading"); header.append(create("h3", null, capability.label), create("span", `evidence-chip ${capability.enabled ? "local-chip" : "stop-chip"}`, capability.enabled ? "LOCAL" : "DISABLED"));
      card.append(header, create("p", null, capability.reason)); target.append(card);
    });
    const contracts = $("contract-inventory"); reset(contracts);
    CONTRACTS.components.forEach((component) => {
      const card = create("article", "contract-card");
      card.append(create("p", "contract-state", component.state), create("h3", null, component.label), create("code", null, component.sourceCommit), create("p", null, component.facts.join(" · ")));
      contracts.append(card);
    });
  };

  const setProtocolError = (targetId, message) => { const node = $(targetId); node.hidden = false; node.textContent = message; };
  const clearProtocolError = (targetId) => { const node = $(targetId); node.hidden = true; node.textContent = ""; };
  const copy = async (value, button) => {
    if (!value) throw new Error("There is nothing to copy.");
    if (navigator.clipboard && window.isSecureContext) await navigator.clipboard.writeText(value);
    else {
      const area = document.createElement("textarea"); area.value = value; area.readOnly = true; area.className = "copy-area"; document.body.append(area); area.select();
      if (!document.execCommand("copy")) { area.remove(); throw new Error("The browser refused the local copy operation."); }
      area.remove();
    }
    const original = button.textContent; button.textContent = "Copied"; window.setTimeout(() => { button.textContent = original; }, 1000);
  };

  const init = () => {
    if (!CONTRACTS || CONTRACTS.schema !== "dragons-clutch.static-protocol-inventory.v1") return;
    CONTRACTS.clusters.forEach((cluster) => { const option = create("option", null, cluster.label); option.value = cluster.id; $("cluster-target").append(option); });
    $("cluster-target").value = "localnet"; $("rpc-target").value = CONTRACTS.clusters[0].rpcEndpoint;
    $("cluster-target").addEventListener("change", () => { const cluster = CONTRACTS.clusters.find((item) => item.id === $("cluster-target").value); $("rpc-target").value = cluster.rpcEndpoint; });
    $("configuration-form").addEventListener("submit", (event) => { event.preventDefault(); clearProtocolError("configuration-error"); try { renderConfiguration(releaseConfiguration()); } catch (error) { setProtocolError("configuration-error", error.message); } });
    $("projection-form").addEventListener("submit", (event) => { event.preventDefault(); clearProtocolError("projection-error"); try { renderProjection(validateProjection(JSON.parse($("projection-input").value))); } catch (error) { setProtocolError("projection-error", error instanceof SyntaxError ? `Projection JSON is invalid: ${error.message}` : error.message); } });
    $("intent-builder-form").addEventListener("submit", (event) => { event.preventDefault(); clearProtocolError("builder-error"); try { const output = construct($("protocol-intent-kind").value, JSON.parse($("protocol-intent-fields").value), u64($("protocol-sequence").value, "sequence")); state.output = JSON.stringify(output, null, 2); $("protocol-intent-output").textContent = state.output; $("protocol-intent-status").textContent = output.construction; $("copy-protocol-intent").disabled = false; } catch (error) { $("protocol-intent-status").textContent = "refused"; $("copy-protocol-intent").disabled = true; setProtocolError("builder-error", error instanceof SyntaxError ? `Intent JSON is invalid: ${error.message}` : error.message); } });
    $("copy-protocol-intent").addEventListener("click", () => copy(state.output, $("copy-protocol-intent")).catch((error) => setProtocolError("builder-error", error.message)));
    renderInventory(); renderConfiguration(releaseConfiguration());
  };

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", init); else init();
  root.StaticProtocolConsole = Object.freeze({ canonicalJson, construct, encodeLiveness, encodeSeriesPayload, encodeStructured, validateProjection, wrapSeriesRequest });
})(typeof globalThis === "object" ? globalThis : this);
