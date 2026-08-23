use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::runtime_contract::{
    DescriptorBasisV1, DescriptorStateV1, StructuredClaimDescriptorV1, StructuredClaimPayloadV1,
    StructuredClaimRuntimeAddressesV1, WrapperQuantityPayloadV1, DESCRIPTOR_ACCOUNT_BYTES,
    DESCRIPTOR_ACCOUNT_TAG, STRUCTURED_CLAIM_FAMILY_TAG, STRUCTURED_CLAIM_FAMILY_VERSION,
    WRAPPER_QUANTITY_PAYLOAD_BYTES,
};
use clutch_structured_claim_adapter::{
    admit_runtime_envelope_v1, bind_descriptor_v1, canonical_native_claim_id_v1,
    canonical_wrapper_product_id_v1, decode_instruction_v1, dispatch_structured_claim_v1,
    AccountFrameV1, Error, PdaVerifierV1, PreparedStructuredClaimRouteV1, Result,
    RuntimeDeploymentsV1, StructuredClaimAccountLoaderV1, ENABLED_STRUCTURED_CLAIM_ACTION_MASK,
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

#[derive(Clone, Copy, Debug)]
struct MustNotLoad;

impl StructuredClaimAccountLoaderV1 for MustNotLoad {
    fn load_and_prepare(
        &self,
        _action: clutch_structured_claim_adapter::runtime_contract::StructuredClaimActionV1,
        _payload: StructuredClaimPayloadV1,
        _accounts: &AccountFrameV1<'_>,
    ) -> Result<PreparedStructuredClaimRouteV1> {
        panic!("disabled dispatch touched the account loader")
    }
}

fn descriptor() -> StructuredClaimDescriptorV1 {
    let mut primitive = [0_u64; 16];
    primitive[0] = 1;
    primitive[1] = 2;
    StructuredClaimDescriptorV1 {
        tag: DESCRIPTOR_ACCOUNT_TAG,
        version: 1,
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
        primitive,
        state: DescriptorStateV1::Active,
        descriptor_bump: 11,
        mint_bump: 12,
        vault_bump: 13,
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
fn canonical_descriptor_is_exactly_0x88_v1_and_old_adapter_tag_refuses() {
    let bytes = descriptor().encode().unwrap();
    assert_eq!(bytes.len(), DESCRIPTOR_ACCOUNT_BYTES);
    assert_eq!(&bytes[..2], &[0x88, 1]);
    assert_eq!(
        StructuredClaimDescriptorV1::decode(&bytes),
        Ok(descriptor())
    );

    let mut historical_parallel_tag = bytes;
    historical_parallel_tag[0] = 0xd1;
    assert!(StructuredClaimDescriptorV1::decode(&historical_parallel_tag).is_err());
}

#[test]
fn family_payload_uses_the_runtime_contract_and_runtime_gate_stays_empty() {
    let payload = WrapperQuantityPayloadV1 {
        wrapper_product_id: key(30),
        quantity: 2,
        user_generation: 3,
        user_replay_sequence: 4,
        vault_generation: 5,
        vault_replay_sequence: 6,
    };
    let body = payload.encode().unwrap();
    let mut instruction = [0_u8; 3 + WRAPPER_QUANTITY_PAYLOAD_BYTES];
    instruction[0] = STRUCTURED_CLAIM_FAMILY_TAG;
    instruction[1] = STRUCTURED_CLAIM_FAMILY_VERSION;
    instruction[2] = 2;
    instruction[3..].copy_from_slice(&body);
    assert_eq!(
        decode_instruction_v1(&instruction),
        Ok(StructuredClaimPayloadV1::WrapCanonical(payload))
    );
    assert_eq!(ENABLED_STRUCTURED_CLAIM_ACTION_MASK, 0);
    assert_eq!(
        admit_runtime_envelope_v1(&instruction),
        Err(Error::CapabilityDisabled)
    );
    assert_eq!(
        dispatch_structured_claim_v1(
            &instruction,
            &AccountFrameV1 { accounts: &[] },
            &MustNotLoad,
        ),
        Err(Error::CapabilityDisabled)
    );
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
    let product = canonical_wrapper_product_id_v1(&identity, native).unwrap();
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
