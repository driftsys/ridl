> **Built, and read by no runtime.** The compiler checks `.rsdl` files against this
> reference, lowers the system to the IR beside the package IR (§13), and
> `ridl diff` compares at the system (§14). No runtime reads the lowered system
> yet, and §12's reserved items are not built. See
> [Describing a system](../rsdl.md) and
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/rsdl-language-reference.md}}
