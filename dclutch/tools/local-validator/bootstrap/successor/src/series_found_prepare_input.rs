//! Live fact admission for the first Series Found -> Prepare compiler pass.
//!
//! This module turns the M0 publisher's finalized records and bounded RPC
//! observations into the typed selection input.  The source, Rent, Clock, and
//! vacancy reads are distinct finalized queries; callers needing a single-slot
//! witness must acquire one before calling this adapter.  Future Core,
//! Custody, Claims and Ticket accounts remain PDA predictions; no account is
//! created or represented as observed here.

use dclutch_claims::{
    founding_v5::ClaimsFoundingAggregateSeedsV5,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_custody::{PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyStateSeedsV2};
use dclutch_market::{
    Action, Identity, ProjectFoundReceiptV2, Request, SeriesFoundingPermitSeedsV1,
};
use dclutch_product::{
    PortfolioV2,
    admission::{PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2},
    payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, TemplateV3, admit_occurrence, admit_ticket,
    pre_founding_series_escrow,
    replay::{SeriesStateV3, TicketStateSeedsV3},
};
use sha2::Digest;
use solana_program::{hash::hashv, rent::Rent};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    core_bump_projection::CoreProductGraphProjectionV1,
    market::{FutureMarketImmutablePublicationV1, record_identity},
    model::SuccessorPlan,
    plan::pubkey,
    rpc::Rpc,
    series_found_prepare_campaign::{
        SeriesClaimsVacancyV1, SeriesFoundPrepareSelectionInputV1, SeriesPhysicalMaterialInputV1,
    },
    series_found_prepare_driver::{
        PublishedSeriesFounderRecordsV1, observe_series_founder_source_v1,
    },
    series_founder::PreparedSeriesFounderV1,
};

/// Inputs already authenticated by the live campaign before compiler
/// construction.  Identity fields are keys, while Registry bodies arrive only
/// through M0's publisher-owned closure.
/// Domain separating a deterministic compiler-only parent-root context from a
/// Trading PDA.  This value is never submitted or described as an account:
/// the final pass replaces it with [`derive_series_parent_root_v1`]'s PDA.
const SERIES_PARENT_ROOT_NORMALIZATION_DOMAIN_V1: &[u8] =
    b"dclutch/series-found-prepare/parent-root-normalization/v1";

/// Canonical compiler context for the first Series selection pass.
///
/// `root` is deliberately a prediction, never an observed or on-chain root.
/// Its fixed layout belongs to the Series release and is the only root geometry
/// available before the selected parent Market has immutable bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeriesPredictedParentRootV1 {
    pub(crate) root: Pubkey,
    pub(crate) data_len: usize,
}

/// Derive the deterministic, non-account root context for compilation before
/// the selected parent Market can derive its Trading PDA.  Every component is
/// already admitted M0/M1 source material.  Callers must compile again with
/// the final parent PDA and require immutable selection invariance.
pub(crate) fn predict_series_parent_root_v1(
    plan: &SuccessorPlan,
    founder: &PreparedSeriesFounderV1,
    selected_release: dclutch_core_contract::ContentId,
    selected_manifest_entry_index: u16,
) -> Result<SeriesPredictedParentRootV1> {
    let trading = pubkey(&plan.trading.program_id)?;
    let root = Pubkey::new_from_array(
        hashv(&[
            SERIES_PARENT_ROOT_NORMALIZATION_DOMAIN_V1,
            &trading.to_bytes(),
            founder.admitted.template(),
            &founder.admitted.occurrences()[0],
            &founder.admitted.occurrences()[1],
            &founder.admitted.tickets()[0],
            &founder.admitted.tickets()[1],
            selected_release.as_bytes(),
            &selected_manifest_entry_index.to_le_bytes(),
        ])
        .to_bytes(),
    );
    if root == Pubkey::default() {
        return Err(Error::new(
            "Series normalized parent-root prediction produced the default key",
        ));
    }
    Ok(SeriesPredictedParentRootV1 {
        root,
        data_len:
            dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    })
}

