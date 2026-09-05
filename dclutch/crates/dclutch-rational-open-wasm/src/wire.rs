//! Strict JSON transport over the canonical Rational request contract.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use dclutch_claims::rational_request::{
    AssetV2, CallerRoleV2, OpenRepresentationHotRequestV3, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2, ABSENT_REVISION, ASSET_BYTES_V3,
};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use solana_program::{hash::hash, pubkey::Pubkey};

/// Strict input schema selected by browser and native callers.
pub const RATIONAL_OPEN_INPUT_FORMAT_V1: &str = "dclutch-rational-open-input-v1";
/// Exact compiled-plan schema returned to browser and native callers.
pub const RATIONAL_OPEN_PLAN_FORMAT_V1: &str = "dclutch-rational-open-plan-v1";

const MAX_JSON_BYTES: usize = 2 * 1024 * 1024;
const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_COLLECTION: usize = 4_096;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ActionWireV1 {
    Denominate,
    Reconstitute,
    IssueStructured,
    UnwrapStructured,
}

impl ActionWireV1 {
    const fn action(self) -> RepresentationActionV2 {
        match self {
            Self::Denominate => RepresentationActionV2::Denominate,
            Self::Reconstitute => RepresentationActionV2::Reconstitute,
            Self::IssueStructured => RepresentationActionV2::IssueStructured,
            Self::UnwrapStructured => RepresentationActionV2::UnwrapStructured,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Denominate => "denominate",
            Self::Reconstitute => "reconstitute",
            Self::IssueStructured => "issue-structured",
            Self::UnwrapStructured => "unwrap-structured",
        }
    }

