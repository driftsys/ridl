Design a RIDL boundary for uploading an ordered plan to a remote component over
a link that can lose messages. Separate flight plans, geofence plans and
safe-point plans. The sender announces a count, the receiver requests numbered
items, and the sender supplies the requested item. The receiver confirms
completion or reports an error. A request that receives no response must be
retried, and an unexpected item must not advance the transfer. Include enough
identity to distinguish the participants and enough metadata to associate
messages with their plan type. Provide a compiling workspace plus a written
transfer procedure; a byte-compatible MAVLink implementation is not required.

These requirements are independently paraphrased from the pinned public
[mission documentation](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/mission.md),
especially Plan Types and Upload a Plan to the Vehicle; the documentation is
cited only and its prose is not copied.

The source is the MAVLink project's public documentation, licensed under
[CC BY 4.0](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/index.md#license).
The requirements above are independently paraphrased factual requirements; no
source documentation prose is copied or translated.
