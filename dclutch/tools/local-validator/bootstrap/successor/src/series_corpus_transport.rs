//! Read-only transport for the native Series acquisition vector.
//!
//! RPC pages are fetched concurrently and accepted only when every context
//! names the same finalized slot. Neither missing pages nor differing slots
//! can be rewritten into a synthetic common observation.

use serde_json::json;
use solana_program::pubkey::Pubkey;

use crate::{
    Error, Result,
    rpc::{
        RPC_GET_MULTIPLE_ACCOUNTS_MAX_ADDRESSES_V1, Rpc, RpcAccount,
        parse_multiple_accounts_result_v1,
    },
};

// Provisional transport retry budget, inherited from no protocol condition.
// A caller may rerun before any journal is created. Lift only after measuring
// finalized-page convergence; changing this cannot widen a native schedule.
const FINALIZED_PAGE_ATTEMPTS_V1: usize = 8;

pub(crate) fn read_series_finalized_corpus_v1(
    rpc: &mut Rpc,
    addresses: &[Pubkey],
    minimum_slot: u64,
) -> Result<(u64, Vec<Option<RpcAccount>>)> {
    if addresses.len() <= RPC_GET_MULTIPLE_ACCOUNTS_MAX_ADDRESSES_V1 {
        return rpc.finalized_accounts(addresses, minimum_slot);
    }
    let mut floor = minimum_slot;
    for _ in 0..FINALIZED_PAGE_ATTEMPTS_V1 {
        let chunks = addresses
            .chunks(RPC_GET_MULTIPLE_ACCOUNTS_MAX_ADDRESSES_V1)
            .collect::<Vec<_>>();
        let calls = chunks.iter().map(|page| ("getMultipleAccounts", json!([
            page.iter().map(ToString::to_string).collect::<Vec<_>>(), {
                "encoding": "base64", "commitment": "finalized", "minContextSlot": floor,
            }
        ]))).collect::<Vec<_>>();
        let responses = rpc.read_parallel_v1(&calls)?;
        if responses.len() != chunks.len() {
            return Err(Error::new(
                "Series finalized acquisition omitted an RPC page",
            ));
        }
        let pages = responses
            .into_iter()
            .zip(chunks)
            .map(|(response, page)| parse_multiple_accounts_result_v1(response, page.len(), floor))
            .collect::<Result<Vec<_>>>()?;
        let first = pages[0].0;
        if pages.iter().all(|(slot, _)| *slot == first) {
            return assemble_series_finalized_pages_v1(pages);
        }
        floor = pages
            .iter()
            .map(|(slot, _)| *slot)
            .max()
            .expect("nonempty pages");
    }
    Err(Error::new(
        "Series finalized RPC pages did not converge to one observation",
    ))
}

fn assemble_series_finalized_pages_v1(
    pages: Vec<(u64, Vec<Option<RpcAccount>>)>,
) -> Result<(u64, Vec<Option<RpcAccount>>)> {
    let slot = pages
        .first()
        .map(|page| page.0)
        .ok_or_else(|| Error::new("Series finalized acquisition had no RPC pages"))?;
    if slot == 0 || pages.iter().any(|page| page.0 != slot) {
        return Err(Error::new(
            "Series finalized acquisition mixed observation slots",
        ));
    }
    Ok((
        slot,
        pages
            .into_iter()
            .flat_map(|(_, accounts)| accounts)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_pages_preserve_order_and_absence_and_refuse_different_finalized_slots() {
        let owner = Pubkey::new_unique();
        let present = || {
            Some(RpcAccount {
                owner,
                lamports: 19,
                executable: false,
                rent_epoch: 0,
                data: vec![7],
            })
        };
        let (slot, accounts) =
            assemble_series_finalized_pages_v1(vec![(41, vec![None]), (41, vec![present()])])
                .expect("same finalized observation");
        assert_eq!(slot, 41);
        assert_eq!(accounts.len(), 2);
        assert!(accounts[0].is_none());
        let second = accounts[1].as_ref().expect("present second account");
        assert_eq!(
            (second.owner, second.lamports, second.data.as_slice()),
            (owner, 19, &[7][..])
        );
        assert_eq!(
            assemble_series_finalized_pages_v1(vec![(41, vec![None]), (42, vec![present()]),])
                .expect_err("different contexts cannot be relabeled as one snapshot")
                .to_string(),
            "Series finalized acquisition mixed observation slots"
        );
    }
}
