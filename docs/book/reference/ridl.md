> **Built.** ridl has a working toolchain in this repository: the five
> interaction kinds, timing annotations, contracts, interfaces and services, with
> Rust and TypeScript code generation and `ridl diff`. There is no runtime you
> can run a contract over from a command in this book — of the transport
> bindings below, the WebSocket one is implemented by `ridl-transport-ws`,
> which a program embeds; the others and their delivery semantics are
> specified and not implemented, and the provider-side contract enforcement
> that is implemented is not reachable from any command. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/ridl-language-reference.md}}