/// Root fact for one compiler pass.  A prediction supplies only the
/// release-owned layout and its prospective Rent minimum; a finalized root is
/// the only variant allowed to carry a validator-observed balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesParentRootFactV1 {
    Predicted(SeriesPredictedParentRootV1),
    Finalized {
        root: Pubkey,
        observed_data_len: usize,
        observed_lamports: u64,
    },
}

pub(crate) struct SeriesFoundPrepareInputFactsV1<'a> {
    pub(crate) plan: &'a SuccessorPlan,
    pub(crate) m0: &'a FutureMarketImmutablePublicationV1,
    pub(crate) founder: &'a PreparedSeriesFounderV1,
    pub(crate) founder_records: &'a PublishedSeriesFounderRecordsV1,
    pub(crate) payer: Pubkey,
    pub(crate) founder_key: Pubkey,
    pub(crate) refund_destination: Pubkey,
    pub(crate) founder_source: Pubkey,
    pub(crate) collateral_mint: Pubkey,
    /// Compiler-only root prediction before activation, or the final root
    /// observed after the parent Market activation.
    pub(crate) parent_root: SeriesParentRootFactV1,
    /// Exact selected funding-ledger width from the Series release owner.
    pub(crate) funding_ledger_slot_count: u16,
    pub(crate) selected_release: dclutch_core_contract::ContentId,
    pub(crate) activation_deadline_slot: u64,
    pub(crate) selected_manifest_entry_index: u16,
}

