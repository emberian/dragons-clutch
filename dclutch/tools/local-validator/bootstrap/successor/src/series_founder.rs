//! Typed, explicit Series founder record admission.
//!
//! This adapter accepts prepared immutable facts. It creates no identities or
//! economics: two supplied occurrences become the two committed leaves, and
//! each child Market address is derived from the canonical Core seed projection.

use std::{fs, path::Path, str::FromStr};

use dclutch_core_contract::ContentId;
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, FoundingFundsV3, admit_occurrence,
    admit_ticket,
    encode::{
        OccurrenceRecordInputV3, TemplateRecordInputV3, encode_occurrence_v3, encode_template_v3,
        encode_ticket_v3,
    },
    future_market_projection, occurrence_content_id,
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use crate::{Error, Result, plan::hex};

pub(crate) const SERIES_FOUNDER_INPUT_COMMAND_V1: &str =
    "local-private-validator-series-founder-input-v1";
pub(crate) const SERIES_FOUNDER_FACTS_SCHEMA_V1: &str = "dclutch-series-founder-facts-v1";
const ADMITTED_SCHEMA: &str = "dclutch-series-founder-admitted-v1";

/// All immutable facts supplied by the prepared Product/Registry/Market plan.
///
/// The caller authenticates the actual Product graph (including its domain and
/// partition) before this constructor. This constructor validates only the
/// Series joins and the child-Market PDA projection it owns.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SeriesFounderFactsV1 {
    pub(crate) schema: String,
    pub(crate) template: TemplateFactsV1,
    pub(crate) product: ProductFactsV1,
    pub(crate) registry_program: String,
    pub(crate) core_program: String,
    pub(crate) founder: String,
    pub(crate) occurrences: [OccurrenceFactsV1; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TemplateFactsV1 {
    pub(crate) realm: String,
    pub(crate) release_set: String,
    pub(crate) product_generator: String,
    pub(crate) occurrence_generator: String,
    pub(crate) capability_template: String,
    pub(crate) product_derivation: String,
    pub(crate) occurrence_derivation: String,
    pub(crate) capability_derivation: String,
    pub(crate) funding_derivation: String,
    pub(crate) refund_owner: String,
    pub(crate) first_slot: u64,
    pub(crate) period_slots: u64,
    pub(crate) retry_window: u64,
    pub(crate) close_rent: u64,
}

/// Product facts authenticated by the upstream Product Runtime V2 reader.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductFactsV1 {
    pub(crate) product_record: String,
    pub(crate) stable_product_id: String,
    pub(crate) result_domain: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OccurrenceFactsV1 {
    pub(crate) product_record: String,
    pub(crate) resolution_policy: String,
    pub(crate) liability_basis: String,
    pub(crate) rational_representation: String,
    pub(crate) capability_manifest: String,
    pub(crate) funding_list: String,
    pub(crate) hoard_principal: u64,
    pub(crate) market_rent: u64,
    pub(crate) capability_native: u64,
    pub(crate) founding_work: u64,
}

/// Final exact records, admitted through the Series semantic owner.
#[derive(Clone, Debug)]
pub(crate) struct AdmittedSeriesFounderV1 {
    pub(crate) template: Vec<u8>,
    pub(crate) occurrences: [Vec<u8>; 2],
    pub(crate) siblings: [Vec<[u8; 32]>; 2],
    pub(crate) tickets: [Vec<u8>; 2],
}

/// Authored generator selection and schedule. Product/Realm/release identities
/// are absent: the Market compiler and checked plan own those facts.
pub(crate) struct SeriesTemplatePolicyV1 {
    pub(crate) product_generator: ContentId,
    pub(crate) occurrence_generator: ContentId,
    pub(crate) capability_template: ContentId,
    pub(crate) product_derivation: ContentId,
    pub(crate) occurrence_derivation: ContentId,
    pub(crate) capability_derivation: ContentId,
    pub(crate) funding_derivation: ContentId,
    pub(crate) first_slot: u64,
    pub(crate) period_slots: u64,
    pub(crate) retry_window: u64,
    pub(crate) close_rent: u64,
}

/// Funding commitments supplied by the future Market funding compiler.
pub(crate) struct SeriesOccurrenceFundingV1 {
    pub(crate) funding_list: ContentId,
    pub(crate) funds: FoundingFundsV3,
}

/// Pre-publication candidates, not observations of accounts on a validator.
pub(crate) struct PreparedSeriesFounderV1 {
    pub(crate) facts: SeriesFounderFactsV1,
    pub(crate) admitted: AdmittedSeriesFounderV1,
    pub(crate) publication: crate::market::MarketPublicationPreviewV1,
}

/// Bind the two occurrences to exactly the bytes ordinary Market founding
/// will publish. This is the bridge from a real prepared Market specification
/// to Series; the raw DTO entrance remains useful for already-authenticated
/// external producers but does not confer Product-graph authority on a caller.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_series_founder_from_market_v1(
    plan: &crate::model::SuccessorPlan,
    market_input: &crate::model::MarketRunInput,
    collateral_mint: Pubkey,
    founder: Pubkey,
    refund_owner: Pubkey,
    policy: SeriesTemplatePolicyV1,
    funding: [SeriesOccurrenceFundingV1; 2],
) -> Result<PreparedSeriesFounderV1> {
    use dclutch_product::{PortfolioV2, admission::ProductRecordV2};
    let publication = crate::market::compile_market_publication_preview_v1(
        crate::plan::pubkey(&plan.registry.program_id)?,
        market_input,
        collateral_mint,
    )?;
    let product = ProductRecordV2::decode(&publication.product)
        .map_err(|error| Error::new(format!("Series Product record: {error:?}")))?;
    let portfolio = PortfolioV2::decode(&publication.portfolio)
        .map_err(|error| Error::new(format!("Series Portfolio record: {error:?}")))?;
    let digest = |bytes: &[u8]| hex(&crate::market::record_identity(bytes));
    let product_record = digest(&publication.product);
    let facts = SeriesFounderFactsV1 {
        schema: SERIES_FOUNDER_FACTS_SCHEMA_V1.into(),
        template: TemplateFactsV1 {
            realm: digest(&publication.realm),
            release_set: plan.release_set_id.clone(),
            product_generator: hex(&policy.product_generator.to_bytes()),
            occurrence_generator: hex(&policy.occurrence_generator.to_bytes()),
            capability_template: hex(&policy.capability_template.to_bytes()),
            product_derivation: hex(&policy.product_derivation.to_bytes()),
            occurrence_derivation: hex(&policy.occurrence_derivation.to_bytes()),
            capability_derivation: hex(&policy.capability_derivation.to_bytes()),
            funding_derivation: hex(&policy.funding_derivation.to_bytes()),
            refund_owner: refund_owner.to_string(),
            first_slot: policy.first_slot,
            period_slots: policy.period_slots,
            retry_window: policy.retry_window,
            close_rent: policy.close_rent,
        },
        product: ProductFactsV1 {
            product_record: product_record.clone(),
            stable_product_id: hex(&product.product_id().to_bytes()),
            result_domain: hex(&product.result_domain_digest().to_bytes()),
        },
        registry_program: plan.registry.program_id.clone(),
        core_program: plan.core.program_id.clone(),
        founder: founder.to_string(),
        occurrences: funding.map(|row| OccurrenceFactsV1 {
            product_record: product_record.clone(),
            resolution_policy: digest(&publication.source),
            liability_basis: hex(&portfolio.liability_basis_id().to_bytes()),
            // Product admission's representation identity is the exact
            // Portfolio content digest. Its evaluator release is a distinct
            // identity stored inside that record.
            rational_representation: hex(&product.portfolio_digest().to_bytes()),
            capability_manifest: digest(&publication.manifest),
            funding_list: hex(&row.funding_list.to_bytes()),
            hoard_principal: row.funds.hoard_principal(),
            market_rent: row.funds.market_rent(),
            capability_native: row.funds.capability_native(),
            founding_work: row.funds.founding_work(),
        }),
    };
    let admitted = admit_series_founder_facts_v1(&facts)?;
    Ok(PreparedSeriesFounderV1 {
        facts,
        admitted,
        publication,
    })
}

