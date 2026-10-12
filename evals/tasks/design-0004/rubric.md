1. **must** add a new interface for seat heating rather than add its
   interactions to `Cabin` or `Horn`.

2. **must** model the level each seat has reached as a signal (one signal that
   carries the seat, or one signal per seat) and the heater fault as an event.

3. **must** model setting a level as a command whose parameters identify the
   seat and the level.

4. **must** add a component that offers a service for the new interface.

5. **must** list the new component in the `Vehicle` system.

6. **must** place the new component on the `Hpc` machine of the `Bench`
   deployment.

7. **must** make the `Panel` component require the new interface.

8. **should** give the heating level a typl type with a range and give the seat
   an enum or a type with a range, not a bare primitive.

9. **must not** change, reorder or remove an existing declaration, field,
   interaction or component, apart from adding the new component to `Vehicle`
   and `Hpc` and a `requires` line to `Panel`.
