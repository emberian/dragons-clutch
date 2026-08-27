//! Adversarial coverage for the emitter's one job: a document the census parses.

use super::*;

fn logs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|line| (*line).to_string()).collect()
}

#[test]
fn a_log_line_carrying_json_metacharacters_still_parses() {
    // Runtime logs are the whole point of this document and they are not
    // sanitised anywhere upstream. A `Program log:` line can carry a quote, a
    // backslash, or a stray control byte; an emitter that concatenated them
    // raw would produce a document the census refuses on exactly the campaigns
    // most worth recording.
    let hostile = logs(&[
        "Program log: he said \"no\"",
        "Program log: path C:\\rollback\\state",
        "Program log: tab\there and newline\nthere",
        "Program log: bell\u{7}",
    ]);
    let rendered = render(&TransactionEvidence {
        label: "quote \" in label",
        signature: "5xSig",
        slot: 42,
        error: None,
        logs: &hostile,
        compute_units_consumed: Some(731_297),
        wire_bytes: None,
    });
    assert!(rendered.contains("\\\"no\\\""));
    assert!(rendered.contains("C:\\\\rollback\\\\state"));
    assert!(rendered.contains("tab\\there"));
    assert!(rendered.contains("newline\\nthere"));
    assert!(rendered.contains("\\u0007"));
    assert!(!rendered.contains("bell\u{7}"));
}

#[test]
fn success_and_refusal_are_distinguishable_by_the_only_field_that_decides_it() {
    // `error: null` is the ONLY thing that makes a route EXECUTED. A refusal
    // rendered as the empty string would read as success.
    let accepted = render(&TransactionEvidence {
        label: "consume the ticket and found its Market",
        signature: "5xAccepted",
        slot: 7,
        error: None,
        logs: &logs(&["Program E3M3 invoke [1]"]),
        compute_units_consumed: Some(651_601),
        wire_bytes: None,
    });
    assert!(accepted.contains("\"error\": null"));

    let refused = render(&TransactionEvidence {
        label: "consume refuses a substituted ProgramData",
        signature: "5xRefused",
        slot: 8,
        error: Some("TransactionError(InstructionError(0, Custom(11)))"),
        logs: &logs(&["Program E3M3 invoke [1]", "custom program error: 0xb"]),
        compute_units_consumed: Some(608_157),
        wire_bytes: None,
    });
    assert!(refused.contains("\"error\": \"TransactionError"));
    assert!(!refused.contains("\"error\": null"));
}

#[test]
fn an_empty_signature_is_refused_before_anything_is_written() {
    // The census dedups on (route, signature). Every transaction sharing an
    // empty signature collapses into one observation, so a campaign would
    // silently claim less coverage than it drove. Refuse, and write nothing.
    let directory = std::env::temp_dir().join("dclutch-evidence-empty-signature");
    let _ = fs::remove_dir_all(&directory);
    let refused = record_into(
        &directory,
        &TransactionEvidence {
            label: "labelled",
            signature: "",
            slot: 1,
            error: None,
            logs: &[],
            compute_units_consumed: None,
        wire_bytes: None,
        },
    );
    assert!(matches!(refused, Err(EvidenceError::EmptySignature)));
    assert!(!directory.exists(), "a refused record must not create the directory");

    let unlabelled = record_into(
        &directory,
        &TransactionEvidence {
            label: "",
            signature: "5xSig",
            slot: 1,
            error: None,
            logs: &[],
            compute_units_consumed: None,
        wire_bytes: None,
        },
    );
    assert!(matches!(unlabelled, Err(EvidenceError::EmptyLabel)));
    assert!(!directory.exists());

    record_into(
        &directory,
        &TransactionEvidence {
            label: "labelled",
            signature: "5xSig",
            slot: 1,
            error: None,
            logs: &[],
            compute_units_consumed: None,
        wire_bytes: None,
        },
    )
    .expect("a labelled, signed transaction records");
    assert!(directory.join("5xSig.json").is_file());
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn an_empty_log_vector_still_renders_a_parseable_array() {
    // A transaction the runtime refused before any program ran has no invoke
    // lines. The census reads that as "no program corroborated", which is the
    // honest answer, but the document still has to parse.
    let rendered = render(&TransactionEvidence {
        label: "frame refused before dispatch",
        signature: "5xEmpty",
        slot: 3,
        error: Some("TransactionError(SanitizeFailure)"),
        logs: &[],
        compute_units_consumed: None,
        wire_bytes: None,
    });
    assert!(rendered.contains("\"logs\": []"));
    assert!(rendered.contains("\"compute_units_consumed\": null"));
}

#[test]
fn fold_emits_one_document_with_every_record_and_no_trailing_comma() {
    let directory = std::env::temp_dir().join("dclutch-evidence-fold-test");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("evidence directory");
    for (index, signature) in ["5xAaa", "5xBbb", "5xCcc"].iter().enumerate() {
        let body = render(&TransactionEvidence {
            label: "step",
            signature,
            slot: index as u64,
            error: None,
            logs: &logs(&["Program E3M3 invoke [1]"]),
            compute_units_consumed: Some(1),
        wire_bytes: None,
        });
        fs::write(directory.join(format!("{signature}.json")), body).expect("record");
    }
    let document = fold(&directory).expect("fold");
    assert_eq!(document.matches("\"label\": \"step\"").count(), 3);
    assert!(!document.contains(",\n  ]"), "a trailing comma is invalid JSON");
    assert!(document.contains("\"transactions\": ["));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn fold_of_an_empty_directory_is_a_valid_empty_campaign() {
    // An empty document is honest evidence that nothing ran. It must not be a
    // parse error, because the census's own message for it ("campaign evidence
    // has no `transactions` array") is far more useful than a JSON error.
    let directory = std::env::temp_dir().join("dclutch-evidence-empty-fold");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("evidence directory");
    let document = fold(&directory).expect("fold");
    assert!(document.contains("\"transactions\": ["));
    assert!(document.trim_end().ends_with('}'));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn a_measured_wire_extent_is_carried_and_an_unmeasured_one_says_so() {
    let measured = render(&TransactionEvidence {
        label: "measured",
        signature: "sig",
        slot: 1,
        error: None,
        logs: &[],
        compute_units_consumed: Some(7),
        wire_bytes: Some(1_232),
    });
    assert!(measured.contains("\"wire_bytes\": 1232"), "{measured}");

    // `None` must render as null rather than vanish: a witness asking whether a
    // campaign fits the packet maximum has to be able to tell "it fits" apart
    // from "nobody looked", and an absent key reads as neither.
    let unmeasured = render(&TransactionEvidence {
        label: "unmeasured",
        signature: "sig",
        slot: 1,
        error: None,
        logs: &[],
        compute_units_consumed: Some(7),
        wire_bytes: None,
    });
    assert!(unmeasured.contains("\"wire_bytes\": null"), "{unmeasured}");
}
