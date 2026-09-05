//! Exact logical AccountProfile for terminal Bearer redemption.

use dclutch_vm::account_profile::v2::{
    AccountPrestateV2, DYNAMIC_FIXED_SPAN_HEADER_BYTES, OPERATION_BYTES as ACCOUNT_OPERATION_BYTES,
    RULE_BYTES as ACCOUNT_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
    TrustedIdentityEnvironmentV2,
    encode::{
        AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
        AccountOperationInputV2, AccountPrivilegesV2, AccountRuleInputV2,
        AccountRuleWithPrestateInputV2, RegisterGeometryV2, ScalarCoordinateV2,
        encode_account_profile_with_dynamic_fixed_span_v2_atomic,
    },
};
use dclutch_product::payoff::runtime_v3::{
    BASIS_HEADER_BYTES_V3, BASIS_WIDTH_OFFSET_V3, ProductBasisV3,
};
use dclutch_claims::rational::{
    RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3, RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3,
    RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3, REPRESENTATION_FRAME_SPEC_V2,
    RepresentationCoordinateV2,
};
use dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_BYTES_V2;

use crate::bearer::{
    Error, RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3,
    RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3, Result,
};

const PROFILE_OPERATION_COUNT: usize = 1;
const LOGICAL_ACCOUNT_COUNT: usize = RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize;
const INJECTED_ACCOUNT_COUNT: usize = RATIONAL_TERMINAL_HOT_INJECTED_ACCOUNT_COUNT_V3 as usize;

/// Active asset rows in the terminal Claims child this profile projects.
///
/// One, and it is a property of the ACTION rather than of this release: a
/// terminal redemption selects a single outcome, so `asset_count == 1` for
/// every request `construct_redeem_terminal` builds. It is named here because
/// it is the parameter the frame spec needs to place the terminal suffix, and a
/// bare `1` at that call site is the kind of number that later reads as an
/// index.
pub(crate) const TERMINAL_ASSET_ROWS_V3: usize = 1;

/// Exact encoded AccountProfile width for terminal Bearer redemption.
pub const RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3: usize = DYNAMIC_FIXED_SPAN_HEADER_BYTES
    + LOGICAL_ACCOUNT_COUNT * ACCOUNT_RULE_BYTES
    + PROFILE_OPERATION_COUNT * ACCOUNT_OPERATION_BYTES;

/// Exact observed data lengths and authenticated Product basis for one profile.
///
/// ProgramData and mutable state widths are deployment facts, so they are
/// supplied from one finalized chain snapshot rather than compiled into the
/// semantic operator. The profile itself pins every supplied width exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTerminalAccountProfileInputV3<'a> {
    /// Exact logical account data lengths in AccountProfile order.
    pub logical_data_lengths: &'a [u32],
    /// Exact finalized ProductBasisV3 bytes at the injected basis coordinate.
    pub product_basis: &'a [u8],
}

/// Encode the exact logical AccountProfile for one finalized deployment.
pub fn encode_rational_terminal_account_profile_v3(
    input: RationalTerminalAccountProfileInputV3<'_>,
) -> Result<[u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3]> {
    if input.logical_data_lengths.len() != LOGICAL_ACCOUNT_COUNT {
        return Err(Error::AccountProfileInput);
    }
    let basis = ProductBasisV3::decode(input.product_basis).map_err(Error::ProductBasis)?;
    let basis_coordinate = logical_index(LogicalCoordinateV3::Injected(
        InjectedCoordinateV3::LinkedProductBasis,
    ))
    .ok_or(Error::AccountProfileInput)?;
    if basis.basis_width() == 0
        || input
            .logical_data_lengths
            .get(basis_coordinate)
            .copied()
            .and_then(|width| usize::try_from(width).ok())
            != Some(input.product_basis.len())
    {
        return Err(Error::AccountProfileInput);
    }
    let mut rules = Vec::with_capacity(LOGICAL_ACCOUNT_COUNT);
    for index in 0..LOGICAL_ACCOUNT_COUNT {
        rules.push(rule(index, input.logical_data_lengths)?);
    }
    let operations = [AccountOperationInputV2::ProjectTailCountU32 {
        account: AccountCoordinateV2::fixed(
            u16::try_from(basis_coordinate).map_err(|_| Error::AccountProfileInput)?,
        ),
        destination: ScalarCoordinateV2::common(
            u16::try_from(RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3)
                .map_err(|_| Error::AccountProfileInput)?,
        ),
        data_offset: u32::try_from(BASIS_WIDTH_OFFSET_V3)
            .map_err(|_| Error::AccountProfileInput)?,
    }];
    let geometry = RegisterGeometryV2 {
        common_scalars: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3)
            .map_err(|_| Error::AccountProfileInput)?,
        item_scalar_stride: 0,
        common_identities: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3)
            .map_err(|_| Error::AccountProfileInput)?,
        item_identity_stride: 0,
    };
    let mut scratch = [0_u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3];
    let mut output = [0_u8; RATIONAL_TERMINAL_ACCOUNT_PROFILE_BYTES_V3];
    encode_account_profile_with_dynamic_fixed_span_v2_atomic(
        TrustedEnvironmentV2::None,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        &[],
        &rules,
        &[],
        &operations,
        geometry,
        &mut scratch,
        &mut output,
    )
    .map_err(Error::AccountProfileArtifact)?;
    Ok(output)
}

