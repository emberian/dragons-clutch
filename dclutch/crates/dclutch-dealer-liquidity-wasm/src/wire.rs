//! Strict JSON transport; protocol data layouts remain in their Rust owners.
use serde::{Deserialize, Serialize};

/// Input transport format, shared by the browser and operator CLI.
pub const INPUT_FORMAT_V1: &str = "dclutch-dealer-liquidity-input-v1";
/// Output transport format.
pub const PLAN_FORMAT_V1: &str = "dclutch-dealer-liquidity-plan-v1";
/// Provisional browser memory bound; lift with measured deployment corpora.
pub const MAX_INPUT_BYTES_V1: usize = 24 * 1024 * 1024;

/// Canonical decimal transport for u64 values (never a JavaScript number).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DecimalV1(
    /// Exact decoded unsigned value.
    pub u64,
);
impl TryFrom<String> for DecimalV1 {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let parsed = value
            .parse::<u64>()
            .map_err(|_| "expected exact decimal u64".to_owned())?;
        if parsed.to_string() != value {
            return Err("expected canonical decimal u64".to_owned());
        }
        Ok(Self(parsed))
    }
}
impl From<DecimalV1> for String {
    fn from(value: DecimalV1) -> Self {
        value.0.to_string()
    }
}

/// One finalized account with requested instruction privileges.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountV1 {
    /// Canonical base58 address.
    pub address: String,
    /// Canonical base58 owner; System for canonical vacancy.
    pub owner: String,
    /// Exact lamports, including zero for vacancy.
    pub lamports: DecimalV1,
    /// Executable bit.
    pub executable: bool,
    /// Exact canonical base64 data (empty for vacancy).
    pub data_base64: String,
    /// Requested signer bit.
    pub is_signer: bool,
    /// Requested writable bit.
    pub is_writable: bool,
}

/// Finalized snapshot boundary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationV1 {
    /// Shared finalized slot.
    pub slot: DecimalV1,
    /// Clock unix timestamp as canonical signed decimal text.
    pub unix_timestamp: String,
}

/// Chain-selected programs, authenticated separately against checked deployment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgramsV1 {
    /// Core program.
    pub core: String,
    /// Registry program.
    pub registry: String,
    /// Trading program.
    pub trading: String,
    /// Claims program.
    pub claims: String,
    /// Custody program.
    pub custody: String,
}

/// Checked deployment tuple; browser must authenticate these bytes on chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckedReleaseV1 {
    /// Execution release set, lowercase SHA-256 hex.
    pub release_set: String,
    /// Trading artifact release identity, lowercase hex.
    pub trading_artifact_release: String,
    /// Checked infrastructure digest, lowercase hex.
    pub checked_manifest_digest: String,
    /// Market generation.
    pub generation: DecimalV1,
}

/// User's economic action, never raw protocol request bytes.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ActionV1 {
    /// Create a vacant canonical LP Position using current rent.
    Open,
    /// Deposit a proportional basket in exchange for exact shares.
    Add {
        /// Present collateral atoms.
        collateral: DecimalV1,
        /// Exact shares to mint.
        shares: DecimalV1,
        /// Native claim units per outcome.
        claims: Vec<DecimalV1>,
    },
    /// Burn exact shares at the native floor-rounding boundary.
    Remove {
        /// Exact shares to burn.
        shares: DecimalV1,
    },
    /// Close a zero-share LP Position to Market LifecycleRentCredit.
    Close,
}

// Internally tagged serde unit variants ignore unknown fields. Decode the
// zero-field actions as empty structs so all actions retain strict fields.
impl<'de> Deserialize<'de> for ActionV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
        enum Wire {
            Open {},
            Add {
                collateral: DecimalV1,
                shares: DecimalV1,
                claims: Vec<DecimalV1>,
            },
            Remove {
                shares: DecimalV1,
            },
            Close {},
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Open {} => Self::Open,
            Wire::Add {
                collateral,
                shares,
                claims,
            } => Self::Add {
                collateral,
                shares,
                claims,
            },
            Wire::Remove { shares } => Self::Remove { shares },
            Wire::Close {} => Self::Close,
        })
    }
}

