1. **must** represent a parameter name with a byte-based bound that admits all
   16-byte names without requiring a trailing null.

2. **must** represent both numeric value and numeric type, either through typed
   alternatives or through an explicitly documented encoded-value/type pair.

3. **must** provide distinct operations for one-value reading, all-value
   discovery and value setting, with a channel for reporting the current stored
   value after a write.

4. **must** include list count and index information and describe detecting
   missing entries rather than assume a completed stream contains every
   parameter.

5. **must** represent errors separately from value updates and explain how
   replies or updates are associated with the intended remote component and
   parameter.

6. **must** explain how an adapter selects byte-wise or numeric-cast encoding if
   it exchanges MAVLink messages, without assuming every numeric type is
   intrinsically float.

7. **must not** require advance knowledge of the remote parameter names or claim
   that a write request proves its requested value was applied.

8. **must** permit a client to request one parameter through an index or another
   explicit name-independent selector without first discovering its name.
