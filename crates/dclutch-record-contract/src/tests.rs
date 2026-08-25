extern crate std;

use std::vec;

use super::*;

const GOOD_CONTENT: [u8; 8] = [0xA5, 1, 2, 0, 0, 0, 0, 0];

fn schema(value: u8) -> SchemaReleaseId {
    SchemaReleaseId::new([value; ID_BYTES]).expect("nonzero schema")
}

fn digest(value: u8) -> ContentDigest {
    ContentDigest::new([value; ID_BYTES]).expect("nonzero digest")
}

fn account(value: u8) -> AccountId {
    AccountId::new([value; ACCOUNT_ID_BYTES]).expect("nonzero account")
}

fn key() -> RecordKeyV1 {
    RecordKeyV1::new(schema(1), digest(2))
}

fn envelope() -> PageEnvelopeV1 {
    PageEnvelopeV1::new(PageEnvelopeKindV1::Provisional, 3, schema(6)).expect("valid envelope")
}

fn liveness_policy() -> StagingLivenessPolicyV1 {
    StagingLivenessPolicyV1::new(schema(5), 100, 10).expect("valid liveness policy")
}

#[test]
fn canonical_deployment_profile_derives_and_refuses_noncanonical_begin_coordinates() {
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    assert_eq!(profile.page_bytes(), CANONICAL_RECORD_PAGE_BYTES_V1);
    assert_eq!(
        profile.maximum_staging_lifetime_slots(),
        CANONICAL_RECORD_MAX_STAGING_LIFETIME_SLOTS_V1
    );

    let envelope = profile.page_envelope().expect("canonical envelope");
    assert!(profile.validates_page_envelope(envelope));
    let wrong_page_bytes = PageEnvelopeV1::new(
        envelope.kind(),
        envelope.page_bytes() - 1,
        envelope.basis_id(),
    )
    .expect("hostile width remains structurally valid");
    assert!(!profile.validates_page_envelope(wrong_page_bytes));
    let wrong_page_release = PageEnvelopeV1::new(envelope.kind(), envelope.page_bytes(), schema(9))
        .expect("hostile release remains structurally valid");
    assert!(!profile.validates_page_envelope(wrong_page_release));

    let liveness = profile
        .staging_liveness_policy(10)
        .expect("canonical liveness");
    assert!(profile.validates_staging_liveness_policy(liveness, 10));
    let wrong_lifetime = StagingLivenessPolicyV1::new(
        liveness.policy_id(),
        liveness.maximum_lifetime_slots() - 1,
        10,
    )
    .expect("hostile lifetime remains structurally valid");
    assert!(!profile.validates_staging_liveness_policy(wrong_lifetime, 10));
    let wrong_liveness_release =
        StagingLivenessPolicyV1::new(schema(10), liveness.maximum_lifetime_slots(), 10)
            .expect("hostile release remains structurally valid");
    assert!(!profile.validates_staging_liveness_policy(wrong_liveness_release, 10));
    assert!(!profile.validates_staging_liveness_policy(liveness, 11));
    assert_eq!(
        profile.staging_liveness_policy(0),
        Err(Error::InsufficientCleanupBounty)
    );
}

#[derive(Clone, Copy)]
struct TestAdapter {
    staging_vacant: bool,
}

impl RecordAdapterV1 for TestAdapter {
    fn validate_page_envelope(&self, envelope: &PageEnvelopeV1) -> bool {
        envelope.basis_id() == schema(6) || envelope.basis_id() == schema(11)
    }

    fn validate_staging_liveness_policy(&self, policy: &StagingLivenessPolicyV1) -> bool {
        policy.policy_id() == schema(5)
            && policy.maximum_lifetime_slots() > 0
            && policy.minimum_cleanup_bounty_lamports() == 10
    }

    fn validate_canonical_addresses(&self, obligation: &AddressDerivationObligationV1) -> bool {
        obligation.key() == key()
            && obligation.raw_record_account() == account(7)
            && obligation.staging_account() == account(8)
    }

