/*
 * Browser-side mirror of the central successor coordinate namespace.
 *
 * This allocates wire identity only. It is not a capability manifest and no
 * entry in this file is evidence that a dispatcher route is enabled.
 */
(function (root) {
  "use strict";

  const defineActions = (names, first = 1) => Object.freeze(names.map((name, index) => Object.freeze({
    name,
    localAction: first + index
  })));

  const GENERAL = defineActions([
    "CreateMarket", "InitEpoch", "InitOrderPage", "PlaceOrder", "CancelOrder",
    "FreezeEpoch", "BeginCandidate", "WriteCandidateFeed", "SealCandidate",
    "InitClearWork", "GrowClearWork", "AdvanceClearOrders", "AdvanceClearSlices",
    "CompleteCandidateVerification", "FinalizeSelection", "ExpireCandidate",
    "MarkWorkClosed", "ClaimCandidateBond", "ClaimCandidateWork", "CleanupCandidate",
    "ClaimSolver", "CloseCandidateIndexPage", "ClaimEpochUnused", "FreezeEntitlement",
    "AccountReceiptEnd", "ConsumeDirectReceiptEggs", "CloseReceipt", "CloseReservation",
    "ClosePage", "ClosePot", "CloseCandidate", "CloseClearWork", "CloseEpoch",
    "ClosePosition", "TransferPositionAssets", "ConsumeVirtualSplitReceiptEggs",
    "ConsumeVirtualMergeReceiptEggs", "FinalizeOwnerSettlement"
  ]);
  const STRUCTURED = defineActions([
    "CreateDescriptor", "WrapCanonical", "WrapFull", "UnwrapCanonical", "UnwrapFull",
    "CompactDonation", "RedeemTerminal", "RetireDescriptor"
  ]);
  const SOURCE = defineActions([
    "RegisterRelease", "InitializeHead", "OpenRawPage", "IngestBoundaryBatch",
    "SealRawPage", "InitializeWindowWork", "FoldWindowPages", "SealWindow",
    "EvaluateStatistic", "EmitFailureHandoff", "ReopenGeneration", "CloseGeneration"
  ]);
  const SERIES = defineActions([
    "RegisterSeries", "ActivateFunding", "AdvanceOccurrence", "LapseOccurrence",
    "ObserveDonation", "CloseFunding"
  ], 13);
  const RECOVERY = defineActions([
    "InitializeFailureRoot", "TriggerSourceFailure", "TriggerRelationRefusal",
    "AdvanceRecoverySchedule", "AcceptRecoveryWork", "ResolveCallerFunded",
    "ResolvePaidRecovery", "CloseRecoveryFunding", "CloseFailureRoot"
  ]);
  const DEALER = defineActions([
    "BeginPolicy", "WritePolicy", "SealPolicy", "AbortPolicy", "Initialize",
    "CreateLpPage", "Contribute", "WithdrawFunding", "Activate", "CancelFunding",
    "RefundCancelledSponsor", "BindEpoch", "LapseEpoch", "SelectLeaseAndBegin",
    "Collect", "Deliver", "FinalizeSettlement", "AbortBeforeCollection", "QueueExit",
    "SponsorHalt", "EnterUnwind", "TimedClose", "Resolve", "Claim", "Retire"
  ]);

  const families = Object.freeze({
    general: Object.freeze({
      name: "general", label: "General V2", tag: 74, version: 1,
      allocationStatus: "reserved-disabled", actions: GENERAL,
      operatorFamily: "general"
    }),
    structured: Object.freeze({
      name: "structured", label: "Structured claims", tag: 75, version: 1,
      allocationStatus: "reserved-disabled", actions: STRUCTURED,
      operatorFamily: "structured-claim"
    }),
    dealer: Object.freeze({
      name: "dealer", label: "Covered Dealer", tag: 76, version: 1,
      allocationStatus: "reserved-disabled", actions: DEALER,
      operatorFamily: "dealer"
    }),
    source: Object.freeze({
      name: "source", label: "SourcePlane V3", tag: 77, version: 2,
      allocationStatus: "reserved-disabled", actions: SOURCE,
      operatorFamily: "source"
    }),
    series: Object.freeze({
      name: "series", label: "Recurring Series", tag: 77, version: 2,
      allocationStatus: "reserved-disabled", actions: SERIES,
      operatorFamily: "series"
    }),
    recovery: Object.freeze({
      name: "recovery", label: "Failure recovery", tag: 78, version: 1,
      allocationStatus: "reserved-disabled", actions: RECOVERY,
      operatorFamily: "failure"
    })
  });

  const keeperCoordinates = Object.freeze({
    "init-epoch": Object.freeze({ family: "general", localAction: 2 }),
    "init-clear-work": Object.freeze({ family: "general", localAction: 10 }),
    "freeze-entitlement": Object.freeze({ family: "general", localAction: 24 }),
    "open-raw-page": Object.freeze({ family: "source", localAction: 3 }),
    "close-position": Object.freeze({ family: "general", localAction: 34 })
  });

  const family = (name) => {
    const value = families[name];
    if (!value) throw new Error(`Unknown successor family ${JSON.stringify(name)}.`);
    return value;
  };

  const action = (familyName, localAction) => {
    const owner = family(familyName);
    if (!Number.isInteger(localAction) || localAction < 1 || localAction > 255) {
      throw new Error("localAction must be an integer in 1..255.");
    }
    const value = owner.actions.find((candidate) => candidate.localAction === localAction);
    if (!value) {
      throw new Error(owner.disabledReason || `Local action ${localAction} is not allocated inside ${owner.label}.`);
    }
    return Object.freeze({
      family: owner.name,
      familyLabel: owner.label,
      tag: owner.tag,
      version: owner.version,
      localAction: value.localAction,
      actionName: value.name,
      allocationStatus: owner.allocationStatus,
      operatorFamily: owner.operatorFamily
    });
  };

  root.GlassSuccessorRegistry = Object.freeze({ families, keeperCoordinates, family, action });
})(typeof globalThis === "object" ? globalThis : this);
