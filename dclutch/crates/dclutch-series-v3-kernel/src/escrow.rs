//! Stateless SeriesEscrow effect planning.
//!
//! These values are semantic expectations, not a Custody wire or state
//! machine. The current Registry-selected Trading interpreter must project
//! them into the canonical Custody request, and Custody alone authenticates
//! accounts, persists replay, moves tokens, and returns receipts.

use crate::{AccountKeyV3, PrefoundingSeriesEscrowV3};

/// One exact edge in the pre-founding SeriesEscrow lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesEscrowEffectKindV3 {
    /// Bind a fresh Custody replay context to this Ticket and refund owner.
    InitializeReplay,
    /// Create the canonical empty SeriesEscrow vault for the future Market.
    OpenEscrowVault,
    /// Lock exact founder collateral into the canonical SeriesEscrow.
    Lock,
    /// After the retry deadline, return SeriesEscrow collateral to refund owner.
    RefundExpired,
    /// Close the now-empty canonical SeriesEscrow vault and return its rent.
    CloseEscrowVault,
    /// Close the terminal Custody replay account and return its rent.
    CloseReplay,
}

/// One semantic effect derived entirely from authenticated Series content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesEscrowEffectV3 {
    kind: SeriesEscrowEffectKindV3,
    escrow: PrefoundingSeriesEscrowV3,
}

impl SeriesEscrowEffectV3 {
    /// Exact semantic effect kind.
    pub const fn kind(self) -> SeriesEscrowEffectKindV3 {
        self.kind
    }

    /// Complete content-authenticated SeriesEscrow projection.
    pub const fn escrow(self) -> PrefoundingSeriesEscrowV3 {
        self.escrow
    }

    /// Required replay revision before this effect.
    pub const fn expected_revision(self) -> u64 {
        match self.kind {
            SeriesEscrowEffectKindV3::InitializeReplay => 0,
            SeriesEscrowEffectKindV3::OpenEscrowVault => 1,
            SeriesEscrowEffectKindV3::Lock => 2,
            SeriesEscrowEffectKindV3::RefundExpired => 3,
            SeriesEscrowEffectKindV3::CloseEscrowVault => 4,
            SeriesEscrowEffectKindV3::CloseReplay => 5,
        }
    }

    /// Required replay revision after this effect.
    pub const fn resulting_revision(self) -> u64 {
        self.expected_revision() + 1
    }

    /// Exact Realm-collateral atoms moved; zero only for replay initialization.
    pub const fn amount(self) -> u64 {
        match self.kind {
            SeriesEscrowEffectKindV3::InitializeReplay
            | SeriesEscrowEffectKindV3::OpenEscrowVault
            | SeriesEscrowEffectKindV3::CloseEscrowVault
            | SeriesEscrowEffectKindV3::CloseReplay => 0,
            SeriesEscrowEffectKindV3::Lock | SeriesEscrowEffectKindV3::RefundExpired => {
                self.escrow.hoard_principal()
            }
        }
    }

    /// External token owner required on the source side, when any.
    pub const fn external_source_owner(self) -> Option<AccountKeyV3> {
        match self.kind {
            SeriesEscrowEffectKindV3::Lock => Some(self.escrow.founder()),
            SeriesEscrowEffectKindV3::InitializeReplay
            | SeriesEscrowEffectKindV3::OpenEscrowVault
            | SeriesEscrowEffectKindV3::RefundExpired
            | SeriesEscrowEffectKindV3::CloseEscrowVault
            | SeriesEscrowEffectKindV3::CloseReplay => None,
        }
    }

    /// External token owner required on the destination side, when any.
    pub const fn external_destination_owner(self) -> Option<AccountKeyV3> {
        match self.kind {
            SeriesEscrowEffectKindV3::RefundExpired => Some(self.escrow.refund_owner()),
            SeriesEscrowEffectKindV3::InitializeReplay
            | SeriesEscrowEffectKindV3::OpenEscrowVault
            | SeriesEscrowEffectKindV3::Lock
            | SeriesEscrowEffectKindV3::CloseEscrowVault
            | SeriesEscrowEffectKindV3::CloseReplay => None,
        }
    }