    fn validate_raw_record(&self, obligation: &RawRecordValidationObligationV1<'_>) -> bool {
        let lifecycle_valid = match obligation.mode() {
            RawRecordValidationModeV1::Finalization => !self.staging_vacant,
            RawRecordValidationModeV1::ConsumerAuthentication => self.staging_vacant,
        };
        lifecycle_valid
            && obligation.key() == key()
            && obligation.raw_record_account() == account(7)
            && obligation.staging_account() == account(8)
            && obligation.exact_content() == GOOD_CONTENT
    }
}

fn begin() -> StagingCursorV1 {
    let transition = prepare_begin_v1(
        &TestAdapter {
            staging_vacant: false,
        },
        BeginRecordV1::new(key(), 8, envelope(), schema(5), 100, 10).expect("begin request"),
        liveness_policy(),
        50,
        account(7),
        account(8),
        account(9),
    )
    .expect("begin transition");
    assert_eq!(transition.allocation().raw_data_length(), 8);
    assert_eq!(
        transition.allocation().staging_data_length(),
        u64::try_from(STAGING_CURSOR_BYTES_V1).expect("cursor width")
    );
    transition.cursor()
}

fn append<'page>(
    cursor: StagingCursorV1,
    index: u64,
    offset: u64,
    page: &'page [u8],
) -> Result<AppendTransitionV1<'page>> {
    prepare_append_page_v1(
        cursor,
        account(7),
        account(8),
        8,
        AppendPageV1::new(index, offset, page).expect("bounded test page"),
    )
}

