//! Caller-side builder for the Claims-owned Fractional atomic child.
//!
//! The returned instruction is a Trading CPI instruction: its caller-authority
//! and Fractional-root signer bits are satisfied only by the activated Trading
//! program's two `invoke_signed` seed sets. It is not a directly submittable
//! wallet instruction.

use dclutch_claims_svm::{
    frame_spec_v1::SignedDeltaFrameSpecV3,
    liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_TERMS_RAW_V3, FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
    FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
    FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3, FRACTIONAL_ROOT_PDA_SEED_V1, FractionalExposureActionV2,
    FractionalExposureRequestV2, FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{Error, Result};

const MARKET: usize = 1;
const REGISTRY: usize = 13;
const TRADING_PROGRAM: usize = 14;
const CLAIMS_PROGRAM: usize = 16;
const POSITION_0: usize = 20;
const POSITION_1: usize = 21;

/// Build one exact atomic Claims CPI after checking every caller-owned key and privilege.
pub fn build_fractional_atomic_claims_instruction_v3(
    request: FractionalExposureRequestV2,
    terms: FractionalExposureTermsV2<'_>,
    root: FractionalRootV1,
    accounts: &[AccountMeta],
) -> Result<Instruction> {
    request.bind_terms(terms).map_err(|_| Error::Claims)?;
    if !matches!(
        request.action(),
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap
    ) || accounts.len() != FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3
    {
        return Err(Error::Claims);
    }
    let claims_program = meta(accounts, CLAIMS_PROGRAM)?.pubkey;
    let trading_program = meta(accounts, TRADING_PROGRAM)?.pubkey;
    let registry = meta(accounts, REGISTRY)?.pubkey;
    let bytes = request.to_bytes().map_err(|_| Error::Claims)?;
    let request_digest = hash(&bytes).to_bytes();
    let input = request.input();
    let caller = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market,
        ExecutionRoleV1::Trading,
        input.terms,
        request_digest,
    )
    .map_err(|_| Error::Claims)?;
    let expected_authority = Pubkey::find_program_address(&caller.as_slices(), &trading_program).0;
    let expected_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_slice()],
        &claims_program,
    )
    .0;
    let root_input = root.input();
    let expected_root = Pubkey::create_program_address(
        &[
            FRACTIONAL_ROOT_PDA_SEED_V1,
            input.terms.as_slice(),
            input.market.as_slice(),
            &[root_input.bump],
        ],
        &trading_program,
    )
    .map_err(|_| Error::Claims)?;
    if root_input.terms != input.terms
        || root_input.market != input.market
        || root_input.revision != input.expected_revision
        || meta(accounts, 0)?.pubkey != expected_authority
        || meta(accounts, MARKET)?.pubkey != expected_market
        || meta(accounts, FRACTIONAL_ATOMIC_ROOT_V3)?.pubkey != expected_root
        || meta(accounts, FRACTIONAL_ATOMIC_ACTOR_V3)?
            .pubkey
            .to_bytes()
            != input.owner
        || meta(accounts, FRACTIONAL_ATOMIC_SHARD_MINT_V3)?
            .pubkey
            .to_bytes()
            != terms
                .shard_mint(input.representation_coordinate)
                .map_err(|_| Error::Claims)?
        || meta(accounts, FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3)?
            .pubkey
            .to_bytes()
            != match request.action() {
                FractionalExposureActionV2::Wrap => input.destination_token_account,
                FractionalExposureActionV2::WholeUnwrap => input.source_token_account,
                _ => return Err(Error::Claims),
            }
        || meta(accounts, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3)?
            .pubkey
            .to_bytes()
            != terms.token_program()
    {
        return Err(Error::Claims);
    }

    require_record_pair(
        accounts,
        registry,
        FRACTIONAL_ATOMIC_TERMS_RAW_V3,
        FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        input.terms,
    )?;
    require_record_pair(
        accounts,
        registry,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        input.token_behavior,
    )?;

    let spec = SignedDeltaFrameSpecV3::new(2).map_err(|_| Error::Claims)?;
    for index in 0..spec.account_count().map_err(|_| Error::Claims)? {
        let expected = spec.account(index).map_err(|_| Error::Claims)?.privileges();
        let observed = meta(accounts, usize::from(index))?;
        if observed.is_signer != expected.signer() || observed.is_writable != expected.writable() {
            return Err(Error::Claims);
        }
    }
    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_market.to_bytes(), input.owner)
            .map_err(|_| Error::Claims)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let reserve_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_market.to_bytes(), expected_root.to_bytes())
            .map_err(|_| Error::Claims)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let expected_positions = if input.owner < expected_root.to_bytes() {
        [actor_position, reserve_position]
    } else {
        [reserve_position, actor_position]
    };
    if meta(accounts, POSITION_0)?.pubkey != expected_positions[0]
        || meta(accounts, POSITION_1)?.pubkey != expected_positions[1]
    {
        return Err(Error::Claims);
    }
    for (index, signer, writable) in [
        (FRACTIONAL_ATOMIC_TERMS_RAW_V3, false, false),
        (FRACTIONAL_ATOMIC_TERMS_STAGING_V3, false, false),
        (FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3, false, false),
        (FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3, false, false),
        (FRACTIONAL_ATOMIC_ROOT_V3, true, false),
        (FRACTIONAL_ATOMIC_ACTOR_V3, true, false),
        (FRACTIONAL_ATOMIC_SHARD_MINT_V3, false, true),
        (FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, false, true),
        (FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3, false, false),
    ] {
        let observed = meta(accounts, index)?;
        if observed.is_signer != signer || observed.is_writable != writable {
            return Err(Error::Claims);
        }
    }
    Ok(Instruction {
        program_id: claims_program,
        accounts: accounts.to_vec(),
        data: bytes.to_vec(),
    })
}

