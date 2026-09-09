//! Portable user-intent binding for canonical recurring-Series actions.
//!
//! The lifecycle planner still selects the action and all request bytes. This
//! adapter only refuses a changed user target or consequence before a wallet
//! is asked to sign. It has no SBF dependency, clock rule, amount, or alias map.

use dclutch_core_contract::ContentId;
use dclutch_trading::series::request::{SeriesActionRequestV3, SeriesActionV3};

/// User-approved immutable target and consequence. Optimistic revisions and
/// proof bytes remain owned by the live native planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesOperationIntentV1 {
    /// Exact controller root selected by the user.
    pub root: [u8; 32],
    /// Requested consequence, never an instruction to override the planner.
    pub action: SeriesActionV3,
    /// Exact selected Template.
    pub template: ContentId,
    /// Exact occurrence for Prepare/Consume/Expire; absent for terminal acts.
    pub occurrence: Option<ContentId>,
    /// Exact Ticket except when closing the root.
    pub ticket: Option<ContentId>,
}

/// Located refusal from matching user intent to a native plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesIntentErrorV1 {
    /// A root was zero or differed from the authenticated physical root.
    Root,
    /// The live planner selected a different action.
    Action,
    /// The selected immutable Template differed.
    Template,
    /// The occurrence changed, including presence versus absence.
    Occurrence,
    /// The Ticket changed, including presence versus absence.
    Ticket,
}

impl SeriesOperationIntentV1 {
    /// Bind an explicitly accepted native request to its authenticated root.
    pub fn from_request(
        root: [u8; 32],
        request: SeriesActionRequestV3<'_>,
    ) -> Result<Self, SeriesIntentErrorV1> {
        let intent = Self {
            root,
            action: request.action(),
            template: request.template(),
            occurrence: request.occurrence(),
            ticket: request.ticket(),
        };
        intent.require_matches(root, request)?;
        Ok(intent)
    }

    /// Check the fresh native result without changing its request or action.
    pub fn require_matches(
        self,
        observed_root: [u8; 32],
        request: SeriesActionRequestV3<'_>,
    ) -> Result<(), SeriesIntentErrorV1> {
        if self.root == [0; 32] || self.root != observed_root {
            return Err(SeriesIntentErrorV1::Root);
        }
        if self.action != request.action() {
            return Err(SeriesIntentErrorV1::Action);
        }
        if self.template != request.template() {
            return Err(SeriesIntentErrorV1::Template);
        }
        if self.occurrence != request.occurrence() {
            return Err(SeriesIntentErrorV1::Occurrence);
        }
        if self.ticket != request.ticket() {
            return Err(SeriesIntentErrorV1::Ticket);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_trading::series::request::encode_series_action_header_v3;

    #[test]
    fn series_intent_refuses_retargeting_and_changed_consequence() {
        let id = |n| ContentId::new([n; 32]).expect("nonzero test id");
        let bytes = encode_series_action_header_v3(
            SeriesActionV3::Consume,
            id(1),
            Some(id(2)),
            Some(id(3)),
            2,
            0,
            0,
        )
        .expect("native request");
        let request = SeriesActionRequestV3::decode(&bytes).expect("native decode");
        let root = [4; 32];
        let honest = SeriesOperationIntentV1::from_request(root, request).expect("bound intent");
        assert_eq!(honest.require_matches(root, request), Ok(()));
        assert_eq!(
            honest.require_matches([5; 32], request),
            Err(SeriesIntentErrorV1::Root)
        );
        for (changed, expected) in [
            (
                SeriesOperationIntentV1 {
                    action: SeriesActionV3::Expire,
                    ..honest
                },
                SeriesIntentErrorV1::Action,
            ),
            (
                SeriesOperationIntentV1 {
                    template: id(5),
                    ..honest
                },
                SeriesIntentErrorV1::Template,
            ),
            (
                SeriesOperationIntentV1 {
                    occurrence: Some(id(5)),
                    ..honest
                },
                SeriesIntentErrorV1::Occurrence,
            ),
            (
                SeriesOperationIntentV1 {
                    ticket: Some(id(5)),
                    ..honest
                },
                SeriesIntentErrorV1::Ticket,
            ),
        ] {
            assert_eq!(changed.require_matches(root, request), Err(expected));
        }
        assert_eq!(
            SeriesOperationIntentV1::from_request([0; 32], request),
            Err(SeriesIntentErrorV1::Root)
        );
    }
}
