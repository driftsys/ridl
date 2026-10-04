1. **must** add the query paramRequestReadByIndex to common.ParameterProtocol
   with targetSystem and targetComponent parameters of existing type Uint8,
   paramIndex of existing type Int16 and return type ParamValue.

2. **must** append the query after the existing interactions so their ordinals
   remain 1 through 5 and the new query receives ordinal 6.

3. **must** reject negative paramIndex values with a require contract while
   accepting the existing Int16 type's nonnegative index domain.

4. **must** preserve all existing interaction signatures, declaration names,
   payload fields and types, wire numbers, package boundaries and imports.

5. **must** explain that an adapter sends the existing ParamRequestRead with the
   supplied target identities and index, whose nonnegative index makes the name
   field irrelevant under the upstream selection rule.

6. **must** provide a modified workspace that checks without an Error and an
   actual JSON diff report with verdict compatible and only the appended
   interaction change.

7. **must not** claim the convenience query is a new upstream protocol message,
   change parameter value encoding or implement an adapter in the frozen corpus.
