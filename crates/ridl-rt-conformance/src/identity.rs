//! Committed identities of the descriptor fixture.

use ridl_rt::contract::{CatalogHash, CatalogRef};

/// The `alpha` catalog of the identity fixture.
pub const ALPHA: CatalogRef = CatalogRef {
    name: "alpha",
    hash: CatalogHash([
        14, 60, 221, 186, 156, 205, 215, 219, 138, 114, 25, 248, 25, 135, 168, 28, 142, 146, 39,
        76, 181, 65, 141, 187, 106, 210, 72, 117, 79, 48, 188, 186,
    ]),
};

/// The `beta` catalog of the identity fixture.
pub const BETA: CatalogRef = CatalogRef {
    name: "beta",
    hash: CatalogHash([
        106, 51, 207, 177, 105, 204, 94, 46, 138, 153, 55, 124, 40, 244, 103, 187, 255, 250, 147,
        162, 24, 114, 164, 87, 184, 81, 11, 222, 103, 180, 64, 60,
    ]),
};

/// The `mapped` catalog of the identity fixture.
pub const MAPPED: CatalogRef = CatalogRef {
    name: "mapped",
    hash: CatalogHash([
        125, 88, 71, 62, 114, 234, 207, 220, 77, 103, 47, 109, 172, 59, 11, 223, 77, 34, 114, 80,
        30, 91, 114, 52, 47, 216, 210, 202, 228, 234, 72, 169,
    ]),
};

/// The `remote` catalog of the identity fixture.
pub const REMOTE: CatalogRef = CatalogRef {
    name: "remote",
    hash: CatalogHash([
        33, 39, 131, 90, 243, 30, 21, 66, 212, 240, 12, 242, 114, 180, 159, 236, 253, 9, 87, 186,
        172, 195, 179, 109, 8, 97, 166, 74, 34, 7, 114, 94,
    ]),
};
