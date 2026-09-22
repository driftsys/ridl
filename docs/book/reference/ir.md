> **What this chapter is for.** It is the contract between this toolchain and a
> program that reads its intermediate representation — a codegen plugin, a
> third-party backend, a tool that diffs artifacts. Nothing in this repository
> needs it: every in-tree backend links the IR types directly. Read it if you
> are writing a consumer outside this repository, or if you are changing the IR
> schema and need to know which changes a consumer can absorb.

{{#include ../../specification/ir-specification.md}}
