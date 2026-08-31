//! The offline probe for the one seal the ZeroBump close arm exists to reach.
//!
//! Cohort-9's plan review makes that arm probe-gated: before the cut ships, the
//! account it is for has to be shown to be exactly the shape the arm reads —
//! and if it is not, the item is dead on arrival and neglect is forced. This
//! module is that check, and it runs against a dumped account with no cluster in
//! the loop.
//!
//! Nothing here restates a byte layout. The body is read by
//! [`SealedDescriptorClosureV1::decode_defunct`] itself, so a probe that passes
//! is a statement about the very decoder the chain will run; the bump is
//! recovered by the same 255-down-to-1 walk `find_program_address` performs, so
//! the candidate it reports is the one the closer will send. The two facts the
//! probe adds on top are the two conjuncts the close makes about the *account*
//! rather than the body: that this Program owns it, and that the release it is
//! sealed under is not the one that will be live at the cut.

use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_BYTES_V1, CapabilitySealKeyV1, Error as CapabilitySealError,
    SealedDescriptorClosureV1,
};
use solana_program::{pubkey::Pubkey, rent::Rent};

/// One account as `solana account --output json` writes it.
///
/// Both shapes that command produces are read: the outer record carrying
/// `pubkey` beside an `account` object, and a bare account object on its own.
/// The address is what the bump walk is checked against, so a dump that does
/// not carry one must be given the address separately rather than probed
/// against a guess.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealAccountDumpV1 {
    address: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data: Vec<u8>,
}

impl SealAccountDumpV1 {
    /// Build one probe input from an account read off a live cluster.
    ///
    /// The offline gate parses a `solana account --output json` dump; the
    /// cut-time one-shot reads the same account over RPC. Both converge on this
    /// value and are judged by [`probe_defunct_seal_v1`], so the command that
    /// actually sends the close cannot reach a verdict the gate could not.
    /// Only the SOURCE of the four fields differs, never their meaning.
    #[must_use]
    pub const fn from_observed_v1(
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data: Vec<u8>,
    ) -> Self {
        Self {
            address,
            owner,
            lamports,
            data,
        }
    }

    /// Read one account dump, with an optional address the caller supplies.
    ///
    /// A dump that names its own `pubkey` and a caller who names a different
    /// one is a refusal rather than a precedence rule: the whole probe is an
    /// address argument, so the two disagreeing means one of them is about some
    /// other account.
    pub fn parse(json: &str, address: Option<[u8; 32]>) -> Result<Self, String> {
        let document: serde_json::Value =
            serde_json::from_str(json).map_err(|error| format!("the dump is not JSON: {error}"))?;
        let account = document.get("account").unwrap_or(&document);
        let dumped = document
            .get("pubkey")
            .and_then(serde_json::Value::as_str)
            .map(|value| parse_base58_v1(value, "the dump's `pubkey`"))
            .transpose()?;
        let address = match (dumped, address) {
            (Some(dumped), Some(named)) if dumped != named => {
                return Err(format!(
                    "the dump names {} and --address names {}; one of them is about another \
                     account",
                    Pubkey::new_from_array(dumped),
                    Pubkey::new_from_array(named)
                ));
            }
            (Some(value), _) | (None, Some(value)) => value,
            (None, None) => {
                return Err(
                    "the dump carries no `pubkey`, so name the account with --address <BASE58>"
                        .to_owned(),
                );
            }
        };
        let owner = parse_base58_v1(
            account
                .get("owner")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "the dump carries no `owner`".to_owned())?,
            "the dump's `owner`",
        )?;
        let lamports = account
            .get("lamports")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "the dump carries no `lamports`".to_owned())?;
        Ok(Self {
            address,
            owner,
            lamports,
            data: parse_account_data_v1(account)?,
        })
    }

    /// The account's own address.
    #[must_use]
    pub const fn address(&self) -> [u8; 32] {
        self.address
    }

    /// The program that owns the account.
    #[must_use]
    pub const fn owner(&self) -> [u8; 32] {
        self.owner
    }

    /// The account's balance.
    #[must_use]
    pub const fn lamports(&self) -> u64 {
        self.lamports
    }

    /// The account's data.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// Everything the probe could establish about one dumped account.
///
/// Each field is one conjunct of the close, kept separate so a failure names
/// the conjunct that failed instead of collapsing into a single "no".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealProbeVerdictV1 {
    /// What the ordinary decoder made of the body, and the bump it persists.
    pub canonical: Result<u8, CapabilitySealError>,
    /// What the defunct decoder made of the body.
    pub defunct: Result<(), CapabilitySealError>,
    /// The four coordinates the body is filed under, when a decoder read them.
    pub key: Option<CapabilitySealKeyV1>,
    /// The bump that reproduces the account's own address, when one does.
    pub bump_candidate: Option<u8>,
    /// Whether the owning program is the one the address was derived under.
    pub owner_is_program: bool,
    /// Whether the balance is rent-exempt at the exact seal width.
    pub rent_exempt: bool,
    /// Whether the sealed release is the one that will be live at the cut.
    pub release_is_live: Option<bool>,
}

