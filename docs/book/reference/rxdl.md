> **Specified, not built.** No compiler in this repository accepts rxdl. The
> parser has three profiles, typl, ridl and rsdl, so the loader reports a
> `.rxdl` file in a package directory with the warning RIDL-417
> (`unsupported-source-file`) and does not compile it, and refuses a lone
> `.rxdl` file as the entry. This reference becomes normative when rxdl is implemented, and
> ridl's boundary-model core (E3) is its precondition. Nothing below can be
> compiled today. See [what is built](../introduction.md#what-is-built).

{{#include ../../specification/rxdl-language-reference.md}}
