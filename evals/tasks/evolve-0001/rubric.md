1. **must** add doc attributes or documentation comments attached to Point and
   Vector3 that survive lowering to exported documentation, using valid RIDL
   syntax.

2. **must** explain that a point represents a position while a vector is
   anchored at the origin and changes only under the rotational part of a
   transform.

3. **must** preserve every existing type, field, number, package, import and
   interaction while retaining the original upstream comments.

4. **must** produce a modified workspace that checks without an Error and
   explain that changed exported documentation yields the compatible verdict
   through a documentation-only change.

5. **must not** merge Point and Vector3 or add unit/range constraints as part of
   this documentation request.
