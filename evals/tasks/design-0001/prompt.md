Design a small RIDL API for components that announce their presence and let a
monitor report whether each discovered component is connected or disconnected.
An announcement includes system and component identity, component or vehicle
type, autopilot classification and operating state. Components send
announcements while operational, even when idle, and stop sending them when the
component is faulted; a separate announcement thread must not continue without
accounting for the component fault state. The deployment chooses the
announcement interval and missed-announcement limit; the protocol does not
prescribe universal values. Identify components by both identity fields and
infer their type from announcement metadata. Provide a compiling workspace and
explain which connection-tracking rules require runtime behavior.

These requirements are independently paraphrased from the pinned public
[heartbeat documentation](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/heartbeat.md);
the documentation is cited only and its prose is not copied.

The source is the MAVLink project's public documentation, licensed under
[CC BY 4.0](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/index.md#license).
The requirements above are independently paraphrased factual requirements; no
source documentation prose is copied or translated.
