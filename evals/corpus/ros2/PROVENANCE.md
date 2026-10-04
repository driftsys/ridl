Upstream: <https://github.com/ros2/common_interfaces>;
<https://github.com/ros-navigation/navigation2>;
<https://github.com/ros2/rcl_interfaces>

Revision: common_interfaces d8dde22160f26cf4fd8f1f8dcd819637b1b88405;
navigation2 d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a; rcl_interfaces
99aea442813391cc20344c5b4c79e5191bf7f2c7

Licence: Apache-2.0

Kind rule: ROS 2 services are queries with request fields as parameters and
returned response structs; action goals are commands and action feedback is an
event.

# Modified ROS 2-to-RIDL translation

Every source file translates the definitions listed below at the exact header
pins. All original comments are retained verbatim, converting only the first ROS
`#` delimiter to `//`. No message interaction is inferred from topic usage.
Exactly four interfaces combine the selected services and actions. Action
results remain data structs; the fixed rule creates no result interaction.

## Licence evidence

All five common_interfaces package.xml files declare Apache License 2.0. Their
package LICENSE files are byte-identical Apache-2.0 texts. LICENSE here is the
byte-exact std_msgs/LICENSE copy. builtin_interfaces/package.xml declares
Apache-2.0 and rcl_interfaces/LICENSE supplies the full text.
navigation2/LICENSE directs readers to package licences and assigns Apache-2.0
to unmarked files; nav2_msgs/package.xml declares Apache-2.0. None of the 130
selected definitions contains an overriding licence or copyright notice. No
NOTICE file was found in the supplied upstream trees. Attribution comments are
retained, including links to the ROS Time design and source headers.

## Package spelling and boundaries

RIDL package segments permit lowercase ASCII letters and digits, but reject
underscores. Workspace member directories retain upstream spellings. Logical
manifest names, source package clauses, and explicit imports remove underscores.
Every mapping below uses the changed logical package name.

- `std_msgs` → `stdmsgs`; member directory `std_msgs`.
- `geometry_msgs` → `geometrymsgs`; member directory `geometry_msgs`.
- `sensor_msgs` → `sensormsgs`; member directory `sensor_msgs`.
- `nav_msgs` → `navmsgs`; member directory `nav_msgs`.
- `std_srvs` → `stdsrvs`; member directory `std_srvs`.
- `nav2_msgs` → `nav2msgs`; member directory `nav2_msgs`.
- `builtin_interfaces` → `builtininterfaces`; member directory
  `builtin_interfaces`.

## Representation rules and limitations

- Message basenames name structs unchanged. Service basenames generate
  NameRequest and NameResponse; action basenames generate NameGoal, NameResult
  and NameFeedback. These helpers represent exact source sections, not
  additional upstream messages. Empty sections remain empty structs. Queries
  take request fields in source order and return the response struct. Commands
  take a goal parameter of the complete NameGoal struct; feedback events carry
  the whole feedback struct. RIDL command parameters require named types, so
  passing the goal struct preserves sequence fields and source defaults without
  an invented sequence wrapper. The goal parameter is a representation helper,
  not an upstream field.
- Interaction names lowercase only the first basename character for camelCase;
  feedback events append Feedback to distinguish them from goal commands.
  StdSrvsInteractions, SensorMsgsInteractions, NavMsgsInteractions and
  Nav2MsgsInteractions represent the four package boundaries. No interaction
  kind is left undecided by the fixed rule.
- ClearEntireCostmap.plugins is a ROS string sequence. RIDL query parameters
  must be named types or streams, and a named scalar cannot alias an array. The
  plugins parameter therefore carries the existing one-field
  ClearEntireCostmapRequest struct, whose plugins field retains the sequence.
  This unavoidable extra call-level nesting changes the parameter representation
  only; the source field and request section remain intact. It is the only
  service request field requiring this representation deviation.
- Constants cannot be nested in RIDL structs. They remain grouped immediately
  after their source definition's structs, prefixed with the SCREAMING_SNAKE
  section-helper name to avoid collisions. An explicit reference comment stays
  in each constant's original position. Original adjacent and inline comments
  remain there; actual constants repeat their inline comments. The exhaustive
  mapping records every namespace change. No constants become enum values.
- bool becomes boolean; byte and char are unsigned 8-bit integers. Signed
  int8/16/32/64 and unsigned uint8/16/32 retain their exact full integer
  domains. Full-range uint64 is outside RIDL's int64 domain and becomes exactly
  eight opaque bytes, following typl section 4.2. All 64 bits survive; numeric
  arithmetic and unsigned ordering do not. Byte order is unspecified because
  these selected source definitions do not state one.
- float32 and float64 become unconstrained float-backed helpers. RIDL resolves
  both to float64 without an invented quantization grid. float32's original
  precision and wire width cannot be expressed exactly. NaN and infinity remain
  representable because neither ranges nor step restrictions are added. Numeric
  sentinels, bitfields, equal-length conditions and conditional validity stay in
  their original comments; no closed enums or optional fields are invented.
- ROS unbounded sequences cannot be represented exactly: RIDL requires finite
  bounds. Sequences remain variable-length arrays accepting empty values with
  the maximal u64 IR bound, [T; 0..18446744073709551615]. This mechanical
  ceiling is a RIDL representation limitation, not an upstream or application
  limit. Fixed arrays keep their exact sizes. ROS strings use the same maximal
  finite scalar-count bound to avoid RIDL's implicit 256-character default. ROS
  UTF-8 byte-count length and RIDL Unicode scalar-count length differ. This port
  is not a byte-compatible ROS serialization schema.
- Explicit defaults become field init values. Integer-looking float defaults are
  spelled with a decimal point as RIDL requires. Query and command parameters
  cannot have init values; defaults remain in the request/goal structs. ROS
  implicit zero, false, empty-string and empty-sequence construction is not
  uniformly guaranteed by RIDL's derived initialization. No implicit ROS
  defaults are invented in source. All explicit source defaults are listed
  below.
- Unit helpers represent only existing fields whose comments state units. UCUM
  conversions: kg-m^2 to kg.m2, Ah to A.h, m^2 to m2, Degrees Celsius to Cel,
  Pascals to Pa, Lux to lx, seconds to s, and unchanged kg, m, V and A. Units
  are not inferred from names or from covariance/variance relationships.
- Prose-only validity conditions and numeric domain statements remain comments.
  Only primitive integer widths and Time/Duration's explicitly stated nanosecond
  domain [0, 1e9) become ranges (closed [0..999999999]), avoiding stricter NaN
  sentinel validation and new enforcement of prose conditions.

## Units retained only in comments

- geometry_msgs/Inertia.com: metres on an existing Vector3 composite. RIDL
  cannot annotate that field with a scalar unit without replacing or splitting
  it.
- sensor_msgs/Imu.angular_velocity: rad/sec contains rad outside the curated
  atom table. linear_acceleration: m/s^2 maps to curated m/s2 but is a Vector3.
  Neither composite is replaced; the explicitly rejected g's are not assigned.
- sensor_msgs/MagneticField.magnetic_field: Tesla is curated as T, but the field
  is an existing Vector3. No replacement component struct is invented.
- sensor_msgs/MultiEchoLaserScan.ranges: metre units refer to LaserEcho values;
  the same LaserEcho also carries intensities, preventing a global metre unit.
- sensor_msgs/LaserScan and MultiEchoLaserScan angle_min, angle_max and
  angle_increment, and Range.field_of_view: rad is outside the curated table.
- sensor_msgs/NavSatFix.latitude and longitude: degrees (deg) are outside the
  curated table. Its altitude m and position_covariance m2 are retained.
- sensor_msgs/JointState.position, velocity and effort: conditional rad or m,
  rad/s or m/s, and Nm or N per joint. One scalar unit would change the mixed
  sequence's semantics; rad is also unavailable.
- nav_msgs/MapMetaData.resolution and nav2_msgs/CostmapMetaData.resolution:
  m/cell contains the unavailable cell atom. No metre substitute is assigned.
  Their origin Pose fields document mixed m, m, rad, which cannot be attached to
  a composite and include an unavailable atom. MapMetaData.width and height use
  cells, also unavailable.
- builtin_interfaces/Time and Duration sec and nanosec: seconds and nanoseconds
  on integer fields. RIDL unit scalars are float-backed; s/ns would discard the
  integer representation. All original comments survive. The sec integer width
  is retained; nanosec's uint32 wire width is represented by the stricter
  upstream comment-stated nanosecond domain [0..999999999], as justified above.
  Header.stamp likewise describes seconds/nanoseconds on a Time composite.
- nav2_msgs/SpeedLimit.speed_limit: percent or m/s conditional on percentage,
  plus a 0.0 no-limit sentinel. Both are curated but a single unit would alter a
  branch. BatteryState.percentage states a fraction from 0 to 1; no percent
  rescaling is performed. All conditional and sentinel wording remains.
- LaserScan/MultiEchoLaserScan.intensities: device-specific units are
  unavailable. LaserEcho.echoes can contain ranges or intensities, so no one
  unit is assigned.
- CameraInfo height/width and RegionOfInterest offsets: pixels are unavailable.
  Image.step, PointCloud2.point_step/row_step and MultiArrayLayout.data_offset:
  bytes are not a curated physical unit and the fields remain integers.
  MultiArrayDimension.size's type units are unspecified. PointField and Image
  datatype bit widths describe representation, not physical units.
- Illuminance's candela, nits and watt/area examples describe excluded readings,
  not the illuminance field. No unit is assigned from these examples.
- BatteryState.cell_voltage and cell_temperature comments do not explicitly name
  Volts or Degrees Celsius; related field names do not justify assigning units.

## Exact inventory and exhaustive source-to-output mapping

All selected paths are repeated below with source repository, pin, output file,
every declaration, service/action section, interaction and constant. Field
tables record original spelling, qualified output, representation and explicit
values. Time, Duration and TrackingFeedback are required dependency definitions.

### `common-interfaces/std_msgs/msg/Bool.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Bool`.

| Source field or constant | Qualified output    | Representation | Explicit default or constant value |
| ------------------------ | ------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Bool.data` | `RosBoolean`   | —                                  |

### `common-interfaces/std_msgs/msg/Byte.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Byte`.

| Source field or constant | Qualified output    | Representation | Explicit default or constant value |
| ------------------------ | ------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Byte.data` | `RosByte`      | —                                  |

### `common-interfaces/std_msgs/msg/ByteMultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.ByteMultiArray`.

| Source field or constant | Qualified output                | Representation                       | Explicit default or constant value |
| ------------------------ | ------------------------------- | ------------------------------------ | ---------------------------------- |
| `layout`                 | `stdmsgs.ByteMultiArray.layout` | `MultiArrayLayout`                   | —                                  |
| `data`                   | `stdmsgs.ByteMultiArray.data`   | `[RosByte; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Char.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Char`.

| Source field or constant | Qualified output    | Representation | Explicit default or constant value |
| ------------------------ | ------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Char.data` | `RosChar`      | —                                  |

### `common-interfaces/std_msgs/msg/ColorRGBA.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.ColorRGBA`.

| Source field or constant | Qualified output      | Representation | Explicit default or constant value |
| ------------------------ | --------------------- | -------------- | ---------------------------------- |
| `r`                      | `stdmsgs.ColorRGBA.r` | `RosFloat32`   | —                                  |
| `g`                      | `stdmsgs.ColorRGBA.g` | `RosFloat32`   | —                                  |
| `b`                      | `stdmsgs.ColorRGBA.b` | `RosFloat32`   | —                                  |
| `a`                      | `stdmsgs.ColorRGBA.a` | `RosFloat32`   | —                                  |

### `common-interfaces/std_msgs/msg/Empty.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Empty`.

### `common-interfaces/std_msgs/msg/Float32.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Float32`.

| Source field or constant | Qualified output       | Representation | Explicit default or constant value |
| ------------------------ | ---------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Float32.data` | `RosFloat32`   | —                                  |

### `common-interfaces/std_msgs/msg/Float32MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Float32MultiArray`.

