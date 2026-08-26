//! Descriptor-selected immutable Token behavior admission.
//!
//! The rational descriptor remains the semantic owner of release-set and Token
//! program identity. The Market remains the semantic owner of Realm identity.
//! This join authenticates one finalized
//! [`dclutch_token_svm::TokenBehaviorSelectionV2`] record against those owners;
//! Bearer and Structured specializations do not acquire a parallel extension
//! policy.

use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_token_svm::{
    Error as TokenError, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};

/// Finalized-record evidence for one descriptor-selected Token behavior
/// selection.
///
/// SHA-256 and Record-program account authentication stay in the adapter. This
/// contract requires the selected descriptor coordinate, finalized record
/// coordinate, and adapter-recomputed digest to be identical before decoding
/// any selection bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenBehaviorRecordAdmissionV2 {
    /// Config schema selected by the immutable capability program descriptor.
    pub selected_schema_id: [u8; 32],
    /// Schema identity authenticated from the finalized config record.
    pub finalized_schema_id: [u8; 32],
    /// Config content digest selected by the immutable capability manifest.
    pub selected_content_digest: [u8; 32],
    /// Content digest authenticated from the finalized config record.
    pub finalized_content_digest: [u8; 32],
    /// SHA-256 of the exact 144 config bytes recomputed by the adapter.
    pub recomputed_content_digest: [u8; 32],
    /// Finalized Record program owner, raw PDA, vacant staging PDA and rent
    /// state were authenticated.
    pub record_authenticated: bool,
    /// The supplied Realm identity came from the authenticated immutable
    /// Market identity rather than an instruction or caller hint.
    pub market_realm_authenticated: bool,
}

/// Refusal from the immutable Token behavior admission join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenBehaviorAdmissionErrorV2 {
    /// The Market Realm was not authenticated or was the reserved zero value.
    RealmAuthentication,
    /// The descriptor-selected and finalized config schemas did not select the
    /// sole Token behavior selection schema.
    SchemaMismatch,
    /// Finalized record authentication was absent or one content digest did
    /// not match exactly.
    ContentDigestMismatch,
    /// The exact selection bytes failed hostile decoding or Realm/release
    /// binding.
    Selection(TokenError),
    /// The Rational descriptor selected another Token program.
    TokenProgramMismatch,
}

/// Result alias for Token behavior admission.
pub type TokenBehaviorAdmissionResultV2<T> = core::result::Result<T, TokenBehaviorAdmissionErrorV2>;

/// Authenticated descriptor-selected Token behavior facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTokenBehaviorV2 {
    descriptor_id: [u8; 32],
    market_realm: [u8; 32],
    content_digest: [u8; 32],
    selection: TokenBehaviorSelectionV2,
}

impl AuthenticatedTokenBehaviorV2 {
    /// Finalized Rational descriptor selecting the release and Token program.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.descriptor_id
    }

    /// Realm authenticated from the immutable Market.
    pub const fn market_realm(self) -> [u8; 32] {
        self.market_realm
    }

    /// Exact digest of the finalized 144-byte selection record.
    pub const fn content_digest(self) -> [u8; 32] {
        self.content_digest
    }

    /// Hostile-decoded immutable selection.
    pub const fn selection(self) -> TokenBehaviorSelectionV2 {
        self.selection
    }
}

