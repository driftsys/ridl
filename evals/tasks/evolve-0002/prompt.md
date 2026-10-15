Prepare a copy of `evals/corpus/mavlink` and add an index-only convenience query
named paramRequestReadByIndex to common.ParameterProtocol. It takes targetSystem
and targetComponent using the existing Uint8 type and paramIndex using the
existing Int16 type, and returns the existing ParamValue type. Reject negative
indices through a require contract. Preserve every existing interaction and its
wire ordinal, every existing payload declaration and its fields, and all package
and import boundaries.

Explain how an adapter would use the pinned parameter protocol's single-value
read procedure to implement this convenience query, including the existing
request's name/index selection rule. This is a local API extension; it does not
introduce a new upstream message. Provide the source patch, check the modified
workspace without an Error, and show the actual JSON `ridl diff` verdict against
the original workspace. Do not edit the frozen corpus or implement an adapter.
