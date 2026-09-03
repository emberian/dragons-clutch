//! `dclutch market` — the Market Core account, decoded and read back plainly.

use dclutch_market_core_codec::{CoreState, Identity, Phase, Readiness, STATE_BYTES, STATE_MAGIC};

use crate::{Arguments, Error, Result, address, rpc};

/// Dispatch `show` and `decode`.
pub fn run(arguments: Vec<String>) -> Result<()> {
    let (head, rest) = arguments.split_first().ok_or_else(|| {
        Error::new("`dclutch market` needs a subcommand: `show <ADDRESS>` or `decode`")
    })?;
    let parsed = Arguments::parse(rest.to_vec())?;
    match head.as_str() {
        "show" => show(&parsed),
        "decode" => {
            let bytes = parsed.offline_bytes()?;
            let state = decode(&bytes)?;
            print!("{}", render(&state, &bytes, None, parsed.json()));
            Ok(())
        }
        other => Err(Error::new(format!(
            "`dclutch market {other}` is not a command. It knows `show` and `decode`."
        ))),
    }
}

fn show(parsed: &Arguments) -> Result<()> {
    let wanted = parsed.one_positional("market address")?;
    // Parsed before the network call, so a typo costs nothing and never
    // reaches an endpoint.
    let _ = crate::parse_address(wanted, "the market address")?;
    let url = parsed.rpc_url();
    let fetched = rpc::fetch_account_v1(&url, wanted)?;
    let state = decode(&fetched.data)?;
    print!(
        "{}",
        render(&state, &fetched.data, Some((&fetched, &url)), parsed.json())
    );
    Ok(())
}

/// Hand the bytes to the emitted decoder, and turn its refusal into a sentence
/// that says what the reader is actually looking at.
fn decode(bytes: &[u8]) -> Result<CoreState> {
    CoreState::decode(bytes).map_err(|error| {
        let mut sentence = format!(
            "these {} bytes are not a Market Core account this binary can read ({error:?}).",
            bytes.len()
        );
        if bytes.len() != STATE_BYTES {
            sentence.push_str(&format!(
                " A Market Core account is exactly {STATE_BYTES} bytes here. A dClutch market \
                 founded before the PDA-bump widening is {} bytes and is refused on length by \
                 this build, which is the intended behaviour and not a corrupt account.",
                STATE_BYTES - 8
            ));
        } else if bytes.get(..STATE_MAGIC.len()) != Some(&STATE_MAGIC) {
            sentence.push_str(
                " The length is right but the leading magic is not `DCLTCOR3`, so this is some \
                 other account of the same size.",
            );
        }
        Error::new(sentence)
    })
}

fn phase_word(phase: Phase) -> &'static str {
    match phase {
        Phase::Founding => "Founding",
        Phase::Open => "Open",
        Phase::Terminal => "Terminal",
        Phase::Retiring => "Retiring",
        Phase::Retired => "Retired",
    }
}

fn readiness_word(readiness: Readiness) -> &'static str {
    match readiness {
        Readiness::Prepaid => "Prepaid",
        Readiness::Ready => "Ready",
        Readiness::Consumed => "Consumed",
    }
}

/// One sentence a reader who knows nothing about this protocol can act on.
fn plain_reading(state: &CoreState) -> String {
    match state.phase {
        Phase::Founding => {
            "This market is still being built. Nobody can buy a claim on it yet.".to_owned()
        }
        Phase::Open => {
            "This market is open: claims can be bought and sold, and it has not been answered yet."
                .to_owned()
        }
        Phase::Terminal => format!(
            "This market has been answered. Outcome {} won; holders of that outcome's claims can \
             redeem each one for one collateral unit, and every other claim is worth nothing.",
            state.terminal_winner
        ),
        Phase::Retiring => format!(
            "This market has been answered (outcome {} won) and is being wound down. Redemptions \
             continue; nothing new can be opened against it.",
            state.terminal_winner
        ),
        Phase::Retired => {
            "This market is finished. Everything it owed has been settled and it holds no \
             outstanding capabilities."
                .to_owned()
        }
    }
}