impl AdmittedSeriesFounderV1 {
    pub(crate) fn template(&self) -> &[u8] {
        &self.template
    }
    pub(crate) fn occurrences(&self) -> &[Vec<u8>; 2] {
        &self.occurrences
    }
    pub(crate) fn siblings(&self) -> &[Vec<[u8; 32]>; 2] {
        &self.siblings
    }
    pub(crate) fn tickets(&self) -> &[Vec<u8>; 2] {
        &self.tickets
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Emitted {
    schema: &'static str,
    template_hex: String,
    occurrence_hex: [String; 2],
    siblings_hex: [[String; 1]; 2],
    ticket_hex: [String; 2],
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-series-founder-input-v1 --input ABSOLUTE_FACTS_JSON\n"
}

/// Load the explicit JSON facts, then return only typed admitted records.
pub(crate) fn load_and_admit_series_founder_v1(path: &Path) -> Result<AdmittedSeriesFounderV1> {
    if !path.is_absolute() {
        return Err(Error::new("--input must be an absolute path"));
    }
    let facts = serde_json::from_slice(&fs::read(path)?)
        .map_err(|error| Error::new(format!("invalid Series founder facts: {error}")))?;
    admit_series_founder_facts_v1(&facts)
}

/// Construct both leaves, canonical one-sibling paths, Template and Tickets.
pub(crate) fn admit_series_founder_facts_v1(
    facts: &SeriesFounderFactsV1,
) -> Result<AdmittedSeriesFounderV1> {
    if facts.schema != SERIES_FOUNDER_FACTS_SCHEMA_V1 {
        return Err(Error::new("unsupported Series founder facts schema"));
    }
    let product_record = content(&facts.product.product_record, "product.productRecord")?;
    let product = AuthenticatedProductProjectionV2::new(
        product_record,
        content(&facts.product.stable_product_id, "product.stableProductId")?,
        content(&facts.product.result_domain, "product.resultDomain")?,
    );
    let registry = key(&facts.registry_program, "registryProgram")?;
    let core = Pubkey::from_str(&facts.core_program)
        .map_err(|_| Error::new("coreProgram must be a base58 Pubkey"))?;
    if core == Pubkey::default() {
        return Err(Error::new("coreProgram must be nonzero"));
    }
    let founder = key(&facts.founder, "founder")?;
    let refund = key(&facts.template.refund_owner, "template.refundOwner")?;
    for occurrence in &facts.occurrences {
        if content(&occurrence.product_record, "occurrence.productRecord")? != product_record {
            return Err(Error::new(
                "each occurrence.productRecord must equal product.productRecord",
            ));
        }
    }

    // Projection seeds exclude the committed Market address. Admit a complete
    // provisional tree only to obtain those seeds; the final tree is built with
    // the PDA addresses and re-admitted below.
    let provisional_market = founder;
    let provisional_occurrences = [
        encode_occurrence_v3(occurrence_input(facts, 0, provisional_market)?)
            .map_err(series_error)?,
        encode_occurrence_v3(occurrence_input(facts, 1, provisional_market)?)
            .map_err(series_error)?,
    ];
    let provisional_leaves = [
        occurrence_content_id(&provisional_occurrences[0]).map_err(series_error)?,
        occurrence_content_id(&provisional_occurrences[1]).map_err(series_error)?,
    ];
    let provisional_template = template(
        facts,
        refund,
        root(
            provisional_leaves[0].to_bytes(),
            provisional_leaves[1].to_bytes(),
        ),
    )?;
    let provisional_admitted = [
        admit_occurrence(
            &provisional_template,
            &provisional_occurrences[0],
            &[provisional_leaves[1].to_bytes()],
        )
        .map_err(series_error)?,
        admit_occurrence(
            &provisional_template,
            &provisional_occurrences[1],
            &[provisional_leaves[0].to_bytes()],
        )
        .map_err(series_error)?,
    ];
    let markets = [
        derive_market(provisional_admitted[0], product, registry, core)?,
        derive_market(provisional_admitted[1], product, registry, core)?,
    ];
    let occurrences = [
        encode_occurrence_v3(occurrence_input(facts, 0, markets[0])?).map_err(series_error)?,
        encode_occurrence_v3(occurrence_input(facts, 1, markets[1])?).map_err(series_error)?,
    ];
    let leaves = [
        occurrence_content_id(&occurrences[0]).map_err(series_error)?,
        occurrence_content_id(&occurrences[1]).map_err(series_error)?,
    ];
    let template = template(
        facts,
        refund,
        root(leaves[0].to_bytes(), leaves[1].to_bytes()),
    )?;
    let siblings = [
        [leaves[1].to_bytes()].to_vec(),
        [leaves[0].to_bytes()].to_vec(),
    ];
    let admitted = [
        admit_occurrence(&template, &occurrences[0], &siblings[0]).map_err(series_error)?,
        admit_occurrence(&template, &occurrences[1], &siblings[1]).map_err(series_error)?,
    ];
    // Re-check the final committed child Market identities, not only the
    // seed-equivalent provisional ones.
    for (admitted, market) in admitted.iter().zip(markets) {
        let projection =
            future_market_projection(*admitted, product, registry).map_err(series_error)?;
        projection.require_address(market).map_err(series_error)?;
    }
    let tickets = [
        encode_ticket_v3(admitted[0], founder).map_err(series_error)?,
        encode_ticket_v3(admitted[1], founder).map_err(series_error)?,
    ];
    for ticket in &tickets {
        admit_ticket(ticket).map_err(series_error)?;
    }
    Ok(AdmittedSeriesFounderV1 {
        template: template.to_vec(),
        occurrences: occurrences.map(Vec::from),
        siblings,
        tickets: tickets.map(Vec::from),
    })
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let [flag, value]: [String; 2] = arguments.try_into().map_err(|_| Error::new(usage()))?;
    if flag != "--input" {
        return Err(Error::new(usage()));
    }
    let admitted = load_and_admit_series_founder_v1(Path::new(&value))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&Emitted {
            schema: ADMITTED_SCHEMA,
            template_hex: hex(&admitted.template),
            occurrence_hex: admitted.occurrences.each_ref().map(|bytes| hex(bytes)),
            siblings_hex: admitted.siblings.each_ref().map(|proof| [hex(&proof[0])]),
            ticket_hex: admitted.tickets.each_ref().map(|bytes| hex(bytes))
        })?
    );
    Ok(())
}

