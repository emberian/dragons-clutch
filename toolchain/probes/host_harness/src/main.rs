use dragon_clutch_toolchain_probe::{classify, debit, fee_atoms, Error};

fn main() {
    assert_eq!(fee_atoms(10_001, 100), Ok(100));
    assert_eq!(fee_atoms(u128::MAX, 10_000), Err(Error::Overflow));
    assert_eq!(classify(5, 0, 10), Ok(0));
    assert_eq!(classify(10, 0, 10), Err(Error::InvalidRange));
    assert_eq!(classify(5, 10, 0), Err(Error::InvalidRange));
    assert_eq!(debit(10, 4), Ok(6));
    assert_eq!(debit(3, 4), Err(Error::Overflow));
    println!("probe-ok");
}
