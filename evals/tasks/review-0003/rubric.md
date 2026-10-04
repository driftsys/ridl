1. **must** identify that missionRequest and missionRequestInt both return
   MissionItemInt even though the deprecated non-integer request historically
   pairs with MissionItem, and explain the deliberate documented adaptation.

2. **must** identify that query-shaped mission requests do not by themselves
   express transfer sequencing, re-request after timeout or missionAck
   completion, and explain why a client cannot infer a reliable transfer from
   the signatures alone.

3. **must** identify missionType and sequence fields as necessary distinctions
   for independently stored plan types and ordered item transfer, using named
   declarations from the port.

4. **should** recommend documenting protocol state transitions and correlation
   between requests, items and acknowledgements alongside the RIDL boundary.

5. **must not** remove targetSystem, targetComponent, sequence or missionType
   fields as redundant solely because calls are presented through a single
   interface.

6. **must not** claim that RIDL timing defaults implement the upstream retry
   procedure.
