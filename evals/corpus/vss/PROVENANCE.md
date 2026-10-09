Upstream: <https://github.com/COVESA/vehicle_signal_specification>

Revision: 923692329b46bd70cda88137030b662af2765770

Licence: MPL-2.0; all files under evals/corpus/vss/ are MPL-2.0.

Kind rule: A sensor or an attribute is a signal; an actuator is a signal plus a
command that sets it.

## Selected source and licence evidence

The pinned local upstream HEAD is `923692329b46bd70cda88137030b662af2765770`.
The complete translated include closure is `spec/Cabin/HVAC.vspec`,
`spec/Cabin/SingleHVACStation.vspec` and `spec/include/PowerOptimize.vspec`: 197
unique upstream physical lines. The selected HVAC branch declaration and
include-site metadata are `spec/Cabin/Cabin.vspec` lines 24–30.
`spec/VehicleSignalSpecification.vspec` supplies Vehicle and Cabin ancestor
metadata only; no ancestor interface, other ancestor signal or include is
ported.

All five selected source/context files carry SPDX-License-Identifier: MPL-2.0,
with no selected-file override. The pinned root licence is Mozilla Public
License Version 2.0 and is copied byte-exactly to [LICENSE](LICENSE). The
translated source is modified source under the same licence. The original
notices are retained in source comments and manifests. The source notices are:

```text
# Copyright (c) 2016 Contributors to COVESA
#
# This program and the accompanying materials are made available under the
# terms of the Mozilla Public License 2.0 which is available at
# https://www.mozilla.org/en-US/MPL/2.0/
#
# SPDX-License-Identifier: MPL-2.0
```

`PowerOptimize.vspec` carries the same notice with the copyright line:

```text
# Copyright (c) 2023 Contributors to COVESA
```

All source/context byte hashes and physical line counts were verified before
translation:

| Source                                  | Physical lines | SHA-256                                                            |
| --------------------------------------- | -------------: | ------------------------------------------------------------------ |
| `spec/Cabin/HVAC.vspec`                 |             47 | `4ab5efb91db5dd0f642591ce6d4fc5ae611d0c3fb13094c02b5c87a322c5b98d` |
| `spec/Cabin/SingleHVACStation.vspec`    |             31 | `4982d7730c3de1c1d21e1d07dba3812fb817d35446a056c6390e8b6dccb78d7c` |
| `spec/include/PowerOptimize.vspec`      |            119 | `8a173aeeaa77e38f4d4a869ce1bb5be03d1526ae57865022a9e9013767145bb1` |
| `spec/Cabin/Cabin.vspec`                |            199 | `bff513cd820de0ae98103f55bd95ab4e42201f94be7a2e32d1e01da05fb3b1f6` |
| `spec/VehicleSignalSpecification.vspec` |            223 | `2ed02e6f9738ac9f1c00bc20f62abaa54fce901d478b7fbc3c3256924fc520be` |

The upstream units and instances documents are protocol evidence only:
`docs-gen/content/rule_set/data_entry/data_units.md` and
`docs-gen/content/rule_set/instances.md`. Their prose is not translated.

Ancestor metadata establishes the qualified path only:

```text
Vehicle:
  type: branch
  description: High-level vehicle data.
Vehicle.Cabin:
  type: branch
  description: All in-cabin components, including doors.
```

## Frozen scope and budget

The approved row of §3.2 of the design
(`docs/archive/2026-10-04-design-lints-design.md`), verbatim:

| `vss` | `COVESA/vehicle_signal_specification` | The `Vehicle.Cabin.HVAC`
branch, and `Vehicle.Powertrain.TractionBattery` if the budget allows | ≤ 1,000
lines |

