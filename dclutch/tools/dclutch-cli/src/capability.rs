//! `dclutch capability` — the Trading root account that decides whether a
//! market can execute a trade at all.
//!
//! A market being Open is necessary and not sufficient. Execution runs through
//! a capability root: an immutable activation header, followed by a mutable
//! family tail. If that account does not exist, or its tail is Retiring, a
//! trade refuses no matter how open the market looks. This command reads it.

use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CAPABILITY_ROOT_MAGIC_V1, CapabilityRootHeaderV1,
};
use dclutch_trading::successor::{DirectRootPhaseV1, DirectRootStateV1};

use crate::{Arguments, Error, Result, address, rpc};

/// Dispatch `show` and `decode`.
pub fn run(arguments: Vec<String>) -> Result<()> {
    let (head, rest) = arguments.split_first().ok_or_else(|| {
        Error::new("`dclutch capability` needs a subcommand: `show <ADDRESS>` or `decode`")
    })?;
    let parsed = Arguments::parse(rest.to_vec())?;
    match head.as_str() {
        "show" => {
            let wanted = parsed.one_positional("capability root address")?;
            let _ = crate::parse_address(wanted, "the capability root address")?;
            let url = parsed.rpc_url();
            let fetched = rpc::fetch_account_v1(&url, wanted)?;
            let root = decode(&fetched.data)?;
            print!(
                "{}",
                render(&root, &fetched.data, Some((&fetched, &url)), parsed.json())
            );
            Ok(())
        }
        "decode" => {
            let bytes = parsed.offline_bytes()?;
            let root = decode(&bytes)?;
            print!("{}", render(&root, &bytes, None, parsed.json()));
            Ok(())
        }
        other => Err(Error::new(format!(
            "`dclutch capability {other}` is not a command. It knows `show` and `decode`."
        ))),
    }
}

/// What this binary could make of a capability root account.
#[derive(Debug)]
pub struct RootReadingV1 {
    header: CapabilityRootHeaderV1,
    tail_bytes: usize,
    direct: Option<DirectRootStateV1>,
}

/// Split header from tail, decode the header with its own decoder, and offer
/// the tail to the Direct decoder. A tail this binary cannot read is reported
/// as unread, never guessed at.
fn decode(bytes: &[u8]) -> Result<RootReadingV1> {
    let header_bytes = bytes.get(..CAPABILITY_ROOT_HEADER_BYTES_V1).ok_or_else(|| {
        Error::new(format!(
            "these {} bytes are shorter than the {CAPABILITY_ROOT_HEADER_BYTES_V1}-byte activation \
             header every capability root starts with, so this is not a capability root.",
            bytes.len()
        ))
    })?;
    let tail = bytes.get(CAPABILITY_ROOT_HEADER_BYTES_V1..).unwrap_or(&[]);

    let header = CapabilityRootHeaderV1::decode(header_bytes).map_err(|error| {
        let mut sentence = format!("this account's activation header does not decode ({error:?}).");
        if header_bytes.get(..CAPABILITY_ROOT_MAGIC_V1.len()) != Some(&CAPABILITY_ROOT_MAGIC_V1) {
            sentence.push_str(
                " Its leading magic is not `DCLTCRT1`, so it is some other account entirely.",
            );
        }
        Error::new(sentence)
    })?;

    Ok(RootReadingV1 {
        header,
        tail_bytes: tail.len(),
        direct: DirectRootStateV1::decode(tail).ok(),
    })
}

fn phase_word(phase: DirectRootPhaseV1) -> &'static str {
    match phase {
        DirectRootPhaseV1::Open => "Open",
        DirectRootPhaseV1::Retiring => "Retiring",
    }
}

