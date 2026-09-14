> **Specified, not built.** No compiler in this repository accepts rsdl. The
> parser has two profiles, typl and ridl, so a `.rsdl` file is not recognised as
> rsdl at all. rsdl is the apex of the family lattice and composes the layers
> below it; roadmap step 1 builds it (Epic 6). This reference
> is complete enough to design against; nothing below can be compiled or deployed
> today. See [what is built](../introduction.md#what-is-built).

{{#include ../../specification/rsdl-language-reference.md}}
