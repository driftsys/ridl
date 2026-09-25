> **Specified, not built.** The frame below is what a runtime and a transport
> binding are written from (roadmap story E11.1). No binding exists yet: the
> WebSocket transport is story E11.9, and on Android a runtime binds the ports
> over its own binder contract, outside this repository (§11.2, which reverses
> the lane P driver's decision D-P5). The one runtime in this repository,
> `ridl-loopback`, runs in process and speaks no frame, so every statement below
> about delivery, timing enforcement and duplicate suppression describes the
> specification and not a shipped behaviour. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/frame-specification.md}}