    /// Whether the canonical SeriesEscrow is the token source.
    pub const fn series_escrow_is_source(self) -> bool {
        matches!(self.kind, SeriesEscrowEffectKindV3::RefundExpired)
    }

    /// Whether the canonical SeriesEscrow is the token destination.
    pub const fn series_escrow_is_destination(self) -> bool {
        matches!(self.kind, SeriesEscrowEffectKindV3::Lock)
    }
}

/// Exact three-effect Prepare plan: initialize replay, open vault, then lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrepareSeriesEscrowPlanV3 {
    effects: [SeriesEscrowEffectV3; 3],
}

impl PrepareSeriesEscrowPlanV3 {
    /// Canonical ordered effects.
    pub const fn effects(self) -> [SeriesEscrowEffectV3; 3] {
        self.effects
    }
}

/// Exact atomic projected-Hoard credit and source cleanup expected on Consume.
///
/// Canonical projected Custody owns one `LockHoardAndCloseSource` transition:
/// it debits the complete SeriesEscrow balance, credits the projected Hoard,
/// and closes the now-empty normal source Vault and replay to RentCredit.  A
/// three-request normal-Custody cleanup is therefore not a valid Consume plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumeSeriesEscrowPlanV3 {
    escrow: PrefoundingSeriesEscrowV3,
}

impl ConsumeSeriesEscrowPlanV3 {
    /// Complete immutable SeriesEscrow/future-Market projection.
    pub const fn escrow(self) -> PrefoundingSeriesEscrowV3 {
        self.escrow
    }

    /// Exact normal source-replay revision consumed and closed atomically.
    pub const fn source_replay_revision(self) -> u64 {
        3
    }

    /// Exact Realm-collateral atoms credited to the projected Hoard.
    pub const fn amount(self) -> u64 {
        self.escrow.hoard_principal()
    }
}

/// Exact terminal refund and cleanup sequence after Expire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSeriesEscrowPlanV3 {
    effects: [SeriesEscrowEffectV3; 3],
}

impl TerminalSeriesEscrowPlanV3 {
    /// Canonical ordered transfer, empty-vault close, and replay close effects.
    pub const fn effects(self) -> [SeriesEscrowEffectV3; 3] {
        self.effects
    }
}

/// Construct the canonical pre-founding replay initialization and lock plan.
pub const fn prepare_series_escrow_v3(
    escrow: PrefoundingSeriesEscrowV3,
) -> PrepareSeriesEscrowPlanV3 {
    PrepareSeriesEscrowPlanV3 {
        effects: [
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::InitializeReplay,
                escrow,
            },
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::OpenEscrowVault,
                escrow,
            },
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::Lock,
                escrow,
            },
        ],
    }
}

/// Construct the exact pre-Found projected-Hoard credit and atomic source close.
pub const fn consume_series_escrow_v3(
    escrow: PrefoundingSeriesEscrowV3,
) -> ConsumeSeriesEscrowPlanV3 {
    ConsumeSeriesEscrowPlanV3 { escrow }
}

