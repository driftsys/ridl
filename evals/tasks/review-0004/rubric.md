1. **must** identify that ParamSet.paramValue and ParamValue.paramValue use a
   float-backed field accompanied by paramType, while the public protocol
   permits byte-wise and numeric-cast encoding, so a float value alone does not
   determine the original parameter value.

2. **must** identify that parameter identifiers are fixed 16-byte arrays whose
   full-length names need no trailing null, and explain why replacing them with
   a null-terminated string changes accepted identifiers.

3. **must** identify that paramSet has no query return and that paramValue is
   also an event, so confirmation of the current stored value and correlation
   with a write require the documented protocol procedure.

4. **must** identify paramCount and paramIndex in ParamValue as
   list-completeness information and explain why the list query stream alone
   does not establish that every parameter arrived.

5. **should** recommend keeping adapter encoding policy and retry/completeness
   rules explicit instead of inferring them from the scalar type.

6. **must not** assume that every stored parameter is intrinsically a
   floating-point value or that receiving any paramValue confirms a particular
   write.

7. **must** identify that the port's Float32 helper lowers to float64 without
   guaranteeing preservation of binary32 payload bits, and explain that
   byte-wise parameter decoding requires the original wire bits or a conversion
   explicitly justified to preserve them rather than assuming the RIDL scalar
   preserves them.
