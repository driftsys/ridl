# Identity fixture

`alpha.First/Unit` (1) and `alpha.Second/Unit` (3) run on `Local`. First offers
`alpha` and maps `mapped`; Second offers `beta` and maps `alpha`.
`alpha.Foreign/Unit` (2) runs on `Other`, offering `mapped` and `remote`. The
local catalog union is therefore `alpha`, `beta`, `mapped`.

From the repository root, regenerate into an ignored directory and copy the
verified descriptor:

```sh
cargo run --locked -p ridl-cli -- build crates/ridl-rt-conformance/fixtures/identity --emit deployment --out-dir .superpowers/codex/identity-generated
cp .superpowers/codex/identity-generated/alpha.Bench.deployment.binfb crates/ridl-rt-conformance/fixtures/identity/identity.deployment.binfb
```

The committed `interfaces.lock` files and `instances.lock` fix the numbers.
After changing the interfaces, update the catalog constants in `src/identity.rs`
of this crate from the descriptor's names and hashes. The loopback test compares
all four constants with the descriptor. The CLI test regenerates the descriptor
from these sources and compares every byte. Neither runtime crate depends on the
compiler to read this fixture.
