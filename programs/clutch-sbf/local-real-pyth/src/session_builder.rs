//! Typed, unsigned transaction and chain-derived projection construction for a
//! daemon-owned local session.
//!
//! This module never reads a key and never submits. It builds the exact legacy
//! transaction bytes for the real-Pyth SourceV2-aware laboratory plane. The
//! daemon owns blockhash replacement, signer-role resolution, and signing as
//! separate steps. The General V2 owner projection is identity-bound to the
//! same real-source Market/Epoch, but remains construction-only. Submission,
//! confirmation, and General V2 account creation are not exposed by this seam.

use crate::plane::{self, ArtifactUpload, GeneralPlane, LabPlane, MarketPrestate};
use clutch_client_contract::owner_settlement::{
    project_owner_settlement_v1, OwnerSettlementProjectionPlanV1, OwnerSettlementProjectionRefusal,
    OwnerSettlementProjectionV1,
};
use clutch_solana_layout::artifact::ARTIFACT_CHUNK_BYTES;
use clutch_svm_fixture::{compute_unit_limit_data, COMPUTE_BUDGET};
use solana_address::Address;
use solana_instruction::Instruction;
use solana_transaction::Transaction;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const PLAN_SCHEMA: &str = "dragons-clutch/operator/local-real-transaction-plan/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerSettlementPlanError {
    ContextMismatch,
    Projection(OwnerSettlementProjectionRefusal),
}

impl core::fmt::Display for OwnerSettlementPlanError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ContextMismatch => {
                "owner settlement projection does not belong to this local session"
            }
            Self::Projection(_) => "owner settlement projection was refused",
        })
    }
}

impl std::error::Error for OwnerSettlementPlanError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignerRole {
    Payer,
    SecondOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltTransaction {
    pub schema: &'static str,
    pub family: &'static str,
    pub unsigned_transaction: Vec<u8>,
    pub required_signers: Vec<SignerRole>,
}

pub struct LocalTradingBuilder {
    payer: Address,
    second_owner: Address,
    lab: LabPlane,
    general: GeneralPlane,
}

impl LocalTradingBuilder {
    /// Rebuild the exact market identity used by the joined real-Pyth
    /// campaign. This is intentionally explicit: a caller choosing a fresh
    /// nonce must use [`Self::new`] and admit that new identity separately.
    pub fn campaign(
        payer: Address,
        second_owner: Address,
        start_bucket: u64,
        end_bucket_exclusive: u64,
    ) -> Result<Self> {
        Self::new(
            payer,
            second_owner,
            start_bucket,
            end_bucket_exclusive,
            clutch_svm_fixture::MARKET_NONCE,
        )
    }

    pub fn new(
        payer: Address,
        second_owner: Address,
        start_bucket: u64,
        end_bucket_exclusive: u64,
        market_nonce: u64,
    ) -> Result<Self> {
        if payer == second_owner {
            return Err("local trading actors alias".into());
        }
        let bucket_count = end_bucket_exclusive
            .checked_sub(start_bucket)
            .ok_or("local source window is reversed")?;
        if bucket_count == 0
            || bucket_count > clutch_sbf::source_archive_v2::SOURCE_ARCHIVE_MAX_RECORDS_V2 as u64
        {
            return Err("local source window has an unsupported boundary count".into());
        }
        let spec = plane::real_spec()?;
        let lab = plane::build(
            payer,
            spec,
            start_bucket,
            end_bucket_exclusive,
            market_nonce,
            MarketPrestate::SignedCreate,
        );
        let general = plane::general_plane(payer, &lab);
        Ok(Self {
            payer,
            second_owner,
            lab,
            general,
        })
    }

    pub fn market_address(&self) -> Address {
        self.lab.plane.market.address
    }

    pub fn source_archive_address(&self) -> Address {
        self.lab.plane.source_archive.address
    }