fn apply_append(
    raw: &mut [u8],
    cursor: StagingCursorV1,
    request: AppendPageV1<'_>,
) -> Result<StagingCursorV1> {
    let transition = prepare_append_page_v1(cursor, account(7), account(8), 8, request)?;
    let start = usize::try_from(transition.write().offset()).map_err(|_| Error::InvalidLength)?;
    let end = start
        .checked_add(transition.write().page().len())
        .ok_or(Error::ArithmeticOverflow)?;
    raw.get_mut(start..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(transition.write().page());
    Ok(transition.next_cursor())
}

fn complete(raw: &mut [u8]) -> StagingCursorV1 {
    let cursor = begin();
    let cursor = apply_append(
        raw,
        cursor,
        AppendPageV1::new(0, 0, &GOOD_CONTENT[..3]).expect("page zero"),
    )
    .expect("append zero");
    let cursor = apply_append(
        raw,
        cursor,
        AppendPageV1::new(1, 3, &GOOD_CONTENT[3..6]).expect("page one"),
    )
    .expect("append one");
    apply_append(
        raw,
        cursor,
        AppendPageV1::new(2, 6, &GOOD_CONTENT[6..]).expect("page two"),
    )
    .expect("append two")
}

#[test]
fn exact_codecs_round_trip_and_reject_hostile_headers() {
    let request = BeginRecordV1::new(key(), 8, envelope(), schema(5), 100, 10).expect("request");
    assert_eq!(
        BeginRecordV1::decode(&request.to_bytes()).expect("decode"),
        request
    );

    let append = AppendPageV1::new(4, 12, &[9, 8, 7]).expect("append");
    let mut bytes = vec![0; append.encoded_len().expect("encoded length")];
    append.encode(&mut bytes).expect("encode append");
    let decoded = AppendPageV1::decode(&bytes).expect("decode append");
    assert_eq!(decoded.page_index(), 4);
    assert_eq!(decoded.offset(), 12);
    assert_eq!(decoded.page(), &[9, 8, 7]);

    assert_eq!(
        FinalizeRecordV1::decode(&FinalizeRecordV1.to_bytes()),
        Ok(FinalizeRecordV1)
    );
    assert_eq!(
        AbortRecordV1::decode(&AbortRecordV1.to_bytes()),
        Ok(AbortRecordV1)
    );

    let mut hostile = request.to_bytes();
    *hostile.get_mut(12).expect("reserved byte") = 1;
    assert_eq!(
        BeginRecordV1::decode(&hostile),
        Err(Error::NonCanonicalReservedBytes)
    );
    let truncated = bytes.get(..bytes.len() - 1).expect("nonempty append");
    assert_eq!(AppendPageV1::decode(truncated), Err(Error::InvalidLength));
}

#[test]
fn cursor_encoding_rechecks_progress_geometry() {
    let cursor = begin();
    let bytes = cursor.to_bytes();
    assert_eq!(
        StagingCursorV1::decode(&bytes).expect("decode cursor"),
        cursor
    );
    assert_eq!(cursor.page_count(), 3);
    assert_eq!(cursor.next_page(), 0);
    assert_eq!(cursor.next_offset(), 0);

    let mut poisoned = bytes;
    let bogus = 2_u64.to_le_bytes();
    let range = CURSOR_NEXT_OFFSET_OFFSET..CURSOR_NEXT_OFFSET_OFFSET + bogus.len();
    poisoned
        .get_mut(range)
        .expect("next offset field")
        .copy_from_slice(&bogus);
    assert_eq!(
        StagingCursorV1::decode(&poisoned),
        Err(Error::GeometryMismatch)
    );
}

#[test]
fn reorder_replay_overlap_and_gap_are_disjoint_refusals() {
    let cursor = begin();
    assert_eq!(append(cursor, 1, 3, &[0, 0, 0]), Err(Error::PageOutOfOrder));

    let cursor = append(cursor, 0, 0, &GOOD_CONTENT[..3])
        .expect("first page")
        .next_cursor();
    assert_eq!(append(cursor, 0, 0, &[1, 2, 3]), Err(Error::PageReplay));
    assert_eq!(append(cursor, 1, 2, &[0, 0, 0]), Err(Error::PageOverlap));
    assert_eq!(append(cursor, 1, 4, &[0, 0, 0]), Err(Error::PageGap));
}

#[test]
fn final_page_has_exact_short_width_and_no_gap() {
    let cursor = begin();
    let cursor = append(cursor, 0, 0, &GOOD_CONTENT[..3])
        .expect("page zero")
        .next_cursor();
    let cursor = append(cursor, 1, 3, &GOOD_CONTENT[3..6])
        .expect("page one")
        .next_cursor();
    assert_eq!(
        append(cursor, 2, 6, &[0, 0, 0]),
        Err(Error::PageLengthMismatch)
    );
    let complete = append(cursor, 2, 6, &GOOD_CONTENT[6..])
        .expect("short final page")
        .next_cursor();
    assert!(complete.is_complete());
    assert_eq!(append(complete, 3, 8, &[]), Err(Error::CursorComplete));
}

#[test]
fn incomplete_even_with_semantically_zero_suffix_cannot_authenticate() {
    let mut raw = [0; GOOD_CONTENT.len()];
    let cursor = begin();
    let cursor = apply_append(
        &mut raw,
        cursor,
        AppendPageV1::new(0, 0, &GOOD_CONTENT[..3]).expect("first page"),
    )
    .expect("append first page");
    assert_eq!(raw, GOOD_CONTENT);
    assert!(!cursor.is_complete());
    assert_eq!(
        prepare_finalize_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            cursor,
            account(7),
            account(8),
            50,
            &raw,
        ),
        Err(Error::CursorIncomplete)
    );
    assert_eq!(
        authenticate_finalized_raw_record_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            key(),
            account(7),
            account(8),
            &raw,
        ),
        Err(Error::AdapterValidationRefused)
    );
}