    const fn structured(self) -> bool {
        matches!(self, Self::IssueStructured | Self::UnwrapStructured)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetWireV1 {
    shard_mint: String,
    actor_shard_account: String,
    structured_custody_account: String,
    claims_custody_owner: String,
    coefficient: String,
    expected_shard_supply: String,
    expected_actor_shards: String,
    expected_structured_shards: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InputWireV1 {
    format: String,
    action: ActionWireV1,
    release_set: String,
    market: String,
    graph_id: String,
    descriptor_id: String,
    actor: String,
    receipt_mint: String,
    receipt_account: Option<String>,
    representation_authority: String,
    token_program: String,
    expected_representation_revision: String,
    expected_claims_market_revision: Option<String>,
    expected_actor_position_revision: Option<String>,
    expected_custody_position_revision: Option<String>,
    generation: String,
    quantity: String,
    denominator: String,
    expected_receipt_supply: String,
    outcome_count: u32,
    selected_outcome: Option<u32>,
    assets: Vec<AssetWireV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanOutputV1 {
    format: &'static str,
    action: &'static str,
    family_base64: String,
    family_sha256: String,
    claims_child_base64: String,
    claims_child_sha256: String,
    asset_count: u32,
    logical_claims_accounts: usize,
    raw_quantity: String,
    receipt_effect: &'static str,
    raw_receipt_delta: String,
    shard_effect: &'static str,
    raw_shard_deltas: Vec<String>,
}

#[derive(Clone, Copy)]
struct ParsedAssetV1 {
    value: AssetV2,
    raw_delta: u64,
}

/// Decode, canonicalize, and compile one exact nonterminal Rational action.
pub fn plan_rational_open_json_v1(source: &[u8]) -> Result<String, String> {
    let value = bounded_exact_json(source)?;
    let wire: InputWireV1 = serde_json::from_value(value)
        .map_err(|error| format!("Rational open input schema: {error}"))?;
    if wire.format != RATIONAL_OPEN_INPUT_FORMAT_V1 {
        return Err("Rational open input has another format".to_owned());
    }
    if wire.outcome_count == 0 {
        return Err("Rational open outcomeCount must be positive".to_owned());
    }
    let action = wire.action;
    let structured = action.structured();
    let expected_assets = if structured { wire.outcome_count } else { 1 };
    if usize::try_from(expected_assets).ok() != Some(wire.assets.len()) {
        return Err(format!(
            "Rational open {} requires exactly {expected_assets} asset rows",
            action.name()
        ));
    }
    let selected_outcome = match (structured, wire.selected_outcome) {
        (true, None) => u32::MAX,
        (false, Some(selected)) if selected < wire.outcome_count => selected,
        (true, Some(_)) => {
            return Err("Structured Rational open must omit selectedOutcome".to_owned());
        }
        (false, _) => {
            return Err("Selected Rational open needs one in-domain selectedOutcome".to_owned());
        }
    };
    let receipt_account = match (structured, wire.receipt_account.as_deref()) {
        (true, Some(value)) => exact_key(value, "receipt account")?,
        (false, None) => [0; 32],
        (true, None) => return Err("Structured Rational open needs a receipt account".to_owned()),
        (false, Some(_)) => {
            return Err("Selected Rational open must omit receiptAccount".to_owned());
        }
    };
    let claims_revision = exact_optional_revision(
        wire.expected_claims_market_revision.as_deref(),
        !structured,
        "Claims Market revision",
    )?;
    let actor_revision = exact_optional_revision(
        wire.expected_actor_position_revision.as_deref(),
        !structured,
        "actor Position revision",
    )?;
    let custody_revision = exact_optional_revision(
        wire.expected_custody_position_revision.as_deref(),
        !structured,
        "custody Position revision",
    )?;
    let quantity = exact_u64(&wire.quantity, "raw quantity", true)?;
    let denominator = exact_u64(&wire.denominator, "denominator", true)?;
    let expected_receipt_supply = exact_u64(
        &wire.expected_receipt_supply,
        "expected receipt supply",
        false,
    )?;
    if matches!(action, ActionWireV1::UnwrapStructured) && expected_receipt_supply < quantity {
        return Err("Structured receipt supply cannot fund the exact unwrap burn".to_owned());
    }
    if matches!(action, ActionWireV1::IssueStructured)
        && expected_receipt_supply.checked_add(quantity).is_none()
    {
        return Err("post-issue Structured receipt supply exceeds u64".to_owned());
    }
    let parsed_assets = wire
        .assets
        .into_iter()
        .enumerate()
        .map(|(index, asset)| parse_asset(action, quantity, denominator, index, asset))
        .collect::<Result<Vec<_>, _>>()?;
    let mut asset_bytes = vec![0_u8; parsed_assets.len() * ASSET_BYTES_V3];
    for (index, asset) in parsed_assets.iter().enumerate() {
        let start = index
            .checked_mul(ASSET_BYTES_V3)
            .ok_or_else(|| "Rational open asset byte width overflowed".to_owned())?;
        asset
            .value
            .encode_into(
                asset_bytes
                    .get_mut(start..start + ASSET_BYTES_V3)
                    .ok_or_else(|| "Rational open asset byte span changed".to_owned())?,
            )
            .map_err(|error| format!("Rational open asset {index}: {error:?}"))?;
    }
    let header = RepresentationRequestHeaderV2 {
        action: action.action(),
        caller_role: CallerRoleV2::Trading,
        release_set: exact_key(&wire.release_set, "release set")?,
        market: exact_key(&wire.market, "Market")?,
        graph_id: exact_key(&wire.graph_id, "representation graph")?,
        descriptor_id: exact_key(&wire.descriptor_id, "representation descriptor")?,
        parent_context: [1; 32],
        actor: exact_key(&wire.actor, "actor")?,
        receipt_mint: exact_key(&wire.receipt_mint, "receipt Mint")?,
        receipt_account,
        representation_authority: exact_key(
            &wire.representation_authority,
            "representation authority",
        )?,
        token_program: exact_key(&wire.token_program, "Token program")?,
        realm: [0; 32],
        collateral_recipient: [0; 32],
        expected_representation_revision: exact_u64(
            &wire.expected_representation_revision,
            "representation replay revision",
            false,
        )?,
        expected_claims_market_revision: claims_revision,
        expected_actor_position_revision: actor_revision,
        expected_custody_position_revision: custody_revision,
        expected_custody_replay_revision: ABSENT_REVISION,
        generation: exact_u64(&wire.generation, "Market generation", true)?,
        quantity,
        denominator,
        expected_receipt_supply,
        outcome_count: wire.outcome_count,
        selected_outcome,
        asset_count: expected_assets,
    };
    let child_template = RepresentationRequestV2::new(header, &asset_bytes)
        .map_err(|error| format!("Rational open request owner: {error:?}"))?;
    // The header width is a function of the action's class in physical ABI v3.
    let request_bytes = child_template
        .wire_len()
        .map_err(|error| format!("Rational open request width: {error:?}"))?;
    let mut template_bytes = vec![0_u8; request_bytes];
    child_template
        .encode_into(&mut template_bytes)
        .map_err(|error| format!("Rational open request encoding: {error:?}"))?;
    let mut family_bytes = vec![0_u8; request_bytes];
    let family = OpenRepresentationHotRequestV3::from_child_into(child_template, &mut family_bytes)
        .map_err(|error| format!("Rational open family owner: {error:?}"))?;
    let family_digest = hash(family.as_bytes()).to_bytes();
    let mut child_bytes = vec![0_u8; request_bytes];
    let child = family
        .specialize_child_into(family_digest, &mut child_bytes)
        .map_err(|error| format!("Rational open child owner: {error:?}"))?;
    if child.header().parent_context != family_digest
        || child.header().action != action.action()
        || child
            .physical_account_count()
            .map_err(|error| format!("Rational open Claims account geometry: {error:?}"))?
            != 32 + 4 * parsed_assets.len()
    {
        return Err("Rational open family specialization changed semantic shape".to_owned());
    }
    let child_digest = hash(&child_bytes).to_bytes();
    serde_json::to_string(&PlanOutputV1 {
        format: RATIONAL_OPEN_PLAN_FORMAT_V1,
        action: action.name(),
        family_base64: STANDARD.encode(&family_bytes),
        family_sha256: hex(family_digest),
        claims_child_base64: STANDARD.encode(&child_bytes),
        claims_child_sha256: hex(child_digest),
        asset_count: expected_assets,
        logical_claims_accounts: 32 + 4 * parsed_assets.len(),
        raw_quantity: quantity.to_string(),
        receipt_effect: receipt_effect(action),
        raw_receipt_delta: if structured { quantity } else { 0 }.to_string(),
        shard_effect: shard_effect(action),
        raw_shard_deltas: parsed_assets
            .iter()
            .map(|asset| asset.raw_delta.to_string())
            .collect(),
    })
    .map_err(|error| format!("Rational open output: {error}"))
}

fn parse_asset(
    action: ActionWireV1,
    quantity: u64,
    denominator: u64,
    index: usize,
    wire: AssetWireV1,
) -> Result<ParsedAssetV1, String> {
    let coefficient = exact_u64(
        &wire.coefficient,
        &format!("asset {index} coefficient"),
        !action.structured(),
    )?;
    let expected_actor_shards = exact_u64(
        &wire.expected_actor_shards,
        &format!("asset {index} actor shards"),
        false,
    )?;
    let expected_structured_shards = exact_u64(
        &wire.expected_structured_shards,
        &format!("asset {index} Structured shards"),
        false,
    )?;
    let expected_shard_supply = exact_u64(
        &wire.expected_shard_supply,
        &format!("asset {index} shard supply"),
        false,
    )?;
    let raw_delta_factor = if action.structured() {
        coefficient
    } else {
        denominator
    };
    let raw_delta = raw_delta_factor
        .checked_mul(quantity)
        .ok_or_else(|| format!("asset {index} raw shard delta exceeds u64"))?;
    if matches!(
        action,
        ActionWireV1::Reconstitute | ActionWireV1::IssueStructured
    ) && expected_actor_shards < raw_delta
    {
        return Err(format!(
            "asset {index} actor shards cannot fund the exact raw debit"
        ));
    }
    if matches!(action, ActionWireV1::UnwrapStructured) && expected_structured_shards < raw_delta {
        return Err(format!(
            "asset {index} Structured custody cannot fund the exact raw return"
        ));
    }
    match action {
        ActionWireV1::Denominate => {
            expected_shard_supply
                .checked_add(raw_delta)
                .ok_or_else(|| format!("asset {index} post-denominate shard supply exceeds u64"))?;
        }
        ActionWireV1::Reconstitute if expected_shard_supply < raw_delta => {
            return Err(format!(
                "asset {index} shard supply cannot fund the exact raw burn"
            ));
        }
        ActionWireV1::Reconstitute
        | ActionWireV1::IssueStructured
        | ActionWireV1::UnwrapStructured => {}
    }
    Ok(ParsedAssetV1 {
        value: AssetV2 {
            shard_mint: exact_key(&wire.shard_mint, &format!("asset {index} shard Mint"))?,
            actor_shard_account: exact_key(
                &wire.actor_shard_account,
                &format!("asset {index} actor shard account"),
            )?,
            structured_custody_account: exact_key(
                &wire.structured_custody_account,
                &format!("asset {index} Structured custody account"),
            )?,
            claims_custody_owner: exact_key(
                &wire.claims_custody_owner,
                &format!("asset {index} Claims custody owner"),
            )?,
            coefficient,
            expected_shard_supply,
            expected_actor_shards,
            expected_structured_shards,
        },
        raw_delta,
    })
}

const fn receipt_effect(action: ActionWireV1) -> &'static str {
    match action {
        ActionWireV1::IssueStructured => "mint",
        ActionWireV1::UnwrapStructured => "burn",
        ActionWireV1::Denominate | ActionWireV1::Reconstitute => "none",
    }
}

const fn shard_effect(action: ActionWireV1) -> &'static str {
    match action {
        ActionWireV1::Denominate => "mint-to-actor",
        ActionWireV1::Reconstitute => "burn-from-actor",
        ActionWireV1::IssueStructured => "actor-to-custody",
        ActionWireV1::UnwrapStructured => "custody-to-actor",
    }
}

fn exact_optional_revision(
    value: Option<&str>,
    required: bool,
    field: &str,
) -> Result<u64, String> {
    match (required, value) {
        (true, Some(value)) => exact_u64(value, field, false),
        (false, None) => Ok(ABSENT_REVISION),
        (true, None) => Err(format!("Selected Rational open needs {field}")),
        (false, Some(_)) => Err(format!("Structured Rational open must omit {field}")),
    }
}

fn exact_key(value: &str, field: &str) -> Result<[u8; 32], String> {
    let key = Pubkey::from_str(value).map_err(|_| format!("{field} is not one Solana address"))?;
    if key.to_string() != value || key == Pubkey::default() {
        return Err(format!("{field} is not canonical nonzero base58 text"));
    }
    Ok(key.to_bytes())
}

fn exact_u64(value: &str, field: &str, positive: bool) -> Result<u64, String> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{field} is not canonical unsigned decimal text"));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{field} exceeds u64"))?;
    if positive && parsed == 0 {
        return Err(format!("{field} must be positive"));
    }
    Ok(parsed)
}