fn derive_market(
    admitted: dclutch_trading::series::AdmittedOccurrenceV3,
    product: AuthenticatedProductProjectionV2,
    registry: AccountKeyV3,
    core: Pubkey,
) -> Result<AccountKeyV3> {
    let projection = future_market_projection(admitted, product, registry).map_err(series_error)?;
    let market = AccountKeyV3::new(
        Pubkey::find_program_address(&projection.seeds().as_slices(), &core)
            .0
            .to_bytes(),
    )
    .map_err(series_error)?;
    // `committed_address` is provisional here. The final occurrence binds this PDA.
    Ok(market)
}

fn template(
    facts: &SeriesFounderFactsV1,
    refund_owner: AccountKeyV3,
    projection_root: [u8; 32],
) -> Result<[u8; dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3]> {
    encode_template_v3(TemplateRecordInputV3 {
        realm: content(&facts.template.realm, "template.realm")?,
        release_set: content(&facts.template.release_set, "template.releaseSet")?,
        product_generator: content(
            &facts.template.product_generator,
            "template.productGenerator",
        )?,
        occurrence_generator: content(
            &facts.template.occurrence_generator,
            "template.occurrenceGenerator",
        )?,
        capability_template: content(
            &facts.template.capability_template,
            "template.capabilityTemplate",
        )?,
        product_derivation: content(
            &facts.template.product_derivation,
            "template.productDerivation",
        )?,
        occurrence_derivation: content(
            &facts.template.occurrence_derivation,
            "template.occurrenceDerivation",
        )?,
        capability_derivation: content(
            &facts.template.capability_derivation,
            "template.capabilityDerivation",
        )?,
        funding_derivation: content(
            &facts.template.funding_derivation,
            "template.fundingDerivation",
        )?,
        projection_root: ContentId::new(projection_root)
            .map_err(|_| Error::new("projection root was zero"))?,
        refund_owner,
        occurrence_count: 2,
        first_slot: facts.template.first_slot,
        period_slots: facts.template.period_slots,
        retry_window: facts.template.retry_window,
        close_rent: facts.template.close_rent,
    })
    .map_err(series_error)
}

