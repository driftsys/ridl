> **Built.** ridl has a working toolchain in this repository (epic E2): the five
> interaction kinds, timing annotations, contracts, interfaces and services, with
> Rust and TypeScript code generation and `ridl diff`. There is no runtime you
> can run a contract over — the transport bindings and the delivery semantics
> below are specified and not implemented, and the provider-side contract
> enforcement that is implemented is not reachable from any command. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/ridl-language-reference.md}}
