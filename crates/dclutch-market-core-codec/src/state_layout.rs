// This module is included as a child of the Lean-generated implementation so
// the public view delegates to the generated private coordinate instead of
// restating it in a consumer.

use super::{CoreState, Error, STATE_IDENTITY_REALM_OFFSET};

/// Canonical patch/projection coordinates of [`CoreState`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreStateLayoutV2;

impl CoreStateLayoutV2 {
    /// Immutable Realm content identity selected by the Market.
    pub const REALM_ID: usize = STATE_IDENTITY_REALM_OFFSET;

    /// Hostile-decode state and copy its exact Realm identity atomically.
    pub fn copy_realm_id_into(input: &[u8], output: &mut [u8; 32]) -> Result<(), Error> {
        let state = CoreState::decode(input)?;
        *output = state.identity.realm_id.to_bytes();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Identity, MarketIdentity, Phase, Readiness, STATE_BYTES};

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    fn state() -> CoreState {
        CoreState {
            phase: Phase::Founding,
            readiness: Readiness::Prepaid,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: identity(1),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(6),
                selected_release_set: identity(7),
                registry_program: identity(8),
                generation: 9,
            },
            outstanding_capabilities: 10,
            rent_beneficiary: identity(11),
            terminal_receipt: None,
        }
    }

    #[test]
    fn public_realm_coordinate_tracks_encoder_and_round_trip() {
        let state = state();
        let bytes = state.encode().expect("state");
        assert_eq!(bytes.len(), STATE_BYTES);
        assert_eq!(
            bytes.get(CoreStateLayoutV2::REALM_ID..CoreStateLayoutV2::REALM_ID + 32),
            Some(state.identity.realm_id.to_bytes().as_slice())
        );
        assert_eq!(CoreState::decode(&bytes), Ok(state));
        let mut projected = [0x55; 32];
        CoreStateLayoutV2::copy_realm_id_into(&bytes, &mut projected).expect("projection");
        assert_eq!(projected, state.identity.realm_id.to_bytes());
    }

    #[test]
    fn hostile_realm_drift_refuses_without_output_mutation() {
        let mut bytes = state().encode().expect("state");
        bytes
            .get_mut(CoreStateLayoutV2::REALM_ID..CoreStateLayoutV2::REALM_ID + 32)
            .expect("Realm field")
            .fill(0);
        assert!(CoreState::decode(&bytes).is_err());
        let mut output = [0x77; 32];
        let before = output;
        assert!(CoreStateLayoutV2::copy_realm_id_into(&bytes, &mut output).is_err());
        assert_eq!(output, before);
    }
}