fn occurrence_input(
    facts: &SeriesFounderFactsV1,
    index: usize,
    market: AccountKeyV3,
) -> Result<OccurrenceRecordInputV3> {
    let fact = &facts.occurrences[index];
    let scheduled_slot = facts
        .template
        .first_slot
        .checked_add(
            facts
                .template
                .period_slots
                .checked_mul(index as u64)
                .ok_or_else(|| Error::new("occurrence schedule overflow"))?,
        )
        .ok_or_else(|| Error::new("occurrence schedule overflow"))?;
    Ok(OccurrenceRecordInputV3 {
        occurrence: index as u32,
        scheduled_slot,
        product_record: content(&fact.product_record, "occurrence.productRecord")?,
        resolution_policy: content(&fact.resolution_policy, "occurrence.resolutionPolicy")?,
        liability_basis: content(&fact.liability_basis, "occurrence.liabilityBasis")?,
        rational_representation: content(
            &fact.rational_representation,
            "occurrence.rationalRepresentation",
        )?,
        capability_manifest: content(&fact.capability_manifest, "occurrence.capabilityManifest")?,
        funding_list: content(&fact.funding_list, "occurrence.fundingList")?,
        market,
        funds: FoundingFundsV3::new(
            fact.hoard_principal,
            fact.market_rent,
            fact.capability_native,
            fact.founding_work,
        )
        .map_err(series_error)?,
    })
}