/// Admit finalized M0/M1 facts and construct the first bounded compiler pass.
pub(crate) fn build_series_found_prepare_selection_input_v1<'a>(
    rpc: &mut Rpc,
    input: SeriesFoundPrepareInputFactsV1<'a>,
) -> Result<SeriesFoundPrepareSelectionInputV1<'a>> {
    let registry = pubkey(&input.plan.registry.program_id)?;
    let core = pubkey(&input.plan.core.program_id)?;
    let trading = pubkey(&input.plan.trading.program_id)?;
    let custody = pubkey(&input.plan.custody.program_id)?;
    let claims = pubkey(&input.plan.claims.program_id)?;
    let rent_program = pubkey(&input.plan.rent_credit.program_id)?;
    if input.payer == Pubkey::default() {
        return Err(Error::new("Series compiler input named a default payer"));
    }
    if input.funding_ledger_slot_count == 0 || input.m0.principal_cap_sets == 0 {
        return Err(Error::new(
            "Series compiler input omitted ledger width or M0 principal cap",
        ));
    }
    let source = observe_series_founder_source_v1(
        rpc,
        input.founder_source,
        input.collateral_mint,
        input.founder_key,
    )?;
    observe_series_refund_destination_v1(rpc, input.refund_destination, input.collateral_mint)?;
    let rent: Rent = bincode::deserialize(
        &rpc.required_account(solana_sdk_ids::sysvar::rent::ID, "Series Rent sysvar")?
            .data,
    )
    .map_err(|_| Error::new("Series Rent sysvar refused decode"))?;
    let (parent_root, observed_root_lamports) = match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) => (prediction.root, None),
        SeriesParentRootFactV1::Finalized {
            root,
            observed_data_len,
            observed_lamports,
        } => (root, Some((observed_data_len, observed_lamports))),
    };
    let canonical_root_width =
        dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5;
    let supplied_root_width = match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) => prediction.data_len,
        SeriesParentRootFactV1::Finalized {
            observed_data_len, ..
        } => observed_data_len,
    };
    if parent_root == Pubkey::default() || supplied_root_width != canonical_root_width {
        return Err(Error::new(
            "Series parent root omitted or disagreed with the release-owned root width",
        ));
    }
    let exact_root_rent = rent.minimum_balance(canonical_root_width);
    let lifecycle_root_lamports = match observed_root_lamports {
        Some((_, lamports)) if lamports >= exact_root_rent => lamports,
        Some(_) => {
            return Err(Error::new(
                "Series finalized parent root balance is below its canonical layout rent",
            ));
        }
        None => exact_root_rent,
    };
    let now_slot = rpc.finalized_slot()?;
    let template = TemplateV3::decode(input.founder.admitted.template())
        .map_err(|_| Error::new("Series founder Template refused decode"))?;
    if template.release_set() != input.selected_release
        || input.activation_deadline_slot <= now_slot
    {
        return Err(Error::new(
            "Series selected release or activation deadline disagreed with live facts",
        ));
    }
    verify_founder_publication(input.founder, input.founder_records)?;
    let occurrence = admit_occurrence(
        input.founder.admitted.template(),
        &input.founder.admitted.occurrences()[0],
        &input.founder.admitted.siblings()[0],
    )
    .map_err(|_| Error::new("Series first occurrence refused admission"))?;
    let ticket = admit_ticket(&input.founder.admitted.tickets()[0])
        .map_err(|_| Error::new("Series first Ticket refused admission"))?;
    let product = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(record_identity(&m0_body(
            input.m0,
            input.m0.product.raw,
            "Product",
        )?))
        .map_err(|_| Error::new("M0 Product identity"))?,
        portfolio_product_id(&m0_body(input.m0, input.m0.portfolio.raw, "Portfolio")?)?,
        portfolio_domain_id(&m0_body(input.m0, input.m0.portfolio.raw, "Portfolio")?)?,
    );
    let escrow = pre_founding_series_escrow(
        occurrence,
        ticket,
        product,
        AccountKeyV3::new(registry.to_bytes()).map_err(|_| Error::new("Registry key"))?,
    )
    .map_err(|_| Error::new("Series future Market projection refused"))?;
    let market = Pubkey::new_from_array(escrow.market().to_bytes());
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(market.to_bytes())
            .map_err(|_| Error::new("Claims aggregate seeds"))?
            .as_slices(),
        &claims,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.founder_key.to_bytes())
            .map_err(|_| Error::new("Claims position seeds"))?
            .as_slices(),
        &claims,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), input.founder_key.to_bytes())
            .map_err(|_| Error::new("Claims admission seeds"))?
            .as_slices(),
        &claims,
    )
    .0;
    let portfolio = PortfolioV2::decode(&m0_body(input.m0, input.m0.portfolio.raw, "Portfolio")?)
        .map_err(|_| Error::new("M0 Portfolio refused decode"))?;
    let count = portfolio.coefficient_count();
    let claims_rent_principals = [
        rent.minimum_balance(
            liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, count)
                .map_err(|_| Error::new("Claims aggregate width"))?,
        ),
        rent.minimum_balance(
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, count)
                .map_err(|_| Error::new("Claims position width"))?,
        ),
        rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
    ];
    let context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        &escrow.ticket_id().to_bytes(),
    ])
    .to_bytes();
    let permit_bump = Pubkey::find_program_address(
        &SeriesFoundingPermitSeedsV1::new(
            Identity::new(escrow.release_set().to_bytes())
                .map_err(|_| Error::new("release identity"))?,
            Identity::new(market.to_bytes()).map_err(|_| Error::new("market identity"))?,
            Identity::new(escrow.ticket_id().to_bytes())
                .map_err(|_| Error::new("Ticket identity"))?,
        )
        .as_slices(),
        &trading,
    )
    .1;
    let ticket_state = Pubkey::find_program_address(
        &TicketStateSeedsV3::new(parent_root.to_bytes(), ticket.content_id()).as_slices(),
        &trading,
    )
    .0;
    let lifecycle = dclutch_operator::series_lifecycle_v3::SeriesLifecycleSnapshotV3 {
        template_bytes: input.founder.admitted.template(),
        series: SeriesStateV3::new(template.close_rent()),
        now_slot,
        current: Some(
            dclutch_operator::series_lifecycle_v3::SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &input.founder.admitted.occurrences()[0],
                ticket_bytes: &input.founder.admitted.tickets()[0],
                siblings: &input.founder.admitted.siblings()[0],
                ticket_state: None,
            },
        ),
        terminal_ticket: None,
        observed_root_lamports: lifecycle_root_lamports,
        exact_root_rent,
        rent_sink: None,
    };
    let parents =
        crate::series_found_prepare_campaign::derive_series_prepare_parents_v1(lifecycle)?;
    let identity = escrow.future_market().identity();
    let found = Request::administrative(Action::Found, escrow.generation(), identity.market_id)
        .encode()
        .map_err(|_| Error::new("Series Core Found request"))?;
    let projection_receipt = ProjectFoundReceiptV2::new(
        identity.market_id,
        escrow.generation(),
        identity.realm_id,
        Identity::new(input.collateral_mint.to_bytes())
            .map_err(|_| Error::new("Series collateral Mint identity"))?,
        Identity::new(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID)
            .map_err(|_| Error::new("Series token program identity"))?,
        Identity::new(crate::collateral_release::founded_collateral_adapter_release_id_v1())
            .map_err(|_| Error::new("Series collateral release"))?,
        identity.product_record,
        identity.product_id,
        identity.resolution_policy,
        identity.selected_release_set,
        Identity::new(rent_program.to_bytes()).map_err(|_| Error::new("Series Rent identity"))?,
        input.m0.principal_cap_sets,
        sha2::Sha256::digest(found).into(),
    )
    .map_err(|_| Error::new("Series ProjectFound receipt"))?;
    let projection_receipt_digest: [u8; 32] = sha2::Sha256::digest(
        projection_receipt
            .encode()
            .map_err(|_| Error::new("Series ProjectFound receipt encoding"))?,
    )
    .into();
    let (projected_state, projected_bump) = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::new(
            market.to_bytes(),
            escrow.release_set().to_bytes(),
            context,
        )
        .as_slices(),
        &custody,
    );
    let (_, vacancies) = rpc.finalized_accounts(
        &[
            aggregate,
            position,
            admission,
            ticket_state,
            projected_state,
        ],
        now_slot,
    )?;
    if vacancies.iter().any(Option::is_some) {
        return Err(Error::new(
            "Series future Claims, Ticket, or projected-Custody PDA was already occupied",
        ));
    }
    Ok(SeriesFoundPrepareSelectionInputV1 {
        lifecycle,
        product,
        registry_program: AccountKeyV3::new(registry.to_bytes())
            .map_err(|_| Error::new("Registry key"))?,
        material: SeriesPhysicalMaterialInputV1 {
            trading,
            core,
            custody,
            claims,
            rent_program,
            market,
            release_set: Identity::new(escrow.release_set().to_bytes())
                .map_err(|_| Error::new("release identity"))?,
            ticket: Identity::new(escrow.ticket_id().to_bytes())
                .map_err(|_| Error::new("Ticket identity"))?,
            parent_root,
            payer: input.payer,
            founder: input.founder_key,
            refund_owner: input.refund_destination,
            founder_source: source.source,
            rent_credit: input.m0.rent_credit,
            mint: input.collateral_mint,
            token_program: Pubkey::new_from_array(
                dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID,
            ),
            collateral_release: Identity::new(
                crate::collateral_release::founded_collateral_adapter_release_id_v1(),
            )
            .map_err(|_| Error::new("collateral release"))?,
            projection_receipt_digest,
            prepare_parent_digest: parents.prepare_digest,
            expire_parent_digest: parents.expire_digest,
            claims_vacancy: SeriesClaimsVacancyV1 {
                aggregate,
                position,
                admission,
                aggregate_lamports: 0,
                position_lamports: 0,
                admission_lamports: 0,
            },
            rent: rent.clone(),
        },
        core_product_graph: product_graph(input.m0)?,
        core_projection: CoreProductGraphProjectionV1::Recorded,
        core_walk: crate::market::CoreProductGraphWalkV1::ProjectedFounding,
        principal_cap_sets: input.m0.principal_cap_sets,
        linked_basis_record_digest: record_identity(body_by_schema(
            input.m0,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            "Basis",
        )?),
        semantic_basis_id: portfolio.liability_basis_id().to_bytes(),
        claims_rent_principals,
        permit_bump,
        projected_bump,
        founder_source_amount: source.amount,
        geometry: None,
        selected_release: input.selected_release,
        funding_ledger_slot_count: input.funding_ledger_slot_count,
        activation_deadline_slot: input.activation_deadline_slot,
        selected_manifest_entry_index: input.selected_manifest_entry_index,
        ticket_state_account: AccountKeyV3::new(ticket_state.to_bytes())
            .map_err(|_| Error::new("Ticket replay key"))?,
    })
}