The controller approved HVAC-only freezing on 2026-10-04. The optional complete
`TractionBattery.vspec` plus `BatteryConditioning.vspec` closure adds 608
physical upstream lines before translation. With all eight station instances and
preserved descriptions/comments, the forecast risks exceeding 1,000 translated
lines. Neither battery closure, a partial battery selection nor a battery stub
is included. Expansion requires an explicit future subset decision and
independent review. A conservative forecast produces a smaller third corpus
while retaining a complete mandatory subtree.

This port has 759 physical source lines across its 14 `.ridl` files, including
comments and blank lines; there are no `.typl` or `.rsdl` files in this set.
With the supplied 4,086-line count for the other committed sets, the aggregate
is 4,845 lines, below 6,000. The third set is below one third of the aggregate.
Manifests, this provenance and the licence are outside the source line budget.

There are two authored branch declarations (HVAC and Station), ten authored leaf
declarations (seven at HVAC and three at Station), eight concrete station
instances, 14 represented branch paths, 31 concrete signal declarations and 30
mechanically generated actuator setter commands. The intermediate row and seat
paths come from documented instance expansion, not additional business
boundaries. No external data type dependency is required.

## Branch, package and output mapping

Each row maps to one interface in the listed `contract.ridl` and one sibling
`ridl.toml` whose package name equals the package declaration. The workspace
manifest lists exactly these 14 directories. Station and the four row interfaces
are empty hierarchy interfaces. Driver and Passenger interfaces contain the
three expanded station leaves; HVAC owns exactly the seven root leaves.

Package segments are lowercased because the existing manifest validator rejects
capitalized segments with MANI-006. This is the lexical case transformation
permitted by rule 1; the full original branch remains explicit below and in the
output directory name. There is no merged or split package boundary. Interface
and signal names remain exactly as upstream; no other lexical rename is needed.

| Upstream branch path                        | Package declaration and manifest name       | Interface   | Output directory (contains `contract.ridl` and `ridl.toml`) |
| ------------------------------------------- | ------------------------------------------- | ----------- | ----------------------------------------------------------- |
| `Vehicle.Cabin.HVAC`                        | `vehicle.cabin.hvac`                        | `HVAC`      | `vehicle.cabin.hvac`                                        |
| `Vehicle.Cabin.HVAC.Station`                | `vehicle.cabin.hvac.station`                | `Station`   | `vehicle.cabin.hvac.station`                                |
| `Vehicle.Cabin.HVAC.Station.Row1`           | `vehicle.cabin.hvac.station.row1`           | `Row1`      | `vehicle.cabin.hvac.station.row1`                           |
| `Vehicle.Cabin.HVAC.Station.Row2`           | `vehicle.cabin.hvac.station.row2`           | `Row2`      | `vehicle.cabin.hvac.station.row2`                           |
| `Vehicle.Cabin.HVAC.Station.Row3`           | `vehicle.cabin.hvac.station.row3`           | `Row3`      | `vehicle.cabin.hvac.station.row3`                           |
| `Vehicle.Cabin.HVAC.Station.Row4`           | `vehicle.cabin.hvac.station.row4`           | `Row4`      | `vehicle.cabin.hvac.station.row4`                           |
| `Vehicle.Cabin.HVAC.Station.Row1.Driver`    | `vehicle.cabin.hvac.station.row1.driver`    | `Driver`    | `vehicle.cabin.hvac.station.row1.driver`                    |
| `Vehicle.Cabin.HVAC.Station.Row1.Passenger` | `vehicle.cabin.hvac.station.row1.passenger` | `Passenger` | `vehicle.cabin.hvac.station.row1.passenger`                 |
| `Vehicle.Cabin.HVAC.Station.Row2.Driver`    | `vehicle.cabin.hvac.station.row2.driver`    | `Driver`    | `vehicle.cabin.hvac.station.row2.driver`                    |
| `Vehicle.Cabin.HVAC.Station.Row2.Passenger` | `vehicle.cabin.hvac.station.row2.passenger` | `Passenger` | `vehicle.cabin.hvac.station.row2.passenger`                 |
| `Vehicle.Cabin.HVAC.Station.Row3.Driver`    | `vehicle.cabin.hvac.station.row3.driver`    | `Driver`    | `vehicle.cabin.hvac.station.row3.driver`                    |
| `Vehicle.Cabin.HVAC.Station.Row3.Passenger` | `vehicle.cabin.hvac.station.row3.passenger` | `Passenger` | `vehicle.cabin.hvac.station.row3.passenger`                 |
| `Vehicle.Cabin.HVAC.Station.Row4.Driver`    | `vehicle.cabin.hvac.station.row4.driver`    | `Driver`    | `vehicle.cabin.hvac.station.row4.driver`                    |
| `Vehicle.Cabin.HVAC.Station.Row4.Passenger` | `vehicle.cabin.hvac.station.row4.passenger` | `Passenger` | `vehicle.cabin.hvac.station.row4.passenger`                 |

