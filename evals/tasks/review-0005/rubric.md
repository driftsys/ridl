1. **must** identify the eight row/occupant station packages that repeat
   FanSpeed, Temperature and AirDistribution declarations, and explain the
   maintenance cost of changing those definitions consistently.

2. **must** discuss a shared station vocabulary as a possible redesign while
   preserving separate addressed station instances and recognizing that the
   frozen one-package-per-branch rule explains the repetition.

3. **must** identify that FanSpeed enforces the integer range 0..100 while its
   percent unit remains comment metadata because integer-backed unit types are
   unavailable, so adding a unit directly would change its representation.

4. **must** identify that AirDistribution preserves the three upstream allowed
   names but encodes them as integer enum values rather than upstream strings,
   requiring an explicit adapter correspondence.

5. **must** identify that a setter command has no reported applied value or
   failure outcome and that the matching signal alone does not specify
   correlation with a particular request.

6. **must not** invent upstream timing, temperature bounds, setter
   acknowledgement guarantees or default values absent from the selected
   definitions.
