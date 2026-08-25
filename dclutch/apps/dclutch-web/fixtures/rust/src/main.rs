use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN,
    RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use solana_program::{hash::hash, pubkey::Pubkey};

const MARKET_PDA_DOMAIN: &[u8] = b"dclutch/market-root/v1";

fn fill(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn hexadecimal(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn main() {
    let program = Pubkey::new_from_array(fill(99));

    let realm = RealmV1::new(RealmV1Input {
        token_program: fill(11),
        collateral_mint: fill(12),
        collateral_adapter_release_id: fill(13),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::AdmitIssuerControl,
    })
    .expect("fixture Realm input is canonical");
    let realm_bytes = realm.to_bytes();
    let realm_digest = hash(&realm_bytes).to_bytes();
    let realm_address = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &program).0;

    let identity = MarketIdentity::new(
        ContentId::new(realm_digest).expect("Realm digest is nonzero"),
        ContentId::new(fill(2)).expect("fixture Product ID is nonzero"),
        ContentId::new(fill(3)).expect("fixture basis ID is nonzero"),
        ContentId::new(fill(4)).expect("fixture policy ID is nonzero"),
        ContentId::new(fill(5)).expect("fixture capability ID is nonzero"),
        7,
    );
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let market_address = Pubkey::find_program_address(&[MARKET_PDA_DOMAIN, &identity_digest], &program).0;
    let root = MarketRoot::founding(identity, fill(8)).expect("fixture founding root is canonical");
    let market = CategoricalMarketV1::<3>::new(
        root,
        0,
        [0, 0, 0],
        CategoricalSettlementSummaryV1::empty(),
    )
    .expect("fixture Market is canonical");
    let mut market_bytes = vec![0; CategoricalMarketV1::<3>::encoded_len().expect("supported width")];
    market.encode(&mut market_bytes).expect("fixture Market output has exact width");

    let owner = fill(22);
    let position = PositionV1::<3>::new(market_address.to_bytes(), owner, 7, [5, 0, 12])
        .expect("fixture Position is canonical");
    let position_address = Pubkey::find_program_address(
        &[POSITION_PDA_DOMAIN, market_address.as_ref(), &owner],
        &program,
    )
    .0;
    let mut position_bytes = vec![0; PositionV1::<3>::encoded_len().expect("supported width")];
    position.encode(&mut position_bytes).expect("fixture Position output has exact width");

    let refund_authority = fill(31);
    let (rent_credit_address, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &refund_authority],
        &program,
    );
    let rent_credit = RentCreditV1::new(
        RefundAuthority::new(refund_authority).expect("fixture authority is nonzero"),
        rent_credit_bump,
    );
    let rent_credit_bytes = rent_credit.to_bytes();

    let raw_record_content = b"dClutch canonical browser fixture\n";
    let schema_release_id = fill(41);
    let content_digest = hash(raw_record_content).to_bytes();
    let raw_record_address = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_release_id, &content_digest],
        &program,
    )
    .0;
    let staging_address = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema_release_id, &content_digest],
        &program,
    )
    .0;

    println!(
        concat!(
            "{{\n",
            "  \"format\": \"dclutch-web/canonical-rust-fixtures/v1\",\n",
            "  \"programId\": \"{}\",\n",
            "  \"accounts\": [\n",
            "    {{ \"kind\": \"Realm\", \"address\": \"{}\", \"dataHex\": \"{}\" }},\n",
            "    {{ \"kind\": \"Market\", \"address\": \"{}\", \"dataHex\": \"{}\" }},\n",
            "    {{ \"kind\": \"Position\", \"address\": \"{}\", \"dataHex\": \"{}\" }},\n",
            "    {{ \"kind\": \"RentCredit\", \"address\": \"{}\", \"dataHex\": \"{}\" }}\n",
            "  ],\n",
            "  \"record\": {{\n",
            "    \"schemaReleaseIdHex\": \"{}\",\n",
            "    \"contentDigestHex\": \"{}\",\n",
            "    \"contentHex\": \"{}\",\n",
            "    \"rawAddress\": \"{}\",\n",
            "    \"stagingAddress\": \"{}\"\n",
            "  }}\n",
            "}}"
        ),
        program,
        realm_address,
        hexadecimal(&realm_bytes),
        market_address,
        hexadecimal(&market_bytes),
        position_address,
        hexadecimal(&position_bytes),
        rent_credit_address,
        hexadecimal(&rent_credit_bytes),
        hexadecimal(&schema_release_id),
        hexadecimal(&content_digest),
        hexadecimal(raw_record_content),
        raw_record_address,
        staging_address,
    );
}
