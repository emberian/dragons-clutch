//! Chain-derived issue, unwrap, terminal-redeem, and retire planning.

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;
use dclutch_structured_v2_contract::{
    StructuredActionV2, StructuredHotTokenKindV2, StructuredRequestInputV2, StructuredRequestV2,
};
use dclutch_structured_v2_kernel::{
    ReceiptEffectV2, STRUCTURED_NO_COORDINATE_V2, ShardMovementV2,
    StructuredCoordinateObservationV2, StructuredPhaseV2, StructuredProjectionV2,
    StructuredSettlementRowV2, StructuredTermsV2, encode_structured_projection_v2,
    plan_structured_issue_v2, plan_structured_retire_v2, plan_structured_terminal_redeem_v2,
    plan_structured_unwrap_v2, structured_projection_bytes_v2,
};

use crate::{Error, Result};

/// Exact semantic coordinates obtained from authenticated chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRequestContextV2 {
    /// Current execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product graph-root digest.
    pub product_record: [u8; 32],
    /// Product-owned ResultDomain digest and ordering.
    pub result_domain: [u8; 32],
    /// Finalized Structured V2 terms digest.
    pub terms: [u8; 32],
    /// Finalized receipt TokenBehavior digest.
    pub token_behavior: [u8; 32],
    /// Finalized exact claim-shard terms digest.
    pub shard_terms: [u8; 32],
    /// Finalized Product-N to Claims-K exposure digest.
    pub shard_exposure: [u8; 32],
}

/// Wallet intent; every authority and amount observation stays chain-derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredIntentV2 {
    /// Exact action.
    pub action: StructuredActionV2,
    /// Exact receipt atoms; zero exactly for retirement.
    pub receipt_atoms: u64,
}

/// One observed Token account holding shard atoms of a single coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredShardAccountObservationV2 {
    /// Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
    /// Exact Token account identity.
    pub account: [u8; 32],
    /// Exact observed raw shard base units.
    pub amount: u64,
}

/// Adapter-parsed Token and lifecycle observations from one finalized snapshot.
///
/// This type is ephemeral and never persisted.  `finalized` is the explicitly
/// named trust boundary: the physical adapter binds it to a finalized RPC
/// observation, and the onchain candidate rechecks every derived value again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredActionObservationV2<'a> {
    /// The snapshot behind every field below was finalized.
    pub finalized: bool,
    /// Authenticated Market lifecycle.
    pub phase: StructuredPhaseV2,
    /// Finalized terminal-coordinate digest; zero for open actions.
    pub terminal_digest: [u8; 32],
    /// Current Structured root replay revision.
    pub revision: u64,
    /// Observed Token-owned Structured receipt Mint supply.
    pub receipt_supply: u64,
    /// Exact per-coordinate observed custody and authenticated payout rows.
    pub rows: &'a [StructuredCoordinateObservationV2],
    /// Exact actor identity; zero only for permissionless retirement.
    pub owner: [u8; 32],
    /// Exact receipt source Token account, or zero when inactive.
    pub receipt_source: [u8; 32],
    /// Exact receipt destination Token account, or zero when inactive.
    pub receipt_destination: [u8; 32],
    /// Actor receipt balance observed on the receipt-side Token account.
    pub actor_receipts: u64,
    /// Terms-selected Token program.
    pub token_program: [u8; 32],
    /// Root PDA controlling mint, permissioned burn, release, and closure.
    pub root: [u8; 32],
    /// Actor-side shard accounts, one per backed coordinate, in ascending order.
    pub holder_shard_accounts: &'a [StructuredShardAccountObservationV2],
    /// Structured custody shard accounts, one per backed coordinate, ascending.
    pub custody_shard_accounts: &'a [StructuredShardAccountObservationV2],
    /// Root-bound lifecycle RentCredit; zero unless retiring.
    pub rent_credit: [u8; 32],
}

/// One exact Token-2022 effect with its independently observed pre/post state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTokenEffectPlanV2 {
    /// Exact effect kind.
    pub kind: StructuredHotTokenKindV2,
    /// Representation coordinate, or the canonical absent sentinel for receipts.
    pub representation_coordinate: u32,
    /// Terms-selected Token program.
    pub token_program: [u8; 32],
    /// Receipt Mint or shard-terms-selected shard Mint.
    pub mint: [u8; 32],
    /// Source Token account when active.
    pub source: Option<[u8; 32]>,
    /// Destination Token account or RentCredit when active.
    pub destination: Option<[u8; 32]>,
    /// Exact signing authority.
    pub authority: [u8; 32],
    /// Exact raw base units; zero only for the two closure kinds.
    pub amount: u64,
    /// Mint supply before the effect.
    pub pre_supply: u64,
    /// Mint supply after the effect.
    pub post_supply: u64,
    /// Source amount before the effect, zero when absent.
    pub pre_source: u64,
    /// Source amount after the effect, zero when absent.
    pub post_source: u64,
    /// Destination amount before the effect, zero when absent.
    pub pre_destination: u64,
    /// Destination amount after the effect, zero when absent.
    pub post_destination: u64,
}