fn require_record_pair(
    accounts: &[AccountMeta],
    registry: Pubkey,
    raw_index: usize,
    staging_index: usize,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<()> {
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if meta(accounts, raw_index)?.pubkey != raw || meta(accounts, staging_index)?.pubkey != staging
    {
        Err(Error::Claims)
    } else {
        Ok(())
    }
}

fn meta(accounts: &[AccountMeta], index: usize) -> Result<&AccountMeta> {
    accounts.get(index).ok_or(Error::Claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_fractional_claim_contract::{
        FractionalExposureRequestInputV2, FractionalRootInputV1,
    };
    use dclutch_fractional_claim_kernel::{
        FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2,
        encode_fractional_exposure_terms_v2, fractional_exposure_terms_bytes_v2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    struct Fixture {
        request: FractionalExposureRequestV2,
        terms_bytes: Vec<u8>,
        terms_id: [u8; 32],
        root: FractionalRootV1,
        accounts: Vec<AccountMeta>,
    }

    impl Fixture {
        fn terms(&self) -> FractionalExposureTermsV2<'_> {
            FractionalExposureTermsV2::decode(
                &self.terms_bytes,
                FractionalExposureTermsAdmissionV2 {
                    selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    selected_terms_id: self.terms_id,
                    finalized_terms_id: self.terms_id,
                    recomputed_terms_digest: self.terms_id,
                    finalized_terms_digest: self.terms_id,
                    record_authenticated: true,
                },
            )
            .expect("terms")
        }
    }

    fn fixture() -> Fixture {
        let market = id(2);
        let release = id(3);
        let token_behavior = id(4);
        let actor = id(5);
        let destination = id(6);
        let mints = [id(7), id(8)];
        let width = fractional_exposure_terms_bytes_v2(mints.len()).expect("width");
        let mut scratch = vec![0; width];
        let mut terms_bytes = vec![0; width];
        encode_fractional_exposure_terms_v2(
            FractionalExposureTermsInputV2 {
                market,
                product_record: id(9),
                result_domain: id(10),
                release_set: release,
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior,
                exposure_id: id(11),
                product_basis: id(12),
                representation_basis: id(13),
                graph_id: id(14),
                product_width: 3,
                denominator: 100,
                shard_mints: &mints,
            },
            &mut scratch,
            &mut terms_bytes,
        )
        .expect("encode terms");
        let terms_id = hash(&terms_bytes).to_bytes();
        let request = FractionalExposureRequestV2::new(
            FractionalExposureActionV2::Wrap,
            FractionalExposureRequestInputV2 {
                release_set: release,
                market,
                product_record: id(9),
                result_domain: id(10),
                terms: terms_id,
                token_behavior,
                exposure: id(11),
                owner: actor,
                source_token_account: [0; 32],
                destination_token_account: destination,
                terminal_digest: [0; 32],
                expected_revision: 4,
                quantity: 2,
                representation_coordinate: 1,
            },
        )
        .expect("request");
        let claims = Pubkey::new_from_array(id(20));
        let trading = Pubkey::new_from_array(id(21));
        let registry = Pubkey::new_from_array(id(22));
        let (root_key, bump) = Pubkey::find_program_address(
            &[FRACTIONAL_ROOT_PDA_SEED_V1, &terms_id, &market],
            &trading,
        );
        let root = FractionalRootV1::new(FractionalRootInputV1 {
            bump,
            terms: terms_id,
            market,
            rent_beneficiary: id(23),
            revision: 4,
            historical_rent_principal: 1,
        })
        .expect("root");
        let mut accounts = Vec::new();
        let spec = SignedDeltaFrameSpecV3::new(2).expect("spec");
        for index in 0..spec.account_count().expect("count") {
            let privileges = spec.account(index).expect("account").privileges();
            accounts.push(if privileges.writable() {
                AccountMeta::new(
                    Pubkey::new_from_array(id(u8::try_from(index + 40).expect("id"))),
                    privileges.signer(),
                )
            } else {
                AccountMeta::new_readonly(
                    Pubkey::new_from_array(id(u8::try_from(index + 40).expect("id"))),
                    privileges.signer(),
                )
            });
        }
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(root_key, true));
        accounts.push(AccountMeta::new_readonly(
            Pubkey::new_from_array(actor),
            true,
        ));
        accounts.push(AccountMeta::new(Pubkey::new_from_array(mints[1]), false));
        accounts.push(AccountMeta::new(Pubkey::new_from_array(destination), false));
        accounts.push(AccountMeta::new_readonly(
            Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            false,
        ));
        accounts[REGISTRY].pubkey = registry;
        accounts[TRADING_PROGRAM].pubkey = trading;
        accounts[CLAIMS_PROGRAM].pubkey = claims;
        let request_digest = hash(&request.to_bytes().expect("bytes")).to_bytes();
        let caller = CallerAuthoritySeedsV1::from_bytes(
            release,
            market,
            ExecutionRoleV1::Trading,
            terms_id,
            request_digest,
        )
        .expect("caller");
        accounts[0].pubkey = Pubkey::find_program_address(&caller.as_slices(), &trading).0;
        let claims_market =
            Pubkey::find_program_address(&[LIABILITY_BASIS_MARKET_SEED_V2, &market], &claims).0;
        accounts[MARKET].pubkey = claims_market;
        let actor_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), actor)
                .expect("actor seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let reserve_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), root_key.to_bytes())
                .expect("reserve seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let ordered = if actor < root_key.to_bytes() {
            [actor_position, reserve_position]
        } else {
            [reserve_position, actor_position]
        };
        accounts[POSITION_0].pubkey = ordered[0];
        accounts[POSITION_1].pubkey = ordered[1];
        for (raw_index, staging_index, schema, digest) in [
            (
                FRACTIONAL_ATOMIC_TERMS_RAW_V3,
                FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
                FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                terms_id,
            ),
            (
                FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
                FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
                TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                token_behavior,
            ),
        ] {
            accounts[raw_index].pubkey = Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
            accounts[staging_index].pubkey = Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
        }
        Fixture {
            request,
            terms_bytes,
            terms_id,
            root,
            accounts,
        }
    }

    #[test]
    fn exact_inner_caller_builds_and_binds_both_program_signers() {
        let fixture = fixture();
        let instruction = build_fractional_atomic_claims_instruction_v3(
            fixture.request,
            fixture.terms(),
            fixture.root,
            &fixture.accounts,
        )
        .expect("instruction");
        assert_eq!(instruction.accounts.len(), 31);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_signer);
        assert!(instruction.accounts[FRACTIONAL_ATOMIC_ACTOR_V3].is_signer);
        assert_eq!(instruction.data.len(), 416);
    }

    #[test]
    fn root_actor_record_and_position_substitution_refuse() {
        for index in [
            0,
            POSITION_0,
            FRACTIONAL_ATOMIC_TERMS_RAW_V3,
            FRACTIONAL_ATOMIC_ROOT_V3,
            FRACTIONAL_ATOMIC_ACTOR_V3,
            FRACTIONAL_ATOMIC_SHARD_MINT_V3,
        ] {
            let mut fixture = fixture();
            fixture.accounts[index].pubkey = Pubkey::new_unique();
            assert!(
                build_fractional_atomic_claims_instruction_v3(
                    fixture.request,
                    fixture.terms(),
                    fixture.root,
                    &fixture.accounts,
                )
                .is_err(),
                "index {index}"
            );
        }
        let mut fixture = fixture();
        fixture.accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_signer = false;
        assert!(
            build_fractional_atomic_claims_instruction_v3(
                fixture.request,
                fixture.terms(),
                fixture.root,
                &fixture.accounts,
            )
            .is_err()
        );
    }
}