| Source field or constant | Qualified output                   | Representation                          | Explicit default or constant value |
| ------------------------ | ---------------------------------- | --------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.Float32MultiArray.layout` | `MultiArrayLayout`                      | —                                  |
| `data`                   | `stdmsgs.Float32MultiArray.data`   | `[RosFloat32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Float64.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Float64`.

| Source field or constant | Qualified output       | Representation | Explicit default or constant value |
| ------------------------ | ---------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Float64.data` | `RosFloat64`   | —                                  |

### `common-interfaces/std_msgs/msg/Float64MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Float64MultiArray`.

| Source field or constant | Qualified output                   | Representation                          | Explicit default or constant value |
| ------------------------ | ---------------------------------- | --------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.Float64MultiArray.layout` | `MultiArrayLayout`                      | —                                  |
| `data`                   | `stdmsgs.Float64MultiArray.data`   | `[RosFloat64; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Header.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Header`.

| Source field or constant | Qualified output         | Representation | Explicit default or constant value |
| ------------------------ | ------------------------ | -------------- | ---------------------------------- |
| `stamp`                  | `stdmsgs.Header.stamp`   | `Time`         | —                                  |
| `frame_id`               | `stdmsgs.Header.frameId` | `RosString`    | —                                  |

### `common-interfaces/std_msgs/msg/Int16.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int16`.

| Source field or constant | Qualified output     | Representation | Explicit default or constant value |
| ------------------------ | -------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Int16.data` | `RosInt16`     | —                                  |

### `common-interfaces/std_msgs/msg/Int16MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int16MultiArray`.

| Source field or constant | Qualified output                 | Representation                        | Explicit default or constant value |
| ------------------------ | -------------------------------- | ------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.Int16MultiArray.layout` | `MultiArrayLayout`                    | —                                  |
| `data`                   | `stdmsgs.Int16MultiArray.data`   | `[RosInt16; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Int32.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int32`.

| Source field or constant | Qualified output     | Representation | Explicit default or constant value |
| ------------------------ | -------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Int32.data` | `RosInt32`     | —                                  |

### `common-interfaces/std_msgs/msg/Int32MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int32MultiArray`.

| Source field or constant | Qualified output                 | Representation                        | Explicit default or constant value |
| ------------------------ | -------------------------------- | ------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.Int32MultiArray.layout` | `MultiArrayLayout`                    | —                                  |
| `data`                   | `stdmsgs.Int32MultiArray.data`   | `[RosInt32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Int64.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int64`.

| Source field or constant | Qualified output     | Representation | Explicit default or constant value |
| ------------------------ | -------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Int64.data` | `RosInt64`     | —                                  |

### `common-interfaces/std_msgs/msg/Int64MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int64MultiArray`.

| Source field or constant | Qualified output                 | Representation                        | Explicit default or constant value |
| ------------------------ | -------------------------------- | ------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.Int64MultiArray.layout` | `MultiArrayLayout`                    | —                                  |
| `data`                   | `stdmsgs.Int64MultiArray.data`   | `[RosInt64; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/Int8.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int8`.

| Source field or constant | Qualified output    | Representation | Explicit default or constant value |
| ------------------------ | ------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.Int8.data` | `RosInt8`      | —                                  |

### `common-interfaces/std_msgs/msg/Int8MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.Int8MultiArray`.

| Source field or constant | Qualified output                | Representation                       | Explicit default or constant value |
| ------------------------ | ------------------------------- | ------------------------------------ | ---------------------------------- |
| `layout`                 | `stdmsgs.Int8MultiArray.layout` | `MultiArrayLayout`                   | —                                  |
| `data`                   | `stdmsgs.Int8MultiArray.data`   | `[RosInt8; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/MultiArrayDimension.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.MultiArrayDimension`.

| Source field or constant | Qualified output                     | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------ | -------------- | ---------------------------------- |
| `label`                  | `stdmsgs.MultiArrayDimension.label`  | `RosString`    | —                                  |
| `size`                   | `stdmsgs.MultiArrayDimension.size`   | `RosUInt32`    | —                                  |
| `stride`                 | `stdmsgs.MultiArrayDimension.stride` | `RosUInt32`    | —                                  |

### `common-interfaces/std_msgs/msg/MultiArrayLayout.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.MultiArrayLayout`.

| Source field or constant | Qualified output                      | Representation                                   | Explicit default or constant value |
| ------------------------ | ------------------------------------- | ------------------------------------------------ | ---------------------------------- |
| `dim`                    | `stdmsgs.MultiArrayLayout.dim`        | `[MultiArrayDimension; 0..18446744073709551615]` | —                                  |
| `data_offset`            | `stdmsgs.MultiArrayLayout.dataOffset` | `RosUInt32`                                      | —                                  |

### `common-interfaces/std_msgs/msg/String.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.String`.

| Source field or constant | Qualified output      | Representation | Explicit default or constant value |
| ------------------------ | --------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.String.data` | `RosString`    | —                                  |

### `common-interfaces/std_msgs/msg/UInt16.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt16`.

| Source field or constant | Qualified output      | Representation | Explicit default or constant value |
| ------------------------ | --------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.UInt16.data` | `RosUInt16`    | —                                  |

### `common-interfaces/std_msgs/msg/UInt16MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt16MultiArray`.

| Source field or constant | Qualified output                  | Representation                         | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.UInt16MultiArray.layout` | `MultiArrayLayout`                     | —                                  |
| `data`                   | `stdmsgs.UInt16MultiArray.data`   | `[RosUInt16; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/UInt32.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt32`.

| Source field or constant | Qualified output      | Representation | Explicit default or constant value |
| ------------------------ | --------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.UInt32.data` | `RosUInt32`    | —                                  |

### `common-interfaces/std_msgs/msg/UInt32MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt32MultiArray`.

| Source field or constant | Qualified output                  | Representation                         | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.UInt32MultiArray.layout` | `MultiArrayLayout`                     | —                                  |
| `data`                   | `stdmsgs.UInt32MultiArray.data`   | `[RosUInt32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/UInt64.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt64`.

| Source field or constant | Qualified output      | Representation | Explicit default or constant value |
| ------------------------ | --------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.UInt64.data` | `RosUInt64`    | —                                  |

### `common-interfaces/std_msgs/msg/UInt64MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt64MultiArray`.

| Source field or constant | Qualified output                  | Representation                         | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.UInt64MultiArray.layout` | `MultiArrayLayout`                     | —                                  |
| `data`                   | `stdmsgs.UInt64MultiArray.data`   | `[RosUInt64; 0..18446744073709551615]` | —                                  |

### `common-interfaces/std_msgs/msg/UInt8.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt8`.

| Source field or constant | Qualified output     | Representation | Explicit default or constant value |
| ------------------------ | -------------------- | -------------- | ---------------------------------- |
| `data`                   | `stdmsgs.UInt8.data` | `RosUInt8`     | —                                  |

### `common-interfaces/std_msgs/msg/UInt8MultiArray.msg`

Output: `std_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `stdmsgs.UInt8MultiArray`.

| Source field or constant | Qualified output                 | Representation                        | Explicit default or constant value |
| ------------------------ | -------------------------------- | ------------------------------------- | ---------------------------------- |
| `layout`                 | `stdmsgs.UInt8MultiArray.layout` | `MultiArrayLayout`                    | —                                  |
| `data`                   | `stdmsgs.UInt8MultiArray.data`   | `[RosUInt8; 0..18446744073709551615]` | —                                  |

### `common-interfaces/geometry_msgs/msg/Accel.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Accel`.

| Source field or constant | Qualified output             | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------- | -------------- | ---------------------------------- |
| `linear`                 | `geometrymsgs.Accel.linear`  | `Vector3`      | —                                  |
| `angular`                | `geometrymsgs.Accel.angular` | `Vector3`      | —                                  |

### `common-interfaces/geometry_msgs/msg/AccelStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.AccelStamped`.

| Source field or constant | Qualified output                   | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.AccelStamped.header` | `Header`       | —                                  |
| `accel`                  | `geometrymsgs.AccelStamped.accel`  | `Accel`        | —                                  |

### `common-interfaces/geometry_msgs/msg/AccelWithCovariance.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.AccelWithCovariance`.

| Source field or constant | Qualified output                              | Representation     | Explicit default or constant value |
| ------------------------ | --------------------------------------------- | ------------------ | ---------------------------------- |
| `accel`                  | `geometrymsgs.AccelWithCovariance.accel`      | `Accel`            | —                                  |
| `covariance`             | `geometrymsgs.AccelWithCovariance.covariance` | `[RosFloat64; 36]` | —                                  |

### `common-interfaces/geometry_msgs/msg/AccelWithCovarianceStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.AccelWithCovarianceStamped`.

| Source field or constant | Qualified output                                 | Representation        | Explicit default or constant value |
| ------------------------ | ------------------------------------------------ | --------------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.AccelWithCovarianceStamped.header` | `Header`              | —                                  |
| `accel`                  | `geometrymsgs.AccelWithCovarianceStamped.accel`  | `AccelWithCovariance` | —                                  |

### `common-interfaces/geometry_msgs/msg/Inertia.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Inertia`.

| Source field or constant | Qualified output           | Representation     | Explicit default or constant value |
| ------------------------ | -------------------------- | ------------------ | ---------------------------------- |
| `m`                      | `geometrymsgs.Inertia.m`   | `RosKilograms`     | —                                  |
| `com`                    | `geometrymsgs.Inertia.com` | `Vector3`          | —                                  |
| `ixx`                    | `geometrymsgs.Inertia.ixx` | `RosInertiaTensor` | —                                  |
| `ixy`                    | `geometrymsgs.Inertia.ixy` | `RosInertiaTensor` | —                                  |
| `ixz`                    | `geometrymsgs.Inertia.ixz` | `RosInertiaTensor` | —                                  |
| `iyy`                    | `geometrymsgs.Inertia.iyy` | `RosInertiaTensor` | —                                  |
| `iyz`                    | `geometrymsgs.Inertia.iyz` | `RosInertiaTensor` | —                                  |
| `izz`                    | `geometrymsgs.Inertia.izz` | `RosInertiaTensor` | —                                  |

### `common-interfaces/geometry_msgs/msg/InertiaStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.InertiaStamped`.

| Source field or constant | Qualified output                      | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.InertiaStamped.header`  | `Header`       | —                                  |
| `inertia`                | `geometrymsgs.InertiaStamped.inertia` | `Inertia`      | —                                  |

### `common-interfaces/geometry_msgs/msg/Point.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Point`.

| Source field or constant | Qualified output       | Representation | Explicit default or constant value |
| ------------------------ | ---------------------- | -------------- | ---------------------------------- |
| `x`                      | `geometrymsgs.Point.x` | `RosFloat64`   | —                                  |
| `y`                      | `geometrymsgs.Point.y` | `RosFloat64`   | —                                  |
| `z`                      | `geometrymsgs.Point.z` | `RosFloat64`   | —                                  |

### `common-interfaces/geometry_msgs/msg/Point32.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Point32`.

| Source field or constant | Qualified output         | Representation | Explicit default or constant value |
| ------------------------ | ------------------------ | -------------- | ---------------------------------- |
| `x`                      | `geometrymsgs.Point32.x` | `RosFloat32`   | —                                  |
| `y`                      | `geometrymsgs.Point32.y` | `RosFloat32`   | —                                  |
| `z`                      | `geometrymsgs.Point32.z` | `RosFloat32`   | —                                  |

### `common-interfaces/geometry_msgs/msg/PointStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PointStamped`.

| Source field or constant | Qualified output                   | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PointStamped.header` | `Header`       | —                                  |
| `point`                  | `geometrymsgs.PointStamped.point`  | `Point`        | —                                  |

### `common-interfaces/geometry_msgs/msg/Polygon.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Polygon`.

| Source field or constant | Qualified output              | Representation                       | Explicit default or constant value |
| ------------------------ | ----------------------------- | ------------------------------------ | ---------------------------------- |
| `points`                 | `geometrymsgs.Polygon.points` | `[Point32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/geometry_msgs/msg/PolygonInstance.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PolygonInstance`.

| Source field or constant | Qualified output                       | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------------- | -------------- | ---------------------------------- |
| `polygon`                | `geometrymsgs.PolygonInstance.polygon` | `Polygon`      | —                                  |
| `id`                     | `geometrymsgs.PolygonInstance.id`      | `RosInt64`     | —                                  |

### `common-interfaces/geometry_msgs/msg/PolygonInstanceStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PolygonInstanceStamped`.

| Source field or constant | Qualified output                              | Representation    | Explicit default or constant value |
| ------------------------ | --------------------------------------------- | ----------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PolygonInstanceStamped.header`  | `Header`          | —                                  |
| `polygon`                | `geometrymsgs.PolygonInstanceStamped.polygon` | `PolygonInstance` | —                                  |

### `common-interfaces/geometry_msgs/msg/PolygonStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PolygonStamped`.

| Source field or constant | Qualified output                      | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PolygonStamped.header`  | `Header`       | —                                  |
| `polygon`                | `geometrymsgs.PolygonStamped.polygon` | `Polygon`      | —                                  |

### `common-interfaces/geometry_msgs/msg/Pose.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Pose`.

| Source field or constant | Qualified output                | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------- | -------------- | ---------------------------------- |
| `position`               | `geometrymsgs.Pose.position`    | `Point`        | —                                  |
| `orientation`            | `geometrymsgs.Pose.orientation` | `Quaternion`   | —                                  |

### `common-interfaces/geometry_msgs/msg/PoseArray.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PoseArray`.

| Source field or constant | Qualified output                | Representation                    | Explicit default or constant value |
| ------------------------ | ------------------------------- | --------------------------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PoseArray.header` | `Header`                          | —                                  |
| `poses`                  | `geometrymsgs.PoseArray.poses`  | `[Pose; 0..18446744073709551615]` | —                                  |

### `common-interfaces/geometry_msgs/msg/PoseStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PoseStamped`.

| Source field or constant | Qualified output                  | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PoseStamped.header` | `Header`       | —                                  |
| `pose`                   | `geometrymsgs.PoseStamped.pose`   | `Pose`         | —                                  |

### `common-interfaces/geometry_msgs/msg/PoseWithCovariance.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PoseWithCovariance`.

| Source field or constant | Qualified output                             | Representation     | Explicit default or constant value |
| ------------------------ | -------------------------------------------- | ------------------ | ---------------------------------- |
| `pose`                   | `geometrymsgs.PoseWithCovariance.pose`       | `Pose`             | —                                  |
| `covariance`             | `geometrymsgs.PoseWithCovariance.covariance` | `[RosFloat64; 36]` | —                                  |

### `common-interfaces/geometry_msgs/msg/PoseWithCovarianceStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.PoseWithCovarianceStamped`.

| Source field or constant | Qualified output                                | Representation       | Explicit default or constant value |
| ------------------------ | ----------------------------------------------- | -------------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.PoseWithCovarianceStamped.header` | `Header`             | —                                  |
| `pose`                   | `geometrymsgs.PoseWithCovarianceStamped.pose`   | `PoseWithCovariance` | —                                  |

### `common-interfaces/geometry_msgs/msg/Quaternion.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Quaternion`.

| Source field or constant | Qualified output            | Representation | Explicit default or constant value |
| ------------------------ | --------------------------- | -------------- | ---------------------------------- |
| `x`                      | `geometrymsgs.Quaternion.x` | `RosFloat64`   | `0`                                |
| `y`                      | `geometrymsgs.Quaternion.y` | `RosFloat64`   | `0`                                |
| `z`                      | `geometrymsgs.Quaternion.z` | `RosFloat64`   | `0`                                |
| `w`                      | `geometrymsgs.Quaternion.w` | `RosFloat64`   | `1`                                |

### `common-interfaces/geometry_msgs/msg/QuaternionStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.QuaternionStamped`.

| Source field or constant | Qualified output                            | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.QuaternionStamped.header`     | `Header`       | —                                  |
| `quaternion`             | `geometrymsgs.QuaternionStamped.quaternion` | `Quaternion`   | —                                  |

### `common-interfaces/geometry_msgs/msg/Transform.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Transform`.

| Source field or constant | Qualified output                     | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------ | -------------- | ---------------------------------- |
| `translation`            | `geometrymsgs.Transform.translation` | `Vector3`      | —                                  |
| `rotation`               | `geometrymsgs.Transform.rotation`    | `Quaternion`   | —                                  |

### `common-interfaces/geometry_msgs/msg/TransformStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.TransformStamped`.

| Source field or constant | Qualified output                             | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.TransformStamped.header`       | `Header`       | —                                  |
| `child_frame_id`         | `geometrymsgs.TransformStamped.childFrameId` | `RosString`    | —                                  |
| `transform`              | `geometrymsgs.TransformStamped.transform`    | `Transform`    | —                                  |

### `common-interfaces/geometry_msgs/msg/Twist.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Twist`.

| Source field or constant | Qualified output             | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------- | -------------- | ---------------------------------- |
| `linear`                 | `geometrymsgs.Twist.linear`  | `Vector3`      | —                                  |
| `angular`                | `geometrymsgs.Twist.angular` | `Vector3`      | —                                  |

### `common-interfaces/geometry_msgs/msg/TwistStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.TwistStamped`.

| Source field or constant | Qualified output                   | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.TwistStamped.header` | `Header`       | —                                  |
| `twist`                  | `geometrymsgs.TwistStamped.twist`  | `Twist`        | —                                  |

### `common-interfaces/geometry_msgs/msg/TwistWithCovariance.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.TwistWithCovariance`.

| Source field or constant | Qualified output                              | Representation     | Explicit default or constant value |
| ------------------------ | --------------------------------------------- | ------------------ | ---------------------------------- |
| `twist`                  | `geometrymsgs.TwistWithCovariance.twist`      | `Twist`            | —                                  |
| `covariance`             | `geometrymsgs.TwistWithCovariance.covariance` | `[RosFloat64; 36]` | —                                  |

### `common-interfaces/geometry_msgs/msg/TwistWithCovarianceStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.TwistWithCovarianceStamped`.

| Source field or constant | Qualified output                                 | Representation        | Explicit default or constant value |
| ------------------------ | ------------------------------------------------ | --------------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.TwistWithCovarianceStamped.header` | `Header`              | —                                  |
| `twist`                  | `geometrymsgs.TwistWithCovarianceStamped.twist`  | `TwistWithCovariance` | —                                  |

### `common-interfaces/geometry_msgs/msg/Vector3.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Vector3`.

| Source field or constant | Qualified output         | Representation | Explicit default or constant value |
| ------------------------ | ------------------------ | -------------- | ---------------------------------- |
| `x`                      | `geometrymsgs.Vector3.x` | `RosFloat64`   | —                                  |
| `y`                      | `geometrymsgs.Vector3.y` | `RosFloat64`   | —                                  |
| `z`                      | `geometrymsgs.Vector3.z` | `RosFloat64`   | —                                  |

### `common-interfaces/geometry_msgs/msg/Vector3Stamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Vector3Stamped`.

| Source field or constant | Qualified output                     | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------ | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.Vector3Stamped.header` | `Header`       | —                                  |
| `vector`                 | `geometrymsgs.Vector3Stamped.vector` | `Vector3`      | —                                  |

### `common-interfaces/geometry_msgs/msg/VelocityStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.VelocityStamped`.

| Source field or constant | Qualified output                                | Representation | Explicit default or constant value |
| ------------------------ | ----------------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.VelocityStamped.header`           | `Header`       | —                                  |
| `body_frame_id`          | `geometrymsgs.VelocityStamped.bodyFrameId`      | `RosString`    | —                                  |
| `reference_frame_id`     | `geometrymsgs.VelocityStamped.referenceFrameId` | `RosString`    | —                                  |
| `velocity`               | `geometrymsgs.VelocityStamped.velocity`         | `Twist`        | —                                  |

### `common-interfaces/geometry_msgs/msg/VelocityWithCovarianceStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.VelocityWithCovarianceStamped`.

| Source field or constant | Qualified output                                              | Representation        | Explicit default or constant value |
| ------------------------ | ------------------------------------------------------------- | --------------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.VelocityWithCovarianceStamped.header`           | `Header`              | —                                  |
| `body_frame_id`          | `geometrymsgs.VelocityWithCovarianceStamped.bodyFrameId`      | `RosString`           | —                                  |
| `reference_frame_id`     | `geometrymsgs.VelocityWithCovarianceStamped.referenceFrameId` | `RosString`           | —                                  |
| `velocity`               | `geometrymsgs.VelocityWithCovarianceStamped.velocity`         | `TwistWithCovariance` | —                                  |

### `common-interfaces/geometry_msgs/msg/Wrench.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.Wrench`.

| Source field or constant | Qualified output             | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------- | -------------- | ---------------------------------- |
| `force`                  | `geometrymsgs.Wrench.force`  | `Vector3`      | —                                  |
| `torque`                 | `geometrymsgs.Wrench.torque` | `Vector3`      | —                                  |

### `common-interfaces/geometry_msgs/msg/WrenchStamped.msg`

Output: `geometry_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `geometrymsgs.WrenchStamped`.

| Source field or constant | Qualified output                    | Representation | Explicit default or constant value |
| ------------------------ | ----------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `geometrymsgs.WrenchStamped.header` | `Header`       | —                                  |
| `wrench`                 | `geometrymsgs.WrenchStamped.wrench` | `Wrench`       | —                                  |

### `common-interfaces/sensor_msgs/msg/BatteryState.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.BatteryState`.
- constant POWER_SUPPLY_STATUS_UNKNOWN:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_UNKNOWN`.
- constant POWER_SUPPLY_STATUS_CHARGING:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_CHARGING`.
- constant POWER_SUPPLY_STATUS_DISCHARGING:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_DISCHARGING`.
- constant POWER_SUPPLY_STATUS_NOT_CHARGING:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_NOT_CHARGING`.
- constant POWER_SUPPLY_STATUS_FULL:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_FULL`.
- constant POWER_SUPPLY_HEALTH_UNKNOWN:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNKNOWN`.
- constant POWER_SUPPLY_HEALTH_GOOD:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_GOOD`.
- constant POWER_SUPPLY_HEALTH_OVERHEAT:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERHEAT`.
- constant POWER_SUPPLY_HEALTH_DEAD:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_DEAD`.
- constant POWER_SUPPLY_HEALTH_OVERVOLTAGE:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERVOLTAGE`.
- constant POWER_SUPPLY_HEALTH_UNSPEC_FAILURE:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNSPEC_FAILURE`.
- constant POWER_SUPPLY_HEALTH_COLD:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_COLD`.
- constant POWER_SUPPLY_HEALTH_WATCHDOG_TIMER_EXPIRE:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_WATCHDOG_TIMER_EXPIRE`.
- constant POWER_SUPPLY_HEALTH_SAFETY_TIMER_EXPIRE:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_SAFETY_TIMER_EXPIRE`.
- constant POWER_SUPPLY_TECHNOLOGY_UNKNOWN:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_UNKNOWN`.
- constant POWER_SUPPLY_TECHNOLOGY_NIMH:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NIMH`.
- constant POWER_SUPPLY_TECHNOLOGY_LION:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LION`.
- constant POWER_SUPPLY_TECHNOLOGY_LIPO:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIPO`.
- constant POWER_SUPPLY_TECHNOLOGY_LIFE:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIFE`.
- constant POWER_SUPPLY_TECHNOLOGY_NICD:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NICD`.
- constant POWER_SUPPLY_TECHNOLOGY_LIMN:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIMN`.
- constant POWER_SUPPLY_TECHNOLOGY_TERNARY:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_TERNARY`.
- constant POWER_SUPPLY_TECHNOLOGY_VRLA:
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_VRLA`.

| Source field or constant                             | Qualified output                                                     | Representation                          | Explicit default or constant value |
| ---------------------------------------------------- | -------------------------------------------------------------------- | --------------------------------------- | ---------------------------------- |
| `constant POWER_SUPPLY_STATUS_UNKNOWN`               | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_UNKNOWN`               | `RosUInt8`                              | `0`                                |
| `constant POWER_SUPPLY_STATUS_CHARGING`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_CHARGING`              | `RosUInt8`                              | `1`                                |
| `constant POWER_SUPPLY_STATUS_DISCHARGING`           | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_DISCHARGING`           | `RosUInt8`                              | `2`                                |
| `constant POWER_SUPPLY_STATUS_NOT_CHARGING`          | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_NOT_CHARGING`          | `RosUInt8`                              | `3`                                |
| `constant POWER_SUPPLY_STATUS_FULL`                  | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_FULL`                  | `RosUInt8`                              | `4`                                |
| `constant POWER_SUPPLY_HEALTH_UNKNOWN`               | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNKNOWN`               | `RosUInt8`                              | `0`                                |
| `constant POWER_SUPPLY_HEALTH_GOOD`                  | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_GOOD`                  | `RosUInt8`                              | `1`                                |
| `constant POWER_SUPPLY_HEALTH_OVERHEAT`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERHEAT`              | `RosUInt8`                              | `2`                                |
| `constant POWER_SUPPLY_HEALTH_DEAD`                  | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_DEAD`                  | `RosUInt8`                              | `3`                                |
| `constant POWER_SUPPLY_HEALTH_OVERVOLTAGE`           | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERVOLTAGE`           | `RosUInt8`                              | `4`                                |
| `constant POWER_SUPPLY_HEALTH_UNSPEC_FAILURE`        | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNSPEC_FAILURE`        | `RosUInt8`                              | `5`                                |
| `constant POWER_SUPPLY_HEALTH_COLD`                  | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_COLD`                  | `RosUInt8`                              | `6`                                |
| `constant POWER_SUPPLY_HEALTH_WATCHDOG_TIMER_EXPIRE` | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_WATCHDOG_TIMER_EXPIRE` | `RosUInt8`                              | `7`                                |
| `constant POWER_SUPPLY_HEALTH_SAFETY_TIMER_EXPIRE`   | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_SAFETY_TIMER_EXPIRE`   | `RosUInt8`                              | `8`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_UNKNOWN`           | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_UNKNOWN`           | `RosUInt8`                              | `0`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_NIMH`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NIMH`              | `RosUInt8`                              | `1`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_LION`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LION`              | `RosUInt8`                              | `2`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_LIPO`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIPO`              | `RosUInt8`                              | `3`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_LIFE`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIFE`              | `RosUInt8`                              | `4`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_NICD`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NICD`              | `RosUInt8`                              | `5`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_LIMN`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIMN`              | `RosUInt8`                              | `6`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_TERNARY`           | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_TERNARY`           | `RosUInt8`                              | `7`                                |
| `constant POWER_SUPPLY_TECHNOLOGY_VRLA`              | `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_VRLA`              | `RosUInt8`                              | `8`                                |
| `header`                                             | `sensormsgs.BatteryState.header`                                     | `Header`                                | —                                  |
| `voltage`                                            | `sensormsgs.BatteryState.voltage`                                    | `RosVolts`                              | —                                  |
| `temperature`                                        | `sensormsgs.BatteryState.temperature`                                | `RosCelsius`                            | —                                  |
| `current`                                            | `sensormsgs.BatteryState.currentValue`                               | `RosAmperes`                            | —                                  |
| `charge`                                             | `sensormsgs.BatteryState.charge`                                     | `RosAmpereHours`                        | —                                  |
| `capacity`                                           | `sensormsgs.BatteryState.capacity`                                   | `RosAmpereHours`                        | —                                  |
| `design_capacity`                                    | `sensormsgs.BatteryState.designCapacity`                             | `RosAmpereHours`                        | —                                  |
| `percentage`                                         | `sensormsgs.BatteryState.percentage`                                 | `RosFloat32`                            | —                                  |
| `power_supply_status`                                | `sensormsgs.BatteryState.powerSupplyStatus`                          | `RosUInt8`                              | —                                  |
| `power_supply_health`                                | `sensormsgs.BatteryState.powerSupplyHealth`                          | `RosUInt8`                              | —                                  |
| `power_supply_technology`                            | `sensormsgs.BatteryState.powerSupplyTechnology`                      | `RosUInt8`                              | —                                  |
| `present`                                            | `sensormsgs.BatteryState.present`                                    | `RosBoolean`                            | —                                  |
| `cell_voltage`                                       | `sensormsgs.BatteryState.cellVoltage`                                | `[RosFloat32; 0..18446744073709551615]` | —                                  |
| `cell_temperature`                                   | `sensormsgs.BatteryState.cellTemperature`                            | `[RosFloat32; 0..18446744073709551615]` | —                                  |
| `location`                                           | `sensormsgs.BatteryState.location`                                   | `RosString`                             | —                                  |
| `serial_number`                                      | `sensormsgs.BatteryState.serialNumber`                               | `RosString`                             | —                                  |

### `common-interfaces/sensor_msgs/msg/CameraInfo.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.CameraInfo`.

| Source field or constant | Qualified output                        | Representation                          | Explicit default or constant value |
| ------------------------ | --------------------------------------- | --------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.CameraInfo.header`          | `Header`                                | —                                  |
| `height`                 | `sensormsgs.CameraInfo.height`          | `RosUInt32`                             | —                                  |
| `width`                  | `sensormsgs.CameraInfo.width`           | `RosUInt32`                             | —                                  |
| `distortion_model`       | `sensormsgs.CameraInfo.distortionModel` | `RosString`                             | —                                  |
| `d`                      | `sensormsgs.CameraInfo.d`               | `[RosFloat64; 0..18446744073709551615]` | —                                  |
| `k`                      | `sensormsgs.CameraInfo.k`               | `[RosFloat64; 9]`                       | —                                  |
| `r`                      | `sensormsgs.CameraInfo.r`               | `[RosFloat64; 9]`                       | —                                  |
| `p`                      | `sensormsgs.CameraInfo.p`               | `[RosFloat64; 12]`                      | —                                  |
| `binning_x`              | `sensormsgs.CameraInfo.binningX`        | `RosUInt32`                             | —                                  |
| `binning_y`              | `sensormsgs.CameraInfo.binningY`        | `RosUInt32`                             | —                                  |
| `roi`                    | `sensormsgs.CameraInfo.roi`             | `RegionOfInterest`                      | —                                  |

### `common-interfaces/sensor_msgs/msg/ChannelFloat32.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.ChannelFloat32`.

| Source field or constant | Qualified output                   | Representation                          | Explicit default or constant value |
| ------------------------ | ---------------------------------- | --------------------------------------- | ---------------------------------- |
| `name`                   | `sensormsgs.ChannelFloat32.name`   | `RosString`                             | —                                  |
| `values`                 | `sensormsgs.ChannelFloat32.values` | `[RosFloat32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/CompressedImage.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.CompressedImage`.

| Source field or constant | Qualified output                    | Representation                        | Explicit default or constant value |
| ------------------------ | ----------------------------------- | ------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.CompressedImage.header` | `Header`                              | —                                  |
| `format`                 | `sensormsgs.CompressedImage.format` | `RosString`                           | —                                  |
| `data`                   | `sensormsgs.CompressedImage.data`   | `[RosUInt8; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/FluidPressure.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.FluidPressure`.

| Source field or constant | Qualified output                         | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.FluidPressure.header`        | `Header`       | —                                  |
| `fluid_pressure`         | `sensormsgs.FluidPressure.fluidPressure` | `RosPascals`   | —                                  |
| `variance`               | `sensormsgs.FluidPressure.variance`      | `RosFloat64`   | —                                  |

### `common-interfaces/sensor_msgs/msg/Illuminance.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Illuminance`.

| Source field or constant | Qualified output                     | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------ | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.Illuminance.header`      | `Header`       | —                                  |
| `illuminance`            | `sensormsgs.Illuminance.illuminance` | `RosLux`       | —                                  |
| `variance`               | `sensormsgs.Illuminance.variance`    | `RosFloat64`   | —                                  |

### `common-interfaces/sensor_msgs/msg/Image.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Image`.

| Source field or constant | Qualified output               | Representation                        | Explicit default or constant value |
| ------------------------ | ------------------------------ | ------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.Image.header`      | `Header`                              | —                                  |
| `height`                 | `sensormsgs.Image.height`      | `RosUInt32`                           | —                                  |
| `width`                  | `sensormsgs.Image.width`       | `RosUInt32`                           | —                                  |
| `encoding`               | `sensormsgs.Image.encoding`    | `RosString`                           | —                                  |
| `is_bigendian`           | `sensormsgs.Image.isBigendian` | `RosUInt8`                            | —                                  |
| `step`                   | `sensormsgs.Image.stepValue`   | `RosUInt32`                           | —                                  |
| `data`                   | `sensormsgs.Image.data`        | `[RosUInt8; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/Imu.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Imu`.

| Source field or constant         | Qualified output                              | Representation    | Explicit default or constant value |
| -------------------------------- | --------------------------------------------- | ----------------- | ---------------------------------- |
| `header`                         | `sensormsgs.Imu.header`                       | `Header`          | —                                  |
| `orientation`                    | `sensormsgs.Imu.orientation`                  | `Quaternion`      | —                                  |
| `orientation_covariance`         | `sensormsgs.Imu.orientationCovariance`        | `[RosFloat64; 9]` | —                                  |
| `angular_velocity`               | `sensormsgs.Imu.angularVelocity`              | `Vector3`         | —                                  |
| `angular_velocity_covariance`    | `sensormsgs.Imu.angularVelocityCovariance`    | `[RosFloat64; 9]` | —                                  |
| `linear_acceleration`            | `sensormsgs.Imu.linearAcceleration`           | `Vector3`         | —                                  |
| `linear_acceleration_covariance` | `sensormsgs.Imu.linearAccelerationCovariance` | `[RosFloat64; 9]` | —                                  |

### `common-interfaces/sensor_msgs/msg/JointState.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.JointState`.

| Source field or constant | Qualified output                 | Representation                          | Explicit default or constant value |
| ------------------------ | -------------------------------- | --------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.JointState.header`   | `Header`                                | —                                  |
| `name`                   | `sensormsgs.JointState.name`     | `[RosString; 0..18446744073709551615]`  | —                                  |
| `position`               | `sensormsgs.JointState.position` | `[RosFloat64; 0..18446744073709551615]` | —                                  |
| `velocity`               | `sensormsgs.JointState.velocity` | `[RosFloat64; 0..18446744073709551615]` | —                                  |
| `effort`                 | `sensormsgs.JointState.effort`   | `[RosFloat64; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/Joy.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Joy`.

| Source field or constant | Qualified output         | Representation                          | Explicit default or constant value |
| ------------------------ | ------------------------ | --------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.Joy.header`  | `Header`                                | —                                  |
| `axes`                   | `sensormsgs.Joy.axes`    | `[RosFloat32; 0..18446744073709551615]` | —                                  |
| `buttons`                | `sensormsgs.Joy.buttons` | `[RosInt32; 0..18446744073709551615]`   | —                                  |

### `common-interfaces/sensor_msgs/msg/JoyFeedback.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.JoyFeedback`.
- constant TYPE_LED: `sensormsgs.JOY_FEEDBACK_TYPE_LED`.
- constant TYPE_RUMBLE: `sensormsgs.JOY_FEEDBACK_TYPE_RUMBLE`.
- constant TYPE_BUZZER: `sensormsgs.JOY_FEEDBACK_TYPE_BUZZER`.

| Source field or constant | Qualified output                      | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------- | -------------- | ---------------------------------- |
| `constant TYPE_LED`      | `sensormsgs.JOY_FEEDBACK_TYPE_LED`    | `RosUInt8`     | `0`                                |
| `constant TYPE_RUMBLE`   | `sensormsgs.JOY_FEEDBACK_TYPE_RUMBLE` | `RosUInt8`     | `1`                                |
| `constant TYPE_BUZZER`   | `sensormsgs.JOY_FEEDBACK_TYPE_BUZZER` | `RosUInt8`     | `2`                                |
| `type`                   | `sensormsgs.JoyFeedback.typeValue`    | `RosUInt8`     | —                                  |
| `id`                     | `sensormsgs.JoyFeedback.id`           | `RosUInt8`     | —                                  |
| `intensity`              | `sensormsgs.JoyFeedback.intensity`    | `RosFloat32`   | —                                  |

### `common-interfaces/sensor_msgs/msg/JoyFeedbackArray.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.JoyFeedbackArray`.

| Source field or constant | Qualified output                    | Representation                           | Explicit default or constant value |
| ------------------------ | ----------------------------------- | ---------------------------------------- | ---------------------------------- |
| `array`                  | `sensormsgs.JoyFeedbackArray.array` | `[JoyFeedback; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/LaserEcho.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.LaserEcho`.

| Source field or constant | Qualified output              | Representation                          | Explicit default or constant value |
| ------------------------ | ----------------------------- | --------------------------------------- | ---------------------------------- |
| `echoes`                 | `sensormsgs.LaserEcho.echoes` | `[RosFloat32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/LaserScan.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.LaserScan`.

| Source field or constant | Qualified output                      | Representation                          | Explicit default or constant value |
| ------------------------ | ------------------------------------- | --------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.LaserScan.header`         | `Header`                                | —                                  |
| `angle_min`              | `sensormsgs.LaserScan.angleMin`       | `RosFloat32`                            | —                                  |
| `angle_max`              | `sensormsgs.LaserScan.angleMax`       | `RosFloat32`                            | —                                  |
| `angle_increment`        | `sensormsgs.LaserScan.angleIncrement` | `RosFloat32`                            | —                                  |
| `time_increment`         | `sensormsgs.LaserScan.timeIncrement`  | `RosSeconds`                            | —                                  |
| `scan_time`              | `sensormsgs.LaserScan.scanTime`       | `RosSeconds`                            | —                                  |
| `range_min`              | `sensormsgs.LaserScan.rangeMin`       | `RosMetres`                             | —                                  |
| `range_max`              | `sensormsgs.LaserScan.rangeMax`       | `RosMetres`                             | —                                  |
| `ranges`                 | `sensormsgs.LaserScan.ranges`         | `[RosMetres; 0..18446744073709551615]`  | —                                  |
| `intensities`            | `sensormsgs.LaserScan.intensities`    | `[RosFloat32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/MagneticField.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.MagneticField`.

| Source field or constant    | Qualified output                                   | Representation    | Explicit default or constant value |
| --------------------------- | -------------------------------------------------- | ----------------- | ---------------------------------- |
| `header`                    | `sensormsgs.MagneticField.header`                  | `Header`          | —                                  |
| `magnetic_field`            | `sensormsgs.MagneticField.magneticField`           | `Vector3`         | —                                  |
| `magnetic_field_covariance` | `sensormsgs.MagneticField.magneticFieldCovariance` | `[RosFloat64; 9]` | —                                  |

### `common-interfaces/sensor_msgs/msg/MultiDOFJointState.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.MultiDOFJointState`.

| Source field or constant | Qualified output                           | Representation                         | Explicit default or constant value |
| ------------------------ | ------------------------------------------ | -------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.MultiDOFJointState.header`     | `Header`                               | —                                  |
| `joint_names`            | `sensormsgs.MultiDOFJointState.jointNames` | `[RosString; 0..18446744073709551615]` | —                                  |
| `transforms`             | `sensormsgs.MultiDOFJointState.transforms` | `[Transform; 0..18446744073709551615]` | —                                  |
| `twist`                  | `sensormsgs.MultiDOFJointState.twist`      | `[Twist; 0..18446744073709551615]`     | —                                  |
| `wrench`                 | `sensormsgs.MultiDOFJointState.wrench`     | `[Wrench; 0..18446744073709551615]`    | —                                  |

### `common-interfaces/sensor_msgs/msg/MultiEchoLaserScan.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.MultiEchoLaserScan`.

| Source field or constant | Qualified output                               | Representation                         | Explicit default or constant value |
| ------------------------ | ---------------------------------------------- | -------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.MultiEchoLaserScan.header`         | `Header`                               | —                                  |
| `angle_min`              | `sensormsgs.MultiEchoLaserScan.angleMin`       | `RosFloat32`                           | —                                  |
| `angle_max`              | `sensormsgs.MultiEchoLaserScan.angleMax`       | `RosFloat32`                           | —                                  |
| `angle_increment`        | `sensormsgs.MultiEchoLaserScan.angleIncrement` | `RosFloat32`                           | —                                  |
| `time_increment`         | `sensormsgs.MultiEchoLaserScan.timeIncrement`  | `RosSeconds`                           | —                                  |
| `scan_time`              | `sensormsgs.MultiEchoLaserScan.scanTime`       | `RosSeconds`                           | —                                  |
| `range_min`              | `sensormsgs.MultiEchoLaserScan.rangeMin`       | `RosMetres`                            | —                                  |
| `range_max`              | `sensormsgs.MultiEchoLaserScan.rangeMax`       | `RosMetres`                            | —                                  |
| `ranges`                 | `sensormsgs.MultiEchoLaserScan.ranges`         | `[LaserEcho; 0..18446744073709551615]` | —                                  |
| `intensities`            | `sensormsgs.MultiEchoLaserScan.intensities`    | `[LaserEcho; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/NavSatFix.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.NavSatFix`.
- constant COVARIANCE_TYPE_UNKNOWN:
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_UNKNOWN`.
- constant COVARIANCE_TYPE_APPROXIMATED:
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_APPROXIMATED`.
- constant COVARIANCE_TYPE_DIAGONAL_KNOWN:
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_DIAGONAL_KNOWN`.
- constant COVARIANCE_TYPE_KNOWN:
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_KNOWN`.

| Source field or constant                  | Qualified output                                        | Representation         | Explicit default or constant value |
| ----------------------------------------- | ------------------------------------------------------- | ---------------------- | ---------------------------------- |
| `header`                                  | `sensormsgs.NavSatFix.header`                           | `Header`               | —                                  |
| `status`                                  | `sensormsgs.NavSatFix.status`                           | `NavSatStatus`         | —                                  |
| `latitude`                                | `sensormsgs.NavSatFix.latitude`                         | `RosFloat64`           | —                                  |
| `longitude`                               | `sensormsgs.NavSatFix.longitude`                        | `RosFloat64`           | —                                  |
| `altitude`                                | `sensormsgs.NavSatFix.altitude`                         | `RosMetres`            | —                                  |
| `position_covariance`                     | `sensormsgs.NavSatFix.positionCovariance`               | `[RosSquareMetres; 9]` | —                                  |
| `constant COVARIANCE_TYPE_UNKNOWN`        | `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_UNKNOWN`        | `RosUInt8`             | `0`                                |
| `constant COVARIANCE_TYPE_APPROXIMATED`   | `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_APPROXIMATED`   | `RosUInt8`             | `1`                                |
| `constant COVARIANCE_TYPE_DIAGONAL_KNOWN` | `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_DIAGONAL_KNOWN` | `RosUInt8`             | `2`                                |
| `constant COVARIANCE_TYPE_KNOWN`          | `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_KNOWN`          | `RosUInt8`             | `3`                                |
| `position_covariance_type`                | `sensormsgs.NavSatFix.positionCovarianceType`           | `RosUInt8`             | —                                  |

### `common-interfaces/sensor_msgs/msg/NavSatStatus.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.NavSatStatus`.
- constant STATUS_UNKNOWN: `sensormsgs.NAV_SAT_STATUS_STATUS_UNKNOWN`.
- constant STATUS_NO_FIX: `sensormsgs.NAV_SAT_STATUS_STATUS_NO_FIX`.
- constant STATUS_FIX: `sensormsgs.NAV_SAT_STATUS_STATUS_FIX`.
- constant STATUS_SBAS_FIX: `sensormsgs.NAV_SAT_STATUS_STATUS_SBAS_FIX`.
- constant STATUS_GBAS_FIX: `sensormsgs.NAV_SAT_STATUS_STATUS_GBAS_FIX`.
- constant SERVICE_UNKNOWN: `sensormsgs.NAV_SAT_STATUS_SERVICE_UNKNOWN`.
- constant SERVICE_GPS: `sensormsgs.NAV_SAT_STATUS_SERVICE_GPS`.
- constant SERVICE_GLONASS: `sensormsgs.NAV_SAT_STATUS_SERVICE_GLONASS`.
- constant SERVICE_COMPASS: `sensormsgs.NAV_SAT_STATUS_SERVICE_COMPASS`.
- constant SERVICE_GALILEO: `sensormsgs.NAV_SAT_STATUS_SERVICE_GALILEO`.

| Source field or constant   | Qualified output                            | Representation | Explicit default or constant value |
| -------------------------- | ------------------------------------------- | -------------- | ---------------------------------- |
| `constant STATUS_UNKNOWN`  | `sensormsgs.NAV_SAT_STATUS_STATUS_UNKNOWN`  | `RosInt8`      | `-2`                               |
| `constant STATUS_NO_FIX`   | `sensormsgs.NAV_SAT_STATUS_STATUS_NO_FIX`   | `RosInt8`      | `-1`                               |
| `constant STATUS_FIX`      | `sensormsgs.NAV_SAT_STATUS_STATUS_FIX`      | `RosInt8`      | `0`                                |
| `constant STATUS_SBAS_FIX` | `sensormsgs.NAV_SAT_STATUS_STATUS_SBAS_FIX` | `RosInt8`      | `1`                                |
| `constant STATUS_GBAS_FIX` | `sensormsgs.NAV_SAT_STATUS_STATUS_GBAS_FIX` | `RosInt8`      | `2`                                |
| `status`                   | `sensormsgs.NavSatStatus.status`            | `RosInt8`      | `-2`                               |
| `constant SERVICE_UNKNOWN` | `sensormsgs.NAV_SAT_STATUS_SERVICE_UNKNOWN` | `RosUInt16`    | `0`                                |
| `constant SERVICE_GPS`     | `sensormsgs.NAV_SAT_STATUS_SERVICE_GPS`     | `RosUInt16`    | `1`                                |
| `constant SERVICE_GLONASS` | `sensormsgs.NAV_SAT_STATUS_SERVICE_GLONASS` | `RosUInt16`    | `2`                                |
| `constant SERVICE_COMPASS` | `sensormsgs.NAV_SAT_STATUS_SERVICE_COMPASS` | `RosUInt16`    | `4`                                |
| `constant SERVICE_GALILEO` | `sensormsgs.NAV_SAT_STATUS_SERVICE_GALILEO` | `RosUInt16`    | `8`                                |
| `service`                  | `sensormsgs.NavSatStatus.serviceValue`      | `RosUInt16`    | —                                  |

### `common-interfaces/sensor_msgs/msg/PointCloud.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.PointCloud`.

| Source field or constant | Qualified output                 | Representation                              | Explicit default or constant value |
| ------------------------ | -------------------------------- | ------------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.PointCloud.header`   | `Header`                                    | —                                  |
| `points`                 | `sensormsgs.PointCloud.points`   | `[Point32; 0..18446744073709551615]`        | —                                  |
| `channels`               | `sensormsgs.PointCloud.channels` | `[ChannelFloat32; 0..18446744073709551615]` | —                                  |

### `common-interfaces/sensor_msgs/msg/PointCloud2.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.PointCloud2`.

| Source field or constant | Qualified output                     | Representation                          | Explicit default or constant value |
| ------------------------ | ------------------------------------ | --------------------------------------- | ---------------------------------- |
| `header`                 | `sensormsgs.PointCloud2.header`      | `Header`                                | —                                  |
| `height`                 | `sensormsgs.PointCloud2.height`      | `RosUInt32`                             | —                                  |
| `width`                  | `sensormsgs.PointCloud2.width`       | `RosUInt32`                             | —                                  |
| `fields`                 | `sensormsgs.PointCloud2.fields`      | `[PointField; 0..18446744073709551615]` | —                                  |
| `is_bigendian`           | `sensormsgs.PointCloud2.isBigendian` | `RosBoolean`                            | —                                  |
| `point_step`             | `sensormsgs.PointCloud2.pointStep`   | `RosUInt32`                             | —                                  |
| `row_step`               | `sensormsgs.PointCloud2.rowStep`     | `RosUInt32`                             | —                                  |
| `data`                   | `sensormsgs.PointCloud2.data`        | `[RosUInt8; 0..18446744073709551615]`   | —                                  |
| `is_dense`               | `sensormsgs.PointCloud2.isDense`     | `RosBoolean`                            | —                                  |

### `common-interfaces/sensor_msgs/msg/PointField.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.PointField`.
- constant INT8: `sensormsgs.POINT_FIELD_INT8`.
- constant UINT8: `sensormsgs.POINT_FIELD_UINT8`.
- constant INT16: `sensormsgs.POINT_FIELD_INT16`.
- constant UINT16: `sensormsgs.POINT_FIELD_UINT16`.
- constant INT32: `sensormsgs.POINT_FIELD_INT32`.
- constant UINT32: `sensormsgs.POINT_FIELD_UINT32`.
- constant FLOAT32: `sensormsgs.POINT_FIELD_FLOAT32`.
- constant FLOAT64: `sensormsgs.POINT_FIELD_FLOAT64`.
- constant INT64: `sensormsgs.POINT_FIELD_INT64`.
- constant UINT64: `sensormsgs.POINT_FIELD_UINT64`.
- constant BOOL: `sensormsgs.POINT_FIELD_BOOL`.

| Source field or constant | Qualified output                 | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------- | -------------- | ---------------------------------- |
| `constant INT8`          | `sensormsgs.POINT_FIELD_INT8`    | `RosUInt8`     | `1`                                |
| `constant UINT8`         | `sensormsgs.POINT_FIELD_UINT8`   | `RosUInt8`     | `2`                                |
| `constant INT16`         | `sensormsgs.POINT_FIELD_INT16`   | `RosUInt8`     | `3`                                |
| `constant UINT16`        | `sensormsgs.POINT_FIELD_UINT16`  | `RosUInt8`     | `4`                                |
| `constant INT32`         | `sensormsgs.POINT_FIELD_INT32`   | `RosUInt8`     | `5`                                |
| `constant UINT32`        | `sensormsgs.POINT_FIELD_UINT32`  | `RosUInt8`     | `6`                                |
| `constant FLOAT32`       | `sensormsgs.POINT_FIELD_FLOAT32` | `RosUInt8`     | `7`                                |
| `constant FLOAT64`       | `sensormsgs.POINT_FIELD_FLOAT64` | `RosUInt8`     | `8`                                |
| `constant INT64`         | `sensormsgs.POINT_FIELD_INT64`   | `RosUInt8`     | `9`                                |
| `constant UINT64`        | `sensormsgs.POINT_FIELD_UINT64`  | `RosUInt8`     | `10`                               |
| `constant BOOL`          | `sensormsgs.POINT_FIELD_BOOL`    | `RosUInt8`     | `11`                               |
| `name`                   | `sensormsgs.PointField.name`     | `RosString`    | —                                  |
| `offset`                 | `sensormsgs.PointField.offset`   | `RosUInt32`    | —                                  |
| `datatype`               | `sensormsgs.PointField.datatype` | `RosUInt8`     | —                                  |
| `count`                  | `sensormsgs.PointField.count`    | `RosUInt32`    | —                                  |

### `common-interfaces/sensor_msgs/msg/Range.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Range`.
- constant ULTRASOUND: `sensormsgs.RANGE_ULTRASOUND`.
- constant INFRARED: `sensormsgs.RANGE_INFRARED`.

| Source field or constant | Qualified output                 | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.Range.header`        | `Header`       | —                                  |
| `constant ULTRASOUND`    | `sensormsgs.RANGE_ULTRASOUND`    | `RosUInt8`     | `0`                                |
| `constant INFRARED`      | `sensormsgs.RANGE_INFRARED`      | `RosUInt8`     | `1`                                |
| `radiation_type`         | `sensormsgs.Range.radiationType` | `RosUInt8`     | —                                  |
| `field_of_view`          | `sensormsgs.Range.fieldOfView`   | `RosFloat32`   | —                                  |
| `min_range`              | `sensormsgs.Range.minRange`      | `RosMetres`    | —                                  |
| `max_range`              | `sensormsgs.Range.maxRange`      | `RosMetres`    | —                                  |
| `range`                  | `sensormsgs.Range.range`         | `RosMetres`    | —                                  |
| `variance`               | `sensormsgs.Range.variance`      | `RosFloat32`   | —                                  |

### `common-interfaces/sensor_msgs/msg/RegionOfInterest.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.RegionOfInterest`.

| Source field or constant | Qualified output                        | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------------- | -------------- | ---------------------------------- |
| `x_offset`               | `sensormsgs.RegionOfInterest.xOffset`   | `RosUInt32`    | —                                  |
| `y_offset`               | `sensormsgs.RegionOfInterest.yOffset`   | `RosUInt32`    | —                                  |
| `height`                 | `sensormsgs.RegionOfInterest.height`    | `RosUInt32`    | —                                  |
| `width`                  | `sensormsgs.RegionOfInterest.width`     | `RosUInt32`    | —                                  |
| `do_rectify`             | `sensormsgs.RegionOfInterest.doRectify` | `RosBoolean`   | —                                  |

### `common-interfaces/sensor_msgs/msg/RelativeHumidity.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.RelativeHumidity`.

| Source field or constant | Qualified output                               | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.RelativeHumidity.header`           | `Header`       | —                                  |
| `relative_humidity`      | `sensormsgs.RelativeHumidity.relativeHumidity` | `RosFloat64`   | —                                  |
| `variance`               | `sensormsgs.RelativeHumidity.variance`         | `RosFloat64`   | —                                  |

### `common-interfaces/sensor_msgs/msg/Temperature.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.Temperature`.

| Source field or constant | Qualified output                     | Representation | Explicit default or constant value |
| ------------------------ | ------------------------------------ | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.Temperature.header`      | `Header`       | —                                  |
| `temperature`            | `sensormsgs.Temperature.temperature` | `RosCelsius`   | —                                  |
| `variance`               | `sensormsgs.Temperature.variance`    | `RosFloat64`   | —                                  |

### `common-interfaces/sensor_msgs/msg/TimeReference.msg`

Output: `sensor_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `sensormsgs.TimeReference`.

| Source field or constant | Qualified output                   | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `sensormsgs.TimeReference.header`  | `Header`       | —                                  |
| `time_ref`               | `sensormsgs.TimeReference.timeRef` | `Time`         | —                                  |
| `source`                 | `sensormsgs.TimeReference.source`  | `RosString`    | —                                  |

### `common-interfaces/sensor_msgs/srv/SetCameraInfo.srv`

Output: `sensor_msgs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `sensormsgs.SetCameraInfoRequest`.
- Response: `sensormsgs.SetCameraInfoResponse`.
- query: `sensormsgs.SensorMsgsInteractions.setCameraInfo`.

| Source field or constant  | Qualified output                                 | Representation | Explicit default or constant value |
| ------------------------- | ------------------------------------------------ | -------------- | ---------------------------------- |
| `Request.camera_info`     | `sensormsgs.SetCameraInfoRequest.cameraInfo`     | `CameraInfo`   | —                                  |
| `Response.success`        | `sensormsgs.SetCameraInfoResponse.success`       | `RosBoolean`   | —                                  |
| `Response.status_message` | `sensormsgs.SetCameraInfoResponse.statusMessage` | `RosString`    | —                                  |

### `common-interfaces/nav_msgs/msg/Goals.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.Goals`.

| Source field or constant | Qualified output       | Representation                           | Explicit default or constant value |
| ------------------------ | ---------------------- | ---------------------------------------- | ---------------------------------- |
| `header`                 | `navmsgs.Goals.header` | `Header`                                 | —                                  |
| `goals`                  | `navmsgs.Goals.goals`  | `[PoseStamped; 0..18446744073709551615]` | —                                  |

### `common-interfaces/nav_msgs/msg/GridCells.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.GridCells`.

| Source field or constant | Qualified output               | Representation                     | Explicit default or constant value |
| ------------------------ | ------------------------------ | ---------------------------------- | ---------------------------------- |
| `header`                 | `navmsgs.GridCells.header`     | `Header`                           | —                                  |
| `cell_width`             | `navmsgs.GridCells.cellWidth`  | `RosFloat32`                       | —                                  |
| `cell_height`            | `navmsgs.GridCells.cellHeight` | `RosFloat32`                       | —                                  |
| `cells`                  | `navmsgs.GridCells.cells`      | `[Point; 0..18446744073709551615]` | —                                  |

### `common-interfaces/nav_msgs/msg/MapMetaData.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.MapMetaData`.

| Source field or constant | Qualified output                  | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------- | ---------------------------------- |
| `map_load_time`          | `navmsgs.MapMetaData.mapLoadTime` | `Time`         | —                                  |
| `resolution`             | `navmsgs.MapMetaData.resolution`  | `RosFloat32`   | —                                  |
| `width`                  | `navmsgs.MapMetaData.width`       | `RosUInt32`    | —                                  |
| `height`                 | `navmsgs.MapMetaData.height`      | `RosUInt32`    | —                                  |
| `origin`                 | `navmsgs.MapMetaData.origin`      | `Pose`         | —                                  |

### `common-interfaces/nav_msgs/msg/OccupancyGrid.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.OccupancyGrid`.

| Source field or constant | Qualified output               | Representation                       | Explicit default or constant value |
| ------------------------ | ------------------------------ | ------------------------------------ | ---------------------------------- |
| `header`                 | `navmsgs.OccupancyGrid.header` | `Header`                             | —                                  |
| `info`                   | `navmsgs.OccupancyGrid.info`   | `MapMetaData`                        | —                                  |
| `data`                   | `navmsgs.OccupancyGrid.data`   | `[RosInt8; 0..18446744073709551615]` | —                                  |

### `common-interfaces/nav_msgs/msg/Odometry.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.Odometry`.

| Source field or constant | Qualified output                | Representation        | Explicit default or constant value |
| ------------------------ | ------------------------------- | --------------------- | ---------------------------------- |
| `header`                 | `navmsgs.Odometry.header`       | `Header`              | —                                  |
| `child_frame_id`         | `navmsgs.Odometry.childFrameId` | `RosString`           | —                                  |
| `pose`                   | `navmsgs.Odometry.pose`         | `PoseWithCovariance`  | —                                  |
| `twist`                  | `navmsgs.Odometry.twist`        | `TwistWithCovariance` | —                                  |

### `common-interfaces/nav_msgs/msg/Path.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.Path`.

| Source field or constant | Qualified output      | Representation                           | Explicit default or constant value |
| ------------------------ | --------------------- | ---------------------------------------- | ---------------------------------- |
| `header`                 | `navmsgs.Path.header` | `Header`                                 | —                                  |
| `poses`                  | `navmsgs.Path.poses`  | `[PoseStamped; 0..18446744073709551615]` | —                                  |

### `common-interfaces/nav_msgs/msg/Trajectory.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.Trajectory`.

| Source field or constant | Qualified output            | Representation                               | Explicit default or constant value |
| ------------------------ | --------------------------- | -------------------------------------------- | ---------------------------------- |
| `header`                 | `navmsgs.Trajectory.header` | `Header`                                     | —                                  |
| `points`                 | `navmsgs.Trajectory.points` | `[TrajectoryPoint; 0..18446744073709551615]` | —                                  |

### `common-interfaces/nav_msgs/msg/TrajectoryPoint.msg`

Output: `nav_msgs/messages.typl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- message: `navmsgs.TrajectoryPoint`.

| Source field or constant | Qualified output                       | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `navmsgs.TrajectoryPoint.header`       | `Header`       | —                                  |
| `pose`                   | `navmsgs.TrajectoryPoint.pose`         | `Pose`         | —                                  |
| `velocity`               | `navmsgs.TrajectoryPoint.velocity`     | `Twist`        | —                                  |
| `acceleration`           | `navmsgs.TrajectoryPoint.acceleration` | `Accel`        | —                                  |
| `effort`                 | `navmsgs.TrajectoryPoint.effort`       | `Wrench`       | —                                  |

### `common-interfaces/nav_msgs/srv/GetMap.srv`

Output: `nav_msgs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `navmsgs.GetMapRequest`.
- Response: `navmsgs.GetMapResponse`.
- query: `navmsgs.NavMsgsInteractions.getMap`.

| Source field or constant | Qualified output             | Representation  | Explicit default or constant value |
| ------------------------ | ---------------------------- | --------------- | ---------------------------------- |
| `Response.map`           | `navmsgs.GetMapResponse.map` | `OccupancyGrid` | —                                  |

### `common-interfaces/nav_msgs/srv/GetPlan.srv`

Output: `nav_msgs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `navmsgs.GetPlanRequest`.
- Response: `navmsgs.GetPlanResponse`.
- query: `navmsgs.NavMsgsInteractions.getPlan`.

| Source field or constant | Qualified output                   | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------- | -------------- | ---------------------------------- |
| `Request.start`          | `navmsgs.GetPlanRequest.start`     | `PoseStamped`  | —                                  |
| `Request.goal`           | `navmsgs.GetPlanRequest.goal`      | `PoseStamped`  | —                                  |
| `Request.tolerance`      | `navmsgs.GetPlanRequest.tolerance` | `RosMetres`    | —                                  |
| `Response.plan`          | `navmsgs.GetPlanResponse.plan`     | `Path`         | —                                  |

### `common-interfaces/nav_msgs/srv/LoadMap.srv`

Output: `nav_msgs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `navmsgs.LoadMapRequest`.
- Response: `navmsgs.LoadMapResponse`.
- constant Response.RESULT_SUCCESS: `navmsgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`.
- constant Response.RESULT_MAP_DOES_NOT_EXIST:
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`.
- constant Response.RESULT_INVALID_MAP_DATA:
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`.
- constant Response.RESULT_INVALID_MAP_METADATA:
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA`.
- constant Response.RESULT_UNDEFINED_FAILURE:
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`.
- query: `navmsgs.NavMsgsInteractions.loadMap`.

| Source field or constant               | Qualified output                                        | Representation  | Explicit default or constant value |
| -------------------------------------- | ------------------------------------------------------- | --------------- | ---------------------------------- |
| `Request.map_url`                      | `navmsgs.LoadMapRequest.mapUrl`                         | `RosString`     | —                                  |
| `constant RESULT_SUCCESS`              | `navmsgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`              | `RosUInt8`      | `0`                                |
| `constant RESULT_MAP_DOES_NOT_EXIST`   | `navmsgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`   | `RosUInt8`      | `1`                                |
| `constant RESULT_INVALID_MAP_DATA`     | `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`     | `RosUInt8`      | `2`                                |
| `constant RESULT_INVALID_MAP_METADATA` | `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA` | `RosUInt8`      | `3`                                |
| `constant RESULT_UNDEFINED_FAILURE`    | `navmsgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`    | `RosUInt8`      | `255`                              |
| `Response.map`                         | `navmsgs.LoadMapResponse.map`                           | `OccupancyGrid` | —                                  |
| `Response.result`                      | `navmsgs.LoadMapResponse.result`                        | `RosUInt8`      | —                                  |

### `common-interfaces/nav_msgs/srv/SetMap.srv`

Output: `nav_msgs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `navmsgs.SetMapRequest`.
- Response: `navmsgs.SetMapResponse`.
- query: `navmsgs.NavMsgsInteractions.setMap`.

| Source field or constant | Qualified output                    | Representation              | Explicit default or constant value |
| ------------------------ | ----------------------------------- | --------------------------- | ---------------------------------- |
| `Request.map`            | `navmsgs.SetMapRequest.map`         | `OccupancyGrid`             | —                                  |
| `Request.initial_pose`   | `navmsgs.SetMapRequest.initialPose` | `PoseWithCovarianceStamped` | —                                  |
| `Response.success`       | `navmsgs.SetMapResponse.success`    | `RosBoolean`                | —                                  |

### `common-interfaces/std_srvs/srv/Empty.srv`

Output: `std_srvs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `stdsrvs.EmptyRequest`.
- Response: `stdsrvs.EmptyResponse`.
- query: `stdsrvs.StdSrvsInteractions.empty`.

### `common-interfaces/std_srvs/srv/SetBool.srv`

Output: `std_srvs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `stdsrvs.SetBoolRequest`.
- Response: `stdsrvs.SetBoolResponse`.
- query: `stdsrvs.StdSrvsInteractions.setBool`.

| Source field or constant | Qualified output                  | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------- | ---------------------------------- |
| `Request.data`           | `stdsrvs.SetBoolRequest.data`     | `RosBoolean`   | —                                  |
| `Response.success`       | `stdsrvs.SetBoolResponse.success` | `RosBoolean`   | —                                  |
| `Response.message`       | `stdsrvs.SetBoolResponse.message` | `RosString`    | —                                  |

### `common-interfaces/std_srvs/srv/Trigger.srv`

Output: `std_srvs/interactions.ridl`. Pin:
`d8dde22160f26cf4fd8f1f8dcd819637b1b88405`.

- Request: `stdsrvs.TriggerRequest`.
- Response: `stdsrvs.TriggerResponse`.
- query: `stdsrvs.StdSrvsInteractions.trigger`.

| Source field or constant | Qualified output                  | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------- | -------------- | ---------------------------------- |
| `Response.success`       | `stdsrvs.TriggerResponse.success` | `RosBoolean`   | —                                  |
| `Response.message`       | `stdsrvs.TriggerResponse.message` | `RosString`    | —                                  |

### `navigation2/nav2_msgs/msg/BehaviorTreeLog.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.BehaviorTreeLog`.

| Source field or constant | Qualified output                     | Representation                                        | Explicit default or constant value |
| ------------------------ | ------------------------------------ | ----------------------------------------------------- | ---------------------------------- |
| `timestamp`              | `nav2msgs.BehaviorTreeLog.timestamp` | `Time`                                                | —                                  |
| `event_log`              | `nav2msgs.BehaviorTreeLog.eventLog`  | `[BehaviorTreeStatusChange; 0..18446744073709551615]` | —                                  |

### `navigation2/nav2_msgs/msg/BehaviorTreeStatusChange.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.BehaviorTreeStatusChange`.

| Source field or constant | Qualified output                                   | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------------------------- | -------------- | ---------------------------------- |
| `timestamp`              | `nav2msgs.BehaviorTreeStatusChange.timestamp`      | `Time`         | —                                  |
| `node_name`              | `nav2msgs.BehaviorTreeStatusChange.nodeName`       | `RosString`    | —                                  |
| `uid`                    | `nav2msgs.BehaviorTreeStatusChange.uid`            | `RosUInt16`    | —                                  |
| `previous_status`        | `nav2msgs.BehaviorTreeStatusChange.previousStatus` | `RosString`    | —                                  |
| `current_status`         | `nav2msgs.BehaviorTreeStatusChange.currentStatus`  | `RosString`    | —                                  |

### `navigation2/nav2_msgs/msg/Costmap.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.Costmap`.

| Source field or constant | Qualified output            | Representation                        | Explicit default or constant value |
| ------------------------ | --------------------------- | ------------------------------------- | ---------------------------------- |
| `header`                 | `nav2msgs.Costmap.header`   | `Header`                              | —                                  |
| `metadata`               | `nav2msgs.Costmap.metadata` | `CostmapMetaData`                     | —                                  |
| `data`                   | `nav2msgs.Costmap.data`     | `[RosUInt8; 0..18446744073709551615]` | —                                  |

### `navigation2/nav2_msgs/msg/CostmapMetaData.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.CostmapMetaData`.

| Source field or constant | Qualified output                       | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------------- | -------------- | ---------------------------------- |
| `map_load_time`          | `nav2msgs.CostmapMetaData.mapLoadTime` | `Time`         | —                                  |
| `update_time`            | `nav2msgs.CostmapMetaData.updateTime`  | `Time`         | —                                  |
| `layer`                  | `nav2msgs.CostmapMetaData.layer`       | `RosString`    | —                                  |
| `resolution`             | `nav2msgs.CostmapMetaData.resolution`  | `RosFloat32`   | —                                  |
| `size_x`                 | `nav2msgs.CostmapMetaData.sizeX`       | `RosUInt32`    | —                                  |
| `size_y`                 | `nav2msgs.CostmapMetaData.sizeY`       | `RosUInt32`    | —                                  |
| `origin`                 | `nav2msgs.CostmapMetaData.origin`      | `Pose`         | —                                  |

### `navigation2/nav2_msgs/msg/Particle.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.Particle`.

| Source field or constant | Qualified output           | Representation | Explicit default or constant value |
| ------------------------ | -------------------------- | -------------- | ---------------------------------- |
| `pose`                   | `nav2msgs.Particle.pose`   | `Pose`         | —                                  |
| `weight`                 | `nav2msgs.Particle.weight` | `RosFloat64`   | —                                  |

### `navigation2/nav2_msgs/msg/ParticleCloud.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.ParticleCloud`.

| Source field or constant | Qualified output                   | Representation                        | Explicit default or constant value |
| ------------------------ | ---------------------------------- | ------------------------------------- | ---------------------------------- |
| `header`                 | `nav2msgs.ParticleCloud.header`    | `Header`                              | —                                  |
| `particles`              | `nav2msgs.ParticleCloud.particles` | `[Particle; 0..18446744073709551615]` | —                                  |

### `navigation2/nav2_msgs/msg/SpeedLimit.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.SpeedLimit`.

| Source field or constant | Qualified output                 | Representation | Explicit default or constant value |
| ------------------------ | -------------------------------- | -------------- | ---------------------------------- |
| `header`                 | `nav2msgs.SpeedLimit.header`     | `Header`       | —                                  |
| `percentage`             | `nav2msgs.SpeedLimit.percentage` | `RosBoolean`   | —                                  |
| `speed_limit`            | `nav2msgs.SpeedLimit.speedLimit` | `RosFloat64`   | —                                  |

### `navigation2/nav2_msgs/msg/TrackingFeedback.msg`

**Required dependency definition.**

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.TrackingFeedback`.

| Source field or constant  | Qualified output                                  | Representation | Explicit default or constant value |
| ------------------------- | ------------------------------------------------- | -------------- | ---------------------------------- |
| `header`                  | `nav2msgs.TrackingFeedback.header`                | `Header`       | —                                  |
| `position_tracking_error` | `nav2msgs.TrackingFeedback.positionTrackingError` | `RosFloat32`   | —                                  |
| `heading_tracking_error`  | `nav2msgs.TrackingFeedback.headingTrackingError`  | `RosFloat32`   | —                                  |
| `current_path_index`      | `nav2msgs.TrackingFeedback.currentPathIndex`      | `RosUInt32`    | —                                  |
| `robot_pose`              | `nav2msgs.TrackingFeedback.robotPose`             | `PoseStamped`  | —                                  |
| `distance_to_goal`        | `nav2msgs.TrackingFeedback.distanceToGoal`        | `RosFloat32`   | —                                  |
| `speed`                   | `nav2msgs.TrackingFeedback.speed`                 | `RosFloat32`   | —                                  |
| `remaining_path_length`   | `nav2msgs.TrackingFeedback.remainingPathLength`   | `RosFloat32`   | —                                  |

### `navigation2/nav2_msgs/msg/VoxelGrid.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.VoxelGrid`.

| Source field or constant | Qualified output                 | Representation                         | Explicit default or constant value |
| ------------------------ | -------------------------------- | -------------------------------------- | ---------------------------------- |
| `header`                 | `nav2msgs.VoxelGrid.header`      | `Header`                               | —                                  |
| `data`                   | `nav2msgs.VoxelGrid.data`        | `[RosUInt32; 0..18446744073709551615]` | —                                  |
| `origin`                 | `nav2msgs.VoxelGrid.origin`      | `Point32`                              | —                                  |
| `resolutions`            | `nav2msgs.VoxelGrid.resolutions` | `Vector3`                              | —                                  |
| `size_x`                 | `nav2msgs.VoxelGrid.sizeX`       | `RosUInt32`                            | —                                  |
| `size_y`                 | `nav2msgs.VoxelGrid.sizeY`       | `RosUInt32`                            | —                                  |
| `size_z`                 | `nav2msgs.VoxelGrid.sizeZ`       | `RosUInt32`                            | —                                  |

### `navigation2/nav2_msgs/msg/WaypointStatus.msg`

Output: `nav2_msgs/messages.typl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- message: `nav2msgs.WaypointStatus`.
- constant PENDING: `nav2msgs.WAYPOINT_STATUS_PENDING`.
- constant COMPLETED: `nav2msgs.WAYPOINT_STATUS_COMPLETED`.
- constant SKIPPED: `nav2msgs.WAYPOINT_STATUS_SKIPPED`.
- constant FAILED: `nav2msgs.WAYPOINT_STATUS_FAILED`.

| Source field or constant | Qualified output                         | Representation | Explicit default or constant value |
| ------------------------ | ---------------------------------------- | -------------- | ---------------------------------- |
| `constant PENDING`       | `nav2msgs.WAYPOINT_STATUS_PENDING`       | `RosUInt8`     | `0`                                |
| `constant COMPLETED`     | `nav2msgs.WAYPOINT_STATUS_COMPLETED`     | `RosUInt8`     | `1`                                |
| `constant SKIPPED`       | `nav2msgs.WAYPOINT_STATUS_SKIPPED`       | `RosUInt8`     | `2`                                |
| `constant FAILED`        | `nav2msgs.WAYPOINT_STATUS_FAILED`        | `RosUInt8`     | `3`                                |
| `waypoint_status`        | `nav2msgs.WaypointStatus.waypointStatus` | `RosUInt8`     | —                                  |
| `waypoint_index`         | `nav2msgs.WaypointStatus.waypointIndex`  | `RosUInt32`    | —                                  |
| `waypoint_pose`          | `nav2msgs.WaypointStatus.waypointPose`   | `PoseStamped`  | —                                  |
| `error_code`             | `nav2msgs.WaypointStatus.errorCode`      | `RosUInt16`    | —                                  |
| `error_msg`              | `nav2msgs.WaypointStatus.errorMsg`       | `RosString`    | —                                  |

### `navigation2/nav2_msgs/srv/ClearEntireCostmap.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.ClearEntireCostmapRequest`.
- Response: `nav2msgs.ClearEntireCostmapResponse`.
- query: `nav2msgs.Nav2MsgsInteractions.clearEntireCostmap`.

| Source field or constant | Qualified output                              | Representation                         | Explicit default or constant value |
| ------------------------ | --------------------------------------------- | -------------------------------------- | ---------------------------------- |
| `Request.plugins`        | `nav2msgs.ClearEntireCostmapRequest.plugins`  | `[RosString; 0..18446744073709551615]` | —                                  |
| `Response.success`       | `nav2msgs.ClearEntireCostmapResponse.success` | `RosBoolean`                           | —                                  |

### `navigation2/nav2_msgs/srv/GetCostmap.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.GetCostmapRequest`.
- Response: `nav2msgs.GetCostmapResponse`.
- query: `nav2msgs.Nav2MsgsInteractions.getCostmap`.

| Source field or constant | Qualified output                   | Representation    | Explicit default or constant value |
| ------------------------ | ---------------------------------- | ----------------- | ---------------------------------- |
| `Request.specs`          | `nav2msgs.GetCostmapRequest.specs` | `CostmapMetaData` | —                                  |
| `Response.map`           | `nav2msgs.GetCostmapResponse.map`  | `Costmap`         | —                                  |

### `navigation2/nav2_msgs/srv/IsPathValid.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.IsPathValidRequest`.
- Response: `nav2msgs.IsPathValidResponse`.
- query: `nav2msgs.Nav2MsgsInteractions.isPathValid`.

| Source field or constant               | Qualified output                                        | Representation                        | Explicit default or constant value |
| -------------------------------------- | ------------------------------------------------------- | ------------------------------------- | ---------------------------------- |
| `Request.path`                         | `nav2msgs.IsPathValidRequest.path`                      | `Path`                                | —                                  |
| `Request.max_cost`                     | `nav2msgs.IsPathValidRequest.maxCost`                   | `RosUInt8`                            | `254`                              |
| `Request.consider_unknown_as_obstacle` | `nav2msgs.IsPathValidRequest.considerUnknownAsObstacle` | `RosBoolean`                          | `false`                            |
| `Request.layer_name`                   | `nav2msgs.IsPathValidRequest.layerName`                 | `RosString`                           | `""`                               |
| `Request.footprint`                    | `nav2msgs.IsPathValidRequest.footprint`                 | `RosString`                           | `""`                               |
| `Request.stop_at_first_collision`      | `nav2msgs.IsPathValidRequest.stopAtFirstCollision`      | `RosBoolean`                          | `true`                             |
| `Request.max_lookahead_distance`       | `nav2msgs.IsPathValidRequest.maxLookaheadDistance`      | `RosFloat64`                          | `-1.0`                             |
| `Response.success`                     | `nav2msgs.IsPathValidResponse.success`                  | `RosBoolean`                          | —                                  |
| `Response.is_valid`                    | `nav2msgs.IsPathValidResponse.isValid`                  | `RosBoolean`                          | —                                  |
| `Response.invalid_pose_indices`        | `nav2msgs.IsPathValidResponse.invalidPoseIndices`       | `[RosInt32; 0..18446744073709551615]` | —                                  |

### `navigation2/nav2_msgs/srv/LoadMap.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.LoadMapRequest`.
- Response: `nav2msgs.LoadMapResponse`.
- constant Response.RESULT_SUCCESS: `nav2msgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`.
- constant Response.RESULT_MAP_DOES_NOT_EXIST:
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`.
- constant Response.RESULT_INVALID_MAP_DATA:
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`.
- constant Response.RESULT_INVALID_MAP_METADATA:
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA`.
- constant Response.RESULT_UNDEFINED_FAILURE:
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`.
- query: `nav2msgs.Nav2MsgsInteractions.loadMap`.

| Source field or constant               | Qualified output                                         | Representation  | Explicit default or constant value |
| -------------------------------------- | -------------------------------------------------------- | --------------- | ---------------------------------- |
| `Request.map_url`                      | `nav2msgs.LoadMapRequest.mapUrl`                         | `RosString`     | —                                  |
| `constant RESULT_SUCCESS`              | `nav2msgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`              | `RosUInt8`      | `0`                                |
| `constant RESULT_MAP_DOES_NOT_EXIST`   | `nav2msgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`   | `RosUInt8`      | `1`                                |
| `constant RESULT_INVALID_MAP_DATA`     | `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`     | `RosUInt8`      | `2`                                |
| `constant RESULT_INVALID_MAP_METADATA` | `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA` | `RosUInt8`      | `3`                                |
| `constant RESULT_UNDEFINED_FAILURE`    | `nav2msgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`    | `RosUInt8`      | `255`                              |
| `Response.map`                         | `nav2msgs.LoadMapResponse.map`                           | `OccupancyGrid` | —                                  |
| `Response.result`                      | `nav2msgs.LoadMapResponse.result`                        | `RosUInt8`      | —                                  |

### `navigation2/nav2_msgs/srv/ManageLifecycleNodes.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.ManageLifecycleNodesRequest`.
- constant Request.STARTUP: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_STARTUP`.
- constant Request.PAUSE: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_PAUSE`.
- constant Request.RESUME: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESUME`.
- constant Request.RESET: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESET`.
- constant Request.SHUTDOWN: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_SHUTDOWN`.
- constant Request.CONFIGURE:
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CONFIGURE`.
- constant Request.CLEANUP: `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CLEANUP`.
- Response: `nav2msgs.ManageLifecycleNodesResponse`.
- query: `nav2msgs.Nav2MsgsInteractions.manageLifecycleNodes`.

| Source field or constant | Qualified output                                    | Representation | Explicit default or constant value |
| ------------------------ | --------------------------------------------------- | -------------- | ---------------------------------- |
| `constant STARTUP`       | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_STARTUP`   | `RosUInt8`     | `0`                                |
| `constant PAUSE`         | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_PAUSE`     | `RosUInt8`     | `1`                                |
| `constant RESUME`        | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESUME`    | `RosUInt8`     | `2`                                |
| `constant RESET`         | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESET`     | `RosUInt8`     | `3`                                |
| `constant SHUTDOWN`      | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_SHUTDOWN`  | `RosUInt8`     | `4`                                |
| `constant CONFIGURE`     | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CONFIGURE` | `RosUInt8`     | `5`                                |
| `constant CLEANUP`       | `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CLEANUP`   | `RosUInt8`     | `6`                                |
| `Request.command`        | `nav2msgs.ManageLifecycleNodesRequest.commandValue` | `RosUInt8`     | —                                  |
| `Response.success`       | `nav2msgs.ManageLifecycleNodesResponse.success`     | `RosBoolean`   | —                                  |

### `navigation2/nav2_msgs/srv/SaveMap.srv`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Request: `nav2msgs.SaveMapRequest`.
- Response: `nav2msgs.SaveMapResponse`.
- query: `nav2msgs.Nav2MsgsInteractions.saveMap`.

| Source field or constant  | Qualified output                         | Representation | Explicit default or constant value |
| ------------------------- | ---------------------------------------- | -------------- | ---------------------------------- |
| `Request.map_topic`       | `nav2msgs.SaveMapRequest.mapTopic`       | `RosString`    | —                                  |
| `Request.map_url`         | `nav2msgs.SaveMapRequest.mapUrl`         | `RosString`    | —                                  |
| `Request.image_format`    | `nav2msgs.SaveMapRequest.imageFormat`    | `RosString`    | —                                  |
| `Request.map_mode`        | `nav2msgs.SaveMapRequest.mapMode`        | `RosString`    | —                                  |
| `Request.free_thresh`     | `nav2msgs.SaveMapRequest.freeThresh`     | `RosFloat32`   | —                                  |
| `Request.occupied_thresh` | `nav2msgs.SaveMapRequest.occupiedThresh` | `RosFloat32`   | —                                  |
| `Response.result`         | `nav2msgs.SaveMapResponse.result`        | `RosBoolean`   | —                                  |

### `navigation2/nav2_msgs/action/AssistedTeleop.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.AssistedTeleopGoal`.
- Result: `nav2msgs.AssistedTeleopResult`.
- constant Result.NONE: `nav2msgs.ASSISTED_TELEOP_RESULT_NONE`.
- constant Result.GOAL_REJECTED:
  `nav2msgs.ASSISTED_TELEOP_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE:
  `nav2msgs.ASSISTED_TELEOP_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.ASSISTED_TELEOP_RESULT_UNKNOWN`.
- constant Result.TIMEOUT: `nav2msgs.ASSISTED_TELEOP_RESULT_TIMEOUT`.
- constant Result.TF_ERROR: `nav2msgs.ASSISTED_TELEOP_RESULT_TF_ERROR`.
- constant Result.TELEOP_INPUT_TIMEOUT:
  `nav2msgs.ASSISTED_TELEOP_RESULT_TELEOP_INPUT_TIMEOUT`.
- Feedback: `nav2msgs.AssistedTeleopFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.assistedTeleop`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.assistedTeleopFeedback`.

| Source field or constant           | Qualified output                                        | Representation | Explicit default or constant value |
| ---------------------------------- | ------------------------------------------------------- | -------------- | ---------------------------------- |
| `Goal.time_allowance`              | `nav2msgs.AssistedTeleopGoal.timeAllowance`             | `Duration`     | —                                  |
| `constant NONE`                    | `nav2msgs.ASSISTED_TELEOP_RESULT_NONE`                  | `RosUInt16`    | `0`                                |
| `constant GOAL_REJECTED`           | `nav2msgs.ASSISTED_TELEOP_RESULT_GOAL_REJECTED`         | `RosUInt16`    | `1`                                |
| `constant SEND_GOAL_FAILURE`       | `nav2msgs.ASSISTED_TELEOP_RESULT_SEND_GOAL_FAILURE`     | `RosUInt16`    | `2`                                |
| `constant UNKNOWN`                 | `nav2msgs.ASSISTED_TELEOP_RESULT_UNKNOWN`               | `RosUInt16`    | `730`                              |
| `constant TIMEOUT`                 | `nav2msgs.ASSISTED_TELEOP_RESULT_TIMEOUT`               | `RosUInt16`    | `731`                              |
| `constant TF_ERROR`                | `nav2msgs.ASSISTED_TELEOP_RESULT_TF_ERROR`              | `RosUInt16`    | `732`                              |
| `constant TELEOP_INPUT_TIMEOUT`    | `nav2msgs.ASSISTED_TELEOP_RESULT_TELEOP_INPUT_TIMEOUT`  | `RosUInt16`    | `733`                              |
| `Result.total_elapsed_time`        | `nav2msgs.AssistedTeleopResult.totalElapsedTime`        | `Duration`     | —                                  |
| `Result.error_code`                | `nav2msgs.AssistedTeleopResult.errorCode`               | `RosUInt16`    | —                                  |
| `Result.error_msg`                 | `nav2msgs.AssistedTeleopResult.errorMsg`                | `RosString`    | —                                  |
| `Feedback.current_teleop_duration` | `nav2msgs.AssistedTeleopFeedback.currentTeleopDuration` | `Duration`     | —                                  |

### `navigation2/nav2_msgs/action/BackUp.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.BackUpGoal`.
- Result: `nav2msgs.BackUpResult`.
- constant Result.NONE: `nav2msgs.BACK_UP_RESULT_NONE`.
- constant Result.GOAL_REJECTED: `nav2msgs.BACK_UP_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE:
  `nav2msgs.BACK_UP_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.BACK_UP_RESULT_UNKNOWN`.
- constant Result.TIMEOUT: `nav2msgs.BACK_UP_RESULT_TIMEOUT`.
- constant Result.TF_ERROR: `nav2msgs.BACK_UP_RESULT_TF_ERROR`.
- constant Result.INVALID_INPUT: `nav2msgs.BACK_UP_RESULT_INVALID_INPUT`.
- constant Result.COLLISION_AHEAD: `nav2msgs.BACK_UP_RESULT_COLLISION_AHEAD`.
- Feedback: `nav2msgs.BackUpFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.backUp`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.backUpFeedback`.

| Source field or constant        | Qualified output                             | Representation | Explicit default or constant value |
| ------------------------------- | -------------------------------------------- | -------------- | ---------------------------------- |
| `Goal.target`                   | `nav2msgs.BackUpGoal.target`                 | `Point`        | —                                  |
| `Goal.speed`                    | `nav2msgs.BackUpGoal.speed`                  | `RosFloat32`   | —                                  |
| `Goal.time_allowance`           | `nav2msgs.BackUpGoal.timeAllowance`          | `Duration`     | —                                  |
| `Goal.disable_collision_checks` | `nav2msgs.BackUpGoal.disableCollisionChecks` | `RosBoolean`   | `false`                            |
| `constant NONE`                 | `nav2msgs.BACK_UP_RESULT_NONE`               | `RosUInt16`    | `0`                                |
| `constant GOAL_REJECTED`        | `nav2msgs.BACK_UP_RESULT_GOAL_REJECTED`      | `RosUInt16`    | `1`                                |
| `constant SEND_GOAL_FAILURE`    | `nav2msgs.BACK_UP_RESULT_SEND_GOAL_FAILURE`  | `RosUInt16`    | `2`                                |
| `constant UNKNOWN`              | `nav2msgs.BACK_UP_RESULT_UNKNOWN`            | `RosUInt16`    | `710`                              |
| `constant TIMEOUT`              | `nav2msgs.BACK_UP_RESULT_TIMEOUT`            | `RosUInt16`    | `711`                              |
| `constant TF_ERROR`             | `nav2msgs.BACK_UP_RESULT_TF_ERROR`           | `RosUInt16`    | `712`                              |
| `constant INVALID_INPUT`        | `nav2msgs.BACK_UP_RESULT_INVALID_INPUT`      | `RosUInt16`    | `713`                              |
| `constant COLLISION_AHEAD`      | `nav2msgs.BACK_UP_RESULT_COLLISION_AHEAD`    | `RosUInt16`    | `714`                              |
| `Result.total_elapsed_time`     | `nav2msgs.BackUpResult.totalElapsedTime`     | `Duration`     | —                                  |
| `Result.error_code`             | `nav2msgs.BackUpResult.errorCode`            | `RosUInt16`    | —                                  |
| `Result.error_msg`              | `nav2msgs.BackUpResult.errorMsg`             | `RosString`    | —                                  |
| `Feedback.distance_traveled`    | `nav2msgs.BackUpFeedback.distanceTraveled`   | `RosFloat32`   | —                                  |

### `navigation2/nav2_msgs/action/FollowPath.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.FollowPathGoal`.
- Result: `nav2msgs.FollowPathResult`.
- constant Result.NONE: `nav2msgs.FOLLOW_PATH_RESULT_NONE`.
- constant Result.GOAL_REJECTED: `nav2msgs.FOLLOW_PATH_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE:
  `nav2msgs.FOLLOW_PATH_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.FOLLOW_PATH_RESULT_UNKNOWN`.
- constant Result.INVALID_CONTROLLER:
  `nav2msgs.FOLLOW_PATH_RESULT_INVALID_CONTROLLER`.
- constant Result.TF_ERROR: `nav2msgs.FOLLOW_PATH_RESULT_TF_ERROR`.
- constant Result.INVALID_PATH: `nav2msgs.FOLLOW_PATH_RESULT_INVALID_PATH`.
- constant Result.PATIENCE_EXCEEDED:
  `nav2msgs.FOLLOW_PATH_RESULT_PATIENCE_EXCEEDED`.
- constant Result.FAILED_TO_MAKE_PROGRESS:
  `nav2msgs.FOLLOW_PATH_RESULT_FAILED_TO_MAKE_PROGRESS`.
- constant Result.NO_VALID_CONTROL:
  `nav2msgs.FOLLOW_PATH_RESULT_NO_VALID_CONTROL`.
- constant Result.CONTROLLER_TIMED_OUT:
  `nav2msgs.FOLLOW_PATH_RESULT_CONTROLLER_TIMED_OUT`.
- constant Result.TIMEOUT: `nav2msgs.FOLLOW_PATH_RESULT_TIMEOUT`.
- Feedback: `nav2msgs.FollowPathFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.followPath`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.followPathFeedback`.

| Source field or constant           | Qualified output                                      | Representation     | Explicit default or constant value |
| ---------------------------------- | ----------------------------------------------------- | ------------------ | ---------------------------------- |
| `Goal.path`                        | `nav2msgs.FollowPathGoal.path`                        | `Path`             | —                                  |
| `Goal.controller_id`               | `nav2msgs.FollowPathGoal.controllerId`                | `RosString`        | —                                  |
| `Goal.goal_checker_id`             | `nav2msgs.FollowPathGoal.goalCheckerId`               | `RosString`        | —                                  |
| `Goal.progress_checker_id`         | `nav2msgs.FollowPathGoal.progressCheckerId`           | `RosString`        | —                                  |
| `Goal.path_handler_id`             | `nav2msgs.FollowPathGoal.pathHandlerId`               | `RosString`        | —                                  |
| `constant NONE`                    | `nav2msgs.FOLLOW_PATH_RESULT_NONE`                    | `RosUInt16`        | `0`                                |
| `constant GOAL_REJECTED`           | `nav2msgs.FOLLOW_PATH_RESULT_GOAL_REJECTED`           | `RosUInt16`        | `1`                                |
| `constant SEND_GOAL_FAILURE`       | `nav2msgs.FOLLOW_PATH_RESULT_SEND_GOAL_FAILURE`       | `RosUInt16`        | `2`                                |
| `constant UNKNOWN`                 | `nav2msgs.FOLLOW_PATH_RESULT_UNKNOWN`                 | `RosUInt16`        | `100`                              |
| `constant INVALID_CONTROLLER`      | `nav2msgs.FOLLOW_PATH_RESULT_INVALID_CONTROLLER`      | `RosUInt16`        | `101`                              |
| `constant TF_ERROR`                | `nav2msgs.FOLLOW_PATH_RESULT_TF_ERROR`                | `RosUInt16`        | `102`                              |
| `constant INVALID_PATH`            | `nav2msgs.FOLLOW_PATH_RESULT_INVALID_PATH`            | `RosUInt16`        | `103`                              |
| `constant PATIENCE_EXCEEDED`       | `nav2msgs.FOLLOW_PATH_RESULT_PATIENCE_EXCEEDED`       | `RosUInt16`        | `104`                              |
| `constant FAILED_TO_MAKE_PROGRESS` | `nav2msgs.FOLLOW_PATH_RESULT_FAILED_TO_MAKE_PROGRESS` | `RosUInt16`        | `105`                              |
| `constant NO_VALID_CONTROL`        | `nav2msgs.FOLLOW_PATH_RESULT_NO_VALID_CONTROL`        | `RosUInt16`        | `106`                              |
| `constant CONTROLLER_TIMED_OUT`    | `nav2msgs.FOLLOW_PATH_RESULT_CONTROLLER_TIMED_OUT`    | `RosUInt16`        | `107`                              |
| `constant TIMEOUT`                 | `nav2msgs.FOLLOW_PATH_RESULT_TIMEOUT`                 | `RosUInt16`        | `108`                              |
| `Result.result`                    | `nav2msgs.FollowPathResult.result`                    | `Empty`            | —                                  |
| `Result.error_code`                | `nav2msgs.FollowPathResult.errorCode`                 | `RosUInt16`        | —                                  |
| `Result.error_msg`                 | `nav2msgs.FollowPathResult.errorMsg`                  | `RosString`        | —                                  |
| `Feedback.tracking_feedback`       | `nav2msgs.FollowPathFeedback.trackingFeedback`        | `TrackingFeedback` | —                                  |

### `navigation2/nav2_msgs/action/FollowWaypoints.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.FollowWaypointsGoal`.
- Result: `nav2msgs.FollowWaypointsResult`.
- constant Result.NONE: `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NONE`.
- constant Result.UNKNOWN: `nav2msgs.FOLLOW_WAYPOINTS_RESULT_UNKNOWN`.
- constant Result.TASK_EXECUTOR_FAILED:
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_TASK_EXECUTOR_FAILED`.
- constant Result.NO_VALID_WAYPOINTS:
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NO_VALID_WAYPOINTS`.
- constant Result.STOP_ON_MISSED_WAYPOINT:
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_STOP_ON_MISSED_WAYPOINT`.
- Feedback: `nav2msgs.FollowWaypointsFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.followWaypoints`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.followWaypointsFeedback`.

| Source field or constant           | Qualified output                                           | Representation                              | Explicit default or constant value |
| ---------------------------------- | ---------------------------------------------------------- | ------------------------------------------- | ---------------------------------- |
| `Goal.number_of_loops`             | `nav2msgs.FollowWaypointsGoal.numberOfLoops`               | `RosUInt32`                                 | —                                  |
| `Goal.goal_index`                  | `nav2msgs.FollowWaypointsGoal.goalIndex`                   | `RosUInt32`                                 | `0`                                |
| `Goal.poses`                       | `nav2msgs.FollowWaypointsGoal.poses`                       | `[PoseStamped; 0..18446744073709551615]`    | —                                  |
| `constant NONE`                    | `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NONE`                    | `RosUInt16`                                 | `0`                                |
| `constant UNKNOWN`                 | `nav2msgs.FOLLOW_WAYPOINTS_RESULT_UNKNOWN`                 | `RosUInt16`                                 | `600`                              |
| `constant TASK_EXECUTOR_FAILED`    | `nav2msgs.FOLLOW_WAYPOINTS_RESULT_TASK_EXECUTOR_FAILED`    | `RosUInt16`                                 | `601`                              |
| `constant NO_VALID_WAYPOINTS`      | `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NO_VALID_WAYPOINTS`      | `RosUInt16`                                 | `602`                              |
| `constant STOP_ON_MISSED_WAYPOINT` | `nav2msgs.FOLLOW_WAYPOINTS_RESULT_STOP_ON_MISSED_WAYPOINT` | `RosUInt16`                                 | `603`                              |
| `Result.missed_waypoints`          | `nav2msgs.FollowWaypointsResult.missedWaypoints`           | `[WaypointStatus; 0..18446744073709551615]` | —                                  |
| `Result.error_code`                | `nav2msgs.FollowWaypointsResult.errorCode`                 | `RosUInt16`                                 | —                                  |
| `Result.error_msg`                 | `nav2msgs.FollowWaypointsResult.errorMsg`                  | `RosString`                                 | —                                  |
| `Feedback.current_waypoint`        | `nav2msgs.FollowWaypointsFeedback.currentWaypoint`         | `RosUInt32`                                 | —                                  |

### `navigation2/nav2_msgs/action/NavigateToPose.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.NavigateToPoseGoal`.
- Result: `nav2msgs.NavigateToPoseResult`.
- constant Result.NONE: `nav2msgs.NAVIGATE_TO_POSE_RESULT_NONE`.
- constant Result.GOAL_REJECTED:
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE:
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.NAVIGATE_TO_POSE_RESULT_UNKNOWN`.
- constant Result.FAILED_TO_LOAD_BEHAVIOR_TREE:
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_FAILED_TO_LOAD_BEHAVIOR_TREE`.
- constant Result.TF_ERROR: `nav2msgs.NAVIGATE_TO_POSE_RESULT_TF_ERROR`.
- constant Result.TIMEOUT: `nav2msgs.NAVIGATE_TO_POSE_RESULT_TIMEOUT`.
- Feedback: `nav2msgs.NavigateToPoseFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.navigateToPose`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.navigateToPoseFeedback`.

| Source field or constant                | Qualified output                                                | Representation | Explicit default or constant value |
| --------------------------------------- | --------------------------------------------------------------- | -------------- | ---------------------------------- |
| `Goal.pose`                             | `nav2msgs.NavigateToPoseGoal.pose`                              | `PoseStamped`  | —                                  |
| `Goal.behavior_tree`                    | `nav2msgs.NavigateToPoseGoal.behaviorTree`                      | `RosString`    | —                                  |
| `constant NONE`                         | `nav2msgs.NAVIGATE_TO_POSE_RESULT_NONE`                         | `RosUInt16`    | `0`                                |
| `constant GOAL_REJECTED`                | `nav2msgs.NAVIGATE_TO_POSE_RESULT_GOAL_REJECTED`                | `RosUInt16`    | `1`                                |
| `constant SEND_GOAL_FAILURE`            | `nav2msgs.NAVIGATE_TO_POSE_RESULT_SEND_GOAL_FAILURE`            | `RosUInt16`    | `2`                                |
| `constant UNKNOWN`                      | `nav2msgs.NAVIGATE_TO_POSE_RESULT_UNKNOWN`                      | `RosUInt16`    | `9000`                             |
| `constant FAILED_TO_LOAD_BEHAVIOR_TREE` | `nav2msgs.NAVIGATE_TO_POSE_RESULT_FAILED_TO_LOAD_BEHAVIOR_TREE` | `RosUInt16`    | `9001`                             |
| `constant TF_ERROR`                     | `nav2msgs.NAVIGATE_TO_POSE_RESULT_TF_ERROR`                     | `RosUInt16`    | `9002`                             |
| `constant TIMEOUT`                      | `nav2msgs.NAVIGATE_TO_POSE_RESULT_TIMEOUT`                      | `RosUInt16`    | `9003`                             |
| `Result.error_code`                     | `nav2msgs.NavigateToPoseResult.errorCode`                       | `RosUInt16`    | —                                  |
| `Result.error_msg`                      | `nav2msgs.NavigateToPoseResult.errorMsg`                        | `RosString`    | —                                  |
| `Feedback.current_pose`                 | `nav2msgs.NavigateToPoseFeedback.currentPose`                   | `PoseStamped`  | —                                  |
| `Feedback.navigation_time`              | `nav2msgs.NavigateToPoseFeedback.navigationTime`                | `Duration`     | —                                  |
| `Feedback.estimated_time_remaining`     | `nav2msgs.NavigateToPoseFeedback.estimatedTimeRemaining`        | `Duration`     | —                                  |
| `Feedback.number_of_recoveries`         | `nav2msgs.NavigateToPoseFeedback.numberOfRecoveries`            | `RosInt16`     | —                                  |
| `Feedback.distance_remaining`           | `nav2msgs.NavigateToPoseFeedback.distanceRemaining`             | `RosFloat32`   | —                                  |
| `Feedback.position_tracking_error`      | `nav2msgs.NavigateToPoseFeedback.positionTrackingError`         | `RosFloat32`   | —                                  |
| `Feedback.heading_tracking_error`       | `nav2msgs.NavigateToPoseFeedback.headingTrackingError`          | `RosFloat32`   | —                                  |

### `navigation2/nav2_msgs/action/Spin.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.SpinGoal`.
- Result: `nav2msgs.SpinResult`.
- constant Result.NONE: `nav2msgs.SPIN_RESULT_NONE`.
- constant Result.GOAL_REJECTED: `nav2msgs.SPIN_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE: `nav2msgs.SPIN_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.SPIN_RESULT_UNKNOWN`.
- constant Result.TIMEOUT: `nav2msgs.SPIN_RESULT_TIMEOUT`.
- constant Result.TF_ERROR: `nav2msgs.SPIN_RESULT_TF_ERROR`.
- constant Result.COLLISION_AHEAD: `nav2msgs.SPIN_RESULT_COLLISION_AHEAD`.
- Feedback: `nav2msgs.SpinFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.spin`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.spinFeedback`.

| Source field or constant             | Qualified output                                | Representation | Explicit default or constant value |
| ------------------------------------ | ----------------------------------------------- | -------------- | ---------------------------------- |
| `Goal.target_yaw`                    | `nav2msgs.SpinGoal.targetYaw`                   | `RosFloat32`   | —                                  |
| `Goal.time_allowance`                | `nav2msgs.SpinGoal.timeAllowance`               | `Duration`     | —                                  |
| `Goal.disable_collision_checks`      | `nav2msgs.SpinGoal.disableCollisionChecks`      | `RosBoolean`   | `false`                            |
| `constant NONE`                      | `nav2msgs.SPIN_RESULT_NONE`                     | `RosUInt16`    | `0`                                |
| `constant GOAL_REJECTED`             | `nav2msgs.SPIN_RESULT_GOAL_REJECTED`            | `RosUInt16`    | `1`                                |
| `constant SEND_GOAL_FAILURE`         | `nav2msgs.SPIN_RESULT_SEND_GOAL_FAILURE`        | `RosUInt16`    | `2`                                |
| `constant UNKNOWN`                   | `nav2msgs.SPIN_RESULT_UNKNOWN`                  | `RosUInt16`    | `700`                              |
| `constant TIMEOUT`                   | `nav2msgs.SPIN_RESULT_TIMEOUT`                  | `RosUInt16`    | `701`                              |
| `constant TF_ERROR`                  | `nav2msgs.SPIN_RESULT_TF_ERROR`                 | `RosUInt16`    | `702`                              |
| `constant COLLISION_AHEAD`           | `nav2msgs.SPIN_RESULT_COLLISION_AHEAD`          | `RosUInt16`    | `703`                              |
| `Result.total_elapsed_time`          | `nav2msgs.SpinResult.totalElapsedTime`          | `Duration`     | —                                  |
| `Result.error_code`                  | `nav2msgs.SpinResult.errorCode`                 | `RosUInt16`    | —                                  |
| `Result.error_msg`                   | `nav2msgs.SpinResult.errorMsg`                  | `RosString`    | —                                  |
| `Feedback.angular_distance_traveled` | `nav2msgs.SpinFeedback.angularDistanceTraveled` | `RosFloat32`   | —                                  |

### `navigation2/nav2_msgs/action/Wait.action`

Output: `nav2_msgs/interactions.ridl`. Pin:
`d7bf2ac06fe778c21c6141eb49b4d3e2c0c82d3a`.

- Goal: `nav2msgs.WaitGoal`.
- Result: `nav2msgs.WaitResult`.
- constant Result.NONE: `nav2msgs.WAIT_RESULT_NONE`.
- constant Result.GOAL_REJECTED: `nav2msgs.WAIT_RESULT_GOAL_REJECTED`.
- constant Result.SEND_GOAL_FAILURE: `nav2msgs.WAIT_RESULT_SEND_GOAL_FAILURE`.
- constant Result.UNKNOWN: `nav2msgs.WAIT_RESULT_UNKNOWN`.
- constant Result.TIMEOUT: `nav2msgs.WAIT_RESULT_TIMEOUT`.
- Feedback: `nav2msgs.WaitFeedback`.
- goal command: `nav2msgs.Nav2MsgsInteractions.wait`.
- feedback event: `nav2msgs.Nav2MsgsInteractions.waitFeedback`.

| Source field or constant     | Qualified output                         | Representation | Explicit default or constant value |
| ---------------------------- | ---------------------------------------- | -------------- | ---------------------------------- |
| `Goal.time`                  | `nav2msgs.WaitGoal.time`                 | `Duration`     | —                                  |
| `Result.total_elapsed_time`  | `nav2msgs.WaitResult.totalElapsedTime`   | `Duration`     | —                                  |
| `Result.error_code`          | `nav2msgs.WaitResult.errorCode`          | `RosUInt16`    | —                                  |
| `Result.error_msg`           | `nav2msgs.WaitResult.errorMsg`           | `RosString`    | —                                  |
| `constant NONE`              | `nav2msgs.WAIT_RESULT_NONE`              | `RosUInt16`    | `0`                                |
| `constant GOAL_REJECTED`     | `nav2msgs.WAIT_RESULT_GOAL_REJECTED`     | `RosUInt16`    | `1`                                |
| `constant SEND_GOAL_FAILURE` | `nav2msgs.WAIT_RESULT_SEND_GOAL_FAILURE` | `RosUInt16`    | `2`                                |
| `constant UNKNOWN`           | `nav2msgs.WAIT_RESULT_UNKNOWN`           | `RosUInt16`    | `740`                              |
| `constant TIMEOUT`           | `nav2msgs.WAIT_RESULT_TIMEOUT`           | `RosUInt16`    | `741`                              |
| `Feedback.time_left`         | `nav2msgs.WaitFeedback.timeLeft`         | `Duration`     | —                                  |

### `rcl-interfaces/builtin_interfaces/msg/Duration.msg`

**Required dependency definition.**

Output: `builtin_interfaces/messages.typl`. Pin:
`99aea442813391cc20344c5b4c79e5191bf7f2c7`.

- message: `builtininterfaces.Duration`.

| Source field or constant | Qualified output                     | Representation   | Explicit default or constant value |
| ------------------------ | ------------------------------------ | ---------------- | ---------------------------------- |
| `sec`                    | `builtininterfaces.Duration.sec`     | `RosInt32`       | —                                  |
| `nanosec`                | `builtininterfaces.Duration.nanosec` | `RosNanoseconds` | —                                  |

### `rcl-interfaces/builtin_interfaces/msg/Time.msg`

**Required dependency definition.**

Output: `builtin_interfaces/messages.typl`. Pin:
`99aea442813391cc20344c5b4c79e5191bf7f2c7`.

- message: `builtininterfaces.Time`.

| Source field or constant | Qualified output                 | Representation   | Explicit default or constant value |
| ------------------------ | -------------------------------- | ---------------- | ---------------------------------- |
| `sec`                    | `builtininterfaces.Time.sec`     | `RosInt32`       | —                                  |
| `nanosec`                | `builtininterfaces.Time.nanosec` | `RosNanoseconds` | —                                  |

## Exhaustive lexical field changes

- `std_msgs/msg/Header.msg` message: `frame_id` → `frameId` (snake_case to
  camelCase).
- `std_msgs/msg/MultiArrayLayout.msg` message: `data_offset` → `dataOffset`
  (snake_case to camelCase).
- `geometry_msgs/msg/TransformStamped.msg` message: `child_frame_id` →
  `childFrameId` (snake_case to camelCase).
- `geometry_msgs/msg/VelocityStamped.msg` message: `body_frame_id` →
  `bodyFrameId` (snake_case to camelCase).
- `geometry_msgs/msg/VelocityStamped.msg` message: `reference_frame_id` →
  `referenceFrameId` (snake_case to camelCase).
- `geometry_msgs/msg/VelocityWithCovarianceStamped.msg` message: `body_frame_id`
  → `bodyFrameId` (snake_case to camelCase).
- `geometry_msgs/msg/VelocityWithCovarianceStamped.msg` message:
  `reference_frame_id` → `referenceFrameId` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `current` → `currentValue`
  (reserved word).
- `sensor_msgs/msg/BatteryState.msg` message: `design_capacity` →
  `designCapacity` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `power_supply_status` →
  `powerSupplyStatus` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `power_supply_health` →
  `powerSupplyHealth` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `power_supply_technology` →
  `powerSupplyTechnology` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `cell_voltage` → `cellVoltage`
  (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `cell_temperature` →
  `cellTemperature` (snake_case to camelCase).
- `sensor_msgs/msg/BatteryState.msg` message: `serial_number` → `serialNumber`
  (snake_case to camelCase).
- `sensor_msgs/msg/CameraInfo.msg` message: `distortion_model` →
  `distortionModel` (snake_case to camelCase).
- `sensor_msgs/msg/CameraInfo.msg` message: `binning_x` → `binningX` (snake_case
  to camelCase).
- `sensor_msgs/msg/CameraInfo.msg` message: `binning_y` → `binningY` (snake_case
  to camelCase).
- `sensor_msgs/msg/FluidPressure.msg` message: `fluid_pressure` →
  `fluidPressure` (snake_case to camelCase).
- `sensor_msgs/msg/Image.msg` message: `is_bigendian` → `isBigendian`
  (snake_case to camelCase).
- `sensor_msgs/msg/Image.msg` message: `step` → `stepValue` (reserved word).
- `sensor_msgs/msg/Imu.msg` message: `orientation_covariance` →
  `orientationCovariance` (snake_case to camelCase).
- `sensor_msgs/msg/Imu.msg` message: `angular_velocity` → `angularVelocity`
  (snake_case to camelCase).
- `sensor_msgs/msg/Imu.msg` message: `angular_velocity_covariance` →
  `angularVelocityCovariance` (snake_case to camelCase).
- `sensor_msgs/msg/Imu.msg` message: `linear_acceleration` →
  `linearAcceleration` (snake_case to camelCase).
- `sensor_msgs/msg/Imu.msg` message: `linear_acceleration_covariance` →
  `linearAccelerationCovariance` (snake_case to camelCase).
- `sensor_msgs/msg/JoyFeedback.msg` message: `type` → `typeValue` (reserved
  word).
- `sensor_msgs/msg/LaserScan.msg` message: `angle_min` → `angleMin` (snake_case
  to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `angle_max` → `angleMax` (snake_case
  to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `angle_increment` → `angleIncrement`
  (snake_case to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `time_increment` → `timeIncrement`
  (snake_case to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `scan_time` → `scanTime` (snake_case
  to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `range_min` → `rangeMin` (snake_case
  to camelCase).
- `sensor_msgs/msg/LaserScan.msg` message: `range_max` → `rangeMax` (snake_case
  to camelCase).
- `sensor_msgs/msg/MagneticField.msg` message: `magnetic_field` →
  `magneticField` (snake_case to camelCase).
- `sensor_msgs/msg/MagneticField.msg` message: `magnetic_field_covariance` →
  `magneticFieldCovariance` (snake_case to camelCase).
- `sensor_msgs/msg/MultiDOFJointState.msg` message: `joint_names` → `jointNames`
  (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `angle_min` → `angleMin`
  (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `angle_max` → `angleMax`
  (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `angle_increment` →
  `angleIncrement` (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `time_increment` →
  `timeIncrement` (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `scan_time` → `scanTime`
  (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `range_min` → `rangeMin`
  (snake_case to camelCase).
- `sensor_msgs/msg/MultiEchoLaserScan.msg` message: `range_max` → `rangeMax`
  (snake_case to camelCase).
- `sensor_msgs/msg/NavSatFix.msg` message: `position_covariance` →
  `positionCovariance` (snake_case to camelCase).
- `sensor_msgs/msg/NavSatFix.msg` message: `position_covariance_type` →
  `positionCovarianceType` (snake_case to camelCase).
- `sensor_msgs/msg/NavSatStatus.msg` message: `service` → `serviceValue`
  (reserved word).
- `sensor_msgs/msg/PointCloud2.msg` message: `is_bigendian` → `isBigendian`
  (snake_case to camelCase).
- `sensor_msgs/msg/PointCloud2.msg` message: `point_step` → `pointStep`
  (snake_case to camelCase).
- `sensor_msgs/msg/PointCloud2.msg` message: `row_step` → `rowStep` (snake_case
  to camelCase).
- `sensor_msgs/msg/PointCloud2.msg` message: `is_dense` → `isDense` (snake_case
  to camelCase).
- `sensor_msgs/msg/Range.msg` message: `radiation_type` → `radiationType`
  (snake_case to camelCase).
- `sensor_msgs/msg/Range.msg` message: `field_of_view` → `fieldOfView`
  (snake_case to camelCase).
- `sensor_msgs/msg/Range.msg` message: `min_range` → `minRange` (snake_case to
  camelCase).
- `sensor_msgs/msg/Range.msg` message: `max_range` → `maxRange` (snake_case to
  camelCase).
- `sensor_msgs/msg/RegionOfInterest.msg` message: `x_offset` → `xOffset`
  (snake_case to camelCase).
- `sensor_msgs/msg/RegionOfInterest.msg` message: `y_offset` → `yOffset`
  (snake_case to camelCase).
- `sensor_msgs/msg/RegionOfInterest.msg` message: `do_rectify` → `doRectify`
  (snake_case to camelCase).
- `sensor_msgs/msg/RelativeHumidity.msg` message: `relative_humidity` →
  `relativeHumidity` (snake_case to camelCase).
- `sensor_msgs/msg/TimeReference.msg` message: `time_ref` → `timeRef`
  (snake_case to camelCase).
- `sensor_msgs/srv/SetCameraInfo.srv` Request: `camera_info` → `cameraInfo`
  (snake_case to camelCase).
- `sensor_msgs/srv/SetCameraInfo.srv` Response: `status_message` →
  `statusMessage` (snake_case to camelCase).
- `nav_msgs/msg/GridCells.msg` message: `cell_width` → `cellWidth` (snake_case
  to camelCase).
- `nav_msgs/msg/GridCells.msg` message: `cell_height` → `cellHeight` (snake_case
  to camelCase).
- `nav_msgs/msg/MapMetaData.msg` message: `map_load_time` → `mapLoadTime`
  (snake_case to camelCase).
- `nav_msgs/msg/Odometry.msg` message: `child_frame_id` → `childFrameId`
  (snake_case to camelCase).
- `nav_msgs/srv/LoadMap.srv` Request: `map_url` → `mapUrl` (snake_case to
  camelCase).
- `nav_msgs/srv/SetMap.srv` Request: `initial_pose` → `initialPose` (snake_case
  to camelCase).
- `nav2_msgs/msg/BehaviorTreeLog.msg` message: `event_log` → `eventLog`
  (snake_case to camelCase).
- `nav2_msgs/msg/BehaviorTreeStatusChange.msg` message: `node_name` → `nodeName`
  (snake_case to camelCase).
- `nav2_msgs/msg/BehaviorTreeStatusChange.msg` message: `previous_status` →
  `previousStatus` (snake_case to camelCase).
- `nav2_msgs/msg/BehaviorTreeStatusChange.msg` message: `current_status` →
  `currentStatus` (snake_case to camelCase).
- `nav2_msgs/msg/CostmapMetaData.msg` message: `map_load_time` → `mapLoadTime`
  (snake_case to camelCase).
- `nav2_msgs/msg/CostmapMetaData.msg` message: `update_time` → `updateTime`
  (snake_case to camelCase).
- `nav2_msgs/msg/CostmapMetaData.msg` message: `size_x` → `sizeX` (snake_case to
  camelCase).
- `nav2_msgs/msg/CostmapMetaData.msg` message: `size_y` → `sizeY` (snake_case to
  camelCase).
- `nav2_msgs/msg/SpeedLimit.msg` message: `speed_limit` → `speedLimit`
  (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `position_tracking_error` →
  `positionTrackingError` (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `heading_tracking_error` →
  `headingTrackingError` (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `current_path_index` →
  `currentPathIndex` (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `robot_pose` → `robotPose`
  (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `distance_to_goal` →
  `distanceToGoal` (snake_case to camelCase).
- `nav2_msgs/msg/TrackingFeedback.msg` message: `remaining_path_length` →
  `remainingPathLength` (snake_case to camelCase).
- `nav2_msgs/msg/VoxelGrid.msg` message: `size_x` → `sizeX` (snake_case to
  camelCase).
- `nav2_msgs/msg/VoxelGrid.msg` message: `size_y` → `sizeY` (snake_case to
  camelCase).
- `nav2_msgs/msg/VoxelGrid.msg` message: `size_z` → `sizeZ` (snake_case to
  camelCase).
- `nav2_msgs/msg/WaypointStatus.msg` message: `waypoint_status` →
  `waypointStatus` (snake_case to camelCase).
- `nav2_msgs/msg/WaypointStatus.msg` message: `waypoint_index` → `waypointIndex`
  (snake_case to camelCase).
- `nav2_msgs/msg/WaypointStatus.msg` message: `waypoint_pose` → `waypointPose`
  (snake_case to camelCase).
- `nav2_msgs/msg/WaypointStatus.msg` message: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/msg/WaypointStatus.msg` message: `error_msg` → `errorMsg`
  (snake_case to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Request: `max_cost` → `maxCost` (snake_case to
  camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Request: `consider_unknown_as_obstacle` →
  `considerUnknownAsObstacle` (snake_case to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Request: `layer_name` → `layerName`
  (snake_case to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Request: `stop_at_first_collision` →
  `stopAtFirstCollision` (snake_case to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Request: `max_lookahead_distance` →
  `maxLookaheadDistance` (snake_case to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Response: `is_valid` → `isValid` (snake_case
  to camelCase).
- `nav2_msgs/srv/IsPathValid.srv` Response: `invalid_pose_indices` →
  `invalidPoseIndices` (snake_case to camelCase).
- `nav2_msgs/srv/LoadMap.srv` Request: `map_url` → `mapUrl` (snake_case to
  camelCase).
- `nav2_msgs/srv/ManageLifecycleNodes.srv` Request: `command` → `commandValue`
  (reserved word).
- `nav2_msgs/srv/SaveMap.srv` Request: `map_topic` → `mapTopic` (snake_case to
  camelCase).
- `nav2_msgs/srv/SaveMap.srv` Request: `map_url` → `mapUrl` (snake_case to
  camelCase).
- `nav2_msgs/srv/SaveMap.srv` Request: `image_format` → `imageFormat`
  (snake_case to camelCase).
- `nav2_msgs/srv/SaveMap.srv` Request: `map_mode` → `mapMode` (snake_case to
  camelCase).
- `nav2_msgs/srv/SaveMap.srv` Request: `free_thresh` → `freeThresh` (snake_case
  to camelCase).
- `nav2_msgs/srv/SaveMap.srv` Request: `occupied_thresh` → `occupiedThresh`
  (snake_case to camelCase).
- `nav2_msgs/action/AssistedTeleop.action` Goal: `time_allowance` →
  `timeAllowance` (snake_case to camelCase).
- `nav2_msgs/action/AssistedTeleop.action` Result: `total_elapsed_time` →
  `totalElapsedTime` (snake_case to camelCase).
- `nav2_msgs/action/AssistedTeleop.action` Result: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/action/AssistedTeleop.action` Result: `error_msg` → `errorMsg`
  (snake_case to camelCase).
- `nav2_msgs/action/AssistedTeleop.action` Feedback: `current_teleop_duration` →
  `currentTeleopDuration` (snake_case to camelCase).
- `nav2_msgs/action/BackUp.action` Goal: `time_allowance` → `timeAllowance`
  (snake_case to camelCase).
- `nav2_msgs/action/BackUp.action` Goal: `disable_collision_checks` →
  `disableCollisionChecks` (snake_case to camelCase).
- `nav2_msgs/action/BackUp.action` Result: `total_elapsed_time` →
  `totalElapsedTime` (snake_case to camelCase).
- `nav2_msgs/action/BackUp.action` Result: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/action/BackUp.action` Result: `error_msg` → `errorMsg` (snake_case
  to camelCase).
- `nav2_msgs/action/BackUp.action` Feedback: `distance_traveled` →
  `distanceTraveled` (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Goal: `controller_id` → `controllerId`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Goal: `goal_checker_id` → `goalCheckerId`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Goal: `progress_checker_id` →
  `progressCheckerId` (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Goal: `path_handler_id` → `pathHandlerId`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Result: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Result: `error_msg` → `errorMsg`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowPath.action` Feedback: `tracking_feedback` →
  `trackingFeedback` (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Goal: `number_of_loops` →
  `numberOfLoops` (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Goal: `goal_index` → `goalIndex`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Result: `missed_waypoints` →
  `missedWaypoints` (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Result: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Result: `error_msg` → `errorMsg`
  (snake_case to camelCase).
- `nav2_msgs/action/FollowWaypoints.action` Feedback: `current_waypoint` →
  `currentWaypoint` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Goal: `behavior_tree` →
  `behaviorTree` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Result: `error_code` → `errorCode`
  (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Result: `error_msg` → `errorMsg`
  (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `current_pose` →
  `currentPose` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `navigation_time` →
  `navigationTime` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `estimated_time_remaining`
  → `estimatedTimeRemaining` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `number_of_recoveries` →
  `numberOfRecoveries` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `distance_remaining` →
  `distanceRemaining` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `position_tracking_error` →
  `positionTrackingError` (snake_case to camelCase).
- `nav2_msgs/action/NavigateToPose.action` Feedback: `heading_tracking_error` →
  `headingTrackingError` (snake_case to camelCase).
- `nav2_msgs/action/Spin.action` Goal: `target_yaw` → `targetYaw` (snake_case to
  camelCase).
- `nav2_msgs/action/Spin.action` Goal: `time_allowance` → `timeAllowance`
  (snake_case to camelCase).
- `nav2_msgs/action/Spin.action` Goal: `disable_collision_checks` →
  `disableCollisionChecks` (snake_case to camelCase).
- `nav2_msgs/action/Spin.action` Result: `total_elapsed_time` →
  `totalElapsedTime` (snake_case to camelCase).
- `nav2_msgs/action/Spin.action` Result: `error_code` → `errorCode` (snake_case
  to camelCase).
- `nav2_msgs/action/Spin.action` Result: `error_msg` → `errorMsg` (snake_case to
  camelCase).
- `nav2_msgs/action/Spin.action` Feedback: `angular_distance_traveled` →
  `angularDistanceTraveled` (snake_case to camelCase).
- `nav2_msgs/action/Wait.action` Result: `total_elapsed_time` →
  `totalElapsedTime` (snake_case to camelCase).
- `nav2_msgs/action/Wait.action` Result: `error_code` → `errorCode` (snake_case
  to camelCase).
- `nav2_msgs/action/Wait.action` Result: `error_msg` → `errorMsg` (snake_case to
  camelCase).
- `nav2_msgs/action/Wait.action` Feedback: `time_left` → `timeLeft` (snake_case
  to camelCase).

## Explicit defaults

- `geometrymsgs.Quaternion.x`: ROS `0` → RIDL `0.0`.
- `geometrymsgs.Quaternion.y`: ROS `0` → RIDL `0.0`.
- `geometrymsgs.Quaternion.z`: ROS `0` → RIDL `0.0`.
- `geometrymsgs.Quaternion.w`: ROS `1` → RIDL `1.0`.
- `sensormsgs.NavSatStatus.status`: ROS `-2` → RIDL `-2`.
- `nav2msgs.IsPathValidRequest.maxCost`: ROS `254` → RIDL `254`.
- `nav2msgs.IsPathValidRequest.considerUnknownAsObstacle`: ROS `false` → RIDL
  `false`.
- `nav2msgs.IsPathValidRequest.layerName`: ROS `""` → RIDL `""`.
- `nav2msgs.IsPathValidRequest.footprint`: ROS `""` → RIDL `""`.
- `nav2msgs.IsPathValidRequest.stopAtFirstCollision`: ROS `true` → RIDL `true`.
- `nav2msgs.IsPathValidRequest.maxLookaheadDistance`: ROS `-1.0` → RIDL `-1.0`.
- `nav2msgs.BackUpGoal.disableCollisionChecks`: ROS `false` → RIDL `false`.
- `nav2msgs.FollowWaypointsGoal.goalIndex`: ROS `0` → RIDL `0`.
- `nav2msgs.SpinGoal.disableCollisionChecks`: ROS `false` → RIDL `false`.

## Sequence representation ceilings

- `std_msgs/messages.typl`, `stdmsgs.ByteMultiArray.data`: `byte[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Float32MultiArray.data`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Float64MultiArray.data`: `float64[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Int16MultiArray.data`: `int16[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Int32MultiArray.data`: `int32[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Int64MultiArray.data`: `int64[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.Int8MultiArray.data`: `int8[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.MultiArrayLayout.dim`:
  `MultiArrayDimension[]` → variable-length array with the maximal mechanical
  bound above.
- `std_msgs/messages.typl`, `stdmsgs.UInt16MultiArray.data`: `uint16[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.UInt32MultiArray.data`: `uint32[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.UInt64MultiArray.data`: `uint64[]` →
  variable-length array with the maximal mechanical bound above.
- `std_msgs/messages.typl`, `stdmsgs.UInt8MultiArray.data`: `uint8[]` →
  variable-length array with the maximal mechanical bound above.
- `geometry_msgs/messages.typl`, `geometrymsgs.Polygon.points`: `Point32[]` →
  variable-length array with the maximal mechanical bound above.
- `geometry_msgs/messages.typl`, `geometrymsgs.PoseArray.poses`: `Pose[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.BatteryState.cellVoltage`:
  `float32[]` → variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.BatteryState.cellTemperature`:
  `float32[]` → variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.CameraInfo.d`: `float64[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.ChannelFloat32.values`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.CompressedImage.data`: `uint8[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.Image.data`: `uint8[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.JointState.name`: `string[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.JointState.position`: `float64[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.JointState.velocity`: `float64[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.JointState.effort`: `float64[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.Joy.axes`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.Joy.buttons`: `int32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.JoyFeedbackArray.array`:
  `JoyFeedback[]` → variable-length array with the maximal mechanical bound
  above.
- `sensor_msgs/messages.typl`, `sensormsgs.LaserEcho.echoes`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.LaserScan.ranges`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.LaserScan.intensities`: `float32[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiDOFJointState.jointNames`:
  `string[]` → variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiDOFJointState.transforms`:
  `geometry_msgs/Transform[]` → variable-length array with the maximal
  mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiDOFJointState.twist`:
  `geometry_msgs/Twist[]` → variable-length array with the maximal mechanical
  bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiDOFJointState.wrench`:
  `geometry_msgs/Wrench[]` → variable-length array with the maximal mechanical
  bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiEchoLaserScan.ranges`:
  `LaserEcho[]` → variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.MultiEchoLaserScan.intensities`:
  `LaserEcho[]` → variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.PointCloud.points`:
  `geometry_msgs/Point32[]` → variable-length array with the maximal mechanical
  bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.PointCloud.channels`:
  `ChannelFloat32[]` → variable-length array with the maximal mechanical bound
  above.
- `sensor_msgs/messages.typl`, `sensormsgs.PointCloud2.fields`: `PointField[]` →
  variable-length array with the maximal mechanical bound above.
- `sensor_msgs/messages.typl`, `sensormsgs.PointCloud2.data`: `uint8[]` →
  variable-length array with the maximal mechanical bound above.
- `nav_msgs/messages.typl`, `navmsgs.Goals.goals`: `geometry_msgs/PoseStamped[]`
  → variable-length array with the maximal mechanical bound above.
- `nav_msgs/messages.typl`, `navmsgs.GridCells.cells`: `geometry_msgs/Point[]` →
  variable-length array with the maximal mechanical bound above.
- `nav_msgs/messages.typl`, `navmsgs.OccupancyGrid.data`: `int8[]` →
  variable-length array with the maximal mechanical bound above.
- `nav_msgs/messages.typl`, `navmsgs.Path.poses`: `geometry_msgs/PoseStamped[]`
  → variable-length array with the maximal mechanical bound above.
- `nav_msgs/messages.typl`, `navmsgs.Trajectory.points`: `TrajectoryPoint[]` →
  variable-length array with the maximal mechanical bound above.
- `nav2_msgs/messages.typl`, `nav2msgs.BehaviorTreeLog.eventLog`:
  `BehaviorTreeStatusChange[]` → variable-length array with the maximal
  mechanical bound above.
- `nav2_msgs/messages.typl`, `nav2msgs.Costmap.data`: `uint8[]` →
  variable-length array with the maximal mechanical bound above.
- `nav2_msgs/messages.typl`, `nav2msgs.ParticleCloud.particles`: `Particle[]` →
  variable-length array with the maximal mechanical bound above.
- `nav2_msgs/messages.typl`, `nav2msgs.VoxelGrid.data`: `uint32[]` →
  variable-length array with the maximal mechanical bound above.
- `nav2_msgs/interactions.ridl`, `nav2msgs.ClearEntireCostmapRequest.plugins`:
  `string[]` → variable-length array with the maximal mechanical bound above.
- `nav2_msgs/interactions.ridl`,
  `nav2msgs.IsPathValidResponse.invalidPoseIndices`: `int32[]` → variable-length
  array with the maximal mechanical bound above.
- `nav2_msgs/interactions.ridl`, `nav2msgs.FollowWaypointsGoal.poses`:
  `geometry_msgs/PoseStamped[]` → variable-length array with the maximal
  mechanical bound above.
- `nav2_msgs/interactions.ridl`,
  `nav2msgs.FollowWaypointsResult.missedWaypoints`: `WaypointStatus[]` →
  variable-length array with the maximal mechanical bound above.

## Source float widths

- `stdmsgs.ColorRGBA.r`: `float32`.
- `stdmsgs.ColorRGBA.g`: `float32`.
- `stdmsgs.ColorRGBA.b`: `float32`.
- `stdmsgs.ColorRGBA.a`: `float32`.
- `stdmsgs.Float32.data`: `float32`.
- `stdmsgs.Float32MultiArray.data`: `float32`.
- `stdmsgs.Float64.data`: `float64`.
- `stdmsgs.Float64MultiArray.data`: `float64`.
- `geometrymsgs.AccelWithCovariance.covariance`: `float64`.
- `geometrymsgs.Inertia.m`: `float64`.
- `geometrymsgs.Inertia.ixx`: `float64`.
- `geometrymsgs.Inertia.ixy`: `float64`.
- `geometrymsgs.Inertia.ixz`: `float64`.
- `geometrymsgs.Inertia.iyy`: `float64`.
- `geometrymsgs.Inertia.iyz`: `float64`.
- `geometrymsgs.Inertia.izz`: `float64`.
- `geometrymsgs.Point.x`: `float64`.
- `geometrymsgs.Point.y`: `float64`.
- `geometrymsgs.Point.z`: `float64`.
- `geometrymsgs.Point32.x`: `float32`.
- `geometrymsgs.Point32.y`: `float32`.
- `geometrymsgs.Point32.z`: `float32`.
- `geometrymsgs.PoseWithCovariance.covariance`: `float64`.
- `geometrymsgs.Quaternion.x`: `float64`.
- `geometrymsgs.Quaternion.y`: `float64`.
- `geometrymsgs.Quaternion.z`: `float64`.
- `geometrymsgs.Quaternion.w`: `float64`.
- `geometrymsgs.TwistWithCovariance.covariance`: `float64`.
- `geometrymsgs.Vector3.x`: `float64`.
- `geometrymsgs.Vector3.y`: `float64`.
- `geometrymsgs.Vector3.z`: `float64`.
- `sensormsgs.BatteryState.voltage`: `float32`.
- `sensormsgs.BatteryState.temperature`: `float32`.
- `sensormsgs.BatteryState.currentValue`: `float32`.
- `sensormsgs.BatteryState.charge`: `float32`.
- `sensormsgs.BatteryState.capacity`: `float32`.
- `sensormsgs.BatteryState.designCapacity`: `float32`.
- `sensormsgs.BatteryState.percentage`: `float32`.
- `sensormsgs.BatteryState.cellVoltage`: `float32`.
- `sensormsgs.BatteryState.cellTemperature`: `float32`.
- `sensormsgs.CameraInfo.d`: `float64`.
- `sensormsgs.CameraInfo.k`: `float64`.
- `sensormsgs.CameraInfo.r`: `float64`.
- `sensormsgs.CameraInfo.p`: `float64`.
- `sensormsgs.ChannelFloat32.values`: `float32`.
- `sensormsgs.FluidPressure.fluidPressure`: `float64`.
- `sensormsgs.FluidPressure.variance`: `float64`.
- `sensormsgs.Illuminance.illuminance`: `float64`.
- `sensormsgs.Illuminance.variance`: `float64`.
- `sensormsgs.Imu.orientationCovariance`: `float64`.
- `sensormsgs.Imu.angularVelocityCovariance`: `float64`.
- `sensormsgs.Imu.linearAccelerationCovariance`: `float64`.
- `sensormsgs.JointState.position`: `float64`.
- `sensormsgs.JointState.velocity`: `float64`.
- `sensormsgs.JointState.effort`: `float64`.
- `sensormsgs.Joy.axes`: `float32`.
- `sensormsgs.JoyFeedback.intensity`: `float32`.
- `sensormsgs.LaserEcho.echoes`: `float32`.
- `sensormsgs.LaserScan.angleMin`: `float32`.
- `sensormsgs.LaserScan.angleMax`: `float32`.
- `sensormsgs.LaserScan.angleIncrement`: `float32`.
- `sensormsgs.LaserScan.timeIncrement`: `float32`.
- `sensormsgs.LaserScan.scanTime`: `float32`.
- `sensormsgs.LaserScan.rangeMin`: `float32`.
- `sensormsgs.LaserScan.rangeMax`: `float32`.
- `sensormsgs.LaserScan.ranges`: `float32`.
- `sensormsgs.LaserScan.intensities`: `float32`.
- `sensormsgs.MagneticField.magneticFieldCovariance`: `float64`.
- `sensormsgs.MultiEchoLaserScan.angleMin`: `float32`.
- `sensormsgs.MultiEchoLaserScan.angleMax`: `float32`.
- `sensormsgs.MultiEchoLaserScan.angleIncrement`: `float32`.
- `sensormsgs.MultiEchoLaserScan.timeIncrement`: `float32`.
- `sensormsgs.MultiEchoLaserScan.scanTime`: `float32`.
- `sensormsgs.MultiEchoLaserScan.rangeMin`: `float32`.
- `sensormsgs.MultiEchoLaserScan.rangeMax`: `float32`.
- `sensormsgs.NavSatFix.latitude`: `float64`.
- `sensormsgs.NavSatFix.longitude`: `float64`.
- `sensormsgs.NavSatFix.altitude`: `float64`.
- `sensormsgs.NavSatFix.positionCovariance`: `float64`.
- `sensormsgs.Range.fieldOfView`: `float32`.
- `sensormsgs.Range.minRange`: `float32`.
- `sensormsgs.Range.maxRange`: `float32`.
- `sensormsgs.Range.range`: `float32`.
- `sensormsgs.Range.variance`: `float32`.
- `sensormsgs.RelativeHumidity.relativeHumidity`: `float64`.
- `sensormsgs.RelativeHumidity.variance`: `float64`.
- `sensormsgs.Temperature.temperature`: `float64`.
- `sensormsgs.Temperature.variance`: `float64`.
- `navmsgs.GridCells.cellWidth`: `float32`.
- `navmsgs.GridCells.cellHeight`: `float32`.
- `navmsgs.MapMetaData.resolution`: `float32`.
- `navmsgs.GetPlanRequest.tolerance`: `float32`.
- `nav2msgs.CostmapMetaData.resolution`: `float32`.
- `nav2msgs.Particle.weight`: `float64`.
- `nav2msgs.SpeedLimit.speedLimit`: `float64`.
- `nav2msgs.TrackingFeedback.positionTrackingError`: `float32`.
- `nav2msgs.TrackingFeedback.headingTrackingError`: `float32`.
- `nav2msgs.TrackingFeedback.distanceToGoal`: `float32`.
- `nav2msgs.TrackingFeedback.speed`: `float32`.
- `nav2msgs.TrackingFeedback.remainingPathLength`: `float32`.
- `nav2msgs.IsPathValidRequest.maxLookaheadDistance`: `float64`.
- `nav2msgs.SaveMapRequest.freeThresh`: `float32`.
- `nav2msgs.SaveMapRequest.occupiedThresh`: `float32`.
- `nav2msgs.BackUpGoal.speed`: `float32`.
- `nav2msgs.BackUpFeedback.distanceTraveled`: `float32`.
- `nav2msgs.NavigateToPoseFeedback.distanceRemaining`: `float32`.
- `nav2msgs.NavigateToPoseFeedback.positionTrackingError`: `float32`.
- `nav2msgs.NavigateToPoseFeedback.headingTrackingError`: `float32`.
- `nav2msgs.SpinGoal.targetYaw`: `float32`.
- `nav2msgs.SpinFeedback.angularDistanceTraveled`: `float32`.

## Primitive and unit helper mapping

Helpers are scalar representations of existing fields, not upstream messages.
They are declared once per package and reused by that package source files.

- `builtininterfaces.RosInt32` in `builtin_interfaces/messages.typl`:
  `integer [-2147483648..2147483647]`; used by `builtininterfaces.Duration.sec`,
  `builtininterfaces.Time.sec`.
- `builtininterfaces.RosNanoseconds` in `builtin_interfaces/messages.typl`:
  `integer [0..999999999]`; used by `builtininterfaces.Duration.nanosec`,
  `builtininterfaces.Time.nanosec`.
- `geometrymsgs.RosFloat32` in `geometry_msgs/messages.typl`: `float`; used by
  `geometrymsgs.Point32.x`, `geometrymsgs.Point32.y`, `geometrymsgs.Point32.z`.
- `geometrymsgs.RosFloat64` in `geometry_msgs/messages.typl`: `float`; used by
  `geometrymsgs.AccelWithCovariance.covariance`, `geometrymsgs.Point.x`,
  `geometrymsgs.Point.y`, `geometrymsgs.Point.z`,
  `geometrymsgs.PoseWithCovariance.covariance`, `geometrymsgs.Quaternion.w`,
  `geometrymsgs.Quaternion.x`, `geometrymsgs.Quaternion.y`,
  `geometrymsgs.Quaternion.z`, `geometrymsgs.TwistWithCovariance.covariance`,
  `geometrymsgs.Vector3.x`, `geometrymsgs.Vector3.y`, `geometrymsgs.Vector3.z`.
- `geometrymsgs.RosInertiaTensor` in `geometry_msgs/messages.typl`: `kg.m2`;
  used by `geometrymsgs.Inertia.ixx`, `geometrymsgs.Inertia.ixy`,
  `geometrymsgs.Inertia.ixz`, `geometrymsgs.Inertia.iyy`,
  `geometrymsgs.Inertia.iyz`, `geometrymsgs.Inertia.izz`.
- `geometrymsgs.RosInt64` in `geometry_msgs/messages.typl`:
  `integer [-9223372036854775808..9223372036854775807]`; used by
  `geometrymsgs.PolygonInstance.id`.
- `geometrymsgs.RosKilograms` in `geometry_msgs/messages.typl`: `kg`; used by
  `geometrymsgs.Inertia.m`.
- `geometrymsgs.RosString` in `geometry_msgs/messages.typl`:
  `string [0..18446744073709551615]`; used by
  `geometrymsgs.TransformStamped.childFrameId`,
  `geometrymsgs.VelocityStamped.bodyFrameId`,
  `geometrymsgs.VelocityStamped.referenceFrameId`,
  `geometrymsgs.VelocityWithCovarianceStamped.bodyFrameId`,
  `geometrymsgs.VelocityWithCovarianceStamped.referenceFrameId`.
- `nav2msgs.RosBoolean` in `nav2_msgs/messages.typl`: `boolean`; used by
  `nav2msgs.BackUpGoal.disableCollisionChecks`,
  `nav2msgs.ClearEntireCostmapResponse.success`,
  `nav2msgs.IsPathValidRequest.considerUnknownAsObstacle`,
  `nav2msgs.IsPathValidRequest.stopAtFirstCollision`,
  `nav2msgs.IsPathValidResponse.isValid`,
  `nav2msgs.IsPathValidResponse.success`,
  `nav2msgs.ManageLifecycleNodesResponse.success`,
  `nav2msgs.SaveMapResponse.result`, `nav2msgs.SpeedLimit.percentage`,
  `nav2msgs.SpinGoal.disableCollisionChecks`.
- `nav2msgs.RosFloat32` in `nav2_msgs/messages.typl`: `float`; used by
  `nav2msgs.BackUpFeedback.distanceTraveled`, `nav2msgs.BackUpGoal.speed`,
  `nav2msgs.CostmapMetaData.resolution`,
  `nav2msgs.NavigateToPoseFeedback.distanceRemaining`,
  `nav2msgs.NavigateToPoseFeedback.headingTrackingError`,
  `nav2msgs.NavigateToPoseFeedback.positionTrackingError`,
  `nav2msgs.SaveMapRequest.freeThresh`,
  `nav2msgs.SaveMapRequest.occupiedThresh`,
  `nav2msgs.SpinFeedback.angularDistanceTraveled`,
  `nav2msgs.SpinGoal.targetYaw`, `nav2msgs.TrackingFeedback.distanceToGoal`,
  `nav2msgs.TrackingFeedback.headingTrackingError`,
  `nav2msgs.TrackingFeedback.positionTrackingError`,
  `nav2msgs.TrackingFeedback.remainingPathLength`,
  `nav2msgs.TrackingFeedback.speed`.
- `nav2msgs.RosFloat64` in `nav2_msgs/messages.typl`: `float`; used by
  `nav2msgs.IsPathValidRequest.maxLookaheadDistance`,
  `nav2msgs.Particle.weight`, `nav2msgs.SpeedLimit.speedLimit`.
- `nav2msgs.RosInt16` in `nav2_msgs/interactions.ridl`:
  `integer [-32768..32767]`; used by
  `nav2msgs.NavigateToPoseFeedback.numberOfRecoveries`.
- `nav2msgs.RosInt32` in `nav2_msgs/interactions.ridl`:
  `integer [-2147483648..2147483647]`; used by
  `nav2msgs.IsPathValidResponse.invalidPoseIndices`.
- `nav2msgs.RosString` in `nav2_msgs/messages.typl`:
  `string [0..18446744073709551615]`; used by
  `nav2msgs.AssistedTeleopResult.errorMsg`, `nav2msgs.BackUpResult.errorMsg`,
  `nav2msgs.BehaviorTreeStatusChange.currentStatus`,
  `nav2msgs.BehaviorTreeStatusChange.nodeName`,
  `nav2msgs.BehaviorTreeStatusChange.previousStatus`,
  `nav2msgs.ClearEntireCostmapRequest.plugins`,
  `nav2msgs.CostmapMetaData.layer`, `nav2msgs.FollowPathGoal.controllerId`,
  `nav2msgs.FollowPathGoal.goalCheckerId`,
  `nav2msgs.FollowPathGoal.pathHandlerId`,
  `nav2msgs.FollowPathGoal.progressCheckerId`,
  `nav2msgs.FollowPathResult.errorMsg`,
  `nav2msgs.FollowWaypointsResult.errorMsg`,
  `nav2msgs.IsPathValidRequest.footprint`,
  `nav2msgs.IsPathValidRequest.layerName`, `nav2msgs.LoadMapRequest.mapUrl`,
  `nav2msgs.NavigateToPoseGoal.behaviorTree`,
  `nav2msgs.NavigateToPoseResult.errorMsg`,
  `nav2msgs.SaveMapRequest.imageFormat`, `nav2msgs.SaveMapRequest.mapMode`,
  `nav2msgs.SaveMapRequest.mapTopic`, `nav2msgs.SaveMapRequest.mapUrl`,
  `nav2msgs.SpinResult.errorMsg`, `nav2msgs.WaitResult.errorMsg`,
  `nav2msgs.WaypointStatus.errorMsg`.
- `nav2msgs.RosUInt16` in `nav2_msgs/messages.typl`: `integer [0..65535]`; used
  by `nav2msgs.ASSISTED_TELEOP_RESULT_GOAL_REJECTED`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_NONE`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_SEND_GOAL_FAILURE`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_TELEOP_INPUT_TIMEOUT`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_TF_ERROR`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_TIMEOUT`,
  `nav2msgs.ASSISTED_TELEOP_RESULT_UNKNOWN`,
  `nav2msgs.AssistedTeleopResult.errorCode`,
  `nav2msgs.BACK_UP_RESULT_COLLISION_AHEAD`,
  `nav2msgs.BACK_UP_RESULT_GOAL_REJECTED`,
  `nav2msgs.BACK_UP_RESULT_INVALID_INPUT`, `nav2msgs.BACK_UP_RESULT_NONE`,
  `nav2msgs.BACK_UP_RESULT_SEND_GOAL_FAILURE`,
  `nav2msgs.BACK_UP_RESULT_TF_ERROR`, `nav2msgs.BACK_UP_RESULT_TIMEOUT`,
  `nav2msgs.BACK_UP_RESULT_UNKNOWN`, `nav2msgs.BackUpResult.errorCode`,
  `nav2msgs.BehaviorTreeStatusChange.uid`,
  `nav2msgs.FOLLOW_PATH_RESULT_CONTROLLER_TIMED_OUT`,
  `nav2msgs.FOLLOW_PATH_RESULT_FAILED_TO_MAKE_PROGRESS`,
  `nav2msgs.FOLLOW_PATH_RESULT_GOAL_REJECTED`,
  `nav2msgs.FOLLOW_PATH_RESULT_INVALID_CONTROLLER`,
  `nav2msgs.FOLLOW_PATH_RESULT_INVALID_PATH`,
  `nav2msgs.FOLLOW_PATH_RESULT_NONE`,
  `nav2msgs.FOLLOW_PATH_RESULT_NO_VALID_CONTROL`,
  `nav2msgs.FOLLOW_PATH_RESULT_PATIENCE_EXCEEDED`,
  `nav2msgs.FOLLOW_PATH_RESULT_SEND_GOAL_FAILURE`,
  `nav2msgs.FOLLOW_PATH_RESULT_TF_ERROR`, `nav2msgs.FOLLOW_PATH_RESULT_TIMEOUT`,
  `nav2msgs.FOLLOW_PATH_RESULT_UNKNOWN`,
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NONE`,
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_NO_VALID_WAYPOINTS`,
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_STOP_ON_MISSED_WAYPOINT`,
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_TASK_EXECUTOR_FAILED`,
  `nav2msgs.FOLLOW_WAYPOINTS_RESULT_UNKNOWN`,
  `nav2msgs.FollowPathResult.errorCode`,
  `nav2msgs.FollowWaypointsResult.errorCode`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_FAILED_TO_LOAD_BEHAVIOR_TREE`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_GOAL_REJECTED`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_NONE`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_SEND_GOAL_FAILURE`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_TF_ERROR`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_TIMEOUT`,
  `nav2msgs.NAVIGATE_TO_POSE_RESULT_UNKNOWN`,
  `nav2msgs.NavigateToPoseResult.errorCode`,
  `nav2msgs.SPIN_RESULT_COLLISION_AHEAD`, `nav2msgs.SPIN_RESULT_GOAL_REJECTED`,
  `nav2msgs.SPIN_RESULT_NONE`, `nav2msgs.SPIN_RESULT_SEND_GOAL_FAILURE`,
  `nav2msgs.SPIN_RESULT_TF_ERROR`, `nav2msgs.SPIN_RESULT_TIMEOUT`,
  `nav2msgs.SPIN_RESULT_UNKNOWN`, `nav2msgs.SpinResult.errorCode`,
  `nav2msgs.WAIT_RESULT_GOAL_REJECTED`, `nav2msgs.WAIT_RESULT_NONE`,
  `nav2msgs.WAIT_RESULT_SEND_GOAL_FAILURE`, `nav2msgs.WAIT_RESULT_TIMEOUT`,
  `nav2msgs.WAIT_RESULT_UNKNOWN`, `nav2msgs.WaitResult.errorCode`,
  `nav2msgs.WaypointStatus.errorCode`.
- `nav2msgs.RosUInt32` in `nav2_msgs/messages.typl`: `integer [0..4294967295]`;
  used by `nav2msgs.CostmapMetaData.sizeX`, `nav2msgs.CostmapMetaData.sizeY`,
  `nav2msgs.FollowWaypointsFeedback.currentWaypoint`,
  `nav2msgs.FollowWaypointsGoal.goalIndex`,
  `nav2msgs.FollowWaypointsGoal.numberOfLoops`,
  `nav2msgs.TrackingFeedback.currentPathIndex`, `nav2msgs.VoxelGrid.data`,
  `nav2msgs.VoxelGrid.sizeX`, `nav2msgs.VoxelGrid.sizeY`,
  `nav2msgs.VoxelGrid.sizeZ`, `nav2msgs.WaypointStatus.waypointIndex`.
- `nav2msgs.RosUInt8` in `nav2_msgs/messages.typl`: `integer [0..255]`; used by
  `nav2msgs.Costmap.data`, `nav2msgs.IsPathValidRequest.maxCost`,
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`,
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA`,
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`,
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`,
  `nav2msgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`,
  `nav2msgs.LoadMapResponse.result`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CLEANUP`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_CONFIGURE`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_PAUSE`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESET`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_RESUME`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_SHUTDOWN`,
  `nav2msgs.MANAGE_LIFECYCLE_NODES_REQUEST_STARTUP`,
  `nav2msgs.ManageLifecycleNodesRequest.commandValue`,
  `nav2msgs.WAYPOINT_STATUS_COMPLETED`, `nav2msgs.WAYPOINT_STATUS_FAILED`,
  `nav2msgs.WAYPOINT_STATUS_PENDING`, `nav2msgs.WAYPOINT_STATUS_SKIPPED`,
  `nav2msgs.WaypointStatus.waypointStatus`.
- `navmsgs.RosBoolean` in `nav_msgs/interactions.ridl`: `boolean`; used by
  `navmsgs.SetMapResponse.success`.
- `navmsgs.RosFloat32` in `nav_msgs/messages.typl`: `float`; used by
  `navmsgs.GridCells.cellHeight`, `navmsgs.GridCells.cellWidth`,
  `navmsgs.MapMetaData.resolution`.
- `navmsgs.RosInt8` in `nav_msgs/messages.typl`: `integer [-128..127]`; used by
  `navmsgs.OccupancyGrid.data`.
- `navmsgs.RosMetres` in `nav_msgs/interactions.ridl`: `m`; used by
  `navmsgs.GetPlanRequest.tolerance`.
- `navmsgs.RosString` in `nav_msgs/messages.typl`:
  `string [0..18446744073709551615]`; used by `navmsgs.LoadMapRequest.mapUrl`,
  `navmsgs.Odometry.childFrameId`.
- `navmsgs.RosUInt32` in `nav_msgs/messages.typl`: `integer [0..4294967295]`;
  used by `navmsgs.MapMetaData.height`, `navmsgs.MapMetaData.width`.
- `navmsgs.RosUInt8` in `nav_msgs/interactions.ridl`: `integer [0..255]`; used
  by `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_DATA`,
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_INVALID_MAP_METADATA`,
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_MAP_DOES_NOT_EXIST`,
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_SUCCESS`,
  `navmsgs.LOAD_MAP_RESPONSE_RESULT_UNDEFINED_FAILURE`,
  `navmsgs.LoadMapResponse.result`.
- `sensormsgs.RosAmpereHours` in `sensor_msgs/messages.typl`: `A.h`; used by
  `sensormsgs.BatteryState.capacity`, `sensormsgs.BatteryState.charge`,
  `sensormsgs.BatteryState.designCapacity`.
- `sensormsgs.RosAmperes` in `sensor_msgs/messages.typl`: `A`; used by
  `sensormsgs.BatteryState.currentValue`.
- `sensormsgs.RosBoolean` in `sensor_msgs/messages.typl`: `boolean`; used by
  `sensormsgs.BatteryState.present`, `sensormsgs.PointCloud2.isBigendian`,
  `sensormsgs.PointCloud2.isDense`, `sensormsgs.RegionOfInterest.doRectify`,
  `sensormsgs.SetCameraInfoResponse.success`.
- `sensormsgs.RosCelsius` in `sensor_msgs/messages.typl`: `Cel`; used by
  `sensormsgs.BatteryState.temperature`, `sensormsgs.Temperature.temperature`.
- `sensormsgs.RosFloat32` in `sensor_msgs/messages.typl`: `float`; used by
  `sensormsgs.BatteryState.cellTemperature`,
  `sensormsgs.BatteryState.cellVoltage`, `sensormsgs.BatteryState.percentage`,
  `sensormsgs.ChannelFloat32.values`, `sensormsgs.Joy.axes`,
  `sensormsgs.JoyFeedback.intensity`, `sensormsgs.LaserEcho.echoes`,
  `sensormsgs.LaserScan.angleIncrement`, `sensormsgs.LaserScan.angleMax`,
  `sensormsgs.LaserScan.angleMin`, `sensormsgs.LaserScan.intensities`,
  `sensormsgs.MultiEchoLaserScan.angleIncrement`,
  `sensormsgs.MultiEchoLaserScan.angleMax`,
  `sensormsgs.MultiEchoLaserScan.angleMin`, `sensormsgs.Range.fieldOfView`,
  `sensormsgs.Range.variance`.
- `sensormsgs.RosFloat64` in `sensor_msgs/messages.typl`: `float`; used by
  `sensormsgs.CameraInfo.d`, `sensormsgs.CameraInfo.k`,
  `sensormsgs.CameraInfo.p`, `sensormsgs.CameraInfo.r`,
  `sensormsgs.FluidPressure.variance`, `sensormsgs.Illuminance.variance`,
  `sensormsgs.Imu.angularVelocityCovariance`,
  `sensormsgs.Imu.linearAccelerationCovariance`,
  `sensormsgs.Imu.orientationCovariance`, `sensormsgs.JointState.effort`,
  `sensormsgs.JointState.position`, `sensormsgs.JointState.velocity`,
  `sensormsgs.MagneticField.magneticFieldCovariance`,
  `sensormsgs.NavSatFix.latitude`, `sensormsgs.NavSatFix.longitude`,
  `sensormsgs.RelativeHumidity.relativeHumidity`,
  `sensormsgs.RelativeHumidity.variance`, `sensormsgs.Temperature.variance`.
- `sensormsgs.RosInt32` in `sensor_msgs/messages.typl`:
  `integer [-2147483648..2147483647]`; used by `sensormsgs.Joy.buttons`.
- `sensormsgs.RosInt8` in `sensor_msgs/messages.typl`: `integer [-128..127]`;
  used by `sensormsgs.NAV_SAT_STATUS_STATUS_FIX`,
  `sensormsgs.NAV_SAT_STATUS_STATUS_GBAS_FIX`,
  `sensormsgs.NAV_SAT_STATUS_STATUS_NO_FIX`,
  `sensormsgs.NAV_SAT_STATUS_STATUS_SBAS_FIX`,
  `sensormsgs.NAV_SAT_STATUS_STATUS_UNKNOWN`, `sensormsgs.NavSatStatus.status`.
- `sensormsgs.RosLux` in `sensor_msgs/messages.typl`: `lx`; used by
  `sensormsgs.Illuminance.illuminance`.
- `sensormsgs.RosMetres` in `sensor_msgs/messages.typl`: `m`; used by
  `sensormsgs.LaserScan.rangeMax`, `sensormsgs.LaserScan.rangeMin`,
  `sensormsgs.LaserScan.ranges`, `sensormsgs.MultiEchoLaserScan.rangeMax`,
  `sensormsgs.MultiEchoLaserScan.rangeMin`, `sensormsgs.NavSatFix.altitude`,
  `sensormsgs.Range.maxRange`, `sensormsgs.Range.minRange`,
  `sensormsgs.Range.range`.
- `sensormsgs.RosPascals` in `sensor_msgs/messages.typl`: `Pa`; used by
  `sensormsgs.FluidPressure.fluidPressure`.
- `sensormsgs.RosSeconds` in `sensor_msgs/messages.typl`: `s`; used by
  `sensormsgs.LaserScan.scanTime`, `sensormsgs.LaserScan.timeIncrement`,
  `sensormsgs.MultiEchoLaserScan.scanTime`,
  `sensormsgs.MultiEchoLaserScan.timeIncrement`.
- `sensormsgs.RosSquareMetres` in `sensor_msgs/messages.typl`: `m2`; used by
  `sensormsgs.NavSatFix.positionCovariance`.
- `sensormsgs.RosString` in `sensor_msgs/messages.typl`:
  `string [0..18446744073709551615]`; used by
  `sensormsgs.BatteryState.location`, `sensormsgs.BatteryState.serialNumber`,
  `sensormsgs.CameraInfo.distortionModel`, `sensormsgs.ChannelFloat32.name`,
  `sensormsgs.CompressedImage.format`, `sensormsgs.Image.encoding`,
  `sensormsgs.JointState.name`, `sensormsgs.MultiDOFJointState.jointNames`,
  `sensormsgs.PointField.name`,
  `sensormsgs.SetCameraInfoResponse.statusMessage`,
  `sensormsgs.TimeReference.source`.
- `sensormsgs.RosUInt16` in `sensor_msgs/messages.typl`: `integer [0..65535]`;
  used by `sensormsgs.NAV_SAT_STATUS_SERVICE_COMPASS`,
  `sensormsgs.NAV_SAT_STATUS_SERVICE_GALILEO`,
  `sensormsgs.NAV_SAT_STATUS_SERVICE_GLONASS`,
  `sensormsgs.NAV_SAT_STATUS_SERVICE_GPS`,
  `sensormsgs.NAV_SAT_STATUS_SERVICE_UNKNOWN`,
  `sensormsgs.NavSatStatus.serviceValue`.
- `sensormsgs.RosUInt32` in `sensor_msgs/messages.typl`:
  `integer [0..4294967295]`; used by `sensormsgs.CameraInfo.binningX`,
  `sensormsgs.CameraInfo.binningY`, `sensormsgs.CameraInfo.height`,
  `sensormsgs.CameraInfo.width`, `sensormsgs.Image.height`,
  `sensormsgs.Image.stepValue`, `sensormsgs.Image.width`,
  `sensormsgs.PointCloud2.height`, `sensormsgs.PointCloud2.pointStep`,
  `sensormsgs.PointCloud2.rowStep`, `sensormsgs.PointCloud2.width`,
  `sensormsgs.PointField.count`, `sensormsgs.PointField.offset`,
  `sensormsgs.RegionOfInterest.height`, `sensormsgs.RegionOfInterest.width`,
  `sensormsgs.RegionOfInterest.xOffset`, `sensormsgs.RegionOfInterest.yOffset`.
- `sensormsgs.RosUInt8` in `sensor_msgs/messages.typl`: `integer [0..255]`; used
  by `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_COLD`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_DEAD`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_GOOD`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERHEAT`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_OVERVOLTAGE`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_SAFETY_TIMER_EXPIRE`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNKNOWN`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_UNSPEC_FAILURE`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_HEALTH_WATCHDOG_TIMER_EXPIRE`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_CHARGING`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_DISCHARGING`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_FULL`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_NOT_CHARGING`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_STATUS_UNKNOWN`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIFE`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIMN`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LION`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_LIPO`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NICD`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_NIMH`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_TERNARY`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_UNKNOWN`,
  `sensormsgs.BATTERY_STATE_POWER_SUPPLY_TECHNOLOGY_VRLA`,
  `sensormsgs.BatteryState.powerSupplyHealth`,
  `sensormsgs.BatteryState.powerSupplyStatus`,
  `sensormsgs.BatteryState.powerSupplyTechnology`,
  `sensormsgs.CompressedImage.data`, `sensormsgs.Image.data`,
  `sensormsgs.Image.isBigendian`, `sensormsgs.JOY_FEEDBACK_TYPE_BUZZER`,
  `sensormsgs.JOY_FEEDBACK_TYPE_LED`, `sensormsgs.JOY_FEEDBACK_TYPE_RUMBLE`,
  `sensormsgs.JoyFeedback.id`, `sensormsgs.JoyFeedback.typeValue`,
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_APPROXIMATED`,
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_DIAGONAL_KNOWN`,
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_KNOWN`,
  `sensormsgs.NAV_SAT_FIX_COVARIANCE_TYPE_UNKNOWN`,
  `sensormsgs.NavSatFix.positionCovarianceType`, `sensormsgs.POINT_FIELD_BOOL`,
  `sensormsgs.POINT_FIELD_FLOAT32`, `sensormsgs.POINT_FIELD_FLOAT64`,
  `sensormsgs.POINT_FIELD_INT16`, `sensormsgs.POINT_FIELD_INT32`,
  `sensormsgs.POINT_FIELD_INT64`, `sensormsgs.POINT_FIELD_INT8`,
  `sensormsgs.POINT_FIELD_UINT16`, `sensormsgs.POINT_FIELD_UINT32`,
  `sensormsgs.POINT_FIELD_UINT64`, `sensormsgs.POINT_FIELD_UINT8`,
  `sensormsgs.PointCloud2.data`, `sensormsgs.PointField.datatype`,
  `sensormsgs.RANGE_INFRARED`, `sensormsgs.RANGE_ULTRASOUND`,
  `sensormsgs.Range.radiationType`.
- `sensormsgs.RosVolts` in `sensor_msgs/messages.typl`: `V`; used by
  `sensormsgs.BatteryState.voltage`.
- `stdmsgs.RosBoolean` in `std_msgs/messages.typl`: `boolean`; used by
  `stdmsgs.Bool.data`.
- `stdmsgs.RosByte` in `std_msgs/messages.typl`: `integer [0..255]`; used by
  `stdmsgs.Byte.data`, `stdmsgs.ByteMultiArray.data`.
- `stdmsgs.RosChar` in `std_msgs/messages.typl`: `integer [0..255]`; used by
  `stdmsgs.Char.data`.
- `stdmsgs.RosFloat32` in `std_msgs/messages.typl`: `float`; used by
  `stdmsgs.ColorRGBA.a`, `stdmsgs.ColorRGBA.b`, `stdmsgs.ColorRGBA.g`,
  `stdmsgs.ColorRGBA.r`, `stdmsgs.Float32.data`,
  `stdmsgs.Float32MultiArray.data`.
- `stdmsgs.RosFloat64` in `std_msgs/messages.typl`: `float`; used by
  `stdmsgs.Float64.data`, `stdmsgs.Float64MultiArray.data`.
- `stdmsgs.RosInt16` in `std_msgs/messages.typl`: `integer [-32768..32767]`;
  used by `stdmsgs.Int16.data`, `stdmsgs.Int16MultiArray.data`.
- `stdmsgs.RosInt32` in `std_msgs/messages.typl`:
  `integer [-2147483648..2147483647]`; used by `stdmsgs.Int32.data`,
  `stdmsgs.Int32MultiArray.data`.
- `stdmsgs.RosInt64` in `std_msgs/messages.typl`:
  `integer [-9223372036854775808..9223372036854775807]`; used by
  `stdmsgs.Int64.data`, `stdmsgs.Int64MultiArray.data`.
- `stdmsgs.RosInt8` in `std_msgs/messages.typl`: `integer [-128..127]`; used by
  `stdmsgs.Int8.data`, `stdmsgs.Int8MultiArray.data`.
- `stdmsgs.RosString` in `std_msgs/messages.typl`:
  `string [0..18446744073709551615]`; used by `stdmsgs.Header.frameId`,
  `stdmsgs.MultiArrayDimension.label`, `stdmsgs.String.data`.
- `stdmsgs.RosUInt16` in `std_msgs/messages.typl`: `integer [0..65535]`; used by
  `stdmsgs.UInt16.data`, `stdmsgs.UInt16MultiArray.data`.
- `stdmsgs.RosUInt32` in `std_msgs/messages.typl`: `integer [0..4294967295]`;
  used by `stdmsgs.MultiArrayDimension.size`,
  `stdmsgs.MultiArrayDimension.stride`, `stdmsgs.MultiArrayLayout.dataOffset`,
  `stdmsgs.UInt32.data`, `stdmsgs.UInt32MultiArray.data`.
- `stdmsgs.RosUInt64` in `std_msgs/messages.typl`: `bytes [8]`; used by
  `stdmsgs.UInt64.data`, `stdmsgs.UInt64MultiArray.data`.
- `stdmsgs.RosUInt8` in `std_msgs/messages.typl`: `integer [0..255]`; used by
  `stdmsgs.UInt8.data`, `stdmsgs.UInt8MultiArray.data`.
- `stdsrvs.RosBoolean` in `std_srvs/interactions.ridl`: `boolean`; used by
  `stdsrvs.SetBoolRequest.data`, `stdsrvs.SetBoolResponse.success`,
  `stdsrvs.TriggerResponse.success`.
- `stdsrvs.RosString` in `std_srvs/interactions.ridl`:
  `string [0..18446744073709551615]`; used by `stdsrvs.SetBoolResponse.message`,
  `stdsrvs.TriggerResponse.message`.
