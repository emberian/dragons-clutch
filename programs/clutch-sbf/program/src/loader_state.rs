//! Pinned Upgradeable Loader (loader-v3) account-state decode.
//!
//! ## Why this exists
//!
//! The R2 pull profile binds a source generation to the *deployed bytes* of a
//! receiver program rather than to a caller-supplied generation number: a
//! `SourceSpec` names the receiver program, its exact ProgramData address, and
//! the deployment slot recorded in that ProgramData account.  See
//! `docs/implementation/PYTH_PULL_PROFILE_R2.md` §"Frozen model contract" rule
//! 1 and `research/source-profile-v1/src/auth_v2.rs`, whose `LoaderStateV1`
//! this module reproduces byte-for-byte as [`LoaderStateV1`].
//!
//! Before this module the runtime had no Upgradeable Loader decoder at all; a
//! `DeploymentAuthenticatorV1` implementation was a compile-time trait with no
//! production instance.  This is the missing primitive, not its wiring.
//!
//! ## What this module is not
//!
//! **Nothing here is reachable from [`crate::dispatch`] or from any
//! instruction family.**  It is a capability module with tests.  Presenting the
//! receiver program and ProgramData accounts to an instruction is the account
//! plane change of `docs/implementation/R2_PULL_PROMOTION_PLAN.md` P0.5, and
//! projecting these refusals onto stable numeric codes is the open P0.8 error
//! granularity decision.  Until both close, [`LoaderStateError`] is a
//! module-local vocabulary in the style of [`crate::source::SourceError`], with
//! no entry in [`crate::error`].
//!
//! ## Primary source
//!
//! The loader states are a `bincode` (1.x, fixint little-endian, `u32` enum
//! variant tag, one-byte `Option` discriminant) serialization of
//! `UpgradeableLoaderState`, read from
//! `solana-loader-v3-interface 8.0.1`, `src/state.rs` (identical in 7.0.0
//! except for the `wincode` derive and its tests).  That file's own size
//! constants pin every offset used below:
//!
//! | state | tag | serialized metadata | crate constant |
//! | --- | --- | --- | --- |
//! | `Uninitialized` | 0 | 4 | `size_of_uninitialized() == 4` |
//! | `Buffer { authority: Option<Pubkey> }` | 1 | 37 | `size_of_buffer_metadata() == 37` |
//! | `Program { programdata_address: Pubkey }` | 2 | 36 | `size_of_program() == 36` |
//! | `ProgramData { slot: u64, authority: Option<Pubkey> }` | 3 | 45 | `size_of_programdata_metadata() == 45` |
//!
//! The exact byte images asserted by this module's tests were captured from
//! `bincode::serialize::<UpgradeableLoaderState>` at
//! `solana-loader-v3-interface 7.0.0` (the newest revision with a `.crate`
//! archive in this host's offline cache; `state.rs` is byte-identical to 8.0.1
//! apart from the `wincode` derive), so the fixtures are real serializer
//! output rather than a reading of the layout.
//!
//! ## The stale-authority hazard
//!
//! `ProgramData` with `upgrade_authority_address: None` serializes to
//! **thirteen** bytes, not forty-five: `03000000 0807060504030201 00`.  The
//! loader writes that image over the front of an account whose data region is
//! fixed at `size_of_programdata_metadata()` bytes before the ELF
//! (`solana-bpf-loader-program 4.2.1`, `src/lib.rs`, the
//! `size_of_programdata_metadata()` offsets at lines 258, 442, 602, 975, and
//! the `set_state(&UpgradeableLoaderState::ProgramData { .., None })` at the
//! `SetAuthority` arm).  Bytes `[13..45)` therefore still hold the *previous*
//! authority after an authority revocation.  [`decode_programdata_state`] must
//! not and does not read them when the discriminant byte is `0`.

/// Upgradeable Loader program id, `BPFLoaderUpgradeab1e11111111111111111111111`.
///
/// Cross-checked against `solana_sdk_ids::bpf_loader_upgradeable::ID` by
/// `pinned_loader_id_matches_the_sdk_declaration`.
pub const UPGRADEABLE_LOADER_ID: [u8; 32] = [
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
];