/// Join one exact Token behavior config record to its Rational descriptor and
/// authenticated Market Realm.
pub fn authenticate_token_behavior_v2(
    descriptor: RepresentationDescriptorV2<'_>,
    market_realm: [u8; 32],
    selection_bytes: &[u8],
    admission: TokenBehaviorRecordAdmissionV2,
) -> TokenBehaviorAdmissionResultV2<AuthenticatedTokenBehaviorV2> {
    if !admission.market_realm_authenticated || market_realm == [0; 32] {
        return Err(TokenBehaviorAdmissionErrorV2::RealmAuthentication);
    }
    if admission.selected_schema_id != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        || admission.finalized_schema_id != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
    {
        return Err(TokenBehaviorAdmissionErrorV2::SchemaMismatch);
    }
    if !admission.record_authenticated
        || admission.selected_content_digest == [0; 32]
        || admission.selected_content_digest != admission.finalized_content_digest
        || admission.selected_content_digest != admission.recomputed_content_digest
    {
        return Err(TokenBehaviorAdmissionErrorV2::ContentDigestMismatch);
    }
    let selection = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        selection_bytes,
        market_realm,
        descriptor.release_set_id(),
    )
    .map_err(TokenBehaviorAdmissionErrorV2::Selection)?;
    if selection.token_program() != descriptor.token_program() {
        return Err(TokenBehaviorAdmissionErrorV2::TokenProgramMismatch);
    }
    Ok(AuthenticatedTokenBehaviorV2 {
        descriptor_id: descriptor.descriptor_id(),
        market_realm,
        content_digest: admission.selected_content_digest,
        selection,
    })
}

#[cfg(test)]
mod tests {
    use dclutch_rational_representation_v2_kernel::{
        DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_MAGIC_V3,
        DescriptorAdmissionV2,
    };
    use dclutch_token_svm::{
        TOKEN_2022_BEHAVIOR_PROFILE_ID_V2, TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_BYTES_V2,
    };

    use super::*;