/// The five accounts the common Hot outer injects before the Claims child.
///
/// They are the outer's evidence, not the child's frame, which is why the frame
/// spec knows nothing about them: `build_hot` places the child at logical
/// coordinate five and every child index is its frame index plus this prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InjectedCoordinateV3 {
    /// Structured capability root.
    Root,
    /// Authenticated Token behavior selection.
    TokenBehaviorSelection,
    /// Finalized Product record, reused by the child frame.
    ProductRecord,
    /// Finalized portfolio record, reused by the child frame.
    PortfolioRecord,
    /// Finalized linked Product basis, reused by the child frame.
    LinkedProductBasis,
}

const INJECTED_COORDINATES_V3: [InjectedCoordinateV3; INJECTED_ACCOUNT_COUNT] = [
    InjectedCoordinateV3::Root,
    InjectedCoordinateV3::TokenBehaviorSelection,
    InjectedCoordinateV3::ProductRecord,
    InjectedCoordinateV3::PortfolioRecord,
    InjectedCoordinateV3::LinkedProductBasis,
];

/// One logical coordinate of the terminal profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LogicalCoordinateV3 {
    /// One of the five accounts the common Hot outer injects.
    Injected(InjectedCoordinateV3),
    /// One coordinate of the canonical Claims child frame.
    Child(RepresentationCoordinateV2),
}

/// The role the profile's logical coordinate `index` carries.
///
/// THE PROFILE'S ONLY SOURCE OF POSITIONS. Everything below matches on the
/// role; nothing matches on the number. Until 2026-09-02 six per-index tables
/// here spelled positions by hand against a 49-account child while the frame
/// spec that owns the frame said 50, and the missing account -- the Resolution
/// program, at the fifth slot of the fourteen-account terminal suffix -- put
/// every later table one index low with nothing to compare them against.
pub(crate) fn logical_coordinate(index: usize) -> Option<LogicalCoordinateV3> {
    if let Some(injected) = INJECTED_COORDINATES_V3.get(index) {
        return Some(LogicalCoordinateV3::Injected(*injected));
    }
    let child = index.checked_sub(INJECTED_ACCOUNT_COUNT)?;
    REPRESENTATION_FRAME_SPEC_V2
        .coordinate(child, TERMINAL_ASSET_ROWS_V3, true)
        .map(LogicalCoordinateV3::Child)
}

/// Where a role sits, which is the inverse of [`logical_coordinate`] and the
/// only way an alias target or a projected coordinate is named in this module.
pub(crate) fn logical_index(coordinate: LogicalCoordinateV3) -> Option<usize> {
    (0..LOGICAL_ACCOUNT_COUNT).find(|index| logical_coordinate(*index) == Some(coordinate))
}

/// Where one coordinate's pinned data width comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeclaredDataV3 {
    /// Exact width observed in the finalized deployment snapshot.
    Observed,
    /// Exact width the artifact itself compiles in.
    Compiled(usize),
    /// Authenticated variable-width Product basis: header pinned, tail projected.
    ProductBasis,
    /// Readonly evidence whose width is a cluster fact, so no width is pinned.
    Opaque,
}