/// `bincode` variant tag of `UpgradeableLoaderState::Uninitialized`.
pub const LOADER_TAG_UNINITIALIZED: u32 = 0;
/// `bincode` variant tag of `UpgradeableLoaderState::Buffer`.
pub const LOADER_TAG_BUFFER: u32 = 1;
/// `bincode` variant tag of `UpgradeableLoaderState::Program`.
pub const LOADER_TAG_PROGRAM: u32 = 2;
/// `bincode` variant tag of `UpgradeableLoaderState::ProgramData`.
pub const LOADER_TAG_PROGRAMDATA: u32 = 3;

/// Width of the `bincode` `u32` enum variant tag that opens every state.
pub const LOADER_TAG_LEN: usize = 4;

/// `UpgradeableLoaderState::size_of_program()`: the `Program` metadata region.
///
/// The loader requires *at least* this much data in a program account rather
/// than exactly this much: `solana-bpf-loader-program 4.2.1`, `src/lib.rs`
/// line 220, refuses `program.get_data().len() < size_of_program()` and
/// `DeployWithMaxDataLen` accepts a larger pre-created account.  This module
/// matches the loader's own bound so it can never refuse a program account the
/// loader itself deployed.
pub const PROGRAM_ACCOUNT_METADATA_LEN: usize = 36;

/// `UpgradeableLoaderState::size_of_programdata_metadata()`: the fixed region
/// that precedes the ELF in every ProgramData account.
pub const PROGRAMDATA_METADATA_LEN: usize = 45;

/// Offset of `Program::programdata_address`.
pub const PROGRAM_LINK_OFFSET: usize = LOADER_TAG_LEN;
/// Offset of `ProgramData::slot`.
pub const PROGRAMDATA_SLOT_OFFSET: usize = LOADER_TAG_LEN;
/// Offset of the `Option<Pubkey>` discriminant byte of
/// `ProgramData::upgrade_authority_address`.
pub const PROGRAMDATA_AUTHORITY_TAG_OFFSET: usize = LOADER_TAG_LEN + 8;
/// Offset of the upgrade authority address, valid only when the discriminant
/// byte at [`PROGRAMDATA_AUTHORITY_TAG_OFFSET`] is [`OPTION_SOME`].
pub const PROGRAMDATA_AUTHORITY_OFFSET: usize = PROGRAMDATA_AUTHORITY_TAG_OFFSET + 1;

/// `bincode` discriminant byte for `Option::None`.
pub const OPTION_NONE: u8 = 0;
/// `bincode` discriminant byte for `Option::Some`.
pub const OPTION_SOME: u8 = 1;

const _: () = {
    assert!(PROGRAM_ACCOUNT_METADATA_LEN == LOADER_TAG_LEN + 32);
    assert!(PROGRAMDATA_METADATA_LEN == LOADER_TAG_LEN + 8 + 1 + 32);
    assert!(PROGRAMDATA_AUTHORITY_OFFSET + 32 == PROGRAMDATA_METADATA_LEN);
};

/// Refusals raised while decoding pinned Upgradeable Loader account bytes.
///
/// Every variant is a refusal.  There is no fallible-but-tolerated case: an
/// account that does not decode to exactly the expected state is not evidence
/// of a deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderStateError {
    /// The account carried fewer bytes than its state's fixed metadata region.
    ShortAccount,
    /// The account was not owned by [`UPGRADEABLE_LOADER_ID`].
    WrongLoaderOwner,
    /// The variant tag was outside the four states the loader can write.
    UnknownVariant,
    /// The program role did not decode as `Program`.
    NotProgramVariant,
    /// The ProgramData role did not decode as `ProgramData`.
    NotProgramDataVariant,
    /// The receiver program account was not executable.
    ProgramNotExecutable,
    /// The ProgramData account was executable.
    ProgramDataExecutable,
    /// An `Option` discriminant byte was neither [`OPTION_NONE`] nor
    /// [`OPTION_SOME`].
    NonCanonicalOptionTag,
    /// A decoded loader identity was the all-zero address.
    ZeroIdentity,
    /// The program account's linked ProgramData address was not the address of
    /// the presented ProgramData account.
    ProgramDataLinkMismatch,
}

/// Metadata-bearing view of one runtime account, in the shape
/// [`crate::source::SourceAccountView`] and
/// [`crate::source_archive::RuntimeAccountViewV1`] already use.
///
/// Constructing one is the *only* place an `AccountInfo` may be read; the
/// decoders below never see a runtime handle, which is what makes every
/// refusal in this module reachable from a byte fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderAccountViewV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    executable: bool,
    data: &'a [u8],
}