## Layout

Each translated VSS branch is one workspace member, in a directory named after
its package and outside every other member's tree, because a unit's tree holds
no second manifest: a `ridl.toml` inside another member's directory tree is an
error (MANI-013).

## Instance and source-comment preservation

Station's authored instance metadata is retained verbatim as comments at its
original source site in the HVAC contract, and in the Station contract with an
explicit source-site repetition label:

```text
Station:
  type: branch
  instances:
    - Row[1,4]
    - ["Driver","Passenger"]
  description: HVAC for single station in the vehicle
#include SingleHVACStation.vspec Station
```

The eight concrete mappings are:

| Authored instance metadata            | Concrete station path                       |
| ------------------------------------- | ------------------------------------------- |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row1.Driver`    |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row1.Passenger` |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row2.Driver`    |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row2.Passenger` |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row3.Driver`    |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row3.Passenger` |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row4.Driver`    |
| `Row[1,4]` × `["Driver","Passenger"]` | `Vehicle.Cabin.HVAC.Station.Row4.Passenger` |

All three station leaves occur at every path above. Their entire 31-line source
file is retained as a comment block in every concrete instance, labelled with
that instance path and the include site. The root HVAC contract preserves the
complete HVAC and PowerOptimize files and the selected Cabin branch block as
comments, including licence notices, descriptions, comment text and all include
directives. Only comment delimiters change: a leading `#` becomes `//`;
non-comment source metadata lines receive a `//` prefix. Whitespace following
the delimiter or inside metadata is retained. Original blank source lines are
represented as empty comments. The licence repetitions in hierarchy contracts
are explicitly labelled. Documentation prose used only as protocol evidence is
not copied into the translated source.

The illustrative non-HVAC names in PowerOptimize comments are retained as
comments only. They do not introduce signals, interfaces, dependencies or stubs.

## Complete concrete leaf and setter mapping

Each output signal below is qualified as `package.interface.signal`. A setter is
qualified as `package.interface.command`; it has exactly one parameter `value`
of the same named type as its signal and no explicit return, error model or
timing. Prefixing the unchanged signal name with `set` is necessary to
distinguish the actuator command from its observed signal. The setter parameter
name is mechanical translation scaffolding, not a renamed upstream field.
AmbientAirTemperature is the sole sensor and has no setter. There are no
selected attribute leaves.

