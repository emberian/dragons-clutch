//! General family integration for the canonical Trading controller.
//!
//! This module is not a dispatcher and owns no executable authority. It
//! consumes the common layer's preauthenticated context and produces family
//! plans which the common layer may apply atomically. General receives no
//! Registry Program binding of its own: the one
//! [`ExecutionRoleV1::Trading`](dclutch_release_set_contract::ExecutionRoleV1)
//! Program is this crate, and every General account below is derived under it.
//!
//! # Root
//!
//! There is one composite Trading-owned root per capability:
//! `CapabilityRootHeaderV1(232) || GeneralRootV2`. The header is immutable,
//! proves identity only, and is derived under `CAPABILITY_ROOT_PDA_DOMAIN_V1`
//! from `[market, generation, manifest, entry_index, kind, capability_release,
//! config]`. `GeneralRootV2` is the mutable family tail selected by the
//! descriptor's `root_schema`; the common layer splits it off with
//! `split_root_account_mut_v1` and hands it to this module, which owns its
//! lifecycle refusal. General mints no root PDA domain of its own.
//!
//! # State
//!
//! Every General-owned account is `[domain, market, ..]` under this Program:
//!
//! - selection cursor, `[GENERAL_SELECTION_PDA_DOMAIN_V1, market, batch]`;
//! - verification cursor, `[GENERAL_VERIFICATION_PDA_DOMAIN_V1, market,
//!   candidate]`;
//! - verified certificate, `[GENERAL_CERTIFICATE_PDA_DOMAIN_V1, market,
//!   candidate]`;
//! - settlement cursor, `[GENERAL_SETTLEMENT_PDA_DOMAIN_V1, market,
//!   candidate]`;
//! - candidate header, `[GENERAL_CANDIDATE_PDA_DOMAIN_V1, market, candidate]`;
//! - selection policy, `[GENERAL_POLICY_PDA_DOMAIN_V1, market, policy]`;
//! - candidate page, `[GENERAL_PAGE_PDA_DOMAIN_V1, market, candidate,
//!   pageIndexLE]`.
//!
//! Candidate, policy, and page accounts are immutable General-owned input
//! records at those addresses. Their bytes are the sole semantic facts; this
//! module persists no second projection. Selection and verification may be
//! zero-filled only at their explicit initialization transition.
//!
//! # Frames
//!
//! Every route starts with the same five readonly accounts: Core Market,
//! Registry activation cache, Registry Program, this Trading Program, and its
//! ProgramData. The common outer, not this module, obtains the
//! `AuthenticatedRoleReceiptV1` behind them.
//!
//! | Action | Accounts | Suffix from index 5 |
//! | --- | ---: | --- |
//! | `Consider` | 12 | selection, verification, certificate, candidate, policy, page, incumbent-or-Market |
//! | `Freeze` | 6 | selection |
//! | `InitializeSettlement` | 9 | selection, settlement cursor, certificate, candidate |
//! | `Collect`/`Materialize`/`Distribute`/`Close` | 28 | see the `controller` module |
//!
//! Each `Consider` call consumes one page and advances the verifier
//! atomically. The last page performs candidate-wide per-order rounding once,
//! writes the certificate, compares immutable policy criteria with the
//! mandatory candidate-ID tie-break, and updates selection.
//!
//! # Child boundary
//!
//! The settlement frame carries the selected Claims and Custody Programs and
//! two request-derived `CallerAuthoritySeedsV1` signers. This family defines no
//! General-private Claims, Custody, or receipt wire: `SettlementChildrenV1`
//! states only the semantic requirements and passes General-owned replay
//! context, while Claims owns its effect plan and request digest and Custody
//! owns its compartment-transfer plan. Each child's canonical return-data
//! receipt is consumed immediately, before another CPI can overwrite it, and
//! General cursor bytes commit only after every active child accepts.

/// Pure, preauthenticated General activation planning.
pub mod activation;
/// Commit-last fixed-role child execution for General settlement.
pub mod controller;
/// Runtime-width candidate selection and settlement dispatch.
pub mod hot_controller;
/// Authenticated action routing and exact account-derived settlement inputs.
pub mod route;
/// Atomic two-pass settlement preparation and exact fixed-role child packets.
pub mod settlement;
