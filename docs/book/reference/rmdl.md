> **Specified, not built.** No compiler in this repository accepts rmdl. The
> parser has three profiles, typl, ridl and rsdl, so the loader reports a
> `.rmdl` file in a package directory with the warning RIDL-417
> (`unsupported-source-file`) and does not compile it, and refuses a lone
> `.rmdl` file as the entry. This reference is complete enough to design
> against; nothing below can be compiled today, and none of the execution
> semantics it describes is implemented. See
> [what is built](../introduction.md#what-is-built).

{{#include ../../specification/rmdl-language-reference.md}}