fn identity_line(label: &str, identity: Identity) -> String {
    format!("  {label:<26}{}\n", address(identity.to_bytes()))
}

/// Render the decoded state, as prose or as JSON.
fn render(
    state: &CoreState,
    bytes: &[u8],
    fetched: Option<(&rpc::FetchedAccountV1, &str)>,
    json: bool,
) -> String {
    if json {
        return render_json(state, bytes, fetched);
    }

    let mut out = String::new();
    out.push_str("Market Core account\n\n");
    if let Some((account, url)) = fetched {
        out.push_str(&format!("  {:<26}{}\n", "read from", rpc::origin(url)));
        out.push_str(&format!("  {:<26}{}\n", "at slot", account.slot));
        out.push_str(&format!("  {:<26}{}\n", "owned by program", account.owner));
    }
    out.push_str(&format!("  {:<26}{}\n", "account bytes", bytes.len()));
    out.push_str(&format!("  {:<26}{}\n", "phase", phase_word(state.phase)));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "resolution fund",
        readiness_word(state.readiness)
    ));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "answered",
        match state.terminal_receipt {
            None => "no".to_owned(),
            Some(receipt) => format!(
                "yes, outcome {} — receipt {}",
                state.terminal_winner,
                address(receipt.to_bytes())
            ),
        }
    ));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "most it can back",
        match state.principal_cap_sets {
            u64::MAX => "unbounded".to_owned(),
            sets => format!("{sets} complete sets"),
        }
    ));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "outstanding capabilities", state.outstanding_capabilities
    ));
    out.push_str(&format!(
        "  {:<26}{}\n",
        "generation", state.identity.generation
    ));

    out.push_str("\n  identities\n");
    out.push_str(&identity_line("market id", state.identity.market_id));
    out.push_str(&identity_line("realm id", state.identity.realm_id));
    out.push_str(&identity_line(
        "product record",
        state.identity.product_record,
    ));
    out.push_str(&identity_line("product id", state.identity.product_id));
    out.push_str(&identity_line(
        "resolution policy",
        state.identity.resolution_policy,
    ));
    out.push_str(&identity_line(
        "capability manifest",
        state.identity.capability_manifest,
    ));
    out.push_str(&identity_line(
        "selected release set",
        state.identity.selected_release_set,
    ));
    out.push_str(&identity_line(
        "registry program",
        state.identity.registry_program,
    ));
    out.push_str(&identity_line("rent beneficiary", state.rent_beneficiary));

    out.push_str("\n  recorded PDA bumps\n");
    for (label, bump) in [
        ("market", state.bumps.market),
        ("realm raw record", state.bumps.realm_raw_record),
        ("realm staging record", state.bumps.realm_staging_record),
    ] {
        out.push_str(&format!(
            "  {label:<26}{}\n",
            match bump {
                Some(value) => value.to_string(),
                None => "not recorded — readers search for it".to_owned(),
            }
        ));
    }

    out.push_str(&format!("\n{}\n", plain_reading(state)));
    out
}