/// Owned exact action result derived from the pure kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredActionPlanV2 {
    /// Canonical family request.
    pub request: StructuredRequestV2,
    /// Ordered Token effects in exactly the order the candidate expects.
    pub effects: Vec<StructuredTokenEffectPlanV2>,
    /// Full-width exact shard movements, including inert zero-coefficient rows.
    pub movements: Vec<ShardMovementV2>,
    /// Full-width terminal settlement rows; empty unless redeeming.
    pub settlement: Vec<StructuredSettlementRowV2>,
    /// Total shard atoms locked or released across every coordinate.
    pub total_shard_atoms: u64,
    /// Exact total collateral atoms the released basket yields at the shard layer.
    pub total_collateral_atoms: u64,
    /// Receipt Mint supply after the action.
    pub post_receipt_supply: u64,
    /// Required Structured replay revision, absent when retirement closes the root.
    pub post_revision: Option<u64>,
}

/// Derive one exact action request, effect plan, and settlement projection.
pub fn plan_structured_action_v2(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    context: StructuredRequestContextV2,
    intent: StructuredIntentV2,
    observed: StructuredActionObservationV2<'_>,
) -> Result<StructuredActionPlanV2> {
    validate_context(terms, shard_terms, context, observed)?;
    let projection_bytes = structured_projection_bytes_v2(terms.representation_width())
        .map_err(|_| Error::ChainObservation)?;
    let mut scratch = vec![0_u8; projection_bytes];
    let mut encoded = vec![0_u8; projection_bytes];
    encode_structured_projection_v2(
        terms,
        observed.phase,
        observed.receipt_supply,
        observed.revision,
        observed.rows,
        &mut scratch,
        &mut encoded,
    )
    .map_err(|_| Error::ChainObservation)?;
    let projection =
        StructuredProjectionV2::decode(&encoded, terms).map_err(|_| Error::ChainObservation)?;
    let request = StructuredRequestV2::new(
        intent.action,
        StructuredRequestInputV2 {
            release_set: context.release_set,
            market: context.market,
            product_record: context.product_record,
            result_domain: context.result_domain,
            terms: context.terms,
            token_behavior: context.token_behavior,
            shard_terms: context.shard_terms,
            shard_exposure: context.shard_exposure,
            owner: observed.owner,
            receipt_source: observed.receipt_source,
            receipt_destination: observed.receipt_destination,
            terminal_digest: observed.terminal_digest,
            expected_revision: observed.revision,
            quantity: intent.receipt_atoms,
        },
    )
    .map_err(|_| Error::Request)?
    .bind_terms(terms)
    .map_err(|_| Error::Request)?;
    let width = usize::try_from(terms.representation_width()).map_err(|_| Error::Terms)?;
    let mut movements = vec![ShardMovementV2::default(); width];
    let mut settlement = Vec::new();
    match intent.action {
        StructuredActionV2::Issue => {
            let plan = plan_structured_issue_v2(
                terms,
                shard_terms,
                projection,
                intent.receipt_atoms,
                observed.actor_receipts,
                &mut movements,
            )
            .map_err(|_| Error::Kernel)?;
            let effects = build_supply_effects(
                terms,
                &movements,
                observed,
                StructuredHotTokenKindV2::MintReceipts,
                StructuredHotTokenKindV2::LockShards,
                plan.receipt,
            )?;
            Ok(StructuredActionPlanV2 {
                request,
                effects,
                movements,
                settlement,
                total_shard_atoms: plan.total_shard_atoms,
                total_collateral_atoms: 0,
                post_receipt_supply: plan.receipt.post_receipt_supply,
                post_revision: Some(plan.receipt.next_revision),
            })
        }
        StructuredActionV2::Unwrap => {
            let plan = plan_structured_unwrap_v2(
                terms,
                shard_terms,
                projection,
                intent.receipt_atoms,
                observed.actor_receipts,
                &mut movements,
            )
            .map_err(|_| Error::Kernel)?;
            let effects = build_supply_effects(
                terms,
                &movements,
                observed,
                StructuredHotTokenKindV2::BurnReceipts,
                StructuredHotTokenKindV2::ReleaseShards,
                plan.receipt,
            )?;
            Ok(StructuredActionPlanV2 {
                request,
                effects,
                movements,
                settlement,
                total_shard_atoms: plan.total_shard_atoms,
                total_collateral_atoms: 0,
                post_receipt_supply: plan.receipt.post_receipt_supply,
                post_revision: Some(plan.receipt.next_revision),
            })
        }
        StructuredActionV2::TerminalRedeem => {
            settlement = vec![StructuredSettlementRowV2::default(); width];
            let plan = plan_structured_terminal_redeem_v2(
                terms,
                shard_terms,
                projection,
                intent.receipt_atoms,
                observed.actor_receipts,
                &mut movements,
                &mut settlement,
            )
            .map_err(|_| Error::Kernel)?;
            let effects = build_supply_effects(
                terms,
                &movements,
                observed,
                StructuredHotTokenKindV2::BurnReceipts,
                StructuredHotTokenKindV2::ReleaseShards,
                plan.release.receipt,
            )?;
            Ok(StructuredActionPlanV2 {
                request,
                effects,
                movements,
                settlement,
                total_shard_atoms: plan.release.total_shard_atoms,
                total_collateral_atoms: plan.total_collateral_atoms,
                post_receipt_supply: plan.release.receipt.post_receipt_supply,
                post_revision: Some(plan.release.receipt.next_revision),
            })
        }
        StructuredActionV2::ZeroSupplyRetire => {
            plan_structured_retire_v2(terms, shard_terms, projection).map_err(|_| Error::Kernel)?;
            let effects = build_retirement_effects(terms, shard_terms, observed)?;
            Ok(StructuredActionPlanV2 {
                request,
                effects,
                movements,
                settlement,
                total_shard_atoms: 0,
                total_collateral_atoms: 0,
                post_receipt_supply: 0,
                post_revision: None,
            })
        }
    }
}