impl SealProbeVerdictV1 {
    /// Whether the ZeroBump arm would close this account at the cut.
    ///
    /// Every conjunct, and they are the route's own: this Program owns it, it
    /// is rent-exempt at the exact width, the body is defunct-canonical, some
    /// candidate reproduces the address, and the release it is sealed under is
    /// not live.
    #[must_use]
    pub fn closable(&self) -> bool {
        self.owner_is_program
            && self.rent_exempt
            && self.defunct.is_ok()
            && self.bump_candidate.is_some()
            && self.release_is_live == Some(false)
    }

    /// The first conjunct that fails, named, or `None` when none does.
    #[must_use]
    pub fn refusal(&self) -> Option<String> {
        if !self.owner_is_program {
            return Some(
                "the account is not owned by the program its address is derived under, so it is \
                 not this Program's seal"
                    .to_owned(),
            );
        }
        if !self.rent_exempt {
            return Some(format!(
                "the balance is not rent-exempt at {CAPABILITY_SEAL_BYTES_V1} bytes, which \
                 `CloseSeal` refuses as `CloseSealAccount`"
            ));
        }
        if let Err(error) = self.defunct {
            return Some(format!(
                "`SealedDescriptorClosureV1::decode_defunct` refuses this body with `{error:?}`"
            ));
        }
        if self.bump_candidate.is_none() {
            return Some(
                "no bump from 255 down to 1 reproduces this account's address from the seeds its \
                 own body states, so the body does not describe this address"
                    .to_owned(),
            );
        }
        match self.release_is_live {
            Some(true) => Some(
                "the release this seal is sealed under is the one that will be live at the cut, \
                 which `CloseSeal` refuses as `CloseSealLiveRelease`"
                    .to_owned(),
            ),
            Some(false) => None,
            None => Some("no decoder could read the release this seal is sealed under".to_owned()),
        }
    }
}

/// Probe one dumped account against the release that will be live at the cut.
///
/// `program_id` is the Program the seal address is derived under, which for a
/// real seal is the account's own owner. It is a parameter rather than a read of
/// the owner field so that a caller checking a dump against a *named* Program
/// gets a refusal instead of a derivation under whatever the dump claims.
#[must_use]
pub fn probe_defunct_seal_v1(
    account: &SealAccountDumpV1,
    program_id: [u8; 32],
    live_trading_release: [u8; 32],
) -> SealProbeVerdictV1 {
    let canonical =
        SealedDescriptorClosureV1::decode(account.data()).and_then(SealedDescriptorClosureV1::bump);
    let decoded = SealedDescriptorClosureV1::decode_defunct(account.data());
    let key = decoded
        .ok()
        .and_then(|closure| closure.key().ok())
        .or_else(|| {
            SealedDescriptorClosureV1::decode(account.data())
                .and_then(SealedDescriptorClosureV1::key)
                .ok()
        });
    SealProbeVerdictV1 {
        canonical,
        defunct: decoded.map(|_| ()),
        key,
        bump_candidate: key
            .and_then(|key| mine_bump_candidate_v1(key, program_id, account.address())),
        owner_is_program: account.owner() == program_id,
        // The route's own conjunct, at the route's own width: exemption is
        // required for a canonical seal, not for whatever width the dump
        // happens to carry, and a body of another width has already been
        // refused by the decoder above. `Rent::default()` is the schedule every
        // Solana cluster this protocol runs on is configured with.
        rent_exempt: Rent::default().is_exempt(account.lamports(), CAPABILITY_SEAL_BYTES_V1),
        release_is_live: key.map(|key| key.trading_semantic_release() == live_trading_release),
    }
}

/// Recover the bump one seal address was derived under, offline.
///
/// This is `find_program_address`'s own search written out: walk 255 down to 1
/// and keep the first candidate whose `create_program_address` IS the account's
/// address. It consults no chain state, which is what lets this probe gate the
/// cut without a cluster, and it returns exactly the byte the close request must
/// carry.
#[must_use]
pub fn mine_bump_candidate_v1(
    key: CapabilitySealKeyV1,
    program_id: [u8; 32],
    address: [u8; 32],
) -> Option<u8> {
    let program = Pubkey::new_from_array(program_id);
    let wanted = Pubkey::new_from_array(address);
    let seeds = key.seeds();
    let base = seeds.as_slices();
    (1..=u8::MAX).rev().find(|candidate| {
        let tail = [*candidate];
        Pubkey::create_program_address(
            &[base[0], base[1], base[2], base[3], base[4], base[5], &tail],
            &program,
        )
        .is_ok_and(|derived| derived == wanted)
    })
}

