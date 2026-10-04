Design a RIDL API for discovering, reading and changing named numeric
configuration values on a remote component. A client can request one value or
all values without knowing the names beforehand, using a numeric index or
another explicit name-independent selector when requesting one value. Each
returned value carries its name, numeric type and list count/index. Names occupy
at most 16 bytes, including the case where all 16 bytes are used without a
terminator. A write is followed by a report of the current stored value, even
when the requested value was not applied. Preserve a distinction between value
updates and errors. Provide a compiling workspace and explain the transport
encoding and list-completeness rules an adapter must handle; a byte-compatible
implementation is not required.

These requirements are independently paraphrased from the pinned public
[parameter documentation](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/parameter.md),
including Message/Enum Summary, Parameter Encoding and Read All Parameters; the
documentation is cited only and its prose is not copied.

The source is the MAVLink project's public documentation, licensed under
[CC BY 4.0](https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/index.md#license).
The requirements above are independently paraphrased factual requirements; no
source documentation prose is copied or translated.