impl<'a> LoaderAccountViewV1<'a> {
    /// Construct a view at the `AccountInfo` boundary.
    pub const fn new(key: [u8; 32], owner: [u8; 32], executable: bool, data: &'a [u8]) -> Self {
        Self {
            key,
            owner,
            executable,
            data,
        }
    }

    /// Address of the viewed account.
    pub const fn key(self) -> [u8; 32] {
        self.key
    }
}

/// Presence of an upgrade authority on a decoded ProgramData account.
///
/// This is decoded evidence, not policy: whether an upgradeable receiver may
/// back a source generation at all is a release decision, and the deployment
/// slot pin already makes any upgrade a new generation by construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpgradeAuthorityV1 {
    /// The authority was revoked; the program's bytes are final.
    Immutable,
    /// The named address may still replace the deployed bytes.
    Present([u8; 32]),
}

/// Everything a `ProgramData` account states about its deployment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramDataStateV1 {
    /// Slot at which the linked program was last deployed or upgraded.
    pub deployment_slot: u64,
    /// Upgrade authority presence at the observed instant.
    pub upgrade_authority: UpgradeAuthorityV1,
}

/// The projection `research/source-profile-v1/src/auth_v2.rs` expects from a
/// pinned Upgradeable Loader parser.
///
/// Field-for-field identical to that crate's `LoaderStateV1`, deliberately: the
/// research model is the executable contract and this module is its runtime
/// implementation, so any divergence must show up as a compile error at the
/// kernel-port step (R2 plan P0.2) rather than as a silent semantic drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoaderStateV1 {
    /// ProgramData address encoded by the receiver program account.
    pub linked_programdata: [u8; 32],
    /// Deployment slot encoded by the linked ProgramData account.
    pub deployment_slot: u64,
}

/// One authenticated program/ProgramData pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedLoaderPairV1 {
    /// The identity and slot pins a `SourceSpec` compares against.
    pub state: LoaderStateV1,
    /// Upgrade authority presence on the linked ProgramData account.
    pub upgrade_authority: UpgradeAuthorityV1,
}

/// Decode a receiver program account and return its linked ProgramData address.
///
/// Refuses a non-loader owner, a non-executable account, a short buffer, an
/// unknown variant tag, any variant other than `Program`, and an all-zero link.
pub fn decode_program_state(view: LoaderAccountViewV1<'_>) -> Result<[u8; 32], LoaderStateError> {
    if view.owner != UPGRADEABLE_LOADER_ID {
        return Err(LoaderStateError::WrongLoaderOwner);
    }
    if !view.executable {
        return Err(LoaderStateError::ProgramNotExecutable);
    }
    if view.data.len() < PROGRAM_ACCOUNT_METADATA_LEN {
        return Err(LoaderStateError::ShortAccount);
    }
    if variant_tag(view.data)? != LOADER_TAG_PROGRAM {
        return Err(LoaderStateError::NotProgramVariant);
    }
    let link = address_at(view.data, PROGRAM_LINK_OFFSET);
    if link == [0_u8; 32] {
        return Err(LoaderStateError::ZeroIdentity);
    }
    Ok(link)
}

/// Decode a ProgramData account's deployment slot and upgrade authority.
///
/// Refuses a non-loader owner, an executable account, a short buffer, an
/// unknown variant tag, any variant other than `ProgramData`, a non-canonical
/// `Option` discriminant, and an all-zero present authority.
///
/// When the discriminant is [`OPTION_NONE`] the thirty-two bytes that follow it
/// are *not* read: see the stale-authority note in the module documentation.
pub fn decode_programdata_state(
    view: LoaderAccountViewV1<'_>,
) -> Result<ProgramDataStateV1, LoaderStateError> {
    if view.owner != UPGRADEABLE_LOADER_ID {
        return Err(LoaderStateError::WrongLoaderOwner);
    }
    if view.executable {
        return Err(LoaderStateError::ProgramDataExecutable);
    }
    if view.data.len() < PROGRAMDATA_METADATA_LEN {
        return Err(LoaderStateError::ShortAccount);
    }
    if variant_tag(view.data)? != LOADER_TAG_PROGRAMDATA {
        return Err(LoaderStateError::NotProgramDataVariant);
    }
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(&view.data[PROGRAMDATA_SLOT_OFFSET..PROGRAMDATA_SLOT_OFFSET + 8]);
    let deployment_slot = u64::from_le_bytes(slot);
    let upgrade_authority = match view.data[PROGRAMDATA_AUTHORITY_TAG_OFFSET] {
        OPTION_NONE => UpgradeAuthorityV1::Immutable,
        OPTION_SOME => {
            let authority = address_at(view.data, PROGRAMDATA_AUTHORITY_OFFSET);
            if authority == [0_u8; 32] {
                return Err(LoaderStateError::ZeroIdentity);
            }
            UpgradeAuthorityV1::Present(authority)
        }
        _ => return Err(LoaderStateError::NonCanonicalOptionTag),
    };
    Ok(ProgramDataStateV1 {
        deployment_slot,
        upgrade_authority,
    })
}