#[test]
fn poisoned_wrong_digest_schema_and_length_refuse_finalize() {
    let mut raw = [0; GOOD_CONTENT.len()];
    let cursor = complete(&mut raw);

    let mut poisoned = raw;
    *poisoned.get_mut(1).expect("semantic byte") = 99;
    assert_eq!(
        prepare_finalize_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            cursor,
            account(7),
            account(8),
            50,
            &poisoned,
        ),
        Err(Error::AdapterValidationRefused)
    );

    let wrong_digest_request = BeginRecordV1::new(
        RecordKeyV1::new(schema(1), digest(3)),
        8,
        envelope(),
        schema(5),
        100,
        10,
    )
    .expect("wrong digest request");
    assert_eq!(
        prepare_begin_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            wrong_digest_request,
            liveness_policy(),
            50,
            account(7),
            account(8),
            account(9),
        ),
        Err(Error::AddressDerivationRefused)
    );

    let wrong_schema_request = BeginRecordV1::new(
        RecordKeyV1::new(schema(4), digest(2)),
        8,
        envelope(),
        schema(5),
        100,
        10,
    )
    .expect("wrong schema request");
    assert_eq!(
        prepare_begin_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            wrong_schema_request,
            liveness_policy(),
            50,
            account(7),
            account(8),
            account(9),
        ),
        Err(Error::AddressDerivationRefused)
    );

    assert_eq!(
        prepare_finalize_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            cursor,
            account(7),
            account(8),
            50,
            raw.get(..7).expect("short content"),
        ),
        Err(Error::CursorBindingMismatch)
    );
}

#[test]
fn alias_and_unauthorized_abort_refuse_without_refund_redirection() {
    let request = BeginRecordV1::new(key(), 8, envelope(), schema(5), 100, 10).expect("request");
    assert_eq!(
        prepare_begin_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            request,
            liveness_policy(),
            50,
            account(7),
            account(7),
            account(9),
        ),
        Err(Error::AccountAlias)
    );

    let cursor = begin();
    assert_eq!(
        prepare_abort_v1(
            cursor,
            AbortObservationV1::new(account(7), account(8), 8, 100, 50, 99, account(10)),
        ),
        Err(Error::AbortBeforeExpiry)
    );
    let abort = prepare_abort_v1(
        cursor,
        AbortObservationV1::new(account(7), account(8), 8, 100, 50, 99, account(9)),
    )
    .expect("abort");
    assert!(abort.sponsor_signature_required());
    assert_eq!(abort.raw_record_close().account(), account(7));
    assert_eq!(abort.raw_record_close().full_lamport_refund(), account(9));
    assert_eq!(abort.raw_record_close().observed_lamports(), 100);
    assert_eq!(abort.staging_close().account(), account(8));
    assert_eq!(abort.staging_close().sponsor_recipient(), account(9));
    assert_eq!(abort.staging_close().sponsor_refund_lamports(), 50);
    assert_eq!(abort.staging_close().cleanup_bounty_lamports(), 0);
}

#[test]
fn poison_and_abandon_enables_bounty_only_permissionless_cleanup() {
    let mut raw = [0; GOOD_CONTENT.len()];
    let cursor = begin();
    let poison = [0xA5, 99, 2];
    let cursor = apply_append(
        &mut raw,
        cursor,
        AppendPageV1::new(0, 0, &poison).expect("poison page"),
    )
    .expect("poison may stage but cannot finalize");

    let cleanup = prepare_abort_v1(
        cursor,
        AbortObservationV1::new(account(7), account(8), 8, 100, 50, 100, account(10)),
    )
    .expect("permissionless cleanup at expiry");
    assert!(!cleanup.sponsor_signature_required());
    assert_eq!(cleanup.raw_record_close().observed_lamports(), 100);
    assert_eq!(cleanup.raw_record_close().full_lamport_refund(), account(9));
    assert_eq!(cleanup.staging_close().cleanup_recipient(), account(10));
    assert_eq!(cleanup.staging_close().cleanup_bounty_lamports(), 10);
    assert_eq!(cleanup.staging_close().sponsor_recipient(), account(9));
    assert_eq!(cleanup.staging_close().sponsor_refund_lamports(), 40);
    cleanup
        .staging_close()
        .validate_conservation()
        .expect("staging balance conserved");
    assert_eq!(
        cleanup
            .raw_record_close()
            .observed_lamports()
            .checked_add(cleanup.staging_close().observed_lamports()),
        Some(150)
    );

    assert_eq!(
        prepare_abort_v1(
            cursor,
            AbortObservationV1::new(account(7), account(8), 8, 100, 9, 100, account(10)),
        ),
        Err(Error::InsufficientCleanupBounty)
    );
}