/// One sentence a reader can act on.
fn plain_reading(reading: &RootReadingV1) -> String {
    match reading.direct {
        None => format!(
            "This is a live capability root, but its {}-byte family tail is not a Direct tail \
             this binary reads, so it cannot say whether trading is open. The header above is \
             still exact.",
            reading.tail_bytes
        ),
        Some(state) => match state.phase() {
            DirectRootPhaseV1::Open => {
                "Direct trading is open on this market: new intents are admitted.".to_owned()
            }
            DirectRootPhaseV1::Retiring => {
                "Direct trading is winding down on this market. No new intent will be admitted; \
                 existing maker positions are draining."
                    .to_owned()
            }
        },
    }
}

fn render(
    reading: &RootReadingV1,
    bytes: &[u8],
    fetched: Option<(&rpc::FetchedAccountV1, &str)>,
    json: bool,
) -> String {
    if json {
        return render_json(reading, bytes, fetched);
    }
    let header = reading.header;
    let selection = header.selection();
    let mut out = String::new();
    out.push_str("Trading capability root\n\n");
    if let Some((account, url)) = fetched {
        out.push_str(&format!("  {:<26}{}\n", "read from", rpc::origin(url)));
        out.push_str(&format!("  {:<26}{}\n", "at slot", account.slot));
        out.push_str(&format!("  {:<26}{}\n", "owned by program", account.owner));
    }
    out.push_str(&format!("  {:<26}{}\n", "account bytes", bytes.len()));
    out.push_str(&format!(
        "  {:<26}{CAPABILITY_ROOT_HEADER_BYTES_V1} header + {} tail\n",
        "split", reading.tail_bytes
    ));
    out.push_str(&format!("  {:<26}{}\n", "market", address(header.market())));
    out.push_str(&format!("  {:<26}{}\n", "generation", header.generation()));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "release set",
        address(header.release_set().to_bytes())
    ));

    out.push_str("\n  activation selection\n");
    out.push_str(&format!(
        "  {:<26}{}\n",
        "entry index",
        selection.entry_index()
    ));
    for (label, id) in [
        ("manifest", selection.manifest()),
        ("kind", selection.kind()),
        ("capability release", selection.capability_release()),
        ("config", selection.config()),
    ] {
        out.push_str(&format!("  {label:<26}{}\n", address(id.to_bytes())));
    }

    out.push_str("\n  family tail\n");
    match reading.direct {
        None => out.push_str(&format!(
            "  {:<26}{} bytes, not read by this binary\n",
            "shape", reading.tail_bytes
        )),
        Some(state) => {
            out.push_str(&format!("  {:<26}Direct\n", "family"));
            out.push_str(&format!("  {:<26}{}\n", "phase", phase_word(state.phase())));
            out.push_str(&format!(
                "  {:<26}{}\n",
                "open maker roots",
                state.open_maker_root_count()
            ));
        }
    }

    out.push_str(&format!("\n{}\n", plain_reading(reading)));
    out
}

