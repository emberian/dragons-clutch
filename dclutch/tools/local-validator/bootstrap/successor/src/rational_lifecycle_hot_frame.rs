//! Family-neutral authenticated carrier for a Rational V6 Hot frame.
//!
//! A family-specific assembler authenticates record coordinates and current
//! account bodies, then puts the resulting physical frame here.  The selected
//! Rational operator consumes this carrier rather than a family-specific route
//! document, so Structured does not duplicate General's frame geometry.

use dclutch_market::capability_program::hot_v3::{
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_operator::rational_lifecycle_hot::{
    CheckedRationalLifecycleHotOuterV3, RationalLifecycleHotStateV3,
};
use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey};

use crate::{Error, Result};

/// A fully authenticated Hot frame held in owned host memory.
#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedRationalLifecycleHotFrameV1 {
    pub(crate) fixed_accounts: Vec<AccountMeta>,
    pub(crate) strategy_accounts: Vec<AccountMeta>,
    pub(crate) root_data: Vec<u8>,
    pub(crate) market_data: Vec<u8>,
    pub(crate) release_set: [u8; 32],
    pub(crate) market: Pubkey,
    pub(crate) generation: u64,
    pub(crate) finalized_slot: u64,
    pub(crate) hot_outer: CheckedRationalLifecycleHotOuterV3,
}

impl AuthenticatedRationalLifecycleHotFrameV1 {
    /// Validate frame geometry before lending it to the unsigned operator.
    pub(crate) fn state(&self) -> Result<RationalLifecycleHotStateV3<'_>> {
        if self.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
            || self.release_set == [0; 32]
            || self.market == Pubkey::default()
            || self.generation == 0
            || self.finalized_slot == 0
            || self.root_data.is_empty()
            || self.market_data.is_empty()
        {
            return Err(Error::new(
                "Rational Hot frame omitted a required finalized fixed fact",
            ));
        }
        let market = self
            .fixed_accounts
            .get(HOT_MARKET_ACCOUNT_V3)
            .ok_or_else(|| Error::new("Rational Hot frame omitted its Market coordinate"))?;
        let root = self
            .fixed_accounts
            .get(HOT_ROOT_ACCOUNT_V3)
            .ok_or_else(|| Error::new("Rational Hot frame omitted its root coordinate"))?;
        let trading = self
            .fixed_accounts
            .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .ok_or_else(|| Error::new("Rational Hot frame omitted its Trading coordinate"))?;
        if market.pubkey != self.market
            || market.is_signer
            || market.is_writable
            || !root.is_writable
            || root.is_signer
            || trading.pubkey != self.hot_outer.trading_program
            || trading.is_signer
            || trading.is_writable
        {
            return Err(Error::new(
                "Rational Hot frame fixed coordinates differ from the authenticated state",
            ));
        }
        Ok(RationalLifecycleHotStateV3 {
            fixed_accounts: &self.fixed_accounts,
            strategy_accounts: &self.strategy_accounts,
            root_data: &self.root_data,
            market_data: &self.market_data,
            release_set: self.release_set,
            market: self.market,
            generation: self.generation,
            finalized_slot: self.finalized_slot,
            hot_outer: Some(self.hot_outer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> AuthenticatedRationalLifecycleHotFrameV1 {
        let market = Pubkey::new_from_array([3; 32]);
        let trading = Pubkey::new_from_array([4; 32]);
        let mut fixed = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| {
                AccountMeta::new_readonly(Pubkey::new_from_array([index as u8; 32]), false)
            })
            .collect::<Vec<_>>();
        fixed[HOT_MARKET_ACCOUNT_V3] = AccountMeta::new_readonly(market, false);
        fixed[HOT_ROOT_ACCOUNT_V3] = AccountMeta::new(Pubkey::new_from_array([5; 32]), false);
        fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = AccountMeta::new_readonly(trading, false);
        AuthenticatedRationalLifecycleHotFrameV1 {
            fixed_accounts: fixed,
            strategy_accounts: Vec::new(),
            root_data: vec![1],
            market_data: vec![2],
            release_set: [6; 32],
            market,
            generation: 1,
            finalized_slot: 1,
            hot_outer: CheckedRationalLifecycleHotOuterV3 {
                trading_program: trading,
                artifact_release: [7; 32],
                checked_manifest_digest: [8; 32],
            },
        }
    }

    #[test]
    fn refuses_market_coordinate_substitution() {
        let mut value = frame();
        value.fixed_accounts[HOT_MARKET_ACCOUNT_V3] =
            AccountMeta::new_readonly(Pubkey::new_from_array([9; 32]), false);
        let error = value.state().expect_err("substituted Market must refuse");
        assert_eq!(
            error.to_string(),
            "Rational Hot frame fixed coordinates differ from the authenticated state"
        );
    }
}