/// Exact declared rule for one logical coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeclaredRuleV3 {
    pub(crate) signer: bool,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
    /// Coordinate this one is a route alias of; `None` when it represents itself.
    pub(crate) alias: Option<LogicalCoordinateV3>,
    data: DeclaredDataV3,
}

const fn readonly(data: DeclaredDataV3) -> DeclaredRuleV3 {
    DeclaredRuleV3 {
        signer: false,
        writable: false,
        executable: false,
        alias: None,
        data,
    }
}

const fn writable(data: DeclaredDataV3) -> DeclaredRuleV3 {
    DeclaredRuleV3 {
        writable: true,
        ..readonly(data)
    }
}

const fn signer() -> DeclaredRuleV3 {
    DeclaredRuleV3 {
        signer: true,
        ..readonly(DeclaredDataV3::Observed)
    }
}

/// A deployed program: executable, and its data is its ELF, a cluster fact.
const fn program() -> DeclaredRuleV3 {
    DeclaredRuleV3 {
        executable: true,
        ..readonly(DeclaredDataV3::Opaque)
    }
}

const fn opaque() -> DeclaredRuleV3 {
    readonly(DeclaredDataV3::Opaque)
}

/// A route alias carries NO privileges of its own: `authenticate` takes
/// `representative_privileges` for any coordinate whose representative is
/// another (`v2.rs:2360-2369`), and `cc228cdd` made a nonzero privilege on an
/// alias a refusal because it is dead weight that reads as authority. The
/// aliased coordinate is the executable one.
const fn alias_of(target: LogicalCoordinateV3) -> DeclaredRuleV3 {
    DeclaredRuleV3 {
        alias: Some(target),
        ..readonly(DeclaredDataV3::Opaque)
    }
}

/// THE PROFILE'S ONE TABLE.
///
/// Exhaustive on both coordinate families on purpose: a coordinate added to the
/// canonical frame is a compile error here rather than a rule the encoder emits
/// from whatever a hand-typed index range happened to cover.
pub(crate) fn declared(coordinate: LogicalCoordinateV3) -> DeclaredRuleV3 {
    use DeclaredDataV3::{Compiled, Observed, Opaque, ProductBasis};
    use InjectedCoordinateV3 as Hot;
    use LogicalCoordinateV3::{Child, Injected};
    use RepresentationCoordinateV2 as Coordinate;
    match coordinate {
        // The common Hot outer's own evidence.
        Injected(Hot::Root) => writable(Observed),
        Injected(Hot::TokenBehaviorSelection) => {
            readonly(Compiled(TOKEN_BEHAVIOR_SELECTION_BYTES_V2))
        }
        Injected(Hot::ProductRecord | Hot::PortfolioRecord) => readonly(Observed),
        Injected(Hot::LinkedProductBasis) => readonly(ProductBasis),

        // The Claims child's fixed base prefix.
        Child(Coordinate::CallerAuthority) => readonly(Observed),
        Child(Coordinate::CallerProgram) => program(),
        Child(Coordinate::CallerProgramdata) => opaque(),
        Child(Coordinate::Actor) => signer(),
        Child(
            Coordinate::RepresentationAuthority
            | Coordinate::DescriptorRaw
            | Coordinate::DescriptorStaging
            | Coordinate::GraphRaw
            | Coordinate::GraphStaging
            | Coordinate::RentSysvar,
        ) => readonly(Observed),
        // 15 is the System program: the sole `executable` coordinate this frame
        // did not also declare opaque before the Resolution program joined it,
        // and a builtin's data is its name, which is a cluster fact. See
        // `open_structured_v3.rs` for the measurement.
        Child(Coordinate::SystemProgram) => program(),
        Child(Coordinate::Replay | Coordinate::ClaimsAggregate) => writable(Observed),
        Child(Coordinate::ActivationCache) => readonly(Observed),
        Child(Coordinate::ClaimsProgram) => program(),
        Child(Coordinate::ClaimsProgramdata) => opaque(),
        Child(Coordinate::RegistryProgram) => program(),
        Child(Coordinate::CoreMarket) => readonly(Observed),
        Child(Coordinate::CoreProgram) => program(),
        Child(Coordinate::CoreProgramdata | Coordinate::ReceiptMint) => opaque(),
        // Claims program placeholders for the receipt and Position accounts a
        // terminal redemption does not carry.
        Child(Coordinate::ActorReceiptAccount | Coordinate::ActorClaimsPosition) => {
            alias_of(Child(Coordinate::ClaimsProgram))
        }
        Child(Coordinate::TokenProgram) => program(),
        // The child frame reuses the Hot-injected Product evidence.
        Child(Coordinate::LinkedBasisRecord) => alias_of(Injected(Hot::LinkedProductBasis)),
        Child(Coordinate::LinkedBasisStaging) => readonly(Observed),
        Child(Coordinate::ProductRecord) => alias_of(Injected(Hot::ProductRecord)),
        Child(
            Coordinate::ProductStaging
            | Coordinate::ResultDomainRecord
            | Coordinate::ResultDomainStaging,
        ) => readonly(Observed),
        Child(Coordinate::PortfolioRecord) => alias_of(Injected(Hot::PortfolioRecord)),
        Child(Coordinate::PortfolioStaging) => readonly(Observed),

        // The one active asset row.
        Child(Coordinate::AssetPosition) => writable(Observed),
        Child(Coordinate::AssetShardMint | Coordinate::AssetActorToken) => writable(Opaque),
        Child(Coordinate::AssetStructuredToken) => opaque(),

        // The terminal suffix.
        Child(Coordinate::TerminalCallerAuthority) => readonly(Observed),
        Child(Coordinate::TerminalCustodyProgram) => program(),
        Child(Coordinate::TerminalCustodyProgramdata | Coordinate::TerminalCertificate) => opaque(),
        Child(Coordinate::TerminalResolutionProgram) => program(),
        Child(
            Coordinate::TerminalResolutionProgramdata
            | Coordinate::TerminalRealm
            | Coordinate::TerminalRealmStaging,
        ) => opaque(),
        Child(Coordinate::TerminalCustodyReplay) => writable(Opaque),
        Child(Coordinate::TerminalCollateralMint) => opaque(),
        Child(Coordinate::TerminalHoard | Coordinate::TerminalRecipient) => writable(Opaque),
        Child(Coordinate::TerminalCustodyAuthority) => opaque(),
        // The terminal suffix reuses the already selected Token program.
        Child(Coordinate::TerminalTokenProgram) => alias_of(Child(Coordinate::TokenProgram)),
    }
}