    /// Admit an authenticated General V2 projection only when its semantic
    /// Market, Epoch, and exact price scale belong to this real-source session.
    ///
    /// The result contains canonical 288-byte open owner bodies and prospective
    /// terminal dispositions. It does not construct a General V2 account
    /// instruction, submit a transaction, or imply that the current General V1
    /// runtime has created or settled those rows.
    pub fn project_owner_settlement(
        &self,
        projection: &OwnerSettlementProjectionV1<'_>,
    ) -> std::result::Result<OwnerSettlementProjectionPlanV1, OwnerSettlementPlanError> {
        if projection.market != self.lab.plane.market_id.bytes()
            || projection.epoch != self.general.epoch_id.bytes()
            || projection.price_scale != self.lab.grid_value.price_scale
        {
            return Err(OwnerSettlementPlanError::ContextMismatch);
        }
        project_owner_settlement_v1(projection).map_err(OwnerSettlementPlanError::Projection)
    }

    fn transaction(
        &self,
        family: &'static str,
        instructions: Vec<Instruction>,
        required_signers: Vec<SignerRole>,
    ) -> Result<BuiltTransaction> {
        if instructions.is_empty() || required_signers.first() != Some(&SignerRole::Payer) {
            return Err("local transaction plan requires instructions and the fee payer".into());
        }
        let transaction = Transaction::new_with_payer(&instructions, Some(&self.payer));
        Ok(BuiltTransaction {
            schema: PLAN_SCHEMA,
            family,
            unsigned_transaction: bincode::serialize(&transaction)?,
            required_signers,
        })
    }

    fn compute_budget() -> Instruction {
        Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
    }

    fn actor(&self, role: SignerRole) -> Address {
        match role {
            SignerRole::Payer => self.payer,
            SignerRole::SecondOwner => self.second_owner,
        }
    }

    fn actor_signers(role: SignerRole) -> Vec<SignerRole> {
        if role == SignerRole::Payer {
            vec![SignerRole::Payer]
        } else {
            vec![SignerRole::Payer, SignerRole::SecondOwner]
        }
    }

    fn artifact_transactions(
        &self,
        name: &'static str,
        upload: &ArtifactUpload,
        expires_slot: u64,
    ) -> Result<Vec<BuiltTransaction>> {
        let mut plans = vec![self.transaction(
            name,
            vec![
                Self::compute_budget(),
                plane::begin_artifact(self.payer, upload, expires_slot),
            ],
            vec![SignerRole::Payer],
        )?];
        for (index, chunk) in upload.body.chunks(ARTIFACT_CHUNK_BYTES).enumerate() {
            let cursor = index
                .checked_mul(ARTIFACT_CHUNK_BYTES)
                .and_then(|offset| u16::try_from(offset).ok())
                .ok_or("artifact cursor overflow")?;
            plans.push(self.transaction(
                name,
                vec![
                    Self::compute_budget(),
                    plane::write_artifact(self.payer, upload, cursor, chunk),
                ],
                vec![SignerRole::Payer],
            )?);
        }
        plans.push(self.transaction(
            name,
            vec![
                Self::compute_budget(),
                plane::seal_artifact(self.payer, upload),
            ],
            vec![SignerRole::Payer],
        )?);
        Ok(plans)
    }

    pub fn price_grid_upload(&self, expires_slot: u64) -> Result<Vec<BuiltTransaction>> {
        self.artifact_transactions(
            "price-grid-artifact",
            &plane::price_grid_upload(self.payer, &self.lab),
            expires_slot,
        )
    }

    pub fn policy_upload(&self, expires_slot: u64) -> Result<Vec<BuiltTransaction>> {
        self.artifact_transactions(
            "general-policy-artifact",
            &self.general.policy,
            expires_slot,
        )
    }

    pub fn create_market(&self) -> Result<BuiltTransaction> {
        self.transaction(
            "create-market",
            vec![
                Self::compute_budget(),
                plane::create_market(self.payer, &self.lab),
            ],
            vec![SignerRole::Payer],
        )
    }

