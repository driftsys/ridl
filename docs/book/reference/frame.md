> **Specified, and built in part.** The frame below is what a runtime and a
> transport binding are written from. Its WebSocket binding
> ([§11.1](#111-websocket)) is built: `ridl-transport-ws` speaks this frame
> over WebSocket as the providing side of a session, and the As-built note
> below says which rules it applies. On Android a runtime binds the ports over
> its own binder contract, outside this repository (§11.2, which reverses an
> earlier AIDL binding). The one runtime in this repository, `ridl-loopback`,
> runs in process and speaks no frame, and neither it nor the transport
> enforces a timing rule or a contract clause on the provider side, so every
> statement below about timing enforcement and provider-side enforcement
> describes the specification and not a shipped behaviour. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/frame-specification.md}}