/// Read one 32-byte identity written the way a cluster writes addresses.
fn parse_base58_v1(value: &str, label: &str) -> Result<[u8; 32], String> {
    value
        .parse::<Pubkey>()
        .map(|pubkey| pubkey.to_bytes())
        .map_err(|_| format!("{label} is not a base58 32-byte address: `{value}`"))
}

/// Read the `data` field, refusing any encoding but base64 by name.
fn parse_account_data_v1(account: &serde_json::Value) -> Result<Vec<u8>, String> {
    use base64::Engine as _;

    let data = account
        .get("data")
        .ok_or_else(|| "the dump carries no `data`".to_owned())?;
    let encoded = match data {
        serde_json::Value::String(text) => text.as_str(),
        serde_json::Value::Array(pair) => {
            match pair.get(1).and_then(serde_json::Value::as_str) {
                Some("base64") | None => {}
                Some(other) => {
                    return Err(format!(
                        "the dump's `data` is {other}-encoded; re-fetch it with \
                         `--output json`, which writes base64"
                    ));
                }
            }
            pair.first()
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "the dump's `data` array carries no string".to_owned())?
        }
        _ => return Err("the dump's `data` is neither a string nor an array".to_owned()),
    };
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| format!("the dump's `data` is not valid base64: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{SealAccountDumpV1, probe_defunct_seal_v1};
    use base64::Engine as _;
    use dclutch_capability_seal_contract::{
        CAPABILITY_SEAL_BUMP_OFFSET_V1, CAPABILITY_SEAL_BYTES_V1, CapabilitySealKeyV1,
        Error as CapabilitySealError, SealedDescriptorClosureV1, SealedRecordRowV1, SealedRoleV1,
    };
    use solana_program::{pubkey::Pubkey, rent::Rent};

    const PROGRAM_ID: [u8; 32] = [9; 32];
    const SEALED_RELEASE: [u8; 32] = [0x33; 32];
    const LIVE_RELEASE: [u8; 32] = [0x44; 32];

    fn key() -> CapabilitySealKeyV1 {
        CapabilitySealKeyV1::new([0x11; 32], [0x22; 32], 7, SEALED_RELEASE, [0x55; 32])
            .expect("canonical seal key")
    }

    fn rows() -> [SealedRecordRowV1; 6] {
        let mut ordinal = 0_u8;
        SealedRoleV1::canonical_order().map(|role| {
            ordinal = ordinal.saturating_add(1);
            let (schema, digest) = if role == SealedRoleV1::Descriptor {
                ([0x11; 32], [0x22; 32])
            } else {
                (
                    [0x40_u8.saturating_add(ordinal); 32],
                    [0x50_u8.saturating_add(ordinal); 32],
                )
            };
            SealedRecordRowV1::new(
                role,
                u32::from(ordinal).saturating_mul(64).max(1),
                schema,
                digest,
                [0x60_u8.saturating_add(ordinal); 32],
                [0x70_u8.saturating_add(ordinal); 32],
            )
            .expect("canonical row")
        })
    }

    /// The seal this key names, at the address its own seeds derive, with the
    /// bump byte either recorded or left as the pre-bump layout left it.
    fn seal(recorded_bump: bool) -> ([u8; 32], u8, Vec<u8>) {
        let key = key();
        let (address, bump) = Pubkey::find_program_address(
            &key.seeds().as_slices(),
            &Pubkey::new_from_array(PROGRAM_ID),
        );
        let mut bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
        SealedDescriptorClosureV1::encode(key, rows(), bump, &mut bytes).expect("canonical seal");
        if !recorded_bump {
            *bytes
                .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
                .expect("sealed canonical bump") = 0;
        }
        (address.to_bytes(), bump, bytes)
    }

    fn dump(address: [u8; 32], owner: [u8; 32], body: &[u8]) -> String {
        format!(
            "{{\"pubkey\":\"{}\",\"account\":{{\"lamports\":{},\"data\":[\"{}\",\"base64\"],\
             \"owner\":\"{}\",\"executable\":false,\"rentEpoch\":0,\"space\":{}}}}}",
            Pubkey::new_from_array(address),
            Rent::default().minimum_balance(CAPABILITY_SEAL_BYTES_V1),
            base64::engine::general_purpose::STANDARD.encode(body),
            Pubkey::new_from_array(owner),
            body.len()
        )
    }

    #[test]
    fn a_defunct_seal_at_its_own_address_passes() {
        let (address, bump, body) = seal(false);
        let account = SealAccountDumpV1::parse(&dump(address, PROGRAM_ID, &body), None)
            .expect("the dump parses");
        let verdict = probe_defunct_seal_v1(&account, PROGRAM_ID, LIVE_RELEASE);
        assert_eq!(verdict.canonical, Err(CapabilitySealError::ZeroBump));
        assert_eq!(verdict.defunct, Ok(()));
        assert_eq!(verdict.bump_candidate, Some(bump));
        assert_eq!(verdict.release_is_live, Some(false));
        assert_eq!(verdict.refusal(), None);
        assert!(verdict.closable());
    }

    #[test]
    fn a_well_formed_seal_is_dead_on_arrival_and_the_probe_names_the_conjunct() {
        let (address, bump, body) = seal(true);
        let account = SealAccountDumpV1::parse(&dump(address, PROGRAM_ID, &body), None)
            .expect("the dump parses");
        let verdict = probe_defunct_seal_v1(&account, PROGRAM_ID, LIVE_RELEASE);
        assert_eq!(verdict.canonical, Ok(bump));
        assert_eq!(verdict.defunct, Err(CapabilitySealError::NotDefunct));
        assert!(!verdict.closable());
        assert!(
            verdict
                .refusal()
                .is_some_and(|reason| reason.contains("NotDefunct")),
            "the probe did not name the conjunct that failed"
        );
    }

    #[test]
    fn a_seal_under_the_release_that_will_be_live_is_dead_on_arrival() {
        let (address, _, body) = seal(false);
        let account = SealAccountDumpV1::parse(&dump(address, PROGRAM_ID, &body), None)
            .expect("the dump parses");
        let verdict = probe_defunct_seal_v1(&account, PROGRAM_ID, SEALED_RELEASE);
        assert_eq!(verdict.defunct, Ok(()));
        assert_eq!(verdict.release_is_live, Some(true));
        assert!(!verdict.closable());
        assert!(
            verdict
                .refusal()
                .is_some_and(|reason| reason.contains("CloseSealLiveRelease"))
        );
    }

    #[test]
    fn a_body_at_someone_elses_address_reproduces_nothing() {
        let (_, _, body) = seal(false);
        let elsewhere = [0xab; 32];
        let account = SealAccountDumpV1::parse(&dump(elsewhere, PROGRAM_ID, &body), None)
            .expect("the dump parses");
        let verdict = probe_defunct_seal_v1(&account, PROGRAM_ID, LIVE_RELEASE);
        assert_eq!(verdict.defunct, Ok(()));
        assert_eq!(verdict.bump_candidate, None);
        assert!(!verdict.closable());
    }

    #[test]
    fn an_account_this_program_does_not_own_is_dead_on_arrival() {
        let (address, _, body) = seal(false);
        let account = SealAccountDumpV1::parse(&dump(address, [0x77; 32], &body), None)
            .expect("the dump parses");
        let verdict = probe_defunct_seal_v1(&account, PROGRAM_ID, LIVE_RELEASE);
        assert!(!verdict.owner_is_program);
        assert!(!verdict.closable());
    }

    #[test]
    fn both_dump_shapes_parse_and_a_disagreeing_address_refuses() {
        let (address, _, body) = seal(false);
        let outer = dump(address, PROGRAM_ID, &body);
        let parsed = SealAccountDumpV1::parse(&outer, None).expect("the outer shape parses");
        assert_eq!(parsed.address(), address);
        assert_eq!(parsed.owner(), PROGRAM_ID);
        assert_eq!(parsed.data(), body.as_slice());

        let bare = format!(
            "{{\"lamports\":{},\"data\":[\"{}\",\"base64\"],\"owner\":\"{}\"}}",
            Rent::default().minimum_balance(CAPABILITY_SEAL_BYTES_V1),
            base64::engine::general_purpose::STANDARD.encode(&body),
            Pubkey::new_from_array(PROGRAM_ID)
        );
        assert!(
            SealAccountDumpV1::parse(&bare, None).is_err(),
            "a dump with no address was probed against a guess"
        );
        assert_eq!(
            SealAccountDumpV1::parse(&bare, Some(address))
                .expect("the bare shape parses with an address")
                .address(),
            address
        );
        assert!(
            SealAccountDumpV1::parse(&outer, Some([0xcd; 32])).is_err(),
            "two disagreeing addresses were reconciled instead of refused"
        );
    }

    #[test]
    fn a_data_encoding_this_probe_cannot_read_is_named_rather_than_guessed() {
        let (address, _, _) = seal(false);
        let hostile = format!(
            "{{\"pubkey\":\"{}\",\"account\":{{\"lamports\":1,\"data\":[\"ZZ\",\"base58\"],\
             \"owner\":\"{}\"}}}}",
            Pubkey::new_from_array(address),
            Pubkey::new_from_array(PROGRAM_ID)
        );
        let refusal = SealAccountDumpV1::parse(&hostile, None).expect_err("must refuse");
        assert!(refusal.contains("base58"), "{refusal}");
    }
}