fn render_json(
    reading: &RootReadingV1,
    bytes: &[u8],
    fetched: Option<(&rpc::FetchedAccountV1, &str)>,
) -> String {
    let header = reading.header;
    let selection = header.selection();
    let mut value = serde_json::json!({
        "schema": "dclutch-cli/capability-root/v1",
        "account_bytes": bytes.len(),
        "header_bytes": CAPABILITY_ROOT_HEADER_BYTES_V1,
        "tail_bytes": reading.tail_bytes,
        "market": address(header.market()),
        "generation": header.generation(),
        "release_set": address(header.release_set().to_bytes()),
        "selection": {
            "entry_index": selection.entry_index(),
            "manifest": address(selection.manifest().to_bytes()),
            "kind": address(selection.kind().to_bytes()),
            "capability_release": address(selection.capability_release().to_bytes()),
            "config": address(selection.config().to_bytes()),
        },
        "direct_tail": reading.direct.map(|state| serde_json::json!({
            "phase": phase_word(state.phase()),
            "open_maker_root_count": state.open_maker_root_count(),
        })),
        "plain_reading": plain_reading(reading),
    });
    if let Some((account, url)) = fetched
        && let Some(object) = value.as_object_mut()
    {
        object.insert("read_from".to_owned(), serde_json::json!(rpc::origin(url)));
        object.insert("slot".to_owned(), serde_json::json!(account.slot));
        object.insert("owner".to_owned(), serde_json::json!(account.owner));
    }
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::{decode, plain_reading, render, run};
    use dclutch_market::capability_program::{
        CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    };
    use dclutch_trading::successor::DirectRootStateV1;
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;

    fn root_account() -> Vec<u8> {
        let content =
            |seed: u8| dclutch_core_contract::ContentId::new([seed; 32]).expect("nonzero");
        let selection = CapabilityExecutionSelectionV1::new(
            3,
            content(21),
            content(22),
            content(23),
            content(24),
        )
        .expect("valid selection")
        .with_capability_release_record_bumps(255, 254);
        let header = CapabilityRootHeaderV1::new(
            content(20),
            [31; 32],
            2,
            selection,
            SelectedRecordBumpsV1::new(255, 254, 253, 252),
        )
        .expect("valid header");
        let tail = DirectRootStateV1::new();
        let mut account = header.to_bytes().to_vec();
        account.extend_from_slice(&tail.encode());
        account
    }

    #[test]
    fn a_real_root_decodes_into_a_header_and_a_direct_tail() {
        let account = root_account();
        let reading = decode(&account).expect("decodes");
        assert_eq!(reading.header.market(), [31; 32]);
        assert_eq!(reading.header.generation(), 2);
        assert!(reading.direct.is_some(), "the Direct tail was not read");
        assert!(plain_reading(&reading).contains("Direct trading is open"));
    }

    #[test]
    fn the_rendering_names_the_selection_and_the_split() {
        let account = root_account();
        let reading = decode(&account).expect("decodes");
        let text = render(&reading, &account, None, false);
        for label in [
            "manifest",
            "kind",
            "capability release",
            "config",
            "entry index",
            "open maker roots",
        ] {
            assert!(text.contains(label), "the rendering drops `{label}`");
        }
        assert!(text.contains("Direct"));
    }

    #[test]
    fn the_json_rendering_parses_and_names_its_schema() {
        let account = root_account();
        let reading = decode(&account).expect("decodes");
        let parsed: serde_json::Value =
            serde_json::from_str(&render(&reading, &account, None, true)).expect("valid JSON");
        let at = |path: &str| parsed.pointer(path).cloned().unwrap_or_default();
        assert_eq!(at("/schema"), "dclutch-cli/capability-root/v1");
        assert_eq!(at("/direct_tail/phase"), "Open");
        assert_eq!(at("/tail_bytes"), 24);
    }

    #[test]
    fn a_tail_this_binary_cannot_read_is_reported_unread_not_guessed() {
        let mut account = root_account();
        account.truncate(CAPABILITY_ROOT_HEADER_BYTES_V1);
        account.extend_from_slice(&[9_u8; 128]);
        let reading = decode(&account).expect("the header still decodes");
        assert!(reading.direct.is_none());
        let text = plain_reading(&reading);
        assert!(text.contains("not a Direct tail"), "{text}");
        assert!(text.contains("128"), "{text}");
    }

    #[test]
    fn a_short_account_is_refused_against_the_header_width() {
        let error = decode(&[0_u8; 32]).expect_err("must refuse");
        assert!(
            error
                .to_string()
                .contains(&CAPABILITY_ROOT_HEADER_BYTES_V1.to_string()),
            "{error}"
        );
    }

    #[test]
    fn a_right_sized_impostor_is_refused_and_says_why() {
        let error = decode(&[0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + 24]).expect_err("must refuse");
        assert!(error.to_string().contains("DCLTCRT1"), "{error}");
    }

    #[test]
    fn an_unknown_capability_subcommand_is_a_refusal() {
        let error = run(vec!["dump".to_owned()]).expect_err("must refuse");
        assert!(error.to_string().contains("`dclutch capability dump`"));
    }
}
