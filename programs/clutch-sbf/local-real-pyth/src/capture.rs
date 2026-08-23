//! Hash-pinned captured provider inputs shared by the campaign and builder.
//!
//! Reading an exact tracked account body does not require linking the Pyth VAA
//! construction stack. Keeping this boundary separate lets a local Operator
//! reuse the real-source plane without acquiring proof-generation code.

use clutch_sbf::pyth_receiver::config_byte_digest;
use std::{fs, path::Path};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&config_byte_digest(bytes))
}

pub fn fixture(name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let expected = match name {
        "receiver.so" => "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        "router.so" => "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        "router-initialize.data" => {
            "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b"
        }
        "receiver-initialize.data" => {
            "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f"
        }
        "receiver-config.account" => {
            "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa"
        }
        "receiver-post-update.data" => {
            "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d"
        }
        other => return Err(format!("no executable fixture hash pin for {other}").into()),
    };
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../svm-tests/tests/fixtures/real-pyth-local");
    let bytes = fs::read(root.join(name))?;
    let actual = sha256(&bytes);
    if actual != expected {
        return Err(format!("fixture {name} hash {actual} differs from {expected}").into());
    }
    Ok(bytes)
}
