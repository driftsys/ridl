1. **must** make the `Telemetry` component require the `Horn` interface.

2. **must** add the activation count to the `Horn` interface as an interaction
   appended after `active`, whose payload type has an explicit range such as
   `integer [0..N]`.

3. **must** keep every existing declaration unchanged except that `Horn` gains
   the appended interaction and `Telemetry` gains the `requires Horn` line;
   adding one new typl type for the count and editing the doc comments of those
   two declarations are allowed.

4. **must** show the actual output of `ridl diff` or of the `ridl_diff` tool,
   with the top-level verdict `compatible`.

5. **should** model the count as a signal, because it is state that a consumer
   reads when it starts; an event does not meet this item.

6. **should** explain that a deployed consumer can observe only the new `Horn`
   signal and its type, and that the added requirement changes the composition
   of the system, not the wire.

7. **must not** add a second `Horn` provider or move a component to another
   machine.
