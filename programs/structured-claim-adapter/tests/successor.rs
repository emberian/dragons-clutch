use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::runtime_contract::{
    decode_historical_descriptor_v1, DescriptorBasisV1, DescriptorStateV1,
    StructuredClaimDescriptorV2, StructuredClaimRuntimeAddressesV1,
    DESCRIPTOR_ACCOUNT_BYTES, DESCRIPTOR_ACCOUNT_TAG,
};
use clutch_structured_claim_adapter::{
    bind_descriptor_v1, canonical_native_claim_id_v1,
    canonical_series_scoped_wrapper_product_id_v2, PdaVerifierV1, RuntimeDeploymentsV1,
};

fn key(marker: u8) -> [u8; 32] {
    [marker; 32]
}

#[derive(Clone, Copy, Debug)]
struct AcceptKnownPdas;

impl PdaVerifierV1 for AcceptKnownPdas {
    fn verify(
        &self,
        _program: &[u8; 32],
        address: &[u8; 32],
        _prefix: &[u8],
        _product_id: &[u8; 32],
        _bump: u8,
    ) -> bool {
        [key(20), key(21), key(22), key(23)].contains(address)
    }
}

fn descriptor() -> StructuredClaimDescriptorV2 {
    let mut primitive = [0_u64; 16];
    primitive[0] = 1;
    primitive[1] = 2;
    StructuredClaimDescriptorV2 {
        tag: DESCRIPTOR_ACCOUNT_TAG,
        version: 2,
        flags: 0,
        base_program: key(1),
        base_program_data: key(2),
        base_deployment_slot: 3,
        wrapper_program_data: key(4),
        wrapper_deployment_slot: 5,
        token_2022_program: key(6),
        token_2022_program_data: key(7),
        token_2022_deployment_slot: 8,
        market: key(9),
        terms_digest: key(10),
        structured_root_id: key(15),
        wrapper_recipe_id: key(16),
        primitive,
        state: DescriptorStateV1::Active,
        descriptor_bump: 11,
        mint_bump: 12,
        mint_authority_bump: 13,
        vault_owner_bump: 14,
    }
}

fn deployments() -> RuntimeDeploymentsV1 {
    let binding = DeploymentBinding {
        wrapper_program: key(19),
        wrapper_program_data: key(4),
        wrapper_deployment_slot: 5,
        base_program: key(1),
        base_program_data: key(2),
        base_deployment_slot: 3,
        token_2022_program: key(6),
        token_2022_program_data: key(7),
        token_2022_deployment_slot: 8,
    };
    RuntimeDeploymentsV1 {
        binding,
        upgradeable_loader: key(18),
        program_owners: [key(18); 3],
        program_data_owners: [key(18); 3],
        linked_program_data: [key(4), key(2), key(7)],
        executable_mask: 0b111,
    }
}

#[test]
fn canonical_descriptor_is_exactly_0x88_v2_and_old_adapter_tag_refuses() {
    let bytes = descriptor().encode().unwrap();
    assert_eq!(bytes.len(), DESCRIPTOR_ACCOUNT_BYTES);
    assert_eq!(&bytes[..2], &[0x88, 2]);
    assert_eq!(
        StructuredClaimDescriptorV2::decode(&bytes),
        Ok(descriptor())
    );

    let mut historical_parallel_tag = bytes;
    historical_parallel_tag[0] = 0xd1;
    assert!(StructuredClaimDescriptorV2::decode(&historical_parallel_tag).is_err());
}

#[test]
fn descriptor_v1_is_archivally_decodable_but_cannot_promote_live() {
    let v2 = descriptor().encode().unwrap();
    let mut v1 = [0_u8; 384];
    v1[..252].copy_from_slice(&v2[..252]);
    v1[252..381].copy_from_slice(&v2[316..445]);
    v1[1] = 1;
    v1[381] = v2[445];
    v1[382] = v2[446];
    v1[383] = v2[448];
    assert_eq!(decode_historical_descriptor_v1(&v1), Ok(()));
    assert!(StructuredClaimDescriptorV2::decode(&v1).is_err());
}

#[test]
fn descriptor_binding_hashes_only_the_runtime_owned_preimages() {
    let descriptor = descriptor();
    let basis = DescriptorBasisV1 {
        market: descriptor.market,
        terms_digest: descriptor.terms_digest,
        basis_degree: 1,
        denominator: 8,
        outcome_count: 2,
    };
    let deployments = deployments();
    let identity =
        clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
            &descriptor,
            basis,
            deployments.binding,
        )
        .unwrap();
    let native = canonical_native_claim_id_v1(&identity).unwrap();
    let product = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .unwrap();
    let addresses = StructuredClaimRuntimeAddressesV1 {
        descriptor: key(20),
        mint: key(21),
        mint_authority: key(22),
        vault_owner: key(23),
    };
    let bound = bind_descriptor_v1(
        descriptor,
        basis,
        deployments,
        native,
        product,
        addresses,
        &AcceptKnownPdas,
    )
    .unwrap();
    assert_eq!(bound.native_claim_id(), native);
    assert_eq!(bound.wrapper_product_id(), product);
    assert_eq!(bound.addresses(), addresses);
}