fn rule(index: usize, lengths: &[u32]) -> Result<AccountRuleWithPrestateInputV2> {
    let coordinate = logical_coordinate(index).ok_or(Error::AccountProfileInput)?;
    let declared = declared(coordinate);
    let alias = match declared.alias {
        None => AccountAliasInputV2::SelfCoordinate,
        Some(target) => AccountAliasInputV2::Fixed(
            u16::try_from(logical_index(target).ok_or(Error::AccountProfileInput)?)
                .map_err(|_| Error::AccountProfileInput)?,
        ),
    };
    let prestate = match declared.data {
        DeclaredDataV3::ProductBasis => AccountPrestateV2::AdapterAuthenticatedVariableData,
        _ if declared.alias.is_some() => AccountPrestateV2::AuthenticatedRouteAlias,
        DeclaredDataV3::Opaque => AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        DeclaredDataV3::Observed | DeclaredDataV3::Compiled(_) => AccountPrestateV2::Exact,
    };
    let data_length = match declared.data {
        DeclaredDataV3::ProductBasis => {
            u32::try_from(BASIS_HEADER_BYTES_V3).map_err(|_| Error::AccountProfileInput)?
        }
        DeclaredDataV3::Compiled(width) => {
            u32::try_from(width).map_err(|_| Error::AccountProfileInput)?
        }
        DeclaredDataV3::Opaque => 0,
        DeclaredDataV3::Observed => *lengths.get(index).ok_or(Error::AccountProfileInput)?,
    };
    Ok(AccountRuleWithPrestateInputV2 {
        rule: AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(
                declared.signer,
                declared.writable,
                declared.executable,
            ),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias,
            data_length,
            data_item_stride: 0,
        },
        prestate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_vm::account_profile::{
        AccountObservationV1,
        v2::{AccountProfileV2, ProjectionRegistersV2, project_tail_count_atomic},
    };
    use dclutch_product::payoff::runtime_v3::{
        BASIS_HEADER_BYTES_V3, BasisInputV3, BasisKindV3, Error as ProductBasisErrorV3,
        compile_basis_v3,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn basis(width: u32) -> [u8; BASIS_HEADER_BYTES_V3] {
        let mut output = [0_u8; BASIS_HEADER_BYTES_V3];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: id(1),
                result_domain_id: id(2),
                coordinate_domain_id: id(3),
                result_unit_id: id(4),
                evaluator_release_id: id(5),
                basis_width: width,
                payout_scale: 1,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                // Exempt by proof: degree 0 and 1 need no price gate,
                // and a digest offered alongside one is refused.
                price_gate_certificate_digest: [0_u8; 32],
            },
            &mut output,
        )
        .expect("basis");
        output
    }

    /// Every logical coordinate's declared rule, read back by ROLE.
    fn declaration(index: usize) -> DeclaredRuleV3 {
        declared(logical_coordinate(index).expect("logical coordinate"))
    }

    fn coordinate(target: LogicalCoordinateV3) -> usize {
        logical_index(target).expect("logical index")
    }

    /// THE FRAME'S WIDTH AND ORDER COME FROM THE CONTRACT THAT SPECIFIES THEM.
    ///
    /// The profile is the five injected Hot accounts plus a complete
    /// single-asset terminal Claims child, and the terminal suffix's roles are
    /// the fourteen the request contract declares -- Resolution program
    /// included, which is the coordinate this profile was missing until
    /// 2026-09-02.
    #[test]
    fn logical_frame_is_the_injected_prefix_plus_the_specified_child() {
        assert_eq!(
            LOGICAL_ACCOUNT_COUNT,
            INJECTED_ACCOUNT_COUNT
                + REPRESENTATION_FRAME_SPEC_V2
                    .shape_account_count(TERMINAL_ASSET_ROWS_V3, true)
                    .expect("terminal frame width"),
        );
        assert_eq!(
            logical_coordinate(0),
            Some(LogicalCoordinateV3::Injected(InjectedCoordinateV3::Root))
        );
        assert_eq!(
            logical_coordinate(INJECTED_ACCOUNT_COUNT),
            Some(LogicalCoordinateV3::Child(
                RepresentationCoordinateV2::CallerAuthority
            ))
        );
        assert_eq!(
            logical_coordinate(LOGICAL_ACCOUNT_COUNT - 1),
            Some(LogicalCoordinateV3::Child(
                RepresentationCoordinateV2::TerminalTokenProgram
            ))
        );
        assert_eq!(logical_coordinate(LOGICAL_ACCOUNT_COUNT), None);
        for index in 0..LOGICAL_ACCOUNT_COUNT {
            let role = logical_coordinate(index).expect("logical coordinate");
            assert_eq!(logical_index(role), Some(index), "role at {index}");
        }
        // The Resolution program is executable, and it is the coordinate whose
        // absence put every later terminal rule one index low.
        let resolution = coordinate(LogicalCoordinateV3::Child(
            RepresentationCoordinateV2::TerminalResolutionProgram,
        ));
        assert!(declaration(resolution).executable);
        assert_eq!(
            resolution + 4,
            coordinate(LogicalCoordinateV3::Child(
                RepresentationCoordinateV2::TerminalCustodyReplay
            )),
        );
        assert!(
            declaration(coordinate(LogicalCoordinateV3::Child(
                RepresentationCoordinateV2::TerminalCustodyReplay
            )))
            .writable
        );
    }

    #[test]
    fn profile_projects_product_basis_width_and_exact_aliases() {
        let basis = basis(258);
        let mut lengths = [0_u32; LOGICAL_ACCOUNT_COUNT];
        *lengths
            .get_mut(coordinate(LogicalCoordinateV3::Injected(
                InjectedCoordinateV3::TokenBehaviorSelection,
            )))
            .expect("Token selection coordinate") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
        let basis_length = u32::try_from(basis.len()).expect("basis width");
        let basis_coordinate = coordinate(LogicalCoordinateV3::Injected(
            InjectedCoordinateV3::LinkedProductBasis,
        ));
        let basis_child = coordinate(LogicalCoordinateV3::Child(
            RepresentationCoordinateV2::LinkedBasisRecord,
        ));
        *lengths
            .get_mut(basis_coordinate)
            .expect("basis logical coordinate") = basis_length;
        *lengths
            .get_mut(basis_child)
            .expect("basis child coordinate") = basis_length;
        let bytes =
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &basis,
            })
            .expect("profile");
        let profile = AccountProfileV2::decode(&bytes).expect("decode profile");
        assert_eq!(
            profile.fixed_account_count(),
            crate::bearer::RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3,
        );
        assert_eq!(profile.item_account_stride(), 0);

        let mut data = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| {
                vec![
                    0_u8;
                    usize::try_from(*lengths.get(index).expect("logical length")).expect("length")
                ]
            })
            .collect::<Vec<_>>();
        data.get_mut(basis_coordinate)
            .expect("basis data")
            .copy_from_slice(&basis);
        data.get_mut(basis_child)
            .expect("basis child data")
            .copy_from_slice(&basis);
        let mut keys = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| id(u8::try_from(index + 40).expect("key")))
            .collect::<Vec<_>>();
        // Every alias in the declaration, resolved by role. A hand-typed pair
        // list here was one of the six index tables that let the frame drift.
        for target in 0..LOGICAL_ACCOUNT_COUNT {
            let Some(alias) = declaration(target).alias else {
                continue;
            };
            let source_key = *keys.get(coordinate(alias)).expect("alias source");
            *keys.get_mut(target).expect("alias target") = source_key;
        }
        let common_owner = id(200);
        let observations = (0..LOGICAL_ACCOUNT_COUNT)
            .map(|index| {
                let rule = declaration(index);
                // An observation's privileges are the REPRESENTATIVE's: an
                // alias declares none of its own and is observed carrying the
                // ones its source carries.
                let representative = declaration(rule.alias.map_or(index, coordinate));
                let arguments = (
                    keys.get(index).expect("logical key"),
                    &common_owner,
                    0,
                    data.get(index).expect("logical data").as_slice(),
                    representative.signer,
                    representative.writable,
                    representative.executable,
                );
                if index == basis_coordinate {
                    AccountObservationV1::new_adapter_authenticated_variable_data(
                        arguments.0,
                        arguments.1,
                        arguments.2,
                        arguments.3,
                        arguments.4,
                        arguments.5,
                        arguments.6,
                    )
                } else {
                    AccountObservationV1::new(
                        arguments.0,
                        arguments.1,
                        arguments.2,
                        arguments.3,
                        arguments.4,
                        arguments.5,
                        arguments.6,
                    )
                }
            })
            .collect::<Vec<_>>();
        let input_scalars = [0_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        let input_identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        let mut scratch_scalars = input_scalars;
        let mut scratch_identities = input_identities;
        let mut output_scalars = input_scalars;
        let mut output_identities = input_identities;
        let projected = project_tail_count_atomic(
            profile,
            &observations,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("tail count");
        assert_eq!(projected, 258);
    }

    #[test]
    fn profile_refuses_nonbasis_bytes_and_unmatched_basis_length() {
        let basis = basis(258);
        let mut lengths = [0_u32; LOGICAL_ACCOUNT_COUNT];
        *lengths.get_mut(1).expect("Token selection coordinate") =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
        *lengths.get_mut(4).expect("basis logical coordinate") =
            u32::try_from(basis.len()).expect("basis length") - 1;
        assert_eq!(
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &basis,
            }),
            Err(Error::AccountProfileInput)
        );
        *lengths.get_mut(4).expect("basis logical coordinate") += 1;
        let mut hostile = basis;
        *hostile.get_mut(0).expect("basis magic") ^= 1;
        // The two halves of this test's name are two different accusations, and
        // until the basis decode carried its cause they were one code: a frame
        // whose declared width is wrong read exactly like an artifact that is
        // not a ProductBasis at all. The codec knew it was the magic.
        assert_eq!(
            encode_rational_terminal_account_profile_v3(RationalTerminalAccountProfileInputV3 {
                logical_data_lengths: &lengths,
                product_basis: &hostile,
            }),
            Err(Error::ProductBasis(ProductBasisErrorV3::InvalidMagic))
        );
    }
}