fn build_supply_effects(
    terms: StructuredTermsV2<'_>,
    movements: &[ShardMovementV2],
    observed: StructuredActionObservationV2<'_>,
    receipt_kind: StructuredHotTokenKindV2,
    shard_kind: StructuredHotTokenKindV2,
    receipt: ReceiptEffectV2,
) -> Result<Vec<StructuredTokenEffectPlanV2>> {
    let receipt_atoms = receipt.receipt_atoms;
    let minting = receipt_kind == StructuredHotTokenKindV2::MintReceipts;
    let (pre_source, post_source, pre_destination, post_destination) = if minting {
        (
            0,
            0,
            observed.actor_receipts,
            observed
                .actor_receipts
                .checked_add(receipt_atoms)
                .ok_or(Error::Token)?,
        )
    } else {
        (
            observed.actor_receipts,
            observed
                .actor_receipts
                .checked_sub(receipt_atoms)
                .ok_or(Error::Token)?,
            0,
            0,
        )
    };
    let mut effects = Vec::new();
    effects.push(StructuredTokenEffectPlanV2 {
        kind: receipt_kind,
        representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
        token_program: observed.token_program,
        mint: terms.receipt_mint(),
        source: if minting {
            None
        } else {
            Some(observed.receipt_source)
        },
        destination: if minting {
            Some(observed.receipt_destination)
        } else {
            None
        },
        authority: observed.root,
        amount: receipt_atoms,
        pre_supply: receipt.pre_receipt_supply,
        post_supply: receipt.post_receipt_supply,
        pre_source,
        post_source,
        pre_destination,
        post_destination,
    });
    let locking = shard_kind == StructuredHotTokenKindV2::LockShards;
    let mut backed = 0_usize;
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        if terms.coefficient(coordinate).map_err(|_| Error::Terms)? != 0 {
            let index = usize::try_from(coordinate).map_err(|_| Error::Terms)?;
            let movement = *movements.get(index).ok_or(Error::Kernel)?;
            if movement.representation_coordinate != coordinate {
                return Err(Error::Kernel);
            }
            let holder = shard_account(observed.holder_shard_accounts, backed, coordinate)?;
            let custody = shard_account(observed.custody_shard_accounts, backed, coordinate)?;
            let (source, destination) = if locking {
                (holder, custody)
            } else {
                (custody, holder)
            };
            effects.push(StructuredTokenEffectPlanV2 {
                kind: shard_kind,
                representation_coordinate: coordinate,
                token_program: observed.token_program,
                mint: movement.shard_mint,
                source: Some(source.account),
                destination: Some(destination.account),
                authority: if locking {
                    observed.owner
                } else {
                    observed.root
                },
                amount: movement.shard_atoms,
                pre_supply: 0,
                post_supply: 0,
                pre_source: source.amount,
                post_source: source
                    .amount
                    .checked_sub(movement.shard_atoms)
                    .ok_or(Error::Token)?,
                pre_destination: destination.amount,
                post_destination: destination
                    .amount
                    .checked_add(movement.shard_atoms)
                    .ok_or(Error::Token)?,
            });
            backed = backed.checked_add(1).ok_or(Error::Token)?;
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::Terms)?;
    }
    if observed.holder_shard_accounts.len() != backed
        || observed.custody_shard_accounts.len() != backed
    {
        return Err(Error::ChainObservation);
    }
    Ok(effects)
}

