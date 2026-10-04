1. **must** represent plan type, participant identity, item count, item sequence
   and item payload in the RIDL declarations.

2. **must** define boundary interactions for announcing a count, requesting a
   numbered item, supplying an item and confirming success or failure, allowing
   either participant to send its required messages.

3. **must** keep different plan types separate in the described storage and
   transfer procedure.

4. **must** describe how the receiver advances only after the expected item and
   re-requests the expected item after an unexpected sequence number.

5. **must** describe response timeout and retry behavior and distinguish final
   successful acknowledgement from delivery of an individual item.

6. **should** keep transfer lifecycle and correlation documented together with
   the focused plan-transfer interface.

7. **must not** claim that a stream signature or RIDL response bound by itself
   supplies loss recovery or ordered-transfer state.