/// Decode a receiver program together with the ProgramData account it links to.
///
/// This is the whole capability: it establishes that the presented ProgramData
/// account is the one the executable program itself names, and reports the slot
/// that account records.  It does **not** compare either against a `SourceSpec`
/// — that comparison belongs to the authenticator (`auth_v2`'s
/// `WrongProgramData` / `ProgramDataLinkMismatch` / `DeploymentSlotMismatch`),
/// which owns the frozen expectations.
pub fn decode_loader_pair_v1(
    program: LoaderAccountViewV1<'_>,
    programdata: LoaderAccountViewV1<'_>,
) -> Result<DecodedLoaderPairV1, LoaderStateError> {
    let linked_programdata = decode_program_state(program)?;
    if linked_programdata != programdata.key {
        return Err(LoaderStateError::ProgramDataLinkMismatch);
    }
    let data = decode_programdata_state(programdata)?;
    Ok(DecodedLoaderPairV1 {
        state: LoaderStateV1 {
            linked_programdata,
            deployment_slot: data.deployment_slot,
        },
        upgrade_authority: data.upgrade_authority,
    })
}

/// Decode the exact synthetic ProgramData sentinel emitted by local
/// `--bpf-program` genesis loading.
///
/// This deliberately does not weaken [`decode_programdata_state`]: the
/// all-zero `Some` authority remains invalid for every observed loader state.
/// The successor registry-release adapter may select this decoder only after
/// a content-addressed release body names `SynthesizedGenesisZero`, and this
/// function then requires slot zero plus the exact `Some(default pubkey)`
/// metadata image. The complete ProgramData bytes are still hashed by that
/// adapter, so the ELF remains part of the admitted identity.
pub fn decode_synthesized_genesis_loader_pair_v1(
    program: LoaderAccountViewV1<'_>,
    programdata: LoaderAccountViewV1<'_>,
) -> Result<LoaderStateV1, LoaderStateError> {
    let linked_programdata = decode_program_state(program)?;
    if linked_programdata != programdata.key {
        return Err(LoaderStateError::ProgramDataLinkMismatch);
    }
    if programdata.owner != UPGRADEABLE_LOADER_ID {
        return Err(LoaderStateError::WrongLoaderOwner);
    }
    if programdata.executable {
        return Err(LoaderStateError::ProgramDataExecutable);
    }
    if programdata.data.len() < PROGRAMDATA_METADATA_LEN {
        return Err(LoaderStateError::ShortAccount);
    }
    if variant_tag(programdata.data)? != LOADER_TAG_PROGRAMDATA {
        return Err(LoaderStateError::NotProgramDataVariant);
    }
    let mut slot = [0_u8; 8];
    slot.copy_from_slice(
        &programdata.data[PROGRAMDATA_SLOT_OFFSET..PROGRAMDATA_SLOT_OFFSET + 8],
    );
    if u64::from_le_bytes(slot) != 0
        || programdata.data[PROGRAMDATA_AUTHORITY_TAG_OFFSET] != OPTION_SOME
        || address_at(programdata.data, PROGRAMDATA_AUTHORITY_OFFSET) != [0_u8; 32]
    {
        return Err(LoaderStateError::ZeroIdentity);
    }
    Ok(LoaderStateV1 {
        linked_programdata,
        deployment_slot: 0,
    })
}

fn variant_tag(data: &[u8]) -> Result<u32, LoaderStateError> {
    let mut tag = [0_u8; LOADER_TAG_LEN];
    tag.copy_from_slice(&data[..LOADER_TAG_LEN]);
    let tag = u32::from_le_bytes(tag);
    if tag > LOADER_TAG_PROGRAMDATA {
        return Err(LoaderStateError::UnknownVariant);
    }
    Ok(tag)
}