| Concrete upstream leaf                                      | Authored source declaration                            | Output signal                                                         | Generated setter command                                                                          | Named payload type                                          |
| ----------------------------------------------------------- | ------------------------------------------------------ | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `Vehicle.Cabin.HVAC.IsRecirculationActive`                  | `spec/Cabin/HVAC.vspec:IsRecirculationActive`          | `vehicle.cabin.hvac.HVAC.IsRecirculationActive`                       | `vehicle.cabin.hvac.HVAC.setIsRecirculationActive(value : Boolean)`                               | `vehicle.cabin.hvac.Boolean`                                |
| `Vehicle.Cabin.HVAC.IsFrontDefrosterActive`                 | `spec/Cabin/HVAC.vspec:IsFrontDefrosterActive`         | `vehicle.cabin.hvac.HVAC.IsFrontDefrosterActive`                      | `vehicle.cabin.hvac.HVAC.setIsFrontDefrosterActive(value : Boolean)`                              | `vehicle.cabin.hvac.Boolean`                                |
| `Vehicle.Cabin.HVAC.IsRearDefrosterActive`                  | `spec/Cabin/HVAC.vspec:IsRearDefrosterActive`          | `vehicle.cabin.hvac.HVAC.IsRearDefrosterActive`                       | `vehicle.cabin.hvac.HVAC.setIsRearDefrosterActive(value : Boolean)`                               | `vehicle.cabin.hvac.Boolean`                                |
| `Vehicle.Cabin.HVAC.IsAirConditioningActive`                | `spec/Cabin/HVAC.vspec:IsAirConditioningActive`        | `vehicle.cabin.hvac.HVAC.IsAirConditioningActive`                     | `vehicle.cabin.hvac.HVAC.setIsAirConditioningActive(value : Boolean)`                             | `vehicle.cabin.hvac.Boolean`                                |
| `Vehicle.Cabin.HVAC.AmbientAirTemperature`                  | `spec/Cabin/HVAC.vspec:AmbientAirTemperature`          | `vehicle.cabin.hvac.HVAC.AmbientAirTemperature`                       | omitted (sensor)                                                                                  | `vehicle.cabin.hvac.AmbientAirTemperature`                  |
| `Vehicle.Cabin.HVAC.PowerOptimizeLevel`                     | `spec/include/PowerOptimize.vspec:PowerOptimizeLevel`  | `vehicle.cabin.hvac.HVAC.PowerOptimizeLevel`                          | `vehicle.cabin.hvac.HVAC.setPowerOptimizeLevel(value : PowerOptimizeLevel)`                       | `vehicle.cabin.hvac.PowerOptimizeLevel`                     |
| `Vehicle.Cabin.HVAC.IsAutoPowerOptimize`                    | `spec/include/PowerOptimize.vspec:IsAutoPowerOptimize` | `vehicle.cabin.hvac.HVAC.IsAutoPowerOptimize`                         | `vehicle.cabin.hvac.HVAC.setIsAutoPowerOptimize(value : Boolean)`                                 | `vehicle.cabin.hvac.Boolean`                                |
| `Vehicle.Cabin.HVAC.Station.Row1.Driver.FanSpeed`           | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row1.driver.Driver.FanSpeed`              | `vehicle.cabin.hvac.station.row1.driver.Driver.setFanSpeed(value : FanSpeed)`                     | `vehicle.cabin.hvac.station.row1.driver.FanSpeed`           |
| `Vehicle.Cabin.HVAC.Station.Row1.Driver.Temperature`        | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row1.driver.Driver.Temperature`           | `vehicle.cabin.hvac.station.row1.driver.Driver.setTemperature(value : Temperature)`               | `vehicle.cabin.hvac.station.row1.driver.Temperature`        |
| `Vehicle.Cabin.HVAC.Station.Row1.Driver.AirDistribution`    | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row1.driver.Driver.AirDistribution`       | `vehicle.cabin.hvac.station.row1.driver.Driver.setAirDistribution(value : AirDistribution)`       | `vehicle.cabin.hvac.station.row1.driver.AirDistribution`    |
| `Vehicle.Cabin.HVAC.Station.Row1.Passenger.FanSpeed`        | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row1.passenger.Passenger.FanSpeed`        | `vehicle.cabin.hvac.station.row1.passenger.Passenger.setFanSpeed(value : FanSpeed)`               | `vehicle.cabin.hvac.station.row1.passenger.FanSpeed`        |
| `Vehicle.Cabin.HVAC.Station.Row1.Passenger.Temperature`     | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row1.passenger.Passenger.Temperature`     | `vehicle.cabin.hvac.station.row1.passenger.Passenger.setTemperature(value : Temperature)`         | `vehicle.cabin.hvac.station.row1.passenger.Temperature`     |
| `Vehicle.Cabin.HVAC.Station.Row1.Passenger.AirDistribution` | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row1.passenger.Passenger.AirDistribution` | `vehicle.cabin.hvac.station.row1.passenger.Passenger.setAirDistribution(value : AirDistribution)` | `vehicle.cabin.hvac.station.row1.passenger.AirDistribution` |
| `Vehicle.Cabin.HVAC.Station.Row2.Driver.FanSpeed`           | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row2.driver.Driver.FanSpeed`              | `vehicle.cabin.hvac.station.row2.driver.Driver.setFanSpeed(value : FanSpeed)`                     | `vehicle.cabin.hvac.station.row2.driver.FanSpeed`           |
| `Vehicle.Cabin.HVAC.Station.Row2.Driver.Temperature`        | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row2.driver.Driver.Temperature`           | `vehicle.cabin.hvac.station.row2.driver.Driver.setTemperature(value : Temperature)`               | `vehicle.cabin.hvac.station.row2.driver.Temperature`        |
| `Vehicle.Cabin.HVAC.Station.Row2.Driver.AirDistribution`    | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row2.driver.Driver.AirDistribution`       | `vehicle.cabin.hvac.station.row2.driver.Driver.setAirDistribution(value : AirDistribution)`       | `vehicle.cabin.hvac.station.row2.driver.AirDistribution`    |
| `Vehicle.Cabin.HVAC.Station.Row2.Passenger.FanSpeed`        | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row2.passenger.Passenger.FanSpeed`        | `vehicle.cabin.hvac.station.row2.passenger.Passenger.setFanSpeed(value : FanSpeed)`               | `vehicle.cabin.hvac.station.row2.passenger.FanSpeed`        |
| `Vehicle.Cabin.HVAC.Station.Row2.Passenger.Temperature`     | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row2.passenger.Passenger.Temperature`     | `vehicle.cabin.hvac.station.row2.passenger.Passenger.setTemperature(value : Temperature)`         | `vehicle.cabin.hvac.station.row2.passenger.Temperature`     |
| `Vehicle.Cabin.HVAC.Station.Row2.Passenger.AirDistribution` | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row2.passenger.Passenger.AirDistribution` | `vehicle.cabin.hvac.station.row2.passenger.Passenger.setAirDistribution(value : AirDistribution)` | `vehicle.cabin.hvac.station.row2.passenger.AirDistribution` |
| `Vehicle.Cabin.HVAC.Station.Row3.Driver.FanSpeed`           | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row3.driver.Driver.FanSpeed`              | `vehicle.cabin.hvac.station.row3.driver.Driver.setFanSpeed(value : FanSpeed)`                     | `vehicle.cabin.hvac.station.row3.driver.FanSpeed`           |
| `Vehicle.Cabin.HVAC.Station.Row3.Driver.Temperature`        | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row3.driver.Driver.Temperature`           | `vehicle.cabin.hvac.station.row3.driver.Driver.setTemperature(value : Temperature)`               | `vehicle.cabin.hvac.station.row3.driver.Temperature`        |
| `Vehicle.Cabin.HVAC.Station.Row3.Driver.AirDistribution`    | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row3.driver.Driver.AirDistribution`       | `vehicle.cabin.hvac.station.row3.driver.Driver.setAirDistribution(value : AirDistribution)`       | `vehicle.cabin.hvac.station.row3.driver.AirDistribution`    |
| `Vehicle.Cabin.HVAC.Station.Row3.Passenger.FanSpeed`        | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row3.passenger.Passenger.FanSpeed`        | `vehicle.cabin.hvac.station.row3.passenger.Passenger.setFanSpeed(value : FanSpeed)`               | `vehicle.cabin.hvac.station.row3.passenger.FanSpeed`        |
| `Vehicle.Cabin.HVAC.Station.Row3.Passenger.Temperature`     | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row3.passenger.Passenger.Temperature`     | `vehicle.cabin.hvac.station.row3.passenger.Passenger.setTemperature(value : Temperature)`         | `vehicle.cabin.hvac.station.row3.passenger.Temperature`     |
| `Vehicle.Cabin.HVAC.Station.Row3.Passenger.AirDistribution` | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row3.passenger.Passenger.AirDistribution` | `vehicle.cabin.hvac.station.row3.passenger.Passenger.setAirDistribution(value : AirDistribution)` | `vehicle.cabin.hvac.station.row3.passenger.AirDistribution` |
| `Vehicle.Cabin.HVAC.Station.Row4.Driver.FanSpeed`           | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row4.driver.Driver.FanSpeed`              | `vehicle.cabin.hvac.station.row4.driver.Driver.setFanSpeed(value : FanSpeed)`                     | `vehicle.cabin.hvac.station.row4.driver.FanSpeed`           |
| `Vehicle.Cabin.HVAC.Station.Row4.Driver.Temperature`        | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row4.driver.Driver.Temperature`           | `vehicle.cabin.hvac.station.row4.driver.Driver.setTemperature(value : Temperature)`               | `vehicle.cabin.hvac.station.row4.driver.Temperature`        |
| `Vehicle.Cabin.HVAC.Station.Row4.Driver.AirDistribution`    | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row4.driver.Driver.AirDistribution`       | `vehicle.cabin.hvac.station.row4.driver.Driver.setAirDistribution(value : AirDistribution)`       | `vehicle.cabin.hvac.station.row4.driver.AirDistribution`    |
| `Vehicle.Cabin.HVAC.Station.Row4.Passenger.FanSpeed`        | `spec/Cabin/SingleHVACStation.vspec:FanSpeed`          | `vehicle.cabin.hvac.station.row4.passenger.Passenger.FanSpeed`        | `vehicle.cabin.hvac.station.row4.passenger.Passenger.setFanSpeed(value : FanSpeed)`               | `vehicle.cabin.hvac.station.row4.passenger.FanSpeed`        |
| `Vehicle.Cabin.HVAC.Station.Row4.Passenger.Temperature`     | `spec/Cabin/SingleHVACStation.vspec:Temperature`       | `vehicle.cabin.hvac.station.row4.passenger.Passenger.Temperature`     | `vehicle.cabin.hvac.station.row4.passenger.Passenger.setTemperature(value : Temperature)`         | `vehicle.cabin.hvac.station.row4.passenger.Temperature`     |
| `Vehicle.Cabin.HVAC.Station.Row4.Passenger.AirDistribution` | `spec/Cabin/SingleHVACStation.vspec:AirDistribution`   | `vehicle.cabin.hvac.station.row4.passenger.Passenger.AirDistribution` | `vehicle.cabin.hvac.station.row4.passenger.Passenger.setAirDistribution(value : AirDistribution)` | `vehicle.cabin.hvac.station.row4.passenger.AirDistribution` |

## Complete vocabulary mapping and scalar limitations

Every vocabulary declaration is in its branch's contract file. There are 19
named scalar helper types and eight enums (27 vocabulary declarations), with all
24 enum member declarations listed below. These helpers carry payload
constraints; they add no interactions or upstream grouping boundary.

| Authored source/domain                                                                                                                       | Output qualified type or enum                               | Representation                                                                                                                                                                                                         |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Five root boolean leaves                                                                                                                     | `vehicle.cabin.hvac.Boolean`                                | `boolean`; shared only within the root branch                                                                                                                                                                          |
| HVAC.vspec AmbientAirTemperature, float, Celsius                                                                                             | `vehicle.cabin.hvac.AmbientAirTemperature`                  | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| PowerOptimize.vspec PowerOptimizeLevel, uint8, min 0, max 10                                                                                 | `vehicle.cabin.hvac.PowerOptimizeLevel`                     | `integer [0..10]`; inferred uint8                                                                                                                                                                                      |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Driver`                       | `vehicle.cabin.hvac.station.row1.driver.FanSpeed`           | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Driver`                                    | `vehicle.cabin.hvac.station.row1.driver.Temperature`        | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Driver`    | `vehicle.cabin.hvac.station.row1.driver.AirDistribution`    | enum: `vehicle.cabin.hvac.station.row1.driver.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row1.driver.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row1.driver.AirDistribution.DOWN = 2`          |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Passenger`                    | `vehicle.cabin.hvac.station.row1.passenger.FanSpeed`        | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Passenger`                                 | `vehicle.cabin.hvac.station.row1.passenger.Temperature`     | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row1.Passenger` | `vehicle.cabin.hvac.station.row1.passenger.AirDistribution` | enum: `vehicle.cabin.hvac.station.row1.passenger.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row1.passenger.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row1.passenger.AirDistribution.DOWN = 2` |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Driver`                       | `vehicle.cabin.hvac.station.row2.driver.FanSpeed`           | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Driver`                                    | `vehicle.cabin.hvac.station.row2.driver.Temperature`        | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Driver`    | `vehicle.cabin.hvac.station.row2.driver.AirDistribution`    | enum: `vehicle.cabin.hvac.station.row2.driver.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row2.driver.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row2.driver.AirDistribution.DOWN = 2`          |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Passenger`                    | `vehicle.cabin.hvac.station.row2.passenger.FanSpeed`        | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Passenger`                                 | `vehicle.cabin.hvac.station.row2.passenger.Temperature`     | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row2.Passenger` | `vehicle.cabin.hvac.station.row2.passenger.AirDistribution` | enum: `vehicle.cabin.hvac.station.row2.passenger.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row2.passenger.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row2.passenger.AirDistribution.DOWN = 2` |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Driver`                       | `vehicle.cabin.hvac.station.row3.driver.FanSpeed`           | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Driver`                                    | `vehicle.cabin.hvac.station.row3.driver.Temperature`        | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Driver`    | `vehicle.cabin.hvac.station.row3.driver.AirDistribution`    | enum: `vehicle.cabin.hvac.station.row3.driver.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row3.driver.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row3.driver.AirDistribution.DOWN = 2`          |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Passenger`                    | `vehicle.cabin.hvac.station.row3.passenger.FanSpeed`        | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Passenger`                                 | `vehicle.cabin.hvac.station.row3.passenger.Temperature`     | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row3.Passenger` | `vehicle.cabin.hvac.station.row3.passenger.AirDistribution` | enum: `vehicle.cabin.hvac.station.row3.passenger.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row3.passenger.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row3.passenger.AirDistribution.DOWN = 2` |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Driver`                       | `vehicle.cabin.hvac.station.row4.driver.FanSpeed`           | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Driver`                                    | `vehicle.cabin.hvac.station.row4.driver.Temperature`        | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Driver`    | `vehicle.cabin.hvac.station.row4.driver.AirDistribution`    | enum: `vehicle.cabin.hvac.station.row4.driver.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row4.driver.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row4.driver.AirDistribution.DOWN = 2`          |
| SingleHVACStation.vspec FanSpeed, uint8, min 0, max 100, percent; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Passenger`                    | `vehicle.cabin.hvac.station.row4.passenger.FanSpeed`        | `integer [0..100]`; inferred uint8; percent metadata retained in comments                                                                                                                                              |
| SingleHVACStation.vspec Temperature, float, Celsius; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Passenger`                                 | `vehicle.cabin.hvac.station.row4.passenger.Temperature`     | `Cel`, no range or step; inferred float64                                                                                                                                                                              |
| SingleHVACStation.vspec AirDistribution, string, allowed `['UP', 'MIDDLE', 'DOWN']`; expanded at `Vehicle.Cabin.HVAC.Station.Row4.Passenger` | `vehicle.cabin.hvac.station.row4.passenger.AirDistribution` | enum: `vehicle.cabin.hvac.station.row4.passenger.AirDistribution.UP = 0`, `vehicle.cabin.hvac.station.row4.passenger.AirDistribution.MIDDLE = 1`, `vehicle.cabin.hvac.station.row4.passenger.AirDistribution.DOWN = 2` |