fn hex(bytes: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in bytes {
        use fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn bounded_exact_json(source: &[u8]) -> Result<Value, String> {
    if source.is_empty() || source.len() > MAX_JSON_BYTES {
        return Err("Rational open JSON is outside its bounded size".to_owned());
    }
    let mut deserializer = serde_json::Deserializer::from_slice(source);
    let value = ExactValueSeedV1 { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|error| format!("Rational open JSON: {error}"))?;
    deserializer
        .end()
        .map_err(|error| format!("Rational open JSON trailing bytes: {error}"))?;
    Ok(value)
}

#[derive(Clone, Copy)]
struct ExactValueSeedV1 {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for ExactValueSeedV1 {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.depth > MAX_JSON_DEPTH {
            return Err(D::Error::custom(
                "nesting exceeds the exact transport bound",
            ));
        }
        deserializer.deserialize_any(ExactValueVisitorV1 { depth: self.depth })
    }
}

struct ExactValueVisitorV1 {
    depth: usize,
}

impl<'de> Visitor<'de> for ExactValueVisitorV1 {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one bounded duplicate-free JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Err(E::custom("floating JSON numbers are forbidden"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        ExactValueSeedV1 {
            depth: self.depth + 1,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut output = Vec::new();
        while let Some(value) = sequence.next_element_seed(ExactValueSeedV1 {
            depth: self.depth + 1,
        })? {
            if output.len() >= MAX_JSON_COLLECTION {
                return Err(A::Error::custom("array exceeds the exact transport bound"));
            }
            output.push(value);
        }
        Ok(Value::Array(output))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut output = serde_json::Map::new();
        let mut names = BTreeSet::new();
        while let Some(name) = map.next_key::<String>()? {
            if output.len() >= MAX_JSON_COLLECTION {
                return Err(A::Error::custom("object exceeds the exact transport bound"));
            }
            if !names.insert(name.clone()) {
                return Err(A::Error::custom(format!("duplicate field {name}")));
            }
            let value = map.next_value_seed(ExactValueSeedV1 {
                depth: self.depth + 1,
            })?;
            output.insert(name, value);
        }
        Ok(Value::Object(output))
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use dclutch_claims::rational_request::{
        OPEN_REPRESENTATION_HOT_MAGIC_V3, REQUEST_MAGIC_V2,
    };
    use serde_json::json;

    fn key(value: u8) -> String {
        Pubkey::new_from_array([value; 32]).to_string()
    }

    fn asset(index: u8) -> Value {
        json!({
            "shardMint": key(20 + index),
            "actorShardAccount": key(30 + index),
            "structuredCustodyAccount": key(40 + index),
            "claimsCustodyOwner": key(50 + index),
            "coefficient": "10",
            "expectedShardSupply": "100",
            "expectedActorShards": "80",
            "expectedStructuredShards": "60"
        })
    }

    fn input(action: &str) -> Value {
        let structured = matches!(action, "issue-structured" | "unwrap-structured");
        json!({
            "format": RATIONAL_OPEN_INPUT_FORMAT_V1,
            "action": action,
            "releaseSet": key(1),
            "market": key(2),
            "graphId": key(3),
            "descriptorId": key(4),
            "actor": key(5),
            "receiptMint": key(6),
            "receiptAccount": structured.then(|| key(7)),
            "representationAuthority": key(8),
            "tokenProgram": Pubkey::new_from_array(dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID).to_string(),
            "expectedRepresentationRevision": "3",
            "expectedClaimsMarketRevision": (!structured).then_some("4"),
            "expectedActorPositionRevision": (!structured).then_some("5"),
            "expectedCustodyPositionRevision": (!structured).then_some("6"),
            "generation": "7",
            "quantity": "2",
            "denominator": "10",
            "expectedReceiptSupply": "9",
            "outcomeCount": 2,
            "selectedOutcome": (!structured).then_some(1),
            "assets": if structured { vec![asset(0), asset(1)] } else { vec![asset(0)] }
        })
    }

    #[test]
    fn all_four_actions_compile_through_the_canonical_owner() {
        for action in [
            "denominate",
            "reconstitute",
            "issue-structured",
            "unwrap-structured",
        ] {
            let source = serde_json::to_vec(&input(action)).expect("JSON");
            let output = plan_rational_open_json_v1(&source).expect("plan");
            let plan: Value = serde_json::from_str(&output).expect("output");
            let family = STANDARD
                .decode(plan["familyBase64"].as_str().expect("family"))
                .expect("base64");
            let child = STANDARD
                .decode(plan["claimsChildBase64"].as_str().expect("child"))
                .expect("base64");
            assert_eq!(
                family.get(..8),
                Some(OPEN_REPRESENTATION_HOT_MAGIC_V3.as_slice())
            );
            assert_eq!(child.get(..8), Some(REQUEST_MAGIC_V2.as_slice()));
            assert_eq!(
                plan["familySha256"],
                Value::String(hex(hash(&family).to_bytes()))
            );
            assert_eq!(
                plan["claimsChildSha256"],
                Value::String(hex(hash(&child).to_bytes()))
            );
            let decoded = RepresentationRequestV2::decode(&child).expect("canonical child");
            assert_eq!(decoded.header().parent_context, hash(&family).to_bytes());
        }
    }

    #[test]
    fn duplicate_unknown_fractional_and_trailing_json_refuse() {
        let duplicate = br#"{"format":"dclutch-rational-open-input-v1","format":"dclutch-rational-open-input-v1"}"#;
        assert!(plan_rational_open_json_v1(duplicate)
            .unwrap_err()
            .contains("duplicate"));
        let mut unknown = input("denominate");
        unknown["invented"] = json!(true);
        assert!(
            plan_rational_open_json_v1(&serde_json::to_vec(&unknown).expect("JSON"))
                .unwrap_err()
                .contains("unknown field")
        );
        assert!(bounded_exact_json(br#"1.5"#)
            .unwrap_err()
            .contains("floating"));
        assert!(bounded_exact_json(br#"{} {}"#)
            .unwrap_err()
            .contains("trailing"));
    }

    #[test]
    fn action_shape_balance_alias_and_integer_hostiles_refuse() {
        let mut selected_with_receipt = input("denominate");
        selected_with_receipt["receiptAccount"] = json!(key(7));
        assert!(plan_rational_open_json_v1(
            &serde_json::to_vec(&selected_with_receipt).expect("JSON")
        )
        .unwrap_err()
        .contains("omit receiptAccount"));

        let mut issue_short = input("issue-structured");
        issue_short["assets"][0]["expectedActorShards"] = json!("19");
        assert!(
            plan_rational_open_json_v1(&serde_json::to_vec(&issue_short).expect("JSON"))
                .unwrap_err()
                .contains("cannot fund")
        );

        let mut unwrap_short = input("unwrap-structured");
        unwrap_short["assets"][0]["expectedStructuredShards"] = json!("19");
        assert!(
            plan_rational_open_json_v1(&serde_json::to_vec(&unwrap_short).expect("JSON"))
                .unwrap_err()
                .contains("cannot fund")
        );

        let mut alias = input("denominate");
        alias["assets"][0]["actorShardAccount"] = alias["assets"][0]["shardMint"].clone();
        assert!(
            plan_rational_open_json_v1(&serde_json::to_vec(&alias).expect("JSON"))
                .unwrap_err()
                .contains("AccountAlias")
        );

        let mut noncanonical = input("denominate");
        noncanonical["quantity"] = json!("02");
        assert!(
            plan_rational_open_json_v1(&serde_json::to_vec(&noncanonical).expect("JSON"))
                .unwrap_err()
                .contains("canonical unsigned")
        );

        let mut denomination_supply_overflow = input("denominate");
        denomination_supply_overflow["assets"][0]["expectedShardSupply"] =
            json!(u64::MAX.to_string());
        assert!(plan_rational_open_json_v1(
            &serde_json::to_vec(&denomination_supply_overflow).expect("JSON")
        )
        .unwrap_err()
        .contains("post-denominate shard supply exceeds u64"));

        let mut reconstitution_supply_short = input("reconstitute");
        reconstitution_supply_short["assets"][0]["expectedShardSupply"] = json!("19");
        assert!(plan_rational_open_json_v1(
            &serde_json::to_vec(&reconstitution_supply_short).expect("JSON")
        )
        .unwrap_err()
        .contains("shard supply cannot fund"));

        let mut receipt_supply_overflow = input("issue-structured");
        receipt_supply_overflow["expectedReceiptSupply"] = json!(u64::MAX.to_string());
        assert!(plan_rational_open_json_v1(
            &serde_json::to_vec(&receipt_supply_overflow).expect("JSON")
        )
        .unwrap_err()
        .contains("post-issue Structured receipt supply exceeds u64"));
    }

    #[test]
    fn output_is_atom_exact_and_deterministic() {
        let source = serde_json::to_vec(&input("issue-structured")).expect("JSON");
        let first = plan_rational_open_json_v1(&source).expect("first");
        let second = plan_rational_open_json_v1(&source).expect("second");
        assert_eq!(first, second);
        let value: Value = serde_json::from_str(&first).expect("output");
        assert_eq!(value["rawQuantity"], "2");
        assert_eq!(value["rawReceiptDelta"], "2");
        assert_eq!(value["rawShardDeltas"], json!(["20", "20"]));
        assert_eq!(value["logicalClaimsAccounts"], 40);

        let mut selected = input("denominate");
        selected["assets"][0]["coefficient"] = json!("3");
        selected["denominator"] = json!("7");
        let selected_output =
            plan_rational_open_json_v1(&serde_json::to_vec(&selected).expect("selected JSON"))
                .expect("selected plan");
        let selected_value: Value =
            serde_json::from_str(&selected_output).expect("selected output");
        assert_eq!(selected_value["rawShardDeltas"], json!(["14"]));
    }
}
