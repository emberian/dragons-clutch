//! Exact two-destination Fund distribution and closure.

use dclutch_pyth_contract::funding::required_resolution_minimum_balance;
use solana_program::{account_info::AccountInfo, program_error::ProgramError};

use crate::{
    AdapterError,
    authenticate::{FailureFrame, FundFacts, PriceFrame, SYSTEM_PROGRAM},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Distribution {
    first: u64,
    second: u64,
}

pub(crate) fn close_price(
    frame: &PriceFrame<'_, '_>,
    facts: FundFacts,
) -> Result<(), ProgramError> {
    let amounts = price_distribution(facts)?;
    distribute_and_close(
        frame.fund,
        frame.resolver,
        amounts.first,
        frame.sponsor,
        amounts.second,
    )
}

pub(crate) fn close_failure(
    frame: &FailureFrame<'_, '_>,
    facts: FundFacts,
) -> Result<(), ProgramError> {
    let amounts = failure_distribution(facts)?;
    distribute_and_close(
        frame.fund,
        frame.bounty_recipient,
        amounts.first,
        frame.sponsor,
        amounts.second,
    )
}

fn price_distribution(facts: FundFacts) -> Result<Distribution, ProgramError> {
    let remaining = facts.funding.remaining();
    let resolver = remaining
        .provider_principal()
        .checked_add(remaining.bounty_principal())
        .ok_or(AdapterError::Arithmetic)?;
    let sponsor = facts
        .required_rent
        .checked_add(facts.sponsor_refund_excess)
        .ok_or(AdapterError::Arithmetic)?;
    validate_minimum(facts)?;
    Ok(Distribution {
        first: resolver,
        second: sponsor,
    })
}

fn failure_distribution(facts: FundFacts) -> Result<Distribution, ProgramError> {
    let remaining = facts.funding.remaining();
    let sponsor = facts
        .required_rent
        .checked_add(remaining.provider_principal())
        .and_then(|value| value.checked_add(facts.sponsor_refund_excess))
        .ok_or(AdapterError::Arithmetic)?;
    validate_minimum(facts)?;
    Ok(Distribution {
        first: remaining.bounty_principal(),
        second: sponsor,
    })
}

fn validate_minimum(facts: FundFacts) -> Result<(), ProgramError> {
    let expected =
        required_resolution_minimum_balance(facts.funding).map_err(|_| AdapterError::FundClose)?;
    let reconstructed = facts
        .required_rent
        .checked_add(facts.funding.remaining().total_principal())
        .ok_or(AdapterError::Arithmetic)?;
    if expected != reconstructed || facts.sponsor_refund == [0; 32] {
        return Err(AdapterError::FundClose.into());
    }
    Ok(())
}

#[inline(never)]
fn distribute_and_close(
    fund: &AccountInfo<'_>,
    first: &AccountInfo<'_>,
    first_amount: u64,
    second: &AccountInfo<'_>,
    second_amount: u64,
) -> Result<(), ProgramError> {
    if fund.key == first.key || fund.key == second.key {
        return Err(AdapterError::FundClose.into());
    }
    let total = first_amount
        .checked_add(second_amount)
        .ok_or(AdapterError::Arithmetic)?;
    if fund.try_lamports().map_err(|_| AdapterError::AccountData)? != total {
        return Err(AdapterError::FundClose.into());
    }

    if first.key == second.key {
        let next = first
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            .checked_add(total)
            .ok_or(AdapterError::Arithmetic)?;
        let mut destination = first
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut source = fund
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **destination = next;
        **source = 0;
    } else {
        let first_next = first
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            .checked_add(first_amount)
            .ok_or(AdapterError::Arithmetic)?;
        let second_next = second
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            .checked_add(second_amount)
            .ok_or(AdapterError::Arithmetic)?;
        let mut first_lamports = first
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut second_lamports = second
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut source = fund
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **first_lamports = first_next;
        **second_lamports = second_next;
        **source = 0;
    }

    // Keep resize authorization unambiguous: data is removed while dClutch
    // still owns the Fund; only then is the empty account assigned to System.
    fund.resize(0)
        .map_err(|_| ProgramError::from(AdapterError::FundClose))?;
    fund.assign(&SYSTEM_PROGRAM);
    if fund.try_lamports().map_err(|_| AdapterError::AccountData)? != 0
        || !fund
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
        || fund.owner != &SYSTEM_PROGRAM
    {
        return Err(AdapterError::FundClose.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        ContentId, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_pyth_contract::funding::construct_required_resolution_funding;
    use solana_program::{hash::hash, pubkey::Pubkey};
    use std::{boxed::Box, vec::Vec};

    use super::*;

    fn facts(actual: u64) -> FundFacts {
        let required_rent = 11;
        let quote = FundingQuoteV1::new(required_rent, 0, 0, 3, 5, 0, 0).expect("quote");
        let entry = CapabilityEntryV1::new(
            content_id(10),
            content_id(11),
            content_id(12),
            content_id(13),
            content_id(14),
            content_id(15),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        let mut manifest_bytes = [0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest =
            CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest");
        let manifest_id =
            ContentId::new(hash(manifest.as_bytes()).to_bytes()).expect("manifest ID");
        let selected = manifest
            .required_founding_entry_for_config(content_id(12))
            .expect("selected entry");
        let funding = construct_required_resolution_funding(
            manifest_id,
            manifest,
            selected,
            required_rent,
            44,
        )
        .expect("funding");
        let minimum = required_resolution_minimum_balance(funding).expect("minimum");
        FundFacts {
            funding,
            required_rent,
            sponsor_refund_excess: actual.checked_sub(minimum).expect("funded"),
            sponsor_refund: [2; 32],
        }
    }

    fn content_id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content ID")
    }

    fn account(key: u8, lamports: u64, owner: Pubkey) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_from_array([key; 32])));
        let lamports = Box::leak(Box::new(lamports));
        let data: &'static mut [u8] = Box::leak(Vec::<u8>::new().into_boxed_slice());
        let owner = Box::leak(Box::new(owner));
        AccountInfo::new(key, false, true, lamports, data, owner, false)
    }

    #[test]
    fn price_and_failure_partition_every_fund_lamport_exactly() {
        let price = price_distribution(facts(26)).expect("price partition");
        assert_eq!(
            price,
            Distribution {
                first: 8,
                second: 18
            }
        );
        let failure = failure_distribution(facts(26)).expect("failure partition");
        assert_eq!(
            failure,
            Distribution {
                first: 5,
                second: 21
            }
        );
        assert_eq!(price.first + price.second, 26);
        assert_eq!(failure.first + failure.second, 26);
    }

    #[test]
    fn distinct_and_aliased_destinations_receive_the_exact_total() {
        let program = Pubkey::new_from_array([9; 32]);
        let fund = account(1, 12, program);
        let first = account(2, 3, SYSTEM_PROGRAM);
        let second = account(3, 5, SYSTEM_PROGRAM);
        distribute_and_close(&fund, &first, 7, &second, 5).expect("distinct close");
        assert_eq!(fund.lamports(), 0);
        assert_eq!(first.lamports(), 10);
        assert_eq!(second.lamports(), 10);
        assert_eq!(fund.owner, &SYSTEM_PROGRAM);

        let fund = account(4, 12, program);
        let destination = account(5, 3, SYSTEM_PROGRAM);
        distribute_and_close(&fund, &destination, 7, &destination, 5).expect("aliased close");
        assert_eq!(fund.lamports(), 0);
        assert_eq!(destination.lamports(), 15);
        assert_eq!(fund.owner, &SYSTEM_PROGRAM);
    }

    #[test]
    fn a_fund_destination_alias_refuses_before_mutation() {
        let program = Pubkey::new_from_array([9; 32]);
        let fund = account(6, 12, program);
        let second = account(7, 0, SYSTEM_PROGRAM);
        assert_eq!(
            distribute_and_close(&fund, &fund, 7, &second, 5),
            Err(AdapterError::FundClose.into())
        );
        assert_eq!(fund.lamports(), 12);
        assert_eq!(second.lamports(), 0);
        assert_eq!(fund.owner, &program);
    }
}
