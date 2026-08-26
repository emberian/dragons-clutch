//! Narrow public byte coordinates of immutable Realm transfer facts.
//!
//! Data-defined account profiles use this view to project only the Realm-owned
//! mint and token-program identities. Token-account parsing remains solely in
//! the Custody adapter.

use crate::{Error, REALM_COLLATERAL_MINT_OFFSET, REALM_TOKEN_PROGRAM_OFFSET, RealmV1};

/// Canonical byte coordinates of the immutable [`RealmV1`] transfer facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmLayoutV1;

impl RealmLayoutV1 {
    /// Realm-selected Token program identity.
    pub const TOKEN_PROGRAM: usize = REALM_TOKEN_PROGRAM_OFFSET;
    /// Realm-selected collateral Mint identity.
    pub const COLLATERAL_MINT: usize = REALM_COLLATERAL_MINT_OFFSET;

    /// Hostile-decode and atomically copy both immutable transfer identities.
    pub fn copy_transfer_identities_into(
        input: &[u8],
        token_program: &mut [u8; 32],
        collateral_mint: &mut [u8; 32],
    ) -> Result<(), Error> {
        let realm = RealmV1::decode(input)?;
        let token_candidate = *realm.token_program();
        let mint_candidate = *realm.collateral_mint();
        *token_program = token_candidate;
        *collateral_mint = mint_candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn realm() -> RealmV1 {
        RealmV1::new(RealmV1Input {
            token_program: id(1),
            collateral_mint: id(2),
            collateral_adapter_release_id: id(3),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm")
    }

    #[test]
    fn public_transfer_coordinates_track_encoder_and_round_trip() {
        let realm = realm();
        let bytes = realm.to_bytes();
        assert_eq!(
            bytes.get(RealmLayoutV1::TOKEN_PROGRAM..RealmLayoutV1::TOKEN_PROGRAM + 32),
            Some(realm.token_program().as_slice())
        );
        assert_eq!(
            bytes.get(RealmLayoutV1::COLLATERAL_MINT..RealmLayoutV1::COLLATERAL_MINT + 32),
            Some(realm.collateral_mint().as_slice())
        );
        assert_eq!(RealmV1::decode(&bytes), Ok(realm));

        let mut token_program = [0x55; 32];
        let mut collateral_mint = [0x66; 32];
        RealmLayoutV1::copy_transfer_identities_into(
            &bytes,
            &mut token_program,
            &mut collateral_mint,
        )
        .expect("projection");
        assert_eq!(token_program, *realm.token_program());
        assert_eq!(collateral_mint, *realm.collateral_mint());
    }

    #[test]
    fn hostile_transfer_identity_refuses_without_output_mutation() {
        let mut bytes = realm().to_bytes();
        bytes
            .get_mut(RealmLayoutV1::TOKEN_PROGRAM..RealmLayoutV1::TOKEN_PROGRAM + 32)
            .expect("Token program")
            .fill(0);
        assert!(RealmV1::decode(&bytes).is_err());
        let mut token_program = [0x55; 32];
        let mut collateral_mint = [0x66; 32];
        let token_before = token_program;
        let mint_before = collateral_mint;
        assert!(
            RealmLayoutV1::copy_transfer_identities_into(
                &bytes,
                &mut token_program,
                &mut collateral_mint,
            )
            .is_err()
        );
        assert_eq!(token_program, token_before);
        assert_eq!(collateral_mint, mint_before);
    }
}
