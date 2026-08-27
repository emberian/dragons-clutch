#[test]
fn replayable_settlement_campaign_never_resolves_or_pays_twice() {
    let mut digest = 0x6a09_e667_f3bc_c909u64;
    let mut cases = 0u64;
    // The fixture reserves seven of its 100 cash atoms, so 93 is the largest
    // split this state admits. This sweep includes that exact boundary.
    for quantity in 1..=93u64 {
        let f = fixture();
        let split = split_state(&f, quantity);
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let resolution_evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        let resolve = resolve_request(1, 1);
        let resolved = apply_with_evidence(
            &resolve,
            state_bytes(&split),
            &resolution_evidence,
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let independently_replayed = apply_with_evidence(
            &resolve,
            state_bytes(&split),
            &resolution_evidence,
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(resolved, independently_replayed);
        assert_eq!(
            apply_with_evidence(
                &resolve,
                state_bytes(&resolved),
                &resolution_evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Replay)
        );

        let resolution_bytes = resolved.resolution.as_ref().unwrap();
        let recorded_evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: resolution_bytes,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(2, 1),
                state_bytes(&resolved),
                &recorded_evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Resolution(ResolutionRefusal::MarketNotActive))
        );

        let mut readonly = f.evidence_metadata;
        readonly.resolution.writable = false;
        let redemption_evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: resolution_bytes,
                window: &[],
            },
            metadata: readonly,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        let redeem = redeem_request(2, 1, quantity);
        let redeemed = apply_with_evidence(
            &redeem,
            state_bytes(&resolved),
            &redemption_evidence,
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let independently_redeemed = apply_with_evidence(
            &redeem,
            state_bytes(&resolved),
            &redemption_evidence,
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(redeemed, independently_redeemed);
        assert_eq!(redeemed.redemption_payout, quantity);
        assert_eq!(
            HoardAccount::decode(&redeemed.hoard)
                .unwrap()
                .collateral_atoms,
            0
        );
        assert_eq!(
            PositionAccount::decode(&redeemed.position)
                .unwrap()
                .cash_atoms,
            100
        );
        assert_eq!(
            apply_with_evidence(
                &redeem,
                state_bytes(&redeemed),
                &redemption_evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Replay)
        );
        assert_eq!(
            apply_with_evidence(
                &redeem_request(3, 1, quantity),
                state_bytes(&redeemed),
                &redemption_evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Kernel(KernelError::InsufficientBalance))
        );

        digest ^= quantity.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        digest = digest.rotate_left(17).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        digest ^= HoardAccount::decode(&redeemed.hoard)
            .unwrap()
            .collateral_atoms;
        digest ^= ReplayAccount::decode(&redeemed.replay).unwrap().sequence;
        cases += 6;
    }
    std::println!(
        "campaign=reference-settlement seed=range-1..93 cases={cases} digest={digest:016x}"
    );
}
