# Identity fixture

`alpha.First/Unit` (1) and `alpha.Second/Unit` (3) run on `Local`. First offers
`alpha.FaceService` and maps `mapped.Face`; Second offers `beta.FaceService` and
maps `alpha.Face`. `alpha.Foreign/Unit` (2) runs on `Other`, offering
`mapped.FaceService` and `remote.FaceService`. The local catalog union is
therefore `alpha`, `beta`, `mapped`.

From the repository root, regenerate into a temporary directory and copy the
verified descriptor:

```sh
identity_output=target/identity-output
mkdir -p "$identity_output"
cargo run --locked -p ridl-cli -- build crates/ridl-rt-conformance/fixtures/identity --emit deployment --out-dir "$identity_output"
cp "$identity_output/alpha.bench.deployment.binfb" crates/ridl-rt-conformance/fixtures/identity/identity.deployment.binfb
```

The committed `interfaces.lock` files and `instances.lock` fix the numbers.
After changing the interfaces, update the catalog constants in `src/identity.rs`
of this crate from the descriptor's names and hashes. The loopback test compares
all four constants with the descriptor. The CLI test regenerates the descriptor
from these sources and compares every byte. Neither runtime crate depends on the
compiler to read this fixture.
