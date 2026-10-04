1. **must** identify that Point and Vector3 have the same x/y/z field types
   while their upstream comments give different transformation semantics, and
   explain why merging them would erase useful nominal meaning.

2. **must** identify that Point32 and Point have distinct upstream precision
   intentions which the RosFloat32 and RosFloat64 helpers both lower to float64,
   and classify that loss as a documented translation limitation.

3. **must** identify that Inertia.com documents metres but uses the general
   Vector3 type without a machine-readable length unit, and explain the risk of
   accepting a vector with another physical meaning.

4. **should** propose a separately typed centre-of-mass vector for a redesigned
   API while explaining that introducing it would change the frozen
   upstream-shaped schema.

5. **must not** assign one physical unit globally to Vector3, which is also used
   for acceleration and angular quantities.

6. **must not** treat the identical field shapes of Point and Vector3 alone as
   proof that either declaration should be removed.