#[test]
fn begin_binds_policy_expiry_and_repeated_squats_prepaid_bounty() {
    let request =
        BeginRecordV1::new(key(), 8, envelope(), schema(5), 151, 10).expect("begin request");
    assert_eq!(
        prepare_begin_v1(
            &TestAdapter {
                staging_vacant: false,
            },
            request,
            liveness_policy(),
            50,
            account(7),
            account(8),
            account(9),
        ),
        Err(Error::InvalidExpiry)
    );

    let admitted = prepare_begin_v1(
        &TestAdapter {
            staging_vacant: false,
        },
        BeginRecordV1::new(key(), 8, envelope(), schema(5), 100, 10).expect("admitted request"),
        liveness_policy(),
        50,
        account(7),
        account(8),
        account(9),
    )
    .expect("admitted begin");
    assert_eq!(admitted.allocation().cleanup_bounty_lamports(), 10);
    assert_eq!(admitted.cursor().expiry_slot(), 100);
    assert_eq!(admitted.cursor().cleanup_bounty_lamports(), 10);
}

#[test]
fn finalize_closes_cursor_and_later_consumer_reauthenticates() {
    let mut raw = [0; GOOD_CONTENT.len()];
    let cursor = complete(&mut raw);
    let finalization = prepare_finalize_v1(
        &TestAdapter {
            staging_vacant: false,
        },
        cursor,
        account(7),
        account(8),
        50,
        &raw,
    )
    .expect("finalization");
    assert_eq!(
        finalization.authenticated_record().exact_content(),
        GOOD_CONTENT
    );
    assert_eq!(finalization.staging_close().account(), account(8));
    assert_eq!(
        finalization.staging_close().full_lamport_refund(),
        account(9)
    );

    let closed_cursor_data: [u8; 0] = [];
    assert_eq!(
        StagingCursorV1::decode(&closed_cursor_data),
        Err(Error::InvalidLength)
    );
    let later = authenticate_finalized_raw_record_v1(
        &TestAdapter {
            staging_vacant: true,
        },
        key(),
        account(7),
        account(8),
        &raw,
    )
    .expect("later consumer authentication");
    assert_eq!(later.key(), key());
}

#[test]
fn abort_after_finalize_has_no_live_cursor_to_authorize_it() {
    let mut raw = [0; GOOD_CONTENT.len()];
    let cursor = complete(&mut raw);
    prepare_finalize_v1(
        &TestAdapter {
            staging_vacant: false,
        },
        cursor,
        account(7),
        account(8),
        50,
        &raw,
    )
    .expect("finalization");

    let cursor_after_close = StagingCursorV1::decode(&[]);
    assert_eq!(cursor_after_close, Err(Error::InvalidLength));
}

#[test]
fn failed_append_has_no_partial_state_mutation() {
    let mut raw = vec![0; GOOD_CONTENT.len()];
    let cursor = begin();
    let raw_before = raw.clone();
    let cursor_before = cursor;
    let result = apply_append(
        &mut raw,
        cursor,
        AppendPageV1::new(0, 0, &[1, 2]).expect("short page"),
    );
    assert_eq!(result, Err(Error::PageLengthMismatch));
    assert_eq!(raw, raw_before);
    assert_eq!(cursor, cursor_before);
}

#[test]
fn u64_geometry_has_no_fixed_total_artifact_cap() {
    let envelope = PageEnvelopeV1::new(PageEnvelopeKindV1::Measured, u32::MAX, schema(11))
        .expect("wide measured page");
    let request = BeginRecordV1::new(key(), u64::MAX, envelope, schema(5), u64::MAX, 10)
        .expect("checked geometry");
    let unbounded_policy =
        StagingLivenessPolicyV1::new(schema(5), u64::MAX, 10).expect("wide policy");
    let transition = prepare_begin_v1(
        &TestAdapter {
            staging_vacant: false,
        },
        request,
        unbounded_policy,
        0,
        account(7),
        account(8),
        account(9),
    )
    .expect("begin has no total cap");
    assert!(transition.cursor().page_count() > u64::from(u32::MAX));
}
