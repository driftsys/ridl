> **Specified, not built.** The frame below is what a runtime and a transport
> binding are written from. No binding exists yet: the
> WebSocket transport is not built, and on Android a runtime binds the ports
> over its own binder contract, outside this repository (§11.2, which reverses
> an earlier AIDL binding). The one runtime in this repository,
> `ridl-loopback`, runs in process and speaks no frame, so every statement below
> about delivery, timing enforcement and duplicate suppression describes the
> specification and not a shipped behaviour. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/frame-specification.md}}
