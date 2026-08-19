#[derive(Clone, Copy, Debug)]
pub struct Transcript {
    lo: u64,
    hi: u64,
}

impl Transcript {
    pub const fn new(domain: u64) -> Self {
        Self {
            lo: 0x243f_6a88_85a3_08d3 ^ domain,
            hi: 0x1319_8a2e_0370_7344 ^ domain.rotate_left(29),
        }
    }

    pub fn byte(&mut self, value: u8) {
        self.lo ^= u64::from(value);
        self.lo = self.lo.wrapping_mul(0x0000_0100_0000_01b3);
        self.hi ^= self.lo.rotate_left(17).wrapping_add(u64::from(value));
        self.hi = self.hi.wrapping_mul(0x9e37_79b1_85eb_ca87);
    }

    pub fn bytes(&mut self, values: &[u8]) {
        for value in values {
            self.byte(*value);
        }
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0xff);
    }

    pub fn finish(self) -> u128 {
        let lo = avalanche(self.lo ^ self.hi.rotate_left(7));
        let hi = avalanche(self.hi ^ self.lo.rotate_right(11));
        (u128::from(hi) << 64) | u128::from(lo)
    }
}

const fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound != 0);
        self.next() % bound
    }
}