/// One typed user intent bound to a Market and LP owner.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntentV1 {
    /// Core Market.
    pub market: String,
    /// LP authority and collateral recipient owner.
    pub owner: String,
    /// Explicit transaction payer.
    pub payer: String,
    /// Economic action.
    pub action: ActionV1,
    /// Request expiry, in slots.
    pub expires_at: DecimalV1,
}

/// Complete corpus, with logical ordering checked by canonical Hot builders.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputV1 {
    /// Exact transport format.
    pub format: String,
    /// Typed user intent.
    pub intent: IntentV1,
    /// Observation boundary.
    pub observation: ObservationV1,
    /// Deployment identities.
    pub programs: ProgramsV1,
    /// Checked release tuple.
    pub checked_release: CheckedReleaseV1,
    /// Exact common Hot frame.
    pub fixed_accounts: Vec<AccountV1>,
    /// Admitted strategy evidence and caller authorities.
    pub strategy_accounts: Vec<AccountV1>,
    /// Runtime suffix after the injected common coordinates.
    pub runtime_suffix_accounts: Vec<AccountV1>,
    /// Current rent quote for the fixed canonical LP Position width.
    pub lp_position_rent_lamports: DecimalV1,
}

/// Reconstruct exact protocol material through the native semantic owners.
pub fn plan_dealer_liquidity_json_v1(source: &str) -> Result<String, String> {
    if source.is_empty() || source.len() > MAX_INPUT_BYTES_V1 {
        return Err("Dealer input exceeds the bounded transport size".to_owned());
    }
    let input: InputV1 =
        serde_json::from_str(source).map_err(|error| format!("Dealer input: {error}"))?;
    if input.format != INPUT_FORMAT_V1 {
        return Err("Dealer input format differs".to_owned());
    }
    if input.fixed_accounts.len()
        + input.strategy_accounts.len()
        + input.runtime_suffix_accounts.len()
        > 256
    {
        return Err("Dealer input exceeds 256 account rows".to_owned());
    }
    let output = crate::projection::plan(&input)?;
    serde_json::to_string(&output).map_err(|error| format!("Dealer output: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{ActionV1, DecimalV1, plan_dealer_liquidity_json_v1};

    #[test]
    fn decimal_transport_is_exact_at_u64_boundary_and_rejects_noncanonical_forms() {
        let maximum: DecimalV1 =
            serde_json::from_str("\"18446744073709551615\"").expect("maximum u64");
        assert_eq!(maximum.0, u64::MAX);
        assert_eq!(
            serde_json::to_string(&maximum).expect("decimal JSON"),
            "\"18446744073709551615\""
        );
        for source in ["\"01\"", "\"1e9\"", "\"-1\"", "\"18446744073709551616\""] {
            assert!(
                serde_json::from_str::<DecimalV1>(source)
                    .expect_err("noncanonical decimal")
                    .to_string()
                    .contains("decimal u64")
            );
        }
        assert!(
            serde_json::from_str::<DecimalV1>("9007199254740993")
                .expect_err("JSON number")
                .to_string()
                .contains("expected a string")
        );
    }

    #[test]
    fn typed_intent_refuses_unknown_and_duplicate_fields_before_projection() {
        assert!(
            serde_json::from_str::<ActionV1>(r#"{"kind":"open","collateral":"4"}"#)
                .expect_err("Open has no amount")
                .to_string()
                .contains("unknown field")
        );
        assert!(
            serde_json::from_str::<ActionV1>(r#"{"kind":"remove","shares":"4","shares":"5"}"#)
                .expect_err("duplicate shares")
                .to_string()
                .contains("duplicate field")
        );
        assert_eq!(
            plan_dealer_liquidity_json_v1(""),
            Err("Dealer input exceeds the bounded transport size".to_owned())
        );
    }
}