    pub fn endow(&self, actor: SignerRole, sequence: u64, amount: u64) -> Result<BuiltTransaction> {
        self.transaction(
            "endow",
            vec![
                Self::compute_budget(),
                plane::endow(self.actor(actor), &self.lab, sequence, amount),
            ],
            Self::actor_signers(actor),
        )
    }

    pub fn split(
        &self,
        actor: SignerRole,
        sequence: u64,
        quantity: u64,
    ) -> Result<BuiltTransaction> {
        self.transaction(
            "split",
            vec![
                Self::compute_budget(),
                plane::split(self.actor(actor), &self.lab, sequence, quantity),
            ],
            Self::actor_signers(actor),
        )
    }

    pub fn init_epoch(&self, freeze_deadline_slot: u64) -> Result<Vec<BuiltTransaction>> {
        Ok(vec![
            self.transaction(
                "init-epoch",
                vec![
                    Self::compute_budget(),
                    plane::init_epoch(self.payer, &self.lab, &self.general, freeze_deadline_slot),
                ],
                vec![SignerRole::Payer],
            )?,
            self.transaction(
                "init-order-page",
                vec![
                    Self::compute_budget(),
                    plane::init_order_page(self.payer, &self.lab, &self.general),
                ],
                vec![SignerRole::Payer],
            )?,
        ])
    }

    pub fn place_single(
        &self,
        actor: SignerRole,
        sequence: u64,
        side: u8,
    ) -> Result<BuiltTransaction> {
        if side > 1 {
            return Err("single-order side must be 0 or 1".into());
        }
        self.transaction(
            "place-single-order",
            vec![
                Self::compute_budget(),
                plane::place_single_order(
                    self.actor(actor),
                    &self.lab,
                    &self.general,
                    sequence,
                    side,
                )
                .instruction,
            ],
            Self::actor_signers(actor),
        )
    }

    pub fn freeze_epoch(&self) -> Result<BuiltTransaction> {
        self.transaction(
            "freeze-epoch",
            vec![
                Self::compute_budget(),
                plane::freeze_epoch(&self.lab, &self.general),
            ],
            vec![SignerRole::Payer],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_client_contract::owner_settlement::{
        CandidateSettlementTotalsV1, ChainOwnerPositionV1, SelectedOwnerFeeV1, SettlementSideV1,
        VerifiedSettlementOrderV1, MAX_ORDERS,
    };

    fn actors() -> (Address, Address) {
        (
            Address::new_from_array([0x31; 32]),
            Address::new_from_array([0x32; 32]),
        )
    }

    fn settlement_projection<'a>(
        builder: &LocalTradingBuilder,
        orders: &'a [VerifiedSettlementOrderV1; MAX_ORDERS],
        fees: &'a [SelectedOwnerFeeV1; MAX_ORDERS],
        positions: &'a [ChainOwnerPositionV1; MAX_ORDERS],
    ) -> OwnerSettlementProjectionV1<'a> {
        OwnerSettlementProjectionV1 {
            market: builder.lab.plane.market_id.bytes(),
            epoch: builder.general.epoch_id.bytes(),
            candidate: [0x55; 32],
            owner_order_set_digest: [0x56; 32],
            price_scale: builder.lab.grid_value.price_scale,
            orders,
            order_len: 3,
            fees,
            fee_len: 2,
            expected: CandidateSettlementTotalsV1 {
                owner_count: 2,
                buy_price_units: 25_000,
                sell_price_units: 25_000,
                selected_fee_atoms: 0,
                rounding_pot_price_units: 10_000,
                owner_slice_end_count: 4,
            },
            positions,
            position_len: 2,
        }
    }