fn address_at(data: &[u8], offset: usize) -> [u8; 32] {
    let mut address = [0_u8; 32];
    address.copy_from_slice(&data[offset..offset + 32]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM: [u8; 32] = [0xb2; 32];
    const PROGRAMDATA: [u8; 32] = [0xb3; 32];
    const AUTHORITY: [u8; 32] = [0xd5; 32];
    const OTHER: [u8; 32] = [0xf0; 32];

    /* Byte images captured from `bincode::serialize` over
     * `solana_loader_v3_interface::state::UpgradeableLoaderState` at crate
     * version 7.0.0.  They are the real serializer's output, not a reading of
     * the layout table. */
    const CAPTURED_PROGRAM: &str =
        "02000000b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3";
    const CAPTURED_PROGRAMDATA_SOME: &str = concat!(
        "030000004d00000000000000",
        "01",
        "d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5"
    );
    const CAPTURED_PROGRAMDATA_NONE: &str = "03000000080706050403020100";
    const CAPTURED_UNINITIALIZED: &str = "00000000";
    const CAPTURED_BUFFER: &str =
        "01000000019a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a";

    fn unhex(text: &str) -> [u8; PROGRAMDATA_METADATA_LEN] {
        let source = text.as_bytes();
        assert!(source.len().is_multiple_of(2));
        assert!(source.len() / 2 <= PROGRAMDATA_METADATA_LEN);
        let mut out = [0_u8; PROGRAMDATA_METADATA_LEN];
        let mut index = 0;
        while index < source.len() / 2 {
            out[index] = (nibble(source[index * 2]) << 4) | nibble(source[index * 2 + 1]);
            index += 1;
        }
        out
    }

    fn unhex_len(text: &str) -> usize {
        text.len() / 2
    }

    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture is lowercase hexadecimal"),
        }
    }

    /// `Program { programdata_address }`, hand-built from the layout table.
    fn program_bytes(link: [u8; 32]) -> [u8; PROGRAM_ACCOUNT_METADATA_LEN] {
        let mut out = [0_u8; PROGRAM_ACCOUNT_METADATA_LEN];
        out[..4].copy_from_slice(&LOADER_TAG_PROGRAM.to_le_bytes());
        out[PROGRAM_LINK_OFFSET..].copy_from_slice(&link);
        out
    }

    /// `ProgramData { slot, upgrade_authority_address }`, hand-built.
    fn programdata_bytes(slot: u64, authority: Option<[u8; 32]>) -> [u8; PROGRAMDATA_METADATA_LEN] {
        let mut out = [0_u8; PROGRAMDATA_METADATA_LEN];
        out[..4].copy_from_slice(&LOADER_TAG_PROGRAMDATA.to_le_bytes());
        out[PROGRAMDATA_SLOT_OFFSET..PROGRAMDATA_SLOT_OFFSET + 8]
            .copy_from_slice(&slot.to_le_bytes());
        match authority {
            Some(address) => {
                out[PROGRAMDATA_AUTHORITY_TAG_OFFSET] = OPTION_SOME;
                out[PROGRAMDATA_AUTHORITY_OFFSET..].copy_from_slice(&address);
            }
            None => out[PROGRAMDATA_AUTHORITY_TAG_OFFSET] = OPTION_NONE,
        }
        out
    }

    fn program_view(data: &[u8]) -> LoaderAccountViewV1<'_> {
        LoaderAccountViewV1::new(PROGRAM, UPGRADEABLE_LOADER_ID, true, data)
    }

    fn programdata_view(data: &[u8]) -> LoaderAccountViewV1<'_> {
        LoaderAccountViewV1::new(PROGRAMDATA, UPGRADEABLE_LOADER_ID, false, data)
    }

    #[test]
    fn pinned_loader_id_matches_the_sdk_declaration() {
        assert_eq!(
            UPGRADEABLE_LOADER_ID,
            solana_sdk_ids::bpf_loader_upgradeable::ID.to_bytes()
        );
    }

    #[test]
    fn hand_built_bytes_equal_the_captured_serializer_output() {
        let program = program_bytes(PROGRAMDATA);
        assert_eq!(unhex_len(CAPTURED_PROGRAM), PROGRAM_ACCOUNT_METADATA_LEN);
        assert_eq!(program[..], unhex(CAPTURED_PROGRAM)[..program.len()]);

        let some = programdata_bytes(77, Some(AUTHORITY));
        assert_eq!(
            unhex_len(CAPTURED_PROGRAMDATA_SOME),
            PROGRAMDATA_METADATA_LEN
        );
        assert_eq!(some, unhex(CAPTURED_PROGRAMDATA_SOME));

        /* `None` serializes to thirteen bytes: the loader leaves the remaining
         * thirty-two bytes of the metadata region untouched. */
        let none = programdata_bytes(0x0102_0304_0506_0708, None);
        assert_eq!(unhex_len(CAPTURED_PROGRAMDATA_NONE), 13);
        assert_eq!(
            none[..13],
            unhex(CAPTURED_PROGRAMDATA_NONE)[..13],
            "captured None image disagrees with the hand-built prefix"
        );
    }

    #[test]
    fn documented_offsets_and_widths_hold() {
        assert_eq!(LOADER_TAG_LEN, 4);
        assert_eq!(PROGRAM_ACCOUNT_METADATA_LEN, 36);
        assert_eq!(PROGRAMDATA_METADATA_LEN, 45);
        assert_eq!(PROGRAM_LINK_OFFSET, 4);
        assert_eq!(PROGRAMDATA_SLOT_OFFSET, 4);
        assert_eq!(PROGRAMDATA_AUTHORITY_TAG_OFFSET, 12);
        assert_eq!(PROGRAMDATA_AUTHORITY_OFFSET, 13);
        /* `size_of_uninitialized()` and `size_of_buffer_metadata()` from the
         * same table, checked against captured images. */
        assert_eq!(unhex_len(CAPTURED_UNINITIALIZED), 4);
        assert_eq!(unhex_len(CAPTURED_BUFFER), 37);
    }

    #[test]
    fn captured_program_and_programdata_decode() {
        let program = unhex(CAPTURED_PROGRAM);
        let link =
            decode_program_state(program_view(&program[..unhex_len(CAPTURED_PROGRAM)])).unwrap();
        assert_eq!(link, PROGRAMDATA);

        let programdata = unhex(CAPTURED_PROGRAMDATA_SOME);
        assert_eq!(
            decode_programdata_state(programdata_view(&programdata)).unwrap(),
            ProgramDataStateV1 {
                deployment_slot: 77,
                upgrade_authority: UpgradeAuthorityV1::Present(AUTHORITY),
            }
        );
    }

    #[test]
    fn joined_pair_reports_the_link_and_slot() {
        let program = program_bytes(PROGRAMDATA);
        let programdata = programdata_bytes(77, Some(AUTHORITY));
        assert_eq!(
            decode_loader_pair_v1(program_view(&program), programdata_view(&programdata)).unwrap(),
            DecodedLoaderPairV1 {
                state: LoaderStateV1 {
                    linked_programdata: PROGRAMDATA,
                    deployment_slot: 77,
                },
                upgrade_authority: UpgradeAuthorityV1::Present(AUTHORITY),
            }
        );
    }

    #[test]
    fn revoked_authority_ignores_the_stale_authority_bytes() {
        /* The exact hazard the loader creates: `set_state` with `None` writes
         * only thirteen bytes, so a revoked account still carries its former
         * authority at [13..45).  Two accounts that differ only there must
         * decode identically, and neither may report an authority. */
        let mut stale = programdata_bytes(4_100, None);
        stale[PROGRAMDATA_AUTHORITY_OFFSET..].copy_from_slice(&AUTHORITY);
        let clean = programdata_bytes(4_100, None);
        let expected = ProgramDataStateV1 {
            deployment_slot: 4_100,
            upgrade_authority: UpgradeAuthorityV1::Immutable,
        };
        assert_eq!(
            decode_programdata_state(programdata_view(&stale)).unwrap(),
            expected
        );
        assert_eq!(
            decode_programdata_state(programdata_view(&clean)).unwrap(),
            expected
        );
    }

    #[test]
    fn a_larger_program_account_still_decodes() {
        /* `DeployWithMaxDataLen` accepts a pre-created program account larger
         * than `size_of_program()`; the loader only refuses shorter ones. */
        let mut oversized = [0xcc_u8; PROGRAM_ACCOUNT_METADATA_LEN + 64];
        oversized[..PROGRAM_ACCOUNT_METADATA_LEN].copy_from_slice(&program_bytes(PROGRAMDATA));
        assert_eq!(
            decode_program_state(program_view(&oversized)).unwrap(),
            PROGRAMDATA
        );
    }

    #[test]
    fn wrong_owner_refuses_both_roles() {
        let program = program_bytes(PROGRAMDATA);
        assert_eq!(
            decode_program_state(LoaderAccountViewV1::new(PROGRAM, OTHER, true, &program)),
            Err(LoaderStateError::WrongLoaderOwner)
        );
        let programdata = programdata_bytes(77, None);
        assert_eq!(
            decode_programdata_state(LoaderAccountViewV1::new(
                PROGRAMDATA,
                OTHER,
                false,
                &programdata
            )),
            Err(LoaderStateError::WrongLoaderOwner)
        );
    }

    #[test]
    fn executability_inversions_refuse() {
        let program = program_bytes(PROGRAMDATA);
        assert_eq!(
            decode_program_state(LoaderAccountViewV1::new(
                PROGRAM,
                UPGRADEABLE_LOADER_ID,
                false,
                &program
            )),
            Err(LoaderStateError::ProgramNotExecutable)
        );
        let programdata = programdata_bytes(77, None);
        assert_eq!(
            decode_programdata_state(LoaderAccountViewV1::new(
                PROGRAMDATA,
                UPGRADEABLE_LOADER_ID,
                true,
                &programdata
            )),
            Err(LoaderStateError::ProgramDataExecutable)
        );
    }

    #[test]
    fn every_wrong_variant_refuses_in_each_role() {
        for tag in [
            LOADER_TAG_UNINITIALIZED,
            LOADER_TAG_BUFFER,
            LOADER_TAG_PROGRAMDATA,
        ] {
            let mut bytes = program_bytes(PROGRAMDATA);
            bytes[..4].copy_from_slice(&tag.to_le_bytes());
            assert_eq!(
                decode_program_state(program_view(&bytes)),
                Err(LoaderStateError::NotProgramVariant),
                "program role accepted tag {tag}"
            );
        }
        for tag in [
            LOADER_TAG_UNINITIALIZED,
            LOADER_TAG_BUFFER,
            LOADER_TAG_PROGRAM,
        ] {
            let mut bytes = programdata_bytes(77, Some(AUTHORITY));
            bytes[..4].copy_from_slice(&tag.to_le_bytes());
            assert_eq!(
                decode_programdata_state(programdata_view(&bytes)),
                Err(LoaderStateError::NotProgramDataVariant),
                "programdata role accepted tag {tag}"
            );
        }
    }

    #[test]
    fn unknown_variant_tags_refuse() {
        for tag in [4_u32, 5, 0x0100, u32::MAX] {
            let mut program = program_bytes(PROGRAMDATA);
            program[..4].copy_from_slice(&tag.to_le_bytes());
            assert_eq!(
                decode_program_state(program_view(&program)),
                Err(LoaderStateError::UnknownVariant),
                "program role accepted unknown tag {tag}"
            );
            let mut programdata = programdata_bytes(77, Some(AUTHORITY));
            programdata[..4].copy_from_slice(&tag.to_le_bytes());
            assert_eq!(
                decode_programdata_state(programdata_view(&programdata)),
                Err(LoaderStateError::UnknownVariant),
                "programdata role accepted unknown tag {tag}"
            );
        }
    }

    #[test]
    fn truncation_at_every_byte_boundary_refuses() {
        let program = program_bytes(PROGRAMDATA);
        for cut in 0..PROGRAM_ACCOUNT_METADATA_LEN {
            assert_eq!(
                decode_program_state(program_view(&program[..cut])),
                Err(LoaderStateError::ShortAccount),
                "program role accepted a {cut}-byte account"
            );
        }
        let programdata = programdata_bytes(77, Some(AUTHORITY));
        for cut in 0..PROGRAMDATA_METADATA_LEN {
            assert_eq!(
                decode_programdata_state(programdata_view(&programdata[..cut])),
                Err(LoaderStateError::ShortAccount),
                "programdata role accepted a {cut}-byte account"
            );
        }
    }

    #[test]
    fn a_thirteen_byte_revoked_image_is_still_too_short() {
        /* The serializer emits thirteen bytes for a revoked authority, but the
         * loader always allocates the full metadata region.  A thirteen-byte
         * *account* is not a ProgramData account. */
        let programdata = programdata_bytes(77, None);
        assert_eq!(
            decode_programdata_state(programdata_view(&programdata[..13])),
            Err(LoaderStateError::ShortAccount)
        );
    }

    #[test]
    fn non_canonical_option_tags_refuse() {
        for tag in [2_u8, 3, 0x80, 0xff] {
            let mut bytes = programdata_bytes(77, Some(AUTHORITY));
            bytes[PROGRAMDATA_AUTHORITY_TAG_OFFSET] = tag;
            assert_eq!(
                decode_programdata_state(programdata_view(&bytes)),
                Err(LoaderStateError::NonCanonicalOptionTag),
                "accepted option discriminant {tag}"
            );
        }
    }

    #[test]
    fn zero_identities_refuse() {
        let program = program_bytes([0_u8; 32]);
        assert_eq!(
            decode_program_state(program_view(&program)),
            Err(LoaderStateError::ZeroIdentity)
        );
        let programdata = programdata_bytes(77, Some([0_u8; 32]));
        assert_eq!(
            decode_programdata_state(programdata_view(&programdata)),
            Err(LoaderStateError::ZeroIdentity)
        );
    }

    #[test]
    fn synthesized_genesis_decoder_is_disjoint_from_strict_loader_decode() {
        let program = program_bytes(PROGRAMDATA);
        let programdata = programdata_bytes(0, Some([0_u8; 32]));
        assert_eq!(
            decode_loader_pair_v1(program_view(&program), programdata_view(&programdata)),
            Err(LoaderStateError::ZeroIdentity)
        );
        assert_eq!(
            decode_synthesized_genesis_loader_pair_v1(
                program_view(&program),
                programdata_view(&programdata),
            ),
            Ok(LoaderStateV1 {
                linked_programdata: PROGRAMDATA,
                deployment_slot: 0,
            })
        );
    }

    #[test]
    fn synthesized_genesis_decoder_refuses_every_nearby_loader_state() {
        let program = program_bytes(PROGRAMDATA);
        for programdata in [
            programdata_bytes(1, Some([0_u8; 32])),
            programdata_bytes(0, None),
            programdata_bytes(0, Some(AUTHORITY)),
        ] {
            assert_eq!(
                decode_synthesized_genesis_loader_pair_v1(
                    program_view(&program),
                    programdata_view(&programdata),
                ),
                Err(LoaderStateError::ZeroIdentity)
            );
        }
    }

    #[test]
    fn a_substituted_programdata_account_refuses() {
        let program = program_bytes(PROGRAMDATA);
        let programdata = programdata_bytes(77, Some(AUTHORITY));
        let substituted =
            LoaderAccountViewV1::new(OTHER, UPGRADEABLE_LOADER_ID, false, &programdata);
        assert_eq!(
            decode_loader_pair_v1(program_view(&program), substituted),
            Err(LoaderStateError::ProgramDataLinkMismatch)
        );
    }

    #[test]
    fn the_pair_decoder_propagates_every_component_refusal() {
        let program = program_bytes(PROGRAMDATA);
        let programdata = programdata_bytes(77, Some(AUTHORITY));
        assert_eq!(
            decode_loader_pair_v1(
                LoaderAccountViewV1::new(PROGRAM, UPGRADEABLE_LOADER_ID, false, &program),
                programdata_view(&programdata)
            ),
            Err(LoaderStateError::ProgramNotExecutable)
        );
        assert_eq!(
            decode_loader_pair_v1(
                program_view(&program),
                LoaderAccountViewV1::new(PROGRAMDATA, UPGRADEABLE_LOADER_ID, true, &programdata)
            ),
            Err(LoaderStateError::ProgramDataExecutable)
        );
        assert_eq!(
            decode_loader_pair_v1(
                program_view(&program),
                programdata_view(&programdata[..PROGRAMDATA_METADATA_LEN - 1])
            ),
            Err(LoaderStateError::ShortAccount)
        );
    }

    #[test]
    fn slot_is_read_little_endian_across_its_full_range() {
        for slot in [0_u64, 1, 77, u64::from(u32::MAX), u64::MAX] {
            let bytes = programdata_bytes(slot, None);
            assert_eq!(
                decode_programdata_state(programdata_view(&bytes))
                    .unwrap()
                    .deployment_slot,
                slot
            );
        }
    }
}