`Celsius` maps to UCUM `Cel`; `percent` maps to UCUM `%`. Both are supported by
the existing curated atom table in `crates/ridl-sem/src/ucum.rs`. There is no
unsupported unit in the selected closure. PowerOptimizeLevel and the boolean
leaves have no stated unit, and none is added.

TYPL unit types currently have float backing even when their range uses integer
literals. Consequently FanSpeed cannot simultaneously have an integer scalar and
a machine-readable unit. Its integer domain and inferred uint8 are retained; its
exact `unit: percent` metadata and UCUM `%` mapping are retained in comments.
The semantic unit is omitted from that named integer type. Using a percent unit
type would change the upstream integer domain, so that alternative is not used.
This is an explicit representation limitation and deviation from rule 2, not an
unsupported UCUM atom.

Explicit width names cannot be written in TYPL. PowerOptimizeLevel and FanSpeed
infer uint8 from their inclusive ranges, with the language's integer backing.
Upstream float values have no range or quantization step; `Cel` retains their
float scalar and unit but infers float64. The VSS float32 width cannot be forced
without adding constraints the source does not state. No such constraints are
added. Boolean remains `boolean`.

String-backed enums are unsupported. AirDistribution becomes an integer-backed
enum retaining the exact UP, MIDDLE and DOWN domain in source order. Codes 0, 1
and 2 are required explicit RIDL enum representation codes; they are not VSS
string values, ordering promises or upstream defaults. This changes the wire
representation from string to integer; a string adapter would need the listed
name/code correspondence. No adapter is included.