    #[test]
    fn campaign_builder_owns_exact_unsigned_transaction_families() {
        let (payer, second_owner) = actors();
        let builder = LocalTradingBuilder::campaign(payer, second_owner, 10, 12).unwrap();
        let freeze = builder.freeze_epoch().unwrap();
        assert_eq!(freeze.schema, PLAN_SCHEMA);
        assert_eq!(freeze.family, "freeze-epoch");
        assert_eq!(freeze.required_signers, [SignerRole::Payer]);
        assert!(!freeze.unsigned_transaction.is_empty());
        assert!(!builder.price_grid_upload(100).unwrap().is_empty());
        assert!(!builder.policy_upload(100).unwrap().is_empty());
    }

    #[test]
    fn nonce_changes_the_market_and_create_instruction_bytes() {
        let (payer, second_owner) = actors();
        let first = LocalTradingBuilder::new(payer, second_owner, 10, 12, 7).unwrap();
        let second = LocalTradingBuilder::new(payer, second_owner, 10, 12, 8).unwrap();
        assert_ne!(first.market_address(), second.market_address());
        assert_ne!(
            first.create_market().unwrap().unsigned_transaction,
            second.create_market().unwrap().unsigned_transaction
        );
    }

    #[test]
    fn refuses_aliased_actors_and_noncanonical_windows() {
        let (payer, second_owner) = actors();
        assert!(LocalTradingBuilder::campaign(payer, payer, 10, 12).is_err());
        assert!(LocalTradingBuilder::campaign(payer, second_owner, 12, 10).is_err());
        assert!(LocalTradingBuilder::campaign(payer, second_owner, 10, 10).is_err());
        assert!(LocalTradingBuilder::campaign(payer, second_owner, 0, 33).is_err());
    }

    #[test]
    fn owner_projection_aggregates_orders_and_refuses_foreign_context() {
        let (payer, second_owner) = actors();
        let builder = LocalTradingBuilder::campaign(payer, second_owner, 10, 12).unwrap();
        let buyer = [0x40; 32];
        let seller = [0x20; 32];
        let mut orders = [VerifiedSettlementOrderV1 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: 0,
            slice_count: 0,
            reserved_cash_atoms: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV1 {
            owner: buyer,
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: 12_500,
            slice_count: 1,
            reserved_cash_atoms: 2,
        };
        orders[1] = VerifiedSettlementOrderV1 {
            owner: seller,
            order_index: 2,
            side: SettlementSideV1::Sell,
            consideration_price_units: 25_000,
            slice_count: 2,
            reserved_cash_atoms: 0,
        };
        orders[2] = VerifiedSettlementOrderV1 {
            owner: buyer,
            order_index: 1,
            side: SettlementSideV1::Buy,
            consideration_price_units: 12_500,
            slice_count: 1,
            reserved_cash_atoms: 2,
        };
        let mut fees = [SelectedOwnerFeeV1::EMPTY; MAX_ORDERS];
        fees[0] = SelectedOwnerFeeV1 {
            owner: buyer,
            fee_atoms: 0,
        };
        fees[1] = SelectedOwnerFeeV1 {
            owner: seller,
            fee_atoms: 0,
        };
        let mut positions = [ChainOwnerPositionV1::EMPTY; MAX_ORDERS];
        positions[0] = ChainOwnerPositionV1 {
            owner: buyer,
            cash_atoms: 10,
            reserved_cash_atoms: 4,
        };
        positions[1] = ChainOwnerPositionV1 {
            owner: seller,
            cash_atoms: 0,
            reserved_cash_atoms: 0,
        };

        let projection = settlement_projection(&builder, &orders, &fees, &positions);
        let plan = builder.project_owner_settlement(&projection).unwrap();
        assert_eq!(plan.owner_count(), 2);
        assert_eq!(plan.rows().len(), 2);
        assert!(plan.rows()[0].expectation().owner < plan.rows()[1].expectation().owner);

        let mut foreign = projection;
        foreign.market = [0x77; 32];
        assert_eq!(
            builder.project_owner_settlement(&foreign),
            Err(OwnerSettlementPlanError::ContextMismatch)
        );
    }
}
