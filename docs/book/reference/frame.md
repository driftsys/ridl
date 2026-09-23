> **Specified, not built.** The frame below is what a runtime and a transport
> binding are written from (roadmap story E11.1). No binding exists yet: the
> WebSocket transport is story E11.9, and the AIDL-over-Binder binding is the
> Kotlin backend's, outside this repository. The one runtime in this
> repository, `ridl-loopback`, runs in process and speaks no frame, so every
> statement below about delivery, timing enforcement and duplicate suppression
> describes the specification and not a shipped behaviour. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/frame-specification.md}}
