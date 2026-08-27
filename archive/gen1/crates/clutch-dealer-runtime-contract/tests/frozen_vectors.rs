mod common;

fn hex(bytes: [u8; 32]) -> String {
    use core::fmt::Write as _;
    let mut output = String::new();
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn canonical_content_ids_are_frozen() {
    let actual = [
        hex(common::policy().policy_id().unwrap().bytes()),
        hex(common::funding_state().state_content_id().unwrap().bytes()),
        hex(common::lp_page().page_content_id().unwrap().bytes()),
        hex(common::lease().lease_id().unwrap().bytes()),
        hex(common::finalizing_pot().pot_content_id().unwrap().bytes()),
        hex(common::fee_budget().budget_content_id().unwrap().bytes()),
        hex(common::liveness_budget()
            .budget_content_id()
            .unwrap()
            .bytes()),
    ];
    let expected = [
        "7db0f47420b59c7b720bccd1e54fd6d493d6540da1600b9e8e0f8cbc43dc1231",
        "7b13c2414fa15f7847390c2801a7833f218f73efd37fc3b0c7441542b9d0bdd9",
        "11ecfc8a9be38f83fe683511563a64d452eba56684caeef1832af9bfc1d0baa6",
        "fb1dfa7996c21a90b2ad5c98dac6b21d023616ff9b94568254187c0a8fb508f4",
        "f92188d05c37437a3e2297d40061ab2ead7363761504802eac7103d876fd8e1c",
        "85aaf8a3941438b1c06232316fe0842abc40b57b629c85b718def069f9e149ff",
        "da8be0b9ba81cb3b67e361b95bd270c364a42837d2819c13adae11e4928b1ed8",
    ];
    assert_eq!(actual, expected);
}