/// Authenticate the normal-Custody token recipient used by Expire.  The
/// Template's refund owner remains a separate Rent authority; this coordinate
/// is an actual initialized Token-2022 account for the collateral Mint.
fn observe_series_refund_destination_v1(
    rpc: &mut Rpc,
    destination: Pubkey,
    mint: Pubkey,
) -> Result<()> {
    use dclutch_custody::token_svm::{AccountState, TOKEN_2022_PROGRAM_ID, TokenAccount};

    if destination == Pubkey::default() || mint == Pubkey::default() {
        return Err(Error::new(
            "Series refund destination named a default token account or Mint",
        ));
    }
    let account = rpc.required_account(destination, "Series refund token destination")?;
    let token = TokenAccount::parse(&account.data)
        .map_err(|error| Error::new(format!("Series refund token destination: {error:?}")))?;
    if account.owner != Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
        || account.executable
        || token.mint != mint.to_bytes()
        || token.state != AccountState::Initialized
    {
        return Err(Error::new(
            "Series refund destination did not authenticate as an initialized collateral Token-2022 account",
        ));
    }
    Ok(())
}

fn m0_body<'a>(
    m0: &'a FutureMarketImmutablePublicationV1,
    raw: Pubkey,
    label: &str,
) -> Result<&'a [u8]> {
    let rows = m0
        .series_prepare_records
        .iter()
        .filter(|row| row.published.raw == raw)
        .collect::<Vec<_>>();
    match rows.as_slice() {
        [row] if row.published.digest == record_identity(&row.body) => Ok(&row.body),
        _ => Err(Error::new(format!(
            "Series M0 publication omitted or substituted {label} body"
        ))),
    }
}
fn portfolio_product_id(bytes: &[u8]) -> Result<dclutch_core_contract::ContentId> {
    let portfolio =
        PortfolioV2::decode(bytes).map_err(|_| Error::new("M0 Portfolio product identity"))?;
    dclutch_core_contract::ContentId::new(portfolio.product_id().to_bytes())
        .map_err(|_| Error::new("M0 Portfolio product identity"))
}
fn portfolio_domain_id(bytes: &[u8]) -> Result<dclutch_core_contract::ContentId> {
    let portfolio =
        PortfolioV2::decode(bytes).map_err(|_| Error::new("M0 Portfolio domain identity"))?;
    dclutch_core_contract::ContentId::new(portfolio.result_domain_id().to_bytes())
        .map_err(|_| Error::new("M0 Portfolio domain identity"))
}
fn body_by_schema<'a>(
    m0: &'a FutureMarketImmutablePublicationV1,
    schema: [u8; 32],
    label: &str,
) -> Result<&'a [u8]> {
    let rows = m0
        .series_prepare_records
        .iter()
        .filter(|row| row.published.schema == schema)
        .collect::<Vec<_>>();
    match rows.as_slice() {
        [row] if row.published.digest == record_identity(&row.body) => Ok(&row.body),
        _ => Err(Error::new(format!(
            "Series M0 publication omitted or duplicated {label} body"
        ))),
    }
}
fn product_graph(m0: &FutureMarketImmutablePublicationV1) -> Result<[([u8; 32], [u8; 32]); 4]> {
    Ok([
        (
            PRODUCT_RECORD_SCHEMA_ID_V2,
            record_identity(&m0_body(m0, m0.product.raw, "Product")?),
        ),
        (
            RESULT_DOMAIN_SCHEMA_ID_V2,
            record_identity(&m0_body(m0, m0.domain.raw, "ResultDomain")?),
        ),
        (
            PORTFOLIO_SCHEMA_ID_V2,
            record_identity(&m0_body(m0, m0.portfolio.raw, "Portfolio")?),
        ),
        (
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            record_identity(body_by_schema(
                m0,
                GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                "Basis",
            )?),
        ),
    ])
}
fn verify_founder_publication(
    founder: &PreparedSeriesFounderV1,
    published: &PublishedSeriesFounderRecordsV1,
) -> Result<()> {
    let expected = [
        (founder.admitted.template(), &published.template),
        (
            &founder.admitted.occurrences()[0],
            &published.occurrences[0],
        ),
        (
            &founder.admitted.occurrences()[1],
            &published.occurrences[1],
        ),
        (&founder.admitted.tickets()[0], &published.tickets[0]),
        (&founder.admitted.tickets()[1], &published.tickets[1]),
    ];
    for (body, record) in expected {
        if record.digest != record_identity(body) {
            return Err(Error::new(
                "Series founder publication body digest differed from admitted leaf",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        market::tests::selected_family_compiler_fixture_v1,
        series_founder::{
            SeriesOccurrenceFundingV1, SeriesTemplatePolicyV1,
            prepare_series_founder_from_market_v1,
        },
    };

    fn content(byte: u8) -> dclutch_core_contract::ContentId {
        dclutch_core_contract::ContentId::new([byte; 32]).expect("nonzero test content")
    }

    fn prepared() -> (SuccessorPlan, PreparedSeriesFounderV1) {
        let (plan, input, mint, founder, refund_owner) = selected_family_compiler_fixture_v1();
        let founder = prepare_series_founder_from_market_v1(
            &plan,
            &input,
            mint,
            founder,
            refund_owner,
            SeriesTemplatePolicyV1 {
                product_generator: content(1),
                occurrence_generator: content(2),
                capability_template: content(3),
                product_derivation: content(4),
                occurrence_derivation: content(5),
                capability_derivation: content(6),
                funding_derivation: content(7),
                first_slot: 100,
                period_slots: 10,
                retry_window: 2,
                close_rent: 1,
            },
            [
                SeriesOccurrenceFundingV1 {
                    funding_list: content(8),
                    funds: dclutch_trading::series::FoundingFundsV3::new(9, 2, 3, 4)
                        .expect("first funding"),
                },
                SeriesOccurrenceFundingV1 {
                    funding_list: content(9),
                    funds: dclutch_trading::series::FoundingFundsV3::new(18, 2, 3, 4)
                        .expect("second funding"),
                },
            ],
        )
        .expect("admitted Series founder");
        (plan, founder)
    }

    #[test]
    fn normalized_parent_root_is_deterministic_non_account_context() {
        let (plan, founder) = prepared();
        let first = predict_series_parent_root_v1(&plan, &founder, content(61), 3)
            .expect("first compiler context");
        let repeated = predict_series_parent_root_v1(&plan, &founder, content(61), 3)
            .expect("repeat compiler context");
        let distinct = predict_series_parent_root_v1(&plan, &founder, content(62), 3)
            .expect("distinct compiler context");
        assert_eq!(first, repeated);
        assert_ne!(first.root, distinct.root);
        assert_ne!(first.root, Pubkey::default());
        assert_eq!(
            first.data_len,
            dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
        );
    }
}