    const DESCRIPTOR_ID: [u8; 32] = [1; 32];
    const MARKET_REALM: [u8; 32] = [2; 32];
    const RELEASE_SET: [u8; 32] = [3; 32];
    const CONFIG_DIGEST: [u8; 32] = [4; 32];

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        output
            .get_mut(offset..offset + value.len())
            .expect("fixture offset")
            .copy_from_slice(value);
    }

    fn descriptor_bytes(token_program: [u8; 32]) -> std::vec::Vec<u8> {
        let mut output = std::vec![0; DESCRIPTOR_HEADER_BYTES + 2 * DESCRIPTOR_COEFFICIENT_BYTES];
        put(&mut output, 0, &DESCRIPTOR_MAGIC_V3);
        put(&mut output, 8, &3_u16.to_le_bytes());
        put(&mut output, 16, &[5; 32]);
        put(&mut output, 48, &[6; 32]);
        put(&mut output, 80, &[7; 32]);
        put(&mut output, 112, &[8; 32]);
        put(&mut output, 144, &RELEASE_SET);
        put(&mut output, 176, &[9; 32]);
        put(&mut output, 208, &token_program);
        put(&mut output, 240, &2_u32.to_le_bytes());
        put(&mut output, 248, &10_u64.to_le_bytes());
        put(&mut output, DESCRIPTOR_HEADER_BYTES, &10_u64.to_le_bytes());
        output
    }

    fn descriptor(token_program: [u8; 32]) -> RepresentationDescriptorV2<'static> {
        let bytes = std::boxed::Box::leak(descriptor_bytes(token_program).into_boxed_slice());
        RepresentationDescriptorV2::decode(
            bytes,
            DescriptorAdmissionV2 {
                selected_descriptor_id: DESCRIPTOR_ID,
                finalized_descriptor_id: DESCRIPTOR_ID,
                recomputed_descriptor_digest: DESCRIPTOR_ID,
                finalized_descriptor_digest: DESCRIPTOR_ID,
                record_authenticated: true,
                derived_representation_authority: [10; 32],
                authority_derivation_authenticated: true,
            },
        )
        .expect("descriptor")
    }

    fn selection_bytes() -> [u8; TOKEN_BEHAVIOR_SELECTION_BYTES_V2] {
        TokenBehaviorSelectionV2::new(MARKET_REALM, RELEASE_SET)
            .expect("selection")
            .to_bytes()
    }

    const fn admission() -> TokenBehaviorRecordAdmissionV2 {
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: CONFIG_DIGEST,
            finalized_content_digest: CONFIG_DIGEST,
            recomputed_content_digest: CONFIG_DIGEST,
            record_authenticated: true,
            market_realm_authenticated: true,
        }
    }

    #[test]
    fn exact_descriptor_market_and_finalized_config_join() {
        let authenticated = authenticate_token_behavior_v2(
            descriptor(TOKEN_2022_PROGRAM_ID),
            MARKET_REALM,
            &selection_bytes(),
            admission(),
        )
        .expect("Token behavior");
        assert_eq!(authenticated.descriptor_id(), DESCRIPTOR_ID);
        assert_eq!(authenticated.market_realm(), MARKET_REALM);
        assert_eq!(authenticated.content_digest(), CONFIG_DIGEST);
        assert_eq!(authenticated.selection().realm(), MARKET_REALM);
        assert_eq!(authenticated.selection().release_set(), RELEASE_SET);
        assert_eq!(
            authenticated.selection().profile_id(),
            TOKEN_2022_BEHAVIOR_PROFILE_ID_V2
        );
    }

    #[test]
    fn config_schema_digest_and_finality_substitutions_refuse() {
        for hostile in [
            TokenBehaviorRecordAdmissionV2 {
                selected_schema_id: [11; 32],
                ..admission()
            },
            TokenBehaviorRecordAdmissionV2 {
                finalized_schema_id: [11; 32],
                ..admission()
            },
        ] {
            assert_eq!(
                authenticate_token_behavior_v2(
                    descriptor(TOKEN_2022_PROGRAM_ID),
                    MARKET_REALM,
                    &selection_bytes(),
                    hostile,
                ),
                Err(TokenBehaviorAdmissionErrorV2::SchemaMismatch)
            );
        }
        for hostile in [
            TokenBehaviorRecordAdmissionV2 {
                selected_content_digest: [11; 32],
                ..admission()
            },
            TokenBehaviorRecordAdmissionV2 {
                finalized_content_digest: [11; 32],
                ..admission()
            },
            TokenBehaviorRecordAdmissionV2 {
                recomputed_content_digest: [11; 32],
                ..admission()
            },
            TokenBehaviorRecordAdmissionV2 {
                record_authenticated: false,
                ..admission()
            },
        ] {
            assert_eq!(
                authenticate_token_behavior_v2(
                    descriptor(TOKEN_2022_PROGRAM_ID),
                    MARKET_REALM,
                    &selection_bytes(),
                    hostile,
                ),
                Err(TokenBehaviorAdmissionErrorV2::ContentDigestMismatch)
            );
        }
    }

    #[test]
    fn realm_release_profile_program_and_data_substitutions_refuse() {
        let canonical = selection_bytes();
        for offset in [16_usize, 48, 80, 112, TOKEN_BEHAVIOR_SELECTION_BYTES_V2 - 1] {
            let mut hostile = canonical;
            *hostile.get_mut(offset).expect("selection byte") ^= 0xff;
            assert!(
                authenticate_token_behavior_v2(
                    descriptor(TOKEN_2022_PROGRAM_ID),
                    MARKET_REALM,
                    &hostile,
                    admission(),
                )
                .is_err()
            );
        }
        assert_eq!(
            authenticate_token_behavior_v2(
                descriptor(TOKEN_2022_PROGRAM_ID),
                [11; 32],
                &canonical,
                admission(),
            ),
            Err(TokenBehaviorAdmissionErrorV2::Selection(
                TokenError::InvalidAdapterRelease
            ))
        );
        assert_eq!(
            authenticate_token_behavior_v2(
                descriptor([12; 32]),
                MARKET_REALM,
                &canonical,
                admission(),
            ),
            Err(TokenBehaviorAdmissionErrorV2::TokenProgramMismatch)
        );
        assert_eq!(
            authenticate_token_behavior_v2(
                descriptor(TOKEN_2022_PROGRAM_ID),
                MARKET_REALM,
                canonical.get(..canonical.len() - 1).expect("prefix"),
                admission(),
            ),
            Err(TokenBehaviorAdmissionErrorV2::Selection(
                TokenError::InvalidLength
            ))
        );
    }
}