The selected closure has no authored defaults. There is no explicit default or
initializer in this port. RIDL may derive its own initial scalar/enum values;
these compiler-derived values are not upstream defaults. All timing annotations
are omitted because upstream states none. The compiler applies its own default
signal timing and reports missing timing; that default is not an upstream
cadence. Setter commands omit response bounds and retain the corresponding
warning. No external return value, error model or interaction is invented. The
manifest version `0.1.0` is local packaging metadata, not an upstream revision,
value default or protocol version.

## Deviations from rules 1–6

1. Lowercase package segments are required by the existing MANI-006 validator.
   Every transformed branch name is listed in the package mapping. Source
   interface, signal, enum value and grouping names are retained. The `set`
   prefix and named payload helpers are the mechanical scaffolding listed above.
   AirDistribution's enum representation and float width limitation are listed
   explicitly. Original comments and descriptions are retained verbatim.
2. Celsius is written as `Cel`. FanSpeed's stated percent maps to `%` but is
   retained as comment metadata only to preserve its integer domain, because
   integer-backed unit types are unavailable. No unstated unit is written.
3. No deviation: one interface per represented VSS branch, including documented
   instance hierarchy nodes. No ancestor interface or invented boundary.
4. No undecided kind case and no deviation: one signal for the sensor, and a
   signal plus setter for every actuator. No selected attribute exists.
5. No boundary deviation: one package per branch. Package case changes only as
   required by the validator; the package declaration and member manifest agree.
6. No deviation: all out-of-subset declarations are omitted. No external type
   dependency is required. The battery closure is omitted completely.

## Ordinary verification

`target/debug/ridl check evals/corpus/vss --format json` exits 0 with no Error
diagnostic. The 70 retained warnings are nine TYPL-102 (unbounded float), 31
RIDL-100 (missing signal timing) and 30 RIDL-112 (missing response bound). No
warnings are suppressed or repaired. No candidate design check, calibration or
dump pass is run. The original capitalized package names were rejected with
MANI-006 before the documented lexical transformation.