fn render_json(
    state: &CoreState,
    bytes: &[u8],
    fetched: Option<(&rpc::FetchedAccountV1, &str)>,
) -> String {
    let mut value = serde_json::json!({
        "schema": "dclutch-cli/market-core/v1",
        "account_bytes": bytes.len(),
        "phase": phase_word(state.phase),
        "resolution_fund": readiness_word(state.readiness),
        "answered": state.terminal_receipt.is_some(),
        "terminal_winner": state.terminal_winner,
        "terminal_receipt": state.terminal_receipt.map(|receipt| address(receipt.to_bytes())),
        "outstanding_capabilities": state.outstanding_capabilities,
        "principal_cap_sets": state.principal_cap_sets,
        "generation": state.identity.generation,
        "market_id": address(state.identity.market_id.to_bytes()),
        "realm_id": address(state.identity.realm_id.to_bytes()),
        "product_record": address(state.identity.product_record.to_bytes()),
        "product_id": address(state.identity.product_id.to_bytes()),
        "resolution_policy": address(state.identity.resolution_policy.to_bytes()),
        "capability_manifest": address(state.identity.capability_manifest.to_bytes()),
        "selected_release_set": address(state.identity.selected_release_set.to_bytes()),
        "registry_program": address(state.identity.registry_program.to_bytes()),
        "rent_beneficiary": address(state.rent_beneficiary.to_bytes()),
        "bumps": {
            "market": state.bumps.market,
            "realm_raw_record": state.bumps.realm_raw_record,
            "realm_staging_record": state.bumps.realm_staging_record,
        },
        "plain_reading": plain_reading(state),
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
    use dclutch_market_core_codec::{
        CoreState, Identity, MarketIdentity, Phase, Readiness, STATE_BYTES, StateBumpsV1,
    };

    fn open_market() -> CoreState {
        let identity = |seed: u8| Identity::new([seed; 32]).expect("nonzero");
        CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
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
                generation: 2,
            },
            outstanding_capabilities: 1,
            principal_cap_sets: u64::MAX,
            rent_beneficiary: identity(9),
            terminal_receipt: None,
            bumps: StateBumpsV1 {
                market: Some(252),
                realm_raw_record: Some(254),
                realm_staging_record: Some(253),
                ..StateBumpsV1::UNRECORDED
            },
        }
    }

    #[test]
    fn a_real_state_round_trips_from_its_own_encoding() {
        let state = open_market();
        let bytes = state.encode().expect("encodes");
        assert_eq!(decode(&bytes).expect("decodes"), state);
    }

    #[test]
    fn the_rendering_carries_every_identity_and_bump() {
        let state = open_market();
        let bytes = state.encode().expect("encodes");
        let text = render(&state, &bytes, None, false);
        for label in [
            "market id",
            "realm id",
            "product record",
            "product id",
            "resolution policy",
            "capability manifest",
            "selected release set",
            "registry program",
            "rent beneficiary",
        ] {
            assert!(text.contains(label), "the rendering drops `{label}`");
        }
        assert!(text.contains("252") && text.contains("254") && text.contains("253"));
        assert!(text.contains("open"), "no plain reading: {text}");
    }

    #[test]
    fn the_json_rendering_parses_and_names_its_schema() {
        let state = open_market();
        let bytes = state.encode().expect("encodes");
        let parsed: serde_json::Value =
            serde_json::from_str(&render(&state, &bytes, None, true)).expect("valid JSON");
        let at = |path: &str| parsed.pointer(path).cloned().unwrap_or_default();
        assert_eq!(at("/schema"), "dclutch-cli/market-core/v1");
        assert_eq!(at("/phase"), "Open");
        assert_eq!(at("/answered"), false);
        assert_eq!(at("/bumps/market"), 252);
    }

    #[test]
    fn an_answered_market_reads_as_answered() {
        let mut state = open_market();
        state.phase = Phase::Terminal;
        state.terminal_winner = 1;
        state.terminal_receipt = Some(Identity::new([11; 32]).expect("nonzero"));
        let text = plain_reading(&state);
        assert!(text.contains("has been answered"), "{text}");
        assert!(text.contains("Outcome 1"), "{text}");
    }

    #[test]
    fn a_short_account_is_refused_with_the_cohort_reason_named() {
        let error = decode(&vec![0_u8; STATE_BYTES - 8]).expect_err("must refuse");
        let message = error.to_string();
        assert!(
            message.contains("before the PDA-bump widening"),
            "{message}"
        );
        assert!(message.contains(&STATE_BYTES.to_string()), "{message}");
    }

    #[test]
    fn a_right_sized_impostor_is_refused_and_says_why() {
        let error = decode(&vec![0_u8; STATE_BYTES]).expect_err("must refuse");
        assert!(error.to_string().contains("DCLTCOR3"), "{error}");
    }

    #[test]
    fn an_unknown_market_subcommand_is_a_refusal() {
        let error = run(vec!["inspect".to_owned()]).expect_err("must refuse");
        assert!(error.to_string().contains("`dclutch market inspect`"));
    }

    #[test]
    fn decode_from_base64_goes_all_the_way_through() {
        use base64::Engine as _;
        let bytes = open_market().encode().expect("encodes");
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        run(vec!["decode".to_owned(), "--base64".to_owned(), encoded]).expect("decodes");
    }
}
