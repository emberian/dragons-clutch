//! Exact Pyth Receiver CPI construction and postconditions.

use alloc::vec::Vec;

use dclutch_pyth_svm::FullPriceUpdateV2;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    AdapterError,
    authenticate::{PriceFrame, ProviderFacts, SYSTEM_PROGRAM},
};

const POST_UPDATE_DISCRIMINATOR: [u8; 8] = [133, 95, 207, 175, 11, 79, 118, 44];
const RECLAIM_RENT_DISCRIMINATOR: [u8; 8] = [218, 200, 19, 197, 227, 89, 192, 22];

#[inline(never)]
pub(crate) fn post_and_load(
    frame: &PriceFrame<'_, '_>,
    facts: ProviderFacts,
    body: &[u8],
    clock_slot: u64,
    expected_feed: [u8; 32],
) -> Result<FullPriceUpdateV2, ProgramError> {
    let resolver_before = frame
        .resolver
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?;
    let treasury_before = frame
        .treasury
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?;
    let debit = facts
        .update_rent
        .checked_add(facts.fee)
        .ok_or(AdapterError::Arithmetic)?;
    let expected_resolver = resolver_before
        .checked_sub(debit)
        .ok_or(AdapterError::Arithmetic)?;
    let expected_treasury = treasury_before
        .checked_add(facts.fee)
        .ok_or(AdapterError::Arithmetic)?;

    let instruction = post_instruction(
        *frame.receiver.key,
        *frame.resolver.key,
        *frame.encoded_vaa.key,
        *frame.config.key,
        *frame.treasury.key,
        *frame.update.key,
        *frame.system.key,
        body,
    )?;
    invoke(
        &instruction,
        &[
            frame.resolver.clone(),
            frame.encoded_vaa.clone(),
            frame.config.clone(),
            frame.treasury.clone(),
            frame.update.clone(),
            frame.system.clone(),
            frame.resolver.clone(),
            frame.receiver.clone(),
        ],
    )
    .map_err(|_| ProgramError::from(AdapterError::ProviderPostCpi))?;

    if frame
        .resolver
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?
        != expected_resolver
        || frame
            .treasury
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            != expected_treasury
        || frame
            .update
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            != facts.update_rent
        || frame.update.owner != frame.receiver.key
    {
        return Err(AdapterError::ProviderPostcondition.into());
    }

    let update = {
        let bytes = frame
            .update
            .try_borrow_data()
            .map_err(|_| AdapterError::AccountData)?;
        FullPriceUpdateV2::parse(&bytes).map_err(|_| AdapterError::ProviderPostcondition)?
    };
    if update.write_authority() != frame.resolver.key.to_bytes()
        || update.feed_id() != expected_feed
        || update.posted_slot() != clock_slot
    {
        return Err(AdapterError::ProviderPostcondition.into());
    }
    Ok(update)
}

#[inline(never)]
pub(crate) fn reclaim(frame: &PriceFrame<'_, '_>) -> Result<(), ProgramError> {
    let resolver_before = frame
        .resolver
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?;
    let update_before = frame
        .update
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?;
    let expected_resolver = resolver_before
        .checked_add(update_before)
        .ok_or(AdapterError::Arithmetic)?;
    let instruction =
        reclaim_instruction(*frame.receiver.key, *frame.resolver.key, *frame.update.key)?;
    invoke(
        &instruction,
        &[
            frame.resolver.clone(),
            frame.update.clone(),
            frame.receiver.clone(),
        ],
    )
    .map_err(|_| ProgramError::from(AdapterError::ProviderReclaimCpi))?;

    if frame
        .resolver
        .try_lamports()
        .map_err(|_| AdapterError::AccountData)?
        != expected_resolver
        || frame
            .update
            .try_lamports()
            .map_err(|_| AdapterError::AccountData)?
            != 0
        || frame.update.owner != &SYSTEM_PROGRAM
        || !frame
            .update
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
    {
        return Err(AdapterError::ProviderReclaimPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn post_instruction(
    receiver: Pubkey,
    resolver: Pubkey,
    encoded_vaa: Pubkey,
    config: Pubkey,
    treasury: Pubkey,
    update: Pubkey,
    system: Pubkey,
    body: &[u8],
) -> Result<Instruction, ProgramError> {
    let data_len = POST_UPDATE_DISCRIMINATOR
        .len()
        .checked_add(body.len())
        .ok_or(AdapterError::Arithmetic)?;
    let mut data = Vec::new();
    data.try_reserve_exact(data_len)
        .map_err(|_| AdapterError::Arithmetic)?;
    data.extend_from_slice(&POST_UPDATE_DISCRIMINATOR);
    data.extend_from_slice(body);

    let mut accounts = Vec::new();
    accounts
        .try_reserve_exact(7)
        .map_err(|_| AdapterError::Arithmetic)?;
    accounts.push(AccountMeta::new(resolver, true));
    accounts.push(AccountMeta::new_readonly(encoded_vaa, false));
    accounts.push(AccountMeta::new_readonly(config, false));
    accounts.push(AccountMeta::new(treasury, false));
    accounts.push(AccountMeta::new(update, true));
    accounts.push(AccountMeta::new_readonly(system, false));
    accounts.push(AccountMeta::new_readonly(resolver, true));
    Ok(Instruction {
        program_id: receiver,
        accounts,
        data,
    })
}

fn reclaim_instruction(
    receiver: Pubkey,
    resolver: Pubkey,
    update: Pubkey,
) -> Result<Instruction, ProgramError> {
    let mut accounts = Vec::new();
    accounts
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    accounts.push(AccountMeta::new(resolver, true));
    accounts.push(AccountMeta::new(update, false));
    Ok(Instruction {
        program_id: receiver,
        accounts,
        data: Vec::from(RECLAIM_RENT_DISCRIMINATOR),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    #[test]
    fn post_instruction_is_the_exact_reviewed_anchor_abi() {
        let body = [8, 9, 10];
        let instruction = post_instruction(
            key(1),
            key(2),
            key(3),
            key(4),
            key(5),
            key(6),
            key(7),
            &body,
        )
        .expect("fixed instruction");
        assert_eq!(instruction.program_id, key(1));
        assert_eq!(
            instruction.data.get(..8),
            Some(&POST_UPDATE_DISCRIMINATOR[..])
        );
        assert_eq!(instruction.data.get(8..), Some(&body[..]));
        assert_eq!(
            instruction.accounts.as_slice(),
            &[
                AccountMeta::new(key(2), true),
                AccountMeta::new_readonly(key(3), false),
                AccountMeta::new_readonly(key(4), false),
                AccountMeta::new(key(5), false),
                AccountMeta::new(key(6), true),
                AccountMeta::new_readonly(key(7), false),
                AccountMeta::new_readonly(key(2), true),
            ]
        );
    }

    #[test]
    fn reclaim_instruction_is_the_exact_reviewed_anchor_abi() {
        let instruction = reclaim_instruction(key(1), key(2), key(3)).expect("fixed instruction");
        assert_eq!(instruction.program_id, key(1));
        assert_eq!(instruction.data, RECLAIM_RENT_DISCRIMINATOR);
        assert_eq!(
            instruction.accounts,
            [
                AccountMeta::new(key(2), true),
                AccountMeta::new(key(3), false)
            ]
        );
    }
}