fn content(value: &str, name: &str) -> Result<ContentId> {
    ContentId::new(hex32(value, name)?).map_err(|_| Error::new(format!("{name} must be nonzero")))
}
fn key(value: &str, name: &str) -> Result<AccountKeyV3> {
    AccountKeyV3::new(
        Pubkey::from_str(value)
            .map_err(|_| Error::new(format!("{name} must be a base58 Pubkey")))?
            .to_bytes(),
    )
    .map_err(series_error)
}
fn hex32(value: &str, name: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::new(format!(
            "{name} must be 64 hexadecimal characters"
        )));
    }
    if !value.is_ascii() {
        return Err(Error::new(format!("{name} must be hexadecimal")));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| Error::new(format!("{name} must be hexadecimal")))?;
    }
    Ok(bytes)
}
fn root(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    solana_program::hash::hashv(&[
        &dclutch_trading::series::generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
        &[0],
        &left,
        &right,
    ])
    .to_bytes()
}
fn series_error(error: dclutch_trading::series::SeriesV3Error) -> Error {
    Error::new(format!("Series semantic admission refused: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> String {
        hex(&[byte; 32])
    }
    fn facts() -> SeriesFounderFactsV1 {
        serde_json::from_str(&format!(r#"{{"schema":"{SERIES_FOUNDER_FACTS_SCHEMA_V1}","template":{{"realm":"{}","releaseSet":"{}","productGenerator":"{}","occurrenceGenerator":"{}","capabilityTemplate":"{}","productDerivation":"{}","occurrenceDerivation":"{}","capabilityDerivation":"{}","fundingDerivation":"{}","refundOwner":"{}","firstSlot":100,"periodSlots":10,"retryWindow":1,"closeRent":1}},"product":{{"productRecord":"{}","stableProductId":"{}","resultDomain":"{}"}},"registryProgram":"{}","coreProgram":"{}","founder":"{}","occurrences":[{{"productRecord":"{}","resolutionPolicy":"{}","liabilityBasis":"{}","rationalRepresentation":"{}","capabilityManifest":"{}","fundingList":"{}","hoardPrincipal":1,"marketRent":2,"capabilityNative":3,"foundingWork":4}},{{"productRecord":"{}","resolutionPolicy":"{}","liabilityBasis":"{}","rationalRepresentation":"{}","capabilityManifest":"{}","fundingList":"{}","hoardPrincipal":5,"marketRent":6,"capabilityNative":7,"foundingWork":8}}]}}"#, id(1),id(2),id(3),id(4),id(5),id(6),id(7),id(8),id(9), Pubkey::new_from_array([10;32]), id(11),id(12),id(13),Pubkey::new_from_array([15;32]),Pubkey::new_from_array([16;32]),Pubkey::new_from_array([17;32]),id(11),id(18),id(19),id(20),id(21),id(22),id(11),id(23),id(24),id(25),id(26),id(27))).expect("facts")
    }
    #[test]
    fn explicit_facts_admit_two_final_child_markets_and_tickets() {
        let admitted = admit_series_founder_facts_v1(&facts()).expect("admitted");
        assert_eq!(admitted.occurrences.len(), 2);
        assert_eq!(admitted.tickets.len(), 2);
        assert_eq!(admitted.siblings[0].len(), 1);
    }
    #[test]
    fn substituted_occurrence_product_refuses_before_records_exist() {
        let mut value = facts();
        value.occurrences[1].product_record = id(99);
        assert_eq!(
            admit_series_founder_facts_v1(&value)
                .expect_err("substituted Product must refuse")
                .to_string(),
            "each occurrence.productRecord must equal product.productRecord"
        );
    }

    #[test]
    fn non_ascii_content_identity_refuses_without_panicking() {
        let mut value = facts();
        // Exactly 64 bytes, with a multibyte character crossing a two-byte
        // hexadecimal slice boundary. A character-count check cannot fix it.
        value.template.realm = format!("aé{}", "0".repeat(61));
        assert_eq!(
            admit_series_founder_facts_v1(&value)
                .expect_err("non-ASCII identity must refuse")
                .to_string(),
            "template.realm must be hexadecimal"
        );
    }
}