fn build_retirement_effects(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    observed: StructuredActionObservationV2<'_>,
) -> Result<Vec<StructuredTokenEffectPlanV2>> {
    if observed.rent_credit == [0; 32] {
        return Err(Error::Rent);
    }
    let mut effects = Vec::new();
    let mut backed = 0_usize;
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        if terms.coefficient(coordinate).map_err(|_| Error::Terms)? != 0 {
            let custody = shard_account(observed.custody_shard_accounts, backed, coordinate)?;
            if custody.amount != 0 {
                return Err(Error::Token);
            }
            effects.push(StructuredTokenEffectPlanV2 {
                kind: StructuredHotTokenKindV2::CloseCustody,
                representation_coordinate: coordinate,
                token_program: observed.token_program,
                mint: shard_terms
                    .shard_mint(coordinate)
                    .map_err(|_| Error::Terms)?,
                source: Some(custody.account),
                destination: Some(observed.rent_credit),
                authority: observed.root,
                amount: 0,
                pre_supply: 0,
                post_supply: 0,
                pre_source: 0,
                post_source: 0,
                pre_destination: 0,
                post_destination: 0,
            });
            backed = backed.checked_add(1).ok_or(Error::Token)?;
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::Terms)?;
    }
    if observed.custody_shard_accounts.len() != backed {
        return Err(Error::ChainObservation);
    }
    effects.push(StructuredTokenEffectPlanV2 {
        kind: StructuredHotTokenKindV2::CloseReceiptMint,
        representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
        token_program: observed.token_program,
        mint: terms.receipt_mint(),
        source: None,
        destination: Some(observed.rent_credit),
        authority: observed.root,
        amount: 0,
        pre_supply: 0,
        post_supply: 0,
        pre_source: 0,
        post_source: 0,
        pre_destination: 0,
        post_destination: 0,
    });
    Ok(effects)
}

fn shard_account(
    accounts: &[StructuredShardAccountObservationV2],
    index: usize,
    representation_coordinate: u32,
) -> Result<StructuredShardAccountObservationV2> {
    let observed = accounts
        .get(index)
        .copied()
        .ok_or(Error::ChainObservation)?;
    if observed.representation_coordinate != representation_coordinate
        || observed.account == [0; 32]
    {
        return Err(Error::ChainObservation);
    }
    Ok(observed)
}

fn validate_context(
    terms: StructuredTermsV2<'_>,
    shard_terms: FractionalExposureTermsV2<'_>,
    context: StructuredRequestContextV2,
    observed: StructuredActionObservationV2<'_>,
) -> Result<()> {
    terms
        .bind_shard_terms(shard_terms)
        .map_err(|_| Error::Terms)?;
    if context.market != terms.market()
        || context.product_record != terms.product_record()
        || context.result_domain != terms.result_domain()
        || context.release_set != terms.release_set()
        || context.terms != terms.terms_id()
        || context.token_behavior != terms.token_behavior()
        || context.shard_terms != terms.shard_terms()
        || context.shard_exposure != terms.shard_exposure()
    {
        return Err(Error::Terms);
    }
    if !observed.finalized
        || observed.token_program != terms.token_program()
        || observed.root == [0; 32]
        || observed.rows.len()
            != usize::try_from(terms.representation_width()).map_err(|_| Error::Terms)?
    {
        return Err(Error::ChainObservation);
    }
    let terminal_present = observed.terminal_digest != [0; 32];
    let consistent = match observed.phase {
        StructuredPhaseV2::Open => !terminal_present,
        StructuredPhaseV2::Terminal => terminal_present,
        StructuredPhaseV2::Retired => false,
    };
    if !consistent {
        return Err(Error::ChainObservation);
    }
    Ok(())
}
