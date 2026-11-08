use four_cc::FourCC;

pub const WOFF2_SIGNATURE: FourCC = FourCC(*b"wOF2");
pub const TTF_COLLECTION_FLAVOR: FourCC = FourCC(*b"ttcf");
pub const TTF_TRUE_TYPE_FLAVOR: FourCC = FourCC([0, 1, 0, 0]);
pub const TTF_CFF_FLAVOR: FourCC = FourCC(*b"OTTO");