/// Construct the exact post-deadline refund and terminal cleanup plan.
pub const fn expire_series_escrow_v3(
    escrow: PrefoundingSeriesEscrowV3,
) -> TerminalSeriesEscrowPlanV3 {
    TerminalSeriesEscrowPlanV3 {
        effects: [
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::RefundExpired,
                escrow,
            },
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::CloseEscrowVault,
                escrow,
            },
            SeriesEscrowEffectV3 {
                kind: SeriesEscrowEffectKindV3::CloseReplay,
                escrow,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;

    use super::*;
    use crate::{
        AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_BYTES_V3, SERIES_TEMPLATE_BYTES_V3,
        SERIES_TICKET_BYTES_V3, admit_occurrence, admit_ticket, generated, occurrence_content_id,
        pre_founding_series_escrow, template_content_id,
    };
    use sha2::{Digest, Sha256};

    const HASH_SEPARATOR: [u8; 1] = [0];

    fn key(byte: u8) -> AccountKeyV3 {
        AccountKeyV3::new([byte; 32]).expect("nonzero key")
    }

    fn put<const N: usize>(target: &mut [u8], offset: usize, value: &[u8; N]) {
        target
            .get_mut(offset..offset + N)
            .expect("fixture field")
            .copy_from_slice(value);
    }

    fn node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(generated::SERIES_PROJECTION_NODE_DOMAIN_V3);
        hasher.update(HASH_SEPARATOR);
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    fn projection() -> PrefoundingSeriesEscrowV3 {
        let mut template: [u8; SERIES_TEMPLATE_BYTES_V3] = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3] = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket: [u8; SERIES_TICKET_BYTES_V3] = generated::SERIES_EXAMPLE_TICKET_V3;
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let siblings = [[90; 32], [91; 32]];
        let root = node(&node(&siblings[0], &occurrence_id.to_bytes()), &siblings[1]);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("Template ID");
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        let admitted = admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
        let product = AuthenticatedProductProjectionV2::new(
            admitted.occurrence().product_record(),
            ContentId::new([61; 32]).expect("stable Product"),
            ContentId::new([62; 32]).expect("result domain"),
        );
        pre_founding_series_escrow(
            admitted,
            admit_ticket(&ticket).expect("Ticket"),
            product,
            key(59),
        )
        .expect("SeriesEscrow projection")
    }

    #[test]
    fn prepare_and_mutually_exclusive_terminals_share_one_replay_edge() {
        let escrow = projection();
        let prepare = prepare_series_escrow_v3(escrow).effects();
        assert_eq!(
            prepare[0].kind(),
            SeriesEscrowEffectKindV3::InitializeReplay
        );
        assert_eq!(prepare[0].expected_revision(), 0);
        assert_eq!(prepare[0].resulting_revision(), 1);
        assert_eq!(prepare[0].amount(), 0);
        assert_eq!(prepare[1].kind(), SeriesEscrowEffectKindV3::OpenEscrowVault);
        assert_eq!(prepare[1].expected_revision(), 1);
        assert_eq!(prepare[1].resulting_revision(), 2);
        assert_eq!(prepare[1].amount(), 0);
        assert_eq!(prepare[2].kind(), SeriesEscrowEffectKindV3::Lock);
        assert_eq!(prepare[2].expected_revision(), 2);
        assert_eq!(prepare[2].resulting_revision(), 3);
        assert_eq!(prepare[2].amount(), escrow.hoard_principal());
        assert_eq!(prepare[2].external_source_owner(), Some(escrow.founder()));
        assert!(prepare[2].series_escrow_is_destination());

        let consume = consume_series_escrow_v3(escrow);
        let expire = expire_series_escrow_v3(escrow).effects();
        let expire_transfer = expire[0];
        assert_eq!(consume.source_replay_revision(), 3);
        assert_eq!(expire_transfer.expected_revision(), 3);
        assert_eq!(expire_transfer.resulting_revision(), 4);
        assert_eq!(consume.amount(), expire_transfer.amount());
        assert_eq!(
            consume.escrow().ticket_id(),
            expire_transfer.escrow().ticket_id()
        );
        assert_eq!(
            expire_transfer.external_destination_owner(),
            Some(escrow.refund_owner())
        );
        assert_eq!(expire[1].kind(), SeriesEscrowEffectKindV3::CloseEscrowVault);
        assert_eq!(expire[1].expected_revision(), 4);
        assert_eq!(expire[1].resulting_revision(), 5);
        assert_eq!(expire[1].amount(), 0);
        assert_eq!(expire[2].kind(), SeriesEscrowEffectKindV3::CloseReplay);
        assert_eq!(expire[2].expected_revision(), 5);
        assert_eq!(expire[2].resulting_revision(), 6);
        assert_eq!(expire[2].amount(), 0);
    }
}
