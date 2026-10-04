Upstream: <https://github.com/mavlink/mavlink>

Revision: `527637cb39cb4e52293bea40441810b53f23ff25`

Licence: MIT for the selected message-definition XML; full upstream COPYING is
retained as LICENSE; LGPLv3 generator code is excluded.

Kind rule: A message documented as streamed telemetry is a signal; a request and
its acknowledgement follow the microservice's documented protocol (command or
query); any other message is an event. Cases not decided by this rule are listed
individually below with protocol evidence.

## Source selection and licence evidence

Only `message_definitions/v1.0/minimal.xml`, `standard.xml` and `common.xml` at
the pinned definitions revision are translated. They contain no conflicting
licence notice. Their include order is `common` → `standard` → `minimal`. All
other XML inputs, generator code and generated headers are excluded. The full
upstream COPYING, including its MIT exception and LGPL generator licence, is
retained byte-exactly as LICENSE for traceability; it does not make the selected
XML LGPL.

The official guide at revision `7412790c2a38162a3f31fa1c2fdac9263d65a1d3`
explicitly distinguishes MIT XML/C headers from the LGPLv3 generator in its
License section, lines 98–105:
<https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/index.md#license>.
The live licence reference is <https://mavlink.io/en/#license>. Documentation
has a separate CC BY 4.0 licence and is cited as protocol evidence only; no
guide prose is translated or copied.

Protocol evidence, all at that guide revision:

- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/heartbeat.md>
- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/mission.md>
- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/command.md>
- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/services/parameter.md>
- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/guide/general_telemetry.md>
- <https://github.com/mavlink/mavlink-devguide/blob/7412790c2a38162a3f31fa1c2fdac9263d65a1d3/en/guide/message_rates.md>

## Frozen declaration inventory

27 messages and 16 enums, 43 upstream declarations. Helpers and five interfaces
are counted separately. All message fields and extension fields are included;
all selected enums are complete except the five-entry MavCmd subset. The
selected raw XML declaration blocks occupy 1,130 physical lines before
translation.

| Kind    | XML input                             | Upstream name                  | Output file                                 | RIDL declaration                  |
| ------- | ------------------------------------- | ------------------------------ | ------------------------------------------- | --------------------------------- |
| message | message_definitions/v1.0/minimal.xml  | HEARTBEAT                      | evals/corpus/mavlink/minimal/messages.typl  | minimal.Heartbeat                 |
| message | message_definitions/v1.0/common.xml   | SYS_STATUS                     | evals/corpus/mavlink/common/messages.typl   | common.SysStatus                  |
| message | message_definitions/v1.0/common.xml   | MISSION_ITEM                   | evals/corpus/mavlink/common/messages.typl   | common.MissionItem                |
| message | message_definitions/v1.0/common.xml   | MISSION_REQUEST                | evals/corpus/mavlink/common/messages.typl   | common.MissionRequest             |
| message | message_definitions/v1.0/common.xml   | MISSION_SET_CURRENT            | evals/corpus/mavlink/common/messages.typl   | common.MissionSetCurrent          |
| message | message_definitions/v1.0/common.xml   | MISSION_CURRENT                | evals/corpus/mavlink/common/messages.typl   | common.MissionCurrent             |
| message | message_definitions/v1.0/common.xml   | MISSION_REQUEST_LIST           | evals/corpus/mavlink/common/messages.typl   | common.MissionRequestList         |
| message | message_definitions/v1.0/common.xml   | MISSION_COUNT                  | evals/corpus/mavlink/common/messages.typl   | common.MissionCount               |
| message | message_definitions/v1.0/common.xml   | MISSION_CLEAR_ALL              | evals/corpus/mavlink/common/messages.typl   | common.MissionClearAll            |
| message | message_definitions/v1.0/common.xml   | MISSION_ITEM_REACHED           | evals/corpus/mavlink/common/messages.typl   | common.MissionItemReached         |
| message | message_definitions/v1.0/common.xml   | MISSION_ACK                    | evals/corpus/mavlink/common/messages.typl   | common.MissionAck                 |
| message | message_definitions/v1.0/common.xml   | MISSION_REQUEST_INT            | evals/corpus/mavlink/common/messages.typl   | common.MissionRequestInt          |
| message | message_definitions/v1.0/common.xml   | MISSION_ITEM_INT               | evals/corpus/mavlink/common/messages.typl   | common.MissionItemInt             |
| message | message_definitions/v1.0/common.xml   | COMMAND_INT                    | evals/corpus/mavlink/common/messages.typl   | common.CommandInt                 |
| message | message_definitions/v1.0/common.xml   | COMMAND_LONG                   | evals/corpus/mavlink/common/messages.typl   | common.CommandLong                |
| message | message_definitions/v1.0/common.xml   | COMMAND_ACK                    | evals/corpus/mavlink/common/messages.typl   | common.CommandAck                 |
| message | message_definitions/v1.0/common.xml   | COMMAND_CANCEL                 | evals/corpus/mavlink/common/messages.typl   | common.CommandCancel              |
| message | message_definitions/v1.0/common.xml   | PARAM_REQUEST_READ             | evals/corpus/mavlink/common/messages.typl   | common.ParamRequestRead           |
| message | message_definitions/v1.0/common.xml   | PARAM_REQUEST_LIST             | evals/corpus/mavlink/common/messages.typl   | common.ParamRequestList           |
| message | message_definitions/v1.0/common.xml   | PARAM_VALUE                    | evals/corpus/mavlink/common/messages.typl   | common.ParamValue                 |
| message | message_definitions/v1.0/common.xml   | PARAM_SET                      | evals/corpus/mavlink/common/messages.typl   | common.ParamSet                   |
| message | message_definitions/v1.0/common.xml   | PARAM_ERROR                    | evals/corpus/mavlink/common/messages.typl   | common.ParamError                 |
| message | message_definitions/v1.0/common.xml   | GPS_RAW_INT                    | evals/corpus/mavlink/common/messages.typl   | common.GpsRawInt                  |
| message | message_definitions/v1.0/common.xml   | ATTITUDE                       | evals/corpus/mavlink/common/messages.typl   | common.Attitude                   |
| message | message_definitions/v1.0/common.xml   | LOCAL_POSITION_NED             | evals/corpus/mavlink/common/messages.typl   | common.LocalPositionNed           |
| message | message_definitions/v1.0/standard.xml | GLOBAL_POSITION_INT            | evals/corpus/mavlink/standard/messages.typl | standard.GlobalPositionInt        |
| message | message_definitions/v1.0/common.xml   | VFR_HUD                        | evals/corpus/mavlink/common/messages.typl   | common.VfrHud                     |
| enum    | message_definitions/v1.0/common.xml   | GPS_FIX_TYPE                   | evals/corpus/mavlink/common/enums.typl      | common.GpsFixType                 |
| enum    | message_definitions/v1.0/minimal.xml  | MAV_AUTOPILOT                  | evals/corpus/mavlink/minimal/enums.typl     | minimal.MavAutopilot              |
| enum    | message_definitions/v1.0/standard.xml | MAV_BOOL                       | evals/corpus/mavlink/standard/enums.typl    | standard.MavBool                  |
| enum    | message_definitions/v1.0/common.xml   | MAV_CMD                        | evals/corpus/mavlink/common/enums.typl      | common.MavCmd                     |
| enum    | message_definitions/v1.0/common.xml   | MAV_FRAME                      | evals/corpus/mavlink/common/enums.typl      | common.MavFrame                   |
| enum    | message_definitions/v1.0/common.xml   | MAV_MISSION_RESULT             | evals/corpus/mavlink/common/enums.typl      | common.MavMissionResult           |
| enum    | message_definitions/v1.0/common.xml   | MAV_MISSION_TYPE               | evals/corpus/mavlink/common/enums.typl      | common.MavMissionType             |
| enum    | message_definitions/v1.0/minimal.xml  | MAV_MODE_FLAG                  | evals/corpus/mavlink/minimal/enums.typl     | minimal.MavModeFlag               |
| enum    | message_definitions/v1.0/common.xml   | MAV_PARAM_ERROR                | evals/corpus/mavlink/common/enums.typl      | common.MavParamError              |
| enum    | message_definitions/v1.0/common.xml   | MAV_PARAM_TYPE                 | evals/corpus/mavlink/common/enums.typl      | common.MavParamType               |
| enum    | message_definitions/v1.0/common.xml   | MAV_RESULT                     | evals/corpus/mavlink/common/enums.typl      | common.MavResult                  |
| enum    | message_definitions/v1.0/minimal.xml  | MAV_STATE                      | evals/corpus/mavlink/minimal/enums.typl     | minimal.MavState                  |
| enum    | message_definitions/v1.0/common.xml   | MAV_SYS_STATUS_SENSOR          | evals/corpus/mavlink/common/enums.typl      | common.MavSysStatusSensor         |
| enum    | message_definitions/v1.0/common.xml   | MAV_SYS_STATUS_SENSOR_EXTENDED | evals/corpus/mavlink/common/enums.typl      | common.MavSysStatusSensorExtended |
| enum    | message_definitions/v1.0/minimal.xml  | MAV_TYPE                       | evals/corpus/mavlink/minimal/enums.typl     | minimal.MavType                   |
| enum    | message_definitions/v1.0/common.xml   | MISSION_STATE                  | evals/corpus/mavlink/common/enums.typl      | common.MissionState               |

## Selected MAV_CMD entries

The five entries retain their original XML order in common.MavCmd, not the order
of the prose selection list. This is not the complete MAVLink command
vocabulary.

| Source entry                   | Value | Output entry                                 |
| ------------------------------ | ----- | -------------------------------------------- |
| MAV_CMD_NAV_WAYPOINT           | 16    | common.MavCmd.MAV_CMD_NAV_WAYPOINT           |
| MAV_CMD_DO_SET_MISSION_CURRENT | 224   | common.MavCmd.MAV_CMD_DO_SET_MISSION_CURRENT |
| MAV_CMD_COMPONENT_ARM_DISARM   | 400   | common.MavCmd.MAV_CMD_COMPONENT_ARM_DISARM   |
| MAV_CMD_SET_MESSAGE_INTERVAL   | 511   | common.MavCmd.MAV_CMD_SET_MESSAGE_INTERVAL   |
| MAV_CMD_REQUEST_MESSAGE        | 512   | common.MavCmd.MAV_CMD_REQUEST_MESSAGE        |

The following 165 MAV_CMD entries are outside the frozen subset and are omitted,
not stubbed:

`MAV_CMD_NAV_LOITER_UNLIM`, `MAV_CMD_NAV_LOITER_TURNS`,
`MAV_CMD_NAV_LOITER_TIME`, `MAV_CMD_NAV_RETURN_TO_LAUNCH`, `MAV_CMD_NAV_LAND`,
`MAV_CMD_NAV_TAKEOFF`, `MAV_CMD_NAV_LAND_LOCAL`, `MAV_CMD_NAV_TAKEOFF_LOCAL`,
`MAV_CMD_NAV_CONTINUE_AND_CHANGE_ALT`, `MAV_CMD_NAV_LOITER_TO_ALT`,
`MAV_CMD_DO_FOLLOW`, `MAV_CMD_DO_FOLLOW_REPOSITION`, `MAV_CMD_DO_ORBIT`,
`MAV_CMD_DO_FIGURE_EIGHT`, `MAV_CMD_NAV_ARC_WAYPOINT`, `MAV_CMD_NAV_ROI`,
`MAV_CMD_NAV_PATHPLANNING`, `MAV_CMD_NAV_SPLINE_WAYPOINT`,
`MAV_CMD_NAV_VTOL_TAKEOFF`, `MAV_CMD_NAV_VTOL_LAND`,
`MAV_CMD_NAV_GUIDED_ENABLE`, `MAV_CMD_NAV_DELAY`, `MAV_CMD_NAV_PAYLOAD_PLACE`,
`MAV_CMD_NAV_LAST`, `MAV_CMD_CONDITION_DELAY`, `MAV_CMD_CONDITION_CHANGE_ALT`,
`MAV_CMD_CONDITION_DISTANCE`, `MAV_CMD_CONDITION_YAW`, `MAV_CMD_CONDITION_LAST`,
`MAV_CMD_DO_SET_MODE`, `MAV_CMD_DO_JUMP`, `MAV_CMD_DO_CHANGE_SPEED`,
`MAV_CMD_DO_SET_HOME`, `MAV_CMD_DO_SET_PARAMETER`, `MAV_CMD_DO_SET_RELAY`,
`MAV_CMD_DO_REPEAT_RELAY`, `MAV_CMD_DO_SET_SERVO`, `MAV_CMD_DO_REPEAT_SERVO`,
`MAV_CMD_DO_FLIGHTTERMINATION`, `MAV_CMD_DO_CHANGE_ALTITUDE`,
`MAV_CMD_DO_SET_ACTUATOR`, `MAV_CMD_DO_RETURN_PATH_START`,
`MAV_CMD_DO_LAND_START`, `MAV_CMD_DO_RALLY_LAND`, `MAV_CMD_DO_GO_AROUND`,
`MAV_CMD_DO_REPOSITION`, `MAV_CMD_DO_PAUSE_CONTINUE`, `MAV_CMD_DO_SET_REVERSE`,
`MAV_CMD_DO_SET_ROI_LOCATION`, `MAV_CMD_DO_SET_ROI_WPNEXT_OFFSET`,
`MAV_CMD_DO_SET_ROI_NONE`, `MAV_CMD_DO_SET_ROI_SYSID`,
`MAV_CMD_DO_CONTROL_VIDEO`, `MAV_CMD_DO_SET_ROI`,
`MAV_CMD_DO_DIGICAM_CONFIGURE`, `MAV_CMD_DO_DIGICAM_CONTROL`,
`MAV_CMD_DO_MOUNT_CONFIGURE`, `MAV_CMD_DO_MOUNT_CONTROL`,
`MAV_CMD_DO_SET_CAM_TRIGG_DIST`, `MAV_CMD_DO_FENCE_ENABLE`,
`MAV_CMD_DO_PARACHUTE`, `MAV_CMD_DO_MOTOR_TEST`, `MAV_CMD_DO_INVERTED_FLIGHT`,
`MAV_CMD_DO_GRIPPER`, `MAV_CMD_DO_AUTOTUNE_ENABLE`, `MAV_CMD_NAV_SET_YAW_SPEED`,
`MAV_CMD_DO_SET_CAM_TRIGG_INTERVAL`, `MAV_CMD_DO_MOUNT_CONTROL_QUAT`,
`MAV_CMD_DO_GUIDED_MASTER`, `MAV_CMD_DO_GUIDED_LIMITS`,
`MAV_CMD_DO_ENGINE_CONTROL`, `MAV_CMD_DO_LAST`, `MAV_CMD_PREFLIGHT_CALIBRATION`,
`MAV_CMD_PREFLIGHT_SET_SENSOR_OFFSETS`, `MAV_CMD_PREFLIGHT_UAVCAN`,
`MAV_CMD_PREFLIGHT_STORAGE`, `MAV_CMD_PREFLIGHT_REBOOT_SHUTDOWN`,
`MAV_CMD_OVERRIDE_GOTO`, `MAV_CMD_OBLIQUE_SURVEY`,
`MAV_CMD_DO_SET_STANDARD_MODE`, `MAV_CMD_MISSION_START`,
`MAV_CMD_ACTUATOR_TEST`, `MAV_CMD_CONFIGURE_ACTUATOR`,
`MAV_CMD_RUN_PREARM_CHECKS`, `MAV_CMD_ILLUMINATOR_ON_OFF`,
`MAV_CMD_DO_ILLUMINATOR_CONFIGURE`, `MAV_CMD_GET_HOME_POSITION`,
`MAV_CMD_INJECT_FAILURE`, `MAV_CMD_START_RX_PAIR`,
`MAV_CMD_GET_MESSAGE_INTERVAL`, `MAV_CMD_REQUEST_PROTOCOL_VERSION`,
`MAV_CMD_REQUEST_AUTOPILOT_CAPABILITIES`, `MAV_CMD_REQUEST_CAMERA_INFORMATION`,
`MAV_CMD_REQUEST_CAMERA_SETTINGS`, `MAV_CMD_REQUEST_STORAGE_INFORMATION`,
`MAV_CMD_STORAGE_FORMAT`, `MAV_CMD_REQUEST_CAMERA_CAPTURE_STATUS`,
`MAV_CMD_REQUEST_FLIGHT_INFORMATION`, `MAV_CMD_RESET_CAMERA_SETTINGS`,
`MAV_CMD_SET_CAMERA_MODE`, `MAV_CMD_SET_CAMERA_ZOOM`,
`MAV_CMD_SET_CAMERA_FOCUS`, `MAV_CMD_SET_STORAGE_USAGE`,
`MAV_CMD_SET_CAMERA_SOURCE`, `MAV_CMD_JUMP_TAG`, `MAV_CMD_DO_JUMP_TAG`,
`MAV_CMD_DO_SET_GLOBAL_ORIGIN`, `MAV_CMD_DO_GIMBAL_MANAGER_PITCHYAW`,
`MAV_CMD_DO_GIMBAL_MANAGER_CONFIGURE`, `MAV_CMD_IMAGE_START_CAPTURE`,
`MAV_CMD_IMAGE_STOP_CAPTURE`, `MAV_CMD_REQUEST_CAMERA_IMAGE_CAPTURE`,
`MAV_CMD_DO_TRIGGER_CONTROL`, `MAV_CMD_CAMERA_TRACK_POINT`,
`MAV_CMD_CAMERA_TRACK_RECTANGLE`, `MAV_CMD_CAMERA_STOP_TRACKING`,
`MAV_CMD_VIDEO_START_CAPTURE`, `MAV_CMD_VIDEO_STOP_CAPTURE`,
`MAV_CMD_VIDEO_START_STREAMING`, `MAV_CMD_VIDEO_STOP_STREAMING`,
`MAV_CMD_REQUEST_VIDEO_STREAM_INFORMATION`,
`MAV_CMD_REQUEST_VIDEO_STREAM_STATUS`, `MAV_CMD_LOGGING_START`,
`MAV_CMD_LOGGING_STOP`, `MAV_CMD_AIRFRAME_CONFIGURATION`,
`MAV_CMD_CONTROL_HIGH_LATENCY`, `MAV_CMD_PANORAMA_CREATE`,
`MAV_CMD_DO_VTOL_TRANSITION`, `MAV_CMD_ARM_AUTHORIZATION_REQUEST`,
`MAV_CMD_SET_GUIDED_SUBMODE_STANDARD`, `MAV_CMD_SET_GUIDED_SUBMODE_CIRCLE`,
`MAV_CMD_CONDITION_GATE`, `MAV_CMD_NAV_FENCE_RETURN_POINT`,
`MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`,
`MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION`,
`MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION`, `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION`,
`MAV_CMD_NAV_RALLY_POINT`, `MAV_CMD_UAVCAN_GET_NODE_INFO`,
`MAV_CMD_DO_SET_SAFETY_SWITCH_STATE`, `MAV_CMD_DO_ADSB_OUT_IDENT`,
`MAV_CMD_PAYLOAD_PREPARE_DEPLOY`, `MAV_CMD_PAYLOAD_CONTROL_DEPLOY`,
`MAV_CMD_FIXED_MAG_CAL_YAW`, `MAV_CMD_DO_WINCH`, `MAV_CMD_GUIDED_CHANGE_SPEED`,
`MAV_CMD_GUIDED_CHANGE_ALTITUDE`, `MAV_CMD_GUIDED_CHANGE_HEADING`,
`MAV_CMD_EXTERNAL_POSITION_ESTIMATE`, `MAV_CMD_WAYPOINT_USER_1`,
`MAV_CMD_WAYPOINT_USER_2`, `MAV_CMD_WAYPOINT_USER_3`, `MAV_CMD_WAYPOINT_USER_4`,
`MAV_CMD_WAYPOINT_USER_5`, `MAV_CMD_SPATIAL_USER_1`, `MAV_CMD_SPATIAL_USER_2`,
`MAV_CMD_SPATIAL_USER_3`, `MAV_CMD_SPATIAL_USER_4`, `MAV_CMD_SPATIAL_USER_5`,
`MAV_CMD_USER_1`, `MAV_CMD_USER_2`, `MAV_CMD_USER_3`, `MAV_CMD_USER_4`,
`MAV_CMD_USER_5`, `MAV_CMD_CAN_FORWARD`.

All other messages and enums in the three inputs are outside the subset and
omitted. Names cited in descriptions do not expand the data-type dependency
closure. The required included enum dependencies are: `common.GpsFixType`,
`minimal.MavAutopilot`, `standard.MavBool`, `common.MavCmd`, `common.MavFrame`,
`common.MavMissionResult`, `common.MavMissionType`, `minimal.MavModeFlag`,
`common.MavParamError`, `common.MavParamType`, `common.MavResult`,
`minimal.MavState`, `common.MavSysStatusSensor`,
`common.MavSysStatusSensorExtended`, `minimal.MavType`, `common.MissionState`.
MavBool is needed by the selected MavCmd parameters, not by a message field. The
frozen `import standard.MavBool` in `common/enums.typl` is retained for the
selected MAV_CMD parameter metadata, whose enum references are represented only
in comments because the command payloads use generic parameters. The compiler
therefore may report the expected unused-import warning TYPL-007; the closure
and import are preserved. No additional external declaration is required.

## Interface membership and protocol decisions

Exactly five interfaces represent the frozen documented boundaries. No service,
system, component or deployment is introduced. Each command/query accepts one
parameter named `request`, containing the unchanged named message shape.
`request` is a translation parameter wrapper name, not an upstream field rename.
These signatures retain message structs rather than flattening or inventing
per-command parameter lists.

| Interface                 | Upstream message     | Exact signature                                                     | Individual protocol reason                                                                                                                                                                                                        |
| ------------------------- | -------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| minimal.HeartbeatProtocol | HEARTBEAT            | signal heartbeat: Heartbeat                                         | Regular system-presence broadcast is state, so the heartbeat protocol resolves the signal/event ambiguity to signal.                                                                                                              |
| common.MissionProtocol    | MISSION_ITEM         | command missionItem(request: MissionItem)                           | Legacy upload item mutates the mission; retained despite deprecation. MISSION_ACK is separate application data. Download responses can also use this shape; direction is not expressible here.                                    |
| common.MissionProtocol    | MISSION_REQUEST      | query missionRequest(request: MissionRequest): MissionItemInt       | The protocol and deprecation text require MISSION_ITEM_INT for a received legacy request, despite the preserved conflicting description that says MISSION_ITEM. Receiver-driven upload requests travel in the opposite direction. |
| common.MissionProtocol    | MISSION_SET_CURRENT  | command missionSetCurrent(request: MissionSetCurrent)               | Mutates the current execution item. Broadcast MISSION_CURRENT is uncorrelated state, not a private query reply; superseded status is retained.                                                                                    |
| common.MissionProtocol    | MISSION_CURRENT      | signal missionCurrent: MissionCurrent                               | XML and mission protocol document continuous current-item/state/plan-ID streaming; no timing annotation is inferred.                                                                                                              |
| common.MissionProtocol    | MISSION_REQUEST_LIST | query missionRequestList(request: MissionRequestList): MissionCount | Download starts by requesting the list and receiving its count; subsequent item requests remain separate.                                                                                                                         |
| common.MissionProtocol    | MISSION_COUNT        | command missionCount(request: MissionCount)                         | Chosen for its upload-initiation role. The same named struct is also the MISSION_REQUEST_LIST response. Receiver-driven transfer and bidirectional use cannot be encoded by this one callable.                                    |
| common.MissionProtocol    | MISSION_CLEAR_ALL    | command missionClearAll(request: MissionClearAll)                   | Deletes stored mission state. MISSION_ACK event retains the completion result as application data; no RIDL return or transport-ACK substitution.                                                                                  |
| common.MissionProtocol    | MISSION_ITEM_REACHED | event missionItemReached: MissionItemReached                        | Occurs once for each reached item; this is an occurrence, not continuously streamed state.                                                                                                                                        |
| common.MissionProtocol    | MISSION_ACK          | event missionAck: MissionAck                                        | Carries mission transaction result and opaque plan ID. Remains application data for uploads, downloads and clear-all; correlation is not enforced.                                                                                |
| common.MissionProtocol    | MISSION_REQUEST_INT  | query missionRequestInt(request: MissionRequestInt): MissionItemInt | Documented item download request/response. During upload this request is emitted by the receiver; RIDL query direction does not preserve that reciprocal role.                                                                    |
| common.MissionProtocol    | MISSION_ITEM_INT     | command missionItemInt(request: MissionItemInt)                     | Chosen for the upload/set-item role; also used as both item-query responses. Announcements/download direction are not separate new interactions.                                                                                  |
| common.CommandProtocol    | COMMAND_INT          | command commandInt(request: CommandInt)                             | Generic command-ID request mutates or requests action; all request fields stay in CommandInt. COMMAND_ACK event carries outcomes and progress, separately from the RIDL delivery acknowledgement.                                 |
| common.CommandProtocol    | COMMAND_LONG         | command commandLong(request: CommandLong)                           | Same generic command protocol with seven floats; no dedicated signatures for selected MAV_CMD entries. COMMAND_ACK event retains progress and terminal results.                                                                   |
| common.CommandProtocol    | COMMAND_ACK          | event commandAck: CommandAck                                        | Application acknowledgement includes result, progress and result_param2. Multiple IN_PROGRESS messages and the final ACK remain occurrences; no transport acknowledgement replaces them.                                          |
| common.CommandProtocol    | COMMAND_CANCEL       | command commandCancel(request: CommandCancel)                       | Requests cancellation of an in-progress command. The original operation completes through COMMAND_ACK; unsupported or idle cancellation may be ignored.                                                                           |
| common.ParameterProtocol  | PARAM_REQUEST_READ   | query paramRequestRead(request: ParamRequestRead): ParamValue       | Documented single-value read returns ParamValue. The wire response is broadcast and an error can arrive as PARAM_ERROR event; RIDL private reply/correlation is an approximation.                                                 |
| common.ParameterProtocol  | PARAM_REQUEST_LIST   | query paramRequestList(request: ParamRequestList): <ParamValue>     | All parameter values are broadcast individually; a stream of the existing ParamValue retains the multiple-response shape. Wire timeout/retry/completion and multiple responding components are not encoded.                       |
| common.ParameterProtocol  | PARAM_VALUE          | event paramValue: ParamValue                                        | Dual-use reply and unsolicited changed-value broadcast. Retained as an event in addition to the existing named query return shape; repeated list responses are not telemetry.                                                     |
| common.ParameterProtocol  | PARAM_SET            | command paramSet(request: ParamSet)                                 | Writes a parameter. PARAM_VALUE is broadcast even on failure with the current unchanged value, and remains application data on its event; no private success reply is invented.                                                   |
| common.ParameterProtocol  | PARAM_ERROR          | event paramError: ParamError                                        | Protocol read/write error notification remains its original payload. It is not a RIDL functional-error type or synthesized fallible return.                                                                                       |
| common.Telemetry          | SYS_STATUS           | signal sysStatus: SysStatus                                         | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |
| common.Telemetry          | GPS_RAW_INT          | signal gpsRawInt: GpsRawInt                                         | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |
| common.Telemetry          | ATTITUDE             | signal attitude: Attitude                                           | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |
| common.Telemetry          | LOCAL_POSITION_NED   | signal localPositionNed: LocalPositionNed                           | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |
| common.Telemetry          | GLOBAL_POSITION_INT  | signal globalPositionInt: GlobalPositionInt                         | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |
| common.Telemetry          | VFR_HUD              | signal vfrHud: VfrHud                                               | Streamed telemetry from the documented telemetry boundary and selected XML. The original message struct is the payload; typical rates supply no RIDL timing contract.                                                             |

The five interface names are the binding brief's boundary names, not renames of
XML declarations: minimal.HeartbeatProtocol, common.MissionProtocol,
common.CommandProtocol, common.ParameterProtocol and common.Telemetry. Packages
minimal, standard and common are unchanged upstream dialect basenames. File
grouping follows the frozen inventory. References to sibling declarations use
explicit qualified imports from the original package; no wildcard import,
re-export, remote dependency or alias is added. standard includes minimal
upstream, but no selected standard type requires a minimal type, so no unused
include-wide re-export is synthesized.

Protocol limitations apply individually as explained in the table: an interface
has one provider/consumer direction, while MAVLink roles reverse during mission
uploads/downloads. Request/response shapes do not encode target/source routing,
broadcast fan-out, sequence-based correlation, retransmission, timeouts or
state-machine transitions. Query replies are RIDL-correlated private data
although upstream replies may be broadcast. Events carry application
acknowledgement data and do not promise a correlation to a particular RIDL call.
COMMAND_ACK includes repeated progress and final results. RIDL transport
acknowledgements carry no substitute application result. Parameter-list stream
termination does not encode MAVLink's timeout-based completion. PARAM_ERROR
remains a separate event rather than a synthesized error union. Selected
telemetry-rate and one-shot commands remain MAV_CMD IDs inside generic command
payloads. No delivery period, init, default, optionality, require or ensure
contract is inferred from rates, descriptions or sentinels.

## Primitive and unit translation helpers

Each helper is local to the package using it, lives in that package's
messages.typl, and is explicitly identified as a translation helper. No helper
is counted among the 43 upstream declarations. Range-defined integer helpers use
the full source range, not likely operating values. Unconstrained float and unit
helpers introduce no finite limits or quantization. No stored value is rescaled.

| Package  | Helper                     | Exact backing                     |
| -------- | -------------------------- | --------------------------------- |
| minimal  | MavlinkVersion             | integer [0..255]                  |
| minimal  | Uint32                     | integer [0..4294967295]           |
| standard | Int16CentimetersPerSecond  | cm/s [-32768..32767]              |
| standard | Int32                      | integer [-2147483648..2147483647] |
| standard | Int32Millimeters           | mm [-2147483648..2147483647]      |
| standard | Uint16                     | integer [0..65535]                |
| standard | Uint32Milliseconds         | ms [0..4294967295]                |
| common   | Char16                     | bytes [16]                        |
| common   | Float32                    | float                             |
| common   | Float32Meters              | m                                 |
| common   | Float32MetersPerSecond     | m/s                               |
| common   | Float32Microseconds        | us                                |
| common   | Float32Seconds             | s                                 |
| common   | Int16                      | integer [-32768..32767]           |
| common   | Int16Centiamperes          | cA [-32768..32767]                |
| common   | Int32                      | integer [-2147483648..2147483647] |
| common   | Int32Millimeters           | mm [-2147483648..2147483647]      |
| common   | Int8Percent                | % [-128..127]                     |
| common   | Uint16                     | integer [0..65535]                |
| common   | Uint16CentimetersPerSecond | cm/s [0..65535]                   |
| common   | Uint16Millivolts           | mV [0..65535]                     |
| common   | Uint16Percent              | % [0..65535]                      |
| common   | Uint32                     | integer [0..4294967295]           |
| common   | Uint32Millimeters          | mm [0..4294967295]                |
| common   | Uint32MillimetersPerSecond | mm/s [0..4294967295]              |
| common   | Uint32Milliseconds         | ms [0..4294967295]                |
| common   | Uint64Bytes                | struct with data: [Uint8; 8]      |
| common   | Uint8                      | integer [0..255]                  |
| common   | Uint8Percent               | % [0..255]                        |

The unqualified Float32 helper name records the upstream float spelling/width;
its RIDL backing is unconstrained float, which infers float64. It does not claim
an exact float32 wire codec. IEEE binary32 NaN payloads, signed zero and
Infinity behavior are not pinned by the language representation, and no
arbitrary finite range or step has been introduced. This affects every source
float field below, including unit-backed float fields:

`common.PARAM_VALUE.param_value`, `common.PARAM_SET.param_value`,
`common.ATTITUDE.roll`, `common.ATTITUDE.pitch`, `common.ATTITUDE.yaw`,
`common.ATTITUDE.rollspeed`, `common.ATTITUDE.pitchspeed`,
`common.ATTITUDE.yawspeed`, `common.LOCAL_POSITION_NED.x`,
`common.LOCAL_POSITION_NED.y`, `common.LOCAL_POSITION_NED.z`,
`common.LOCAL_POSITION_NED.vx`, `common.LOCAL_POSITION_NED.vy`,
`common.LOCAL_POSITION_NED.vz`, `common.MISSION_ITEM.param1`,
`common.MISSION_ITEM.param2`, `common.MISSION_ITEM.param3`,
`common.MISSION_ITEM.param4`, `common.MISSION_ITEM.x`, `common.MISSION_ITEM.y`,
`common.MISSION_ITEM.z`, `common.MISSION_ITEM_INT.param1`,
`common.MISSION_ITEM_INT.param2`, `common.MISSION_ITEM_INT.param3`,
`common.MISSION_ITEM_INT.param4`, `common.MISSION_ITEM_INT.z`,
`common.VFR_HUD.airspeed`, `common.VFR_HUD.groundspeed`, `common.VFR_HUD.alt`,
`common.VFR_HUD.climb`, `common.COMMAND_INT.param1`,
`common.COMMAND_INT.param2`, `common.COMMAND_INT.param3`,
`common.COMMAND_INT.param4`, `common.COMMAND_INT.z`,
`common.COMMAND_LONG.param1`, `common.COMMAND_LONG.param2`,
`common.COMMAND_LONG.param3`, `common.COMMAND_LONG.param4`,
`common.COMMAND_LONG.param5`, `common.COMMAND_LONG.param6`,
`common.COMMAND_LONG.param7`.

common.GPS_RAW_INT.time_usec uses the Uint64Bytes helper struct
(`data: [Uint8; 8]`) to retain all 64 unsigned bits. The eight uint8 elements
retain all 64 bits. Fixed bytes [8] has no derivable signal init and produced
RIDL-109 in ordinary compilation; the equivalent eight-element array has a
compiler-derived composite init without declaring a source default. This helper
wrapper is a representation necessity, not an upstream declaration. RIDL derives
eight elements with value 0, which is not claimed as a MAVLink startup default.
It is not clipped to signed int64. Numeric operations, integer endian
interpretation and the scalar microseconds unit are no longer compiler-resolved;
the source XML type/unit metadata is retained.

The fixed char[16] buffers use Char16 (`bytes [16]`) at
common.PARAM_REQUEST_READ.param_id, common.PARAM_VALUE.param_id,
common.PARAM_SET.param_id and common.PARAM_ERROR.param_id. Fixed size and every
byte are retained; ASCII character interpretation and the XML-described null
termination/full-length rule are metadata rather than Unicode string
constraints.

minimal.HEARTBEAT.mavlink_version uses MavlinkVersion (`integer [0..255]`),
preserving uint8 range. The upstream magic uint8_t_mavlink_version is
generator-populated, not user-writable; RIDL does not enforce generator-specific
insertion or writability. No version default is supplied.

### Scalar-unit representation differences

UCUM units are resolved only where the XML units attribute supplies them. Unit
backings in this toolchain are float64 even for integer bounds. The following
sites retain the entire source numeric range and exact scale but cannot retain
the source integer/floating wire representation. Parameter-specific units cannot
type generic command slots without inventing signatures; their helper is
recorded but the slot remains generic.

| Source site                                        | Source representation           | Unit | Helper                     | Exact difference                                                                                                                                                                                                    |
| -------------------------------------------------- | ------------------------------- | ---- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| standard.GLOBAL_POSITION_INT.time_boot_ms          | uint32_t                        | ms   | Uint32Milliseconds         | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| standard.GLOBAL_POSITION_INT.alt                   | int32_t                         | mm   | Int32Millimeters           | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| standard.GLOBAL_POSITION_INT.relative_alt          | int32_t                         | mm   | Int32Millimeters           | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| standard.GLOBAL_POSITION_INT.vx                    | int16_t                         | cm/s | Int16CentimetersPerSecond  | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| standard.GLOBAL_POSITION_INT.vy                    | int16_t                         | cm/s | Int16CentimetersPerSecond  | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| standard.GLOBAL_POSITION_INT.vz                    | int16_t                         | cm/s | Int16CentimetersPerSecond  | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param1         | generic command float parameter | s    | Float32Seconds             | Parameter-index-specific units remain metadata: generic CommandInt/CommandLong parameters cannot change type with commandValue. This helper records the accepted UCUM spelling, but is not a per-command signature. |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param2         | generic command float parameter | m    | Float32Meters              | Parameter-index-specific units remain metadata: generic CommandInt/CommandLong parameters cannot change type with commandValue. This helper records the accepted UCUM spelling, but is not a per-command signature. |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param3         | generic command float parameter | m    | Float32Meters              | Parameter-index-specific units remain metadata: generic CommandInt/CommandLong parameters cannot change type with commandValue. This helper records the accepted UCUM spelling, but is not a per-command signature. |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param7         | generic command float parameter | m    | Float32Meters              | Parameter-index-specific units remain metadata: generic CommandInt/CommandLong parameters cannot change type with commandValue. This helper records the accepted UCUM spelling, but is not a per-command signature. |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param2 | generic command float parameter | us   | Float32Microseconds        | Parameter-index-specific units remain metadata: generic CommandInt/CommandLong parameters cannot change type with commandValue. This helper records the accepted UCUM spelling, but is not a per-command signature. |
| common.SYS_STATUS.voltage_battery                  | uint16_t                        | mV   | Uint16Millivolts           | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.SYS_STATUS.current_battery                  | int16_t                         | cA   | Int16Centiamperes          | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.SYS_STATUS.battery_remaining                | int8_t                          | %    | Int8Percent                | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.alt                             | int32_t                         | mm   | Int32Millimeters           | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.vel                             | uint16_t                        | cm/s | Uint16CentimetersPerSecond | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.alt_ellipsoid                   | int32_t                         | mm   | Int32Millimeters           | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.h_acc                           | uint32_t                        | mm   | Uint32Millimeters          | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.v_acc                           | uint32_t                        | mm   | Uint32Millimeters          | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.GPS_RAW_INT.vel_acc                         | uint32_t                        | mm/s | Uint32MillimetersPerSecond | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.ATTITUDE.time_boot_ms                       | uint32_t                        | ms   | Uint32Milliseconds         | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.LOCAL_POSITION_NED.time_boot_ms             | uint32_t                        | ms   | Uint32Milliseconds         | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.LOCAL_POSITION_NED.x                        | float                           | m    | Float32Meters              | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.LOCAL_POSITION_NED.y                        | float                           | m    | Float32Meters              | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.LOCAL_POSITION_NED.z                        | float                           | m    | Float32Meters              | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.LOCAL_POSITION_NED.vx                       | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.LOCAL_POSITION_NED.vy                       | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.LOCAL_POSITION_NED.vz                       | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.VFR_HUD.airspeed                            | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.VFR_HUD.groundspeed                         | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.VFR_HUD.throttle                            | uint16_t                        | %    | Uint16Percent              | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |
| common.VFR_HUD.alt                                 | float                           | m    | Float32Meters              | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.VFR_HUD.climb                               | float                           | m/s  | Float32MetersPerSecond     | Unit backing is unconstrained float64; IEEE float32 width and non-finite behavior are not represented exactly.                                                                                                      |
| common.COMMAND_ACK.progress                        | uint8_t                         | %    | Uint8Percent               | Unit backings are float64, not integer. The full integer range is retained, but fractional values are also admitted and the signed/unsigned integer wire width is lost.                                             |

### Omitted units

Each original unit stays literal in its source metadata comment. Units retained
only in comments are not claimed to be compiler-resolved. No units are inferred
from names or description prose.

| Source site                                | Original spelling | Reason                                                                                         |
| ------------------------------------------ | ----------------- | ---------------------------------------------------------------------------------------------- |
| standard.GLOBAL_POSITION_INT.lat           | degE7             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| standard.GLOBAL_POSITION_INT.lon           | degE7             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| standard.GLOBAL_POSITION_INT.hdg           | cdeg              | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param4 | deg               | The curated UCUM atom table excludes deg.                                                      |
| common.SYS_STATUS.load                     | d%                | The curated UCUM table does not permit prefixes on %. Stored values are not rescaled.          |
| common.SYS_STATUS.drop_rate_comm           | c%                | The curated UCUM table does not permit prefixes on %. Stored values are not rescaled.          |
| common.GPS_RAW_INT.time_usec               | us                | Exact eight-element uint8 representation has no scalar unit backing; unit remains metadata.    |
| common.GPS_RAW_INT.lat                     | degE7             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.GPS_RAW_INT.lon                     | degE7             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.GPS_RAW_INT.cog                     | cdeg              | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.GPS_RAW_INT.hdg_acc                 | degE5             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.GPS_RAW_INT.yaw                     | cdeg              | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.roll                       | rad               | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.pitch                      | rad               | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.yaw                        | rad               | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.rollspeed                  | rad/s             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.pitchspeed                 | rad/s             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.ATTITUDE.yawspeed                   | rad/s             | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |
| common.VFR_HUD.heading                     | deg               | The curated UCUM atoms exclude deg and rad, including prefixed/scaled forms and angular rates. |

### Enum field representation

Ordinary enum fields reference their original named enums. The uint8 base_mode
and full uint32 sensor bitmask fields reference enumsets, never mutually
exclusive enums. The three extended SYS_STATUS mask fields use the full-range
Uint32 helper to retain their original 32-bit storage and all bit combinations;
their enum association stays literal in field metadata. Enum wire width is
inferred from its values rather than the XML field storage width. Closed enum
domains also do not admit unnamed integer values, including omitted MAV_CMD IDs.
The following table identifies every affected field and its original integer
storage. This is a translation limitation, not a restriction inferred from a
likely operating range. MavSysStatusSensorExtended uses highest bit 7 and
therefore infers uint8. Using it directly for its three uint32 fields would lose
the upstream wire width. Those fields therefore retain Uint32, with named-bit
semantics documented by the original enum attribute and the complete enumset
declaration. The compiler cannot enforce that association or label its raw bits.
Other bitmask fields retain their named-bit combinations and inferred source
width.

| Source field                                               | XML storage | RIDL enum/enumset                                                      |
| ---------------------------------------------------------- | ----------- | ---------------------------------------------------------------------- |
| minimal.HEARTBEAT.type                                     | uint8_t     | minimal.MavType                                                        |
| minimal.HEARTBEAT.autopilot                                | uint8_t     | minimal.MavAutopilot                                                   |
| minimal.HEARTBEAT.base_mode                                | uint8_t     | minimal.MavModeFlag                                                    |
| minimal.HEARTBEAT.system_status                            | uint8_t     | minimal.MavState                                                       |
| common.SYS_STATUS.onboard_control_sensors_present          | uint32_t    | common.MavSysStatusSensor                                              |
| common.SYS_STATUS.onboard_control_sensors_enabled          | uint32_t    | common.MavSysStatusSensor                                              |
| common.SYS_STATUS.onboard_control_sensors_health           | uint32_t    | common.MavSysStatusSensor                                              |
| common.SYS_STATUS.onboard_control_sensors_present_extended | uint32_t    | common.Uint32; metadata association: common.MavSysStatusSensorExtended |
| common.SYS_STATUS.onboard_control_sensors_enabled_extended | uint32_t    | common.Uint32; metadata association: common.MavSysStatusSensorExtended |
| common.SYS_STATUS.onboard_control_sensors_health_extended  | uint32_t    | common.Uint32; metadata association: common.MavSysStatusSensorExtended |
| common.PARAM_VALUE.param_type                              | uint8_t     | common.MavParamType                                                    |
| common.PARAM_SET.param_type                                | uint8_t     | common.MavParamType                                                    |
| common.GPS_RAW_INT.fix_type                                | uint8_t     | common.GpsFixType                                                      |
| common.MISSION_ITEM.frame                                  | uint8_t     | common.MavFrame                                                        |
| common.MISSION_ITEM.command                                | uint16_t    | common.MavCmd                                                          |
| common.MISSION_ITEM.mission_type                           | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_REQUEST.mission_type                        | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_CURRENT.mission_state                       | uint8_t     | common.MissionState                                                    |
| common.MISSION_REQUEST_LIST.mission_type                   | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_COUNT.mission_type                          | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_CLEAR_ALL.mission_type                      | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_ACK.type                                    | uint8_t     | common.MavMissionResult                                                |
| common.MISSION_ACK.mission_type                            | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_REQUEST_INT.mission_type                    | uint8_t     | common.MavMissionType                                                  |
| common.MISSION_ITEM_INT.frame                              | uint8_t     | common.MavFrame                                                        |
| common.MISSION_ITEM_INT.command                            | uint16_t    | common.MavCmd                                                          |
| common.MISSION_ITEM_INT.mission_type                       | uint8_t     | common.MavMissionType                                                  |
| common.COMMAND_INT.frame                                   | uint8_t     | common.MavFrame                                                        |
| common.COMMAND_INT.command                                 | uint16_t    | common.MavCmd                                                          |
| common.COMMAND_LONG.command                                | uint16_t    | common.MavCmd                                                          |
| common.COMMAND_ACK.command                                 | uint16_t    | common.MavCmd                                                          |
| common.COMMAND_ACK.result                                  | uint8_t     | common.MavResult                                                       |
| common.COMMAND_CANCEL.command                              | uint16_t    | common.MavCmd                                                          |
| common.PARAM_ERROR.error                                   | uint8_t     | common.MavParamError                                                   |

### Bitmask conversion

Standalone enumsets assign bit positions, not power-of-two masks. Every
conversion below retains the source name and description; original masks also
remain in entry metadata comments.

| Source entry                                                              | Source mask | RIDL bit position |
| ------------------------------------------------------------------------- | ----------- | ----------------- |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_SAFETY_ARMED                          | 128         | 7                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_MANUAL_INPUT_ENABLED                  | 64          | 6                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_HIL_ENABLED                           | 32          | 5                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_STABILIZE_ENABLED                     | 16          | 4                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_GUIDED_ENABLED                        | 8           | 3                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_AUTO_ENABLED                          | 4           | 2                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_TEST_ENABLED                          | 2           | 1                 |
| minimal.MAV_MODE_FLAG.MAV_MODE_FLAG_CUSTOM_MODE_ENABLED                   | 1           | 0                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_GYRO                | 1           | 0                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_ACCEL               | 2           | 1                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_MAG                 | 4           | 2                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_ABSOLUTE_PRESSURE      | 8           | 3                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_DIFFERENTIAL_PRESSURE  | 16          | 4                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_GPS                    | 32          | 5                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_OPTICAL_FLOW           | 64          | 6                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_VISION_POSITION        | 128         | 7                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_LASER_POSITION         | 256         | 8                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_EXTERNAL_GROUND_TRUTH  | 512         | 9                 |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_ANGULAR_RATE_CONTROL   | 1024        | 10                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_ATTITUDE_STABILIZATION | 2048        | 11                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_YAW_POSITION           | 4096        | 12                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_Z_ALTITUDE_CONTROL     | 8192        | 13                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_XY_POSITION_CONTROL    | 16384       | 14                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_MOTOR_OUTPUTS          | 32768       | 15                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_RC_RECEIVER            | 65536       | 16                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_GYRO2               | 131072      | 17                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_ACCEL2              | 262144      | 18                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_3D_MAG2                | 524288      | 19                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_GEOFENCE                      | 1048576     | 20                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_AHRS                          | 2097152     | 21                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_TERRAIN                       | 4194304     | 22                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_REVERSE_MOTOR                 | 8388608     | 23                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_LOGGING                       | 16777216    | 24                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_BATTERY                | 33554432    | 25                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_PROXIMITY              | 67108864    | 26                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_SATCOM                 | 134217728   | 27                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_PREARM_CHECK                  | 268435456   | 28                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_OBSTACLE_AVOIDANCE            | 536870912   | 29                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_SENSOR_PROPULSION             | 1073741824  | 30                |
| common.MAV_SYS_STATUS_SENSOR.MAV_SYS_STATUS_EXTENSION_USED                | 2147483648  | 31                |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_RECOVERY_SYSTEM      | 1           | 0                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_LEAK          | 2           | 1                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_GYRO3      | 4           | 2                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_ACCEL3     | 8           | 3                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_GYRO4      | 16          | 4                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_ACCEL4     | 32          | 5                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_MAG3       | 64          | 6                 |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED.MAV_SYS_STATUS_SENSOR_3D_MAG4       | 128         | 7                 |

### Command-entry flags

These attributes remain source metadata and are not compiler-enforced mission,
command or coordinate semantics:

| Source entry                                  | Original attributes                                    |
| --------------------------------------------- | ------------------------------------------------------ |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT           | hasLocation="true" isDestination="true" mission="true" |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT | command="true"                                         |
| common.MAV_CMD.MAV_CMD_COMPONENT_ARM_DISARM   | command="true"                                         |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL   | command="true"                                         |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE        | command="true"                                         |

## Lexical changes

Container mappings are fixed by the inventory. Every changed field and
interaction name is listed individually below. Abbreviations and unreserved
names are not expanded or corrected; enum entry names remain exact
SCREAMING_SNAKE. The Value suffix is applied only to registry-reserved names.
Packages are unchanged.

| Qualified source                                           | Qualified target                                      | Reason                                                     |
| ---------------------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------- |
| minimal.MAV_AUTOPILOT                                      | minimal.MavAutopilot                                  | Container case required by RIDL; frozen inventory mapping. |
| minimal.MAV_TYPE                                           | minimal.MavType                                       | Container case required by RIDL; frozen inventory mapping. |
| minimal.MAV_MODE_FLAG                                      | minimal.MavModeFlag                                   | Container case required by RIDL; frozen inventory mapping. |
| minimal.MAV_STATE                                          | minimal.MavState                                      | Container case required by RIDL; frozen inventory mapping. |
| minimal.HEARTBEAT                                          | minimal.Heartbeat                                     | Container case required by RIDL; frozen inventory mapping. |
| minimal.HEARTBEAT.type                                     | minimal.Heartbeat.typeValue                           | Reserved word suffix required.                             |
| minimal.HEARTBEAT.base_mode                                | minimal.Heartbeat.baseMode                            | Member case required by RIDL.                              |
| minimal.HEARTBEAT.custom_mode                              | minimal.Heartbeat.customMode                          | Member case required by RIDL.                              |
| minimal.HEARTBEAT.system_status                            | minimal.Heartbeat.systemStatus                        | Member case required by RIDL.                              |
| minimal.HEARTBEAT.mavlink_version                          | minimal.Heartbeat.mavlinkVersion                      | Member case required by RIDL.                              |
| standard.MAV_BOOL                                          | standard.MavBool                                      | Container case required by RIDL; frozen inventory mapping. |
| standard.GLOBAL_POSITION_INT                               | standard.GlobalPositionInt                            | Container case required by RIDL; frozen inventory mapping. |
| standard.GLOBAL_POSITION_INT.time_boot_ms                  | standard.GlobalPositionInt.timeBootMs                 | Member case required by RIDL.                              |
| standard.GLOBAL_POSITION_INT.relative_alt                  | standard.GlobalPositionInt.relativeAlt                | Member case required by RIDL.                              |
| common.MAV_SYS_STATUS_SENSOR                               | common.MavSysStatusSensor                             | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_SYS_STATUS_SENSOR_EXTENDED                      | common.MavSysStatusSensorExtended                     | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_FRAME                                           | common.MavFrame                                       | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_CMD                                             | common.MavCmd                                         | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_PARAM_TYPE                                      | common.MavParamType                                   | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_PARAM_ERROR                                     | common.MavParamError                                  | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_RESULT                                          | common.MavResult                                      | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_MISSION_RESULT                                  | common.MavMissionResult                               | Container case required by RIDL; frozen inventory mapping. |
| common.MAV_MISSION_TYPE                                    | common.MavMissionType                                 | Container case required by RIDL; frozen inventory mapping. |
| common.GPS_FIX_TYPE                                        | common.GpsFixType                                     | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_STATE                                       | common.MissionState                                   | Container case required by RIDL; frozen inventory mapping. |
| common.SYS_STATUS                                          | common.SysStatus                                      | Container case required by RIDL; frozen inventory mapping. |
| common.SYS_STATUS.onboard_control_sensors_present          | common.SysStatus.onboardControlSensorsPresent         | Member case required by RIDL.                              |
| common.SYS_STATUS.onboard_control_sensors_enabled          | common.SysStatus.onboardControlSensorsEnabled         | Member case required by RIDL.                              |
| common.SYS_STATUS.onboard_control_sensors_health           | common.SysStatus.onboardControlSensorsHealth          | Member case required by RIDL.                              |
| common.SYS_STATUS.voltage_battery                          | common.SysStatus.voltageBattery                       | Member case required by RIDL.                              |
| common.SYS_STATUS.current_battery                          | common.SysStatus.currentBattery                       | Member case required by RIDL.                              |
| common.SYS_STATUS.battery_remaining                        | common.SysStatus.batteryRemaining                     | Member case required by RIDL.                              |
| common.SYS_STATUS.drop_rate_comm                           | common.SysStatus.dropRateComm                         | Member case required by RIDL.                              |
| common.SYS_STATUS.errors_comm                              | common.SysStatus.errorsComm                           | Member case required by RIDL.                              |
| common.SYS_STATUS.errors_count1                            | common.SysStatus.errorsCount1                         | Member case required by RIDL.                              |
| common.SYS_STATUS.errors_count2                            | common.SysStatus.errorsCount2                         | Member case required by RIDL.                              |
| common.SYS_STATUS.errors_count3                            | common.SysStatus.errorsCount3                         | Member case required by RIDL.                              |
| common.SYS_STATUS.errors_count4                            | common.SysStatus.errorsCount4                         | Member case required by RIDL.                              |
| common.SYS_STATUS.onboard_control_sensors_present_extended | common.SysStatus.onboardControlSensorsPresentExtended | Member case required by RIDL.                              |
| common.SYS_STATUS.onboard_control_sensors_enabled_extended | common.SysStatus.onboardControlSensorsEnabledExtended | Member case required by RIDL.                              |
| common.SYS_STATUS.onboard_control_sensors_health_extended  | common.SysStatus.onboardControlSensorsHealthExtended  | Member case required by RIDL.                              |
| common.PARAM_REQUEST_READ                                  | common.ParamRequestRead                               | Container case required by RIDL; frozen inventory mapping. |
| common.PARAM_REQUEST_READ.target_system                    | common.ParamRequestRead.targetSystem                  | Member case required by RIDL.                              |
| common.PARAM_REQUEST_READ.target_component                 | common.ParamRequestRead.targetComponent               | Member case required by RIDL.                              |
| common.PARAM_REQUEST_READ.param_id                         | common.ParamRequestRead.paramId                       | Member case required by RIDL.                              |
| common.PARAM_REQUEST_READ.param_index                      | common.ParamRequestRead.paramIndex                    | Member case required by RIDL.                              |
| common.PARAM_REQUEST_LIST                                  | common.ParamRequestList                               | Container case required by RIDL; frozen inventory mapping. |
| common.PARAM_REQUEST_LIST.target_system                    | common.ParamRequestList.targetSystem                  | Member case required by RIDL.                              |
| common.PARAM_REQUEST_LIST.target_component                 | common.ParamRequestList.targetComponent               | Member case required by RIDL.                              |
| common.PARAM_VALUE                                         | common.ParamValue                                     | Container case required by RIDL; frozen inventory mapping. |
| common.PARAM_VALUE.param_id                                | common.ParamValue.paramId                             | Member case required by RIDL.                              |
| common.PARAM_VALUE.param_value                             | common.ParamValue.paramValue                          | Member case required by RIDL.                              |
| common.PARAM_VALUE.param_type                              | common.ParamValue.paramType                           | Member case required by RIDL.                              |
| common.PARAM_VALUE.param_count                             | common.ParamValue.paramCount                          | Member case required by RIDL.                              |
| common.PARAM_VALUE.param_index                             | common.ParamValue.paramIndex                          | Member case required by RIDL.                              |
| common.PARAM_SET                                           | common.ParamSet                                       | Container case required by RIDL; frozen inventory mapping. |
| common.PARAM_SET.target_system                             | common.ParamSet.targetSystem                          | Member case required by RIDL.                              |
| common.PARAM_SET.target_component                          | common.ParamSet.targetComponent                       | Member case required by RIDL.                              |
| common.PARAM_SET.param_id                                  | common.ParamSet.paramId                               | Member case required by RIDL.                              |
| common.PARAM_SET.param_value                               | common.ParamSet.paramValue                            | Member case required by RIDL.                              |
| common.PARAM_SET.param_type                                | common.ParamSet.paramType                             | Member case required by RIDL.                              |
| common.GPS_RAW_INT                                         | common.GpsRawInt                                      | Container case required by RIDL; frozen inventory mapping. |
| common.GPS_RAW_INT.time_usec                               | common.GpsRawInt.timeUsec                             | Member case required by RIDL.                              |
| common.GPS_RAW_INT.fix_type                                | common.GpsRawInt.fixType                              | Member case required by RIDL.                              |
| common.GPS_RAW_INT.satellites_visible                      | common.GpsRawInt.satellitesVisible                    | Member case required by RIDL.                              |
| common.GPS_RAW_INT.alt_ellipsoid                           | common.GpsRawInt.altEllipsoid                         | Member case required by RIDL.                              |
| common.GPS_RAW_INT.h_acc                                   | common.GpsRawInt.hAcc                                 | Member case required by RIDL.                              |
| common.GPS_RAW_INT.v_acc                                   | common.GpsRawInt.vAcc                                 | Member case required by RIDL.                              |
| common.GPS_RAW_INT.vel_acc                                 | common.GpsRawInt.velAcc                               | Member case required by RIDL.                              |
| common.GPS_RAW_INT.hdg_acc                                 | common.GpsRawInt.hdgAcc                               | Member case required by RIDL.                              |
| common.ATTITUDE                                            | common.Attitude                                       | Container case required by RIDL; frozen inventory mapping. |
| common.ATTITUDE.time_boot_ms                               | common.Attitude.timeBootMs                            | Member case required by RIDL.                              |
| common.LOCAL_POSITION_NED                                  | common.LocalPositionNed                               | Container case required by RIDL; frozen inventory mapping. |
| common.LOCAL_POSITION_NED.time_boot_ms                     | common.LocalPositionNed.timeBootMs                    | Member case required by RIDL.                              |
| common.MISSION_ITEM                                        | common.MissionItem                                    | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_ITEM.target_system                          | common.MissionItem.targetSystem                       | Member case required by RIDL.                              |
| common.MISSION_ITEM.target_component                       | common.MissionItem.targetComponent                    | Member case required by RIDL.                              |
| common.MISSION_ITEM.command                                | common.MissionItem.commandValue                       | Reserved word suffix required.                             |
| common.MISSION_ITEM.current                                | common.MissionItem.currentValue                       | Reserved word suffix required.                             |
| common.MISSION_ITEM.mission_type                           | common.MissionItem.missionType                        | Member case required by RIDL.                              |
| common.MISSION_REQUEST                                     | common.MissionRequest                                 | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_REQUEST.target_system                       | common.MissionRequest.targetSystem                    | Member case required by RIDL.                              |
| common.MISSION_REQUEST.target_component                    | common.MissionRequest.targetComponent                 | Member case required by RIDL.                              |
| common.MISSION_REQUEST.mission_type                        | common.MissionRequest.missionType                     | Member case required by RIDL.                              |
| common.MISSION_SET_CURRENT                                 | common.MissionSetCurrent                              | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_SET_CURRENT.target_system                   | common.MissionSetCurrent.targetSystem                 | Member case required by RIDL.                              |
| common.MISSION_SET_CURRENT.target_component                | common.MissionSetCurrent.targetComponent              | Member case required by RIDL.                              |
| common.MISSION_CURRENT                                     | common.MissionCurrent                                 | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_CURRENT.mission_state                       | common.MissionCurrent.missionState                    | Member case required by RIDL.                              |
| common.MISSION_CURRENT.mission_mode                        | common.MissionCurrent.missionMode                     | Member case required by RIDL.                              |
| common.MISSION_CURRENT.mission_id                          | common.MissionCurrent.missionId                       | Member case required by RIDL.                              |
| common.MISSION_CURRENT.fence_id                            | common.MissionCurrent.fenceId                         | Member case required by RIDL.                              |
| common.MISSION_CURRENT.rally_points_id                     | common.MissionCurrent.rallyPointsId                   | Member case required by RIDL.                              |
| common.MISSION_REQUEST_LIST                                | common.MissionRequestList                             | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_REQUEST_LIST.target_system                  | common.MissionRequestList.targetSystem                | Member case required by RIDL.                              |
| common.MISSION_REQUEST_LIST.target_component               | common.MissionRequestList.targetComponent             | Member case required by RIDL.                              |
| common.MISSION_REQUEST_LIST.mission_type                   | common.MissionRequestList.missionType                 | Member case required by RIDL.                              |
| common.MISSION_COUNT                                       | common.MissionCount                                   | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_COUNT.target_system                         | common.MissionCount.targetSystem                      | Member case required by RIDL.                              |
| common.MISSION_COUNT.target_component                      | common.MissionCount.targetComponent                   | Member case required by RIDL.                              |
| common.MISSION_COUNT.mission_type                          | common.MissionCount.missionType                       | Member case required by RIDL.                              |
| common.MISSION_COUNT.opaque_id                             | common.MissionCount.opaqueId                          | Member case required by RIDL.                              |
| common.MISSION_CLEAR_ALL                                   | common.MissionClearAll                                | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_CLEAR_ALL.target_system                     | common.MissionClearAll.targetSystem                   | Member case required by RIDL.                              |
| common.MISSION_CLEAR_ALL.target_component                  | common.MissionClearAll.targetComponent                | Member case required by RIDL.                              |
| common.MISSION_CLEAR_ALL.mission_type                      | common.MissionClearAll.missionType                    | Member case required by RIDL.                              |
| common.MISSION_ITEM_REACHED                                | common.MissionItemReached                             | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_ACK                                         | common.MissionAck                                     | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_ACK.target_system                           | common.MissionAck.targetSystem                        | Member case required by RIDL.                              |
| common.MISSION_ACK.target_component                        | common.MissionAck.targetComponent                     | Member case required by RIDL.                              |
| common.MISSION_ACK.type                                    | common.MissionAck.typeValue                           | Reserved word suffix required.                             |
| common.MISSION_ACK.mission_type                            | common.MissionAck.missionType                         | Member case required by RIDL.                              |
| common.MISSION_ACK.opaque_id                               | common.MissionAck.opaqueId                            | Member case required by RIDL.                              |
| common.MISSION_REQUEST_INT                                 | common.MissionRequestInt                              | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_REQUEST_INT.target_system                   | common.MissionRequestInt.targetSystem                 | Member case required by RIDL.                              |
| common.MISSION_REQUEST_INT.target_component                | common.MissionRequestInt.targetComponent              | Member case required by RIDL.                              |
| common.MISSION_REQUEST_INT.mission_type                    | common.MissionRequestInt.missionType                  | Member case required by RIDL.                              |
| common.MISSION_ITEM_INT                                    | common.MissionItemInt                                 | Container case required by RIDL; frozen inventory mapping. |
| common.MISSION_ITEM_INT.target_system                      | common.MissionItemInt.targetSystem                    | Member case required by RIDL.                              |
| common.MISSION_ITEM_INT.target_component                   | common.MissionItemInt.targetComponent                 | Member case required by RIDL.                              |
| common.MISSION_ITEM_INT.command                            | common.MissionItemInt.commandValue                    | Reserved word suffix required.                             |
| common.MISSION_ITEM_INT.current                            | common.MissionItemInt.currentValue                    | Reserved word suffix required.                             |
| common.MISSION_ITEM_INT.mission_type                       | common.MissionItemInt.missionType                     | Member case required by RIDL.                              |
| common.VFR_HUD                                             | common.VfrHud                                         | Container case required by RIDL; frozen inventory mapping. |
| common.COMMAND_INT                                         | common.CommandInt                                     | Container case required by RIDL; frozen inventory mapping. |
| common.COMMAND_INT.target_system                           | common.CommandInt.targetSystem                        | Member case required by RIDL.                              |
| common.COMMAND_INT.target_component                        | common.CommandInt.targetComponent                     | Member case required by RIDL.                              |
| common.COMMAND_INT.command                                 | common.CommandInt.commandValue                        | Reserved word suffix required.                             |
| common.COMMAND_INT.current                                 | common.CommandInt.currentValue                        | Reserved word suffix required.                             |
| common.COMMAND_LONG                                        | common.CommandLong                                    | Container case required by RIDL; frozen inventory mapping. |
| common.COMMAND_LONG.target_system                          | common.CommandLong.targetSystem                       | Member case required by RIDL.                              |
| common.COMMAND_LONG.target_component                       | common.CommandLong.targetComponent                    | Member case required by RIDL.                              |
| common.COMMAND_LONG.command                                | common.CommandLong.commandValue                       | Reserved word suffix required.                             |
| common.COMMAND_ACK                                         | common.CommandAck                                     | Container case required by RIDL; frozen inventory mapping. |
| common.COMMAND_ACK.command                                 | common.CommandAck.commandValue                        | Reserved word suffix required.                             |
| common.COMMAND_ACK.result_param2                           | common.CommandAck.resultParam2                        | Member case required by RIDL.                              |
| common.COMMAND_ACK.target_system                           | common.CommandAck.targetSystem                        | Member case required by RIDL.                              |
| common.COMMAND_ACK.target_component                        | common.CommandAck.targetComponent                     | Member case required by RIDL.                              |
| common.COMMAND_CANCEL                                      | common.CommandCancel                                  | Container case required by RIDL; frozen inventory mapping. |
| common.COMMAND_CANCEL.target_system                        | common.CommandCancel.targetSystem                     | Member case required by RIDL.                              |
| common.COMMAND_CANCEL.target_component                     | common.CommandCancel.targetComponent                  | Member case required by RIDL.                              |
| common.COMMAND_CANCEL.command                              | common.CommandCancel.commandValue                     | Reserved word suffix required.                             |
| common.PARAM_ERROR                                         | common.ParamError                                     | Container case required by RIDL; frozen inventory mapping. |
| common.PARAM_ERROR.target_system                           | common.ParamError.targetSystem                        | Member case required by RIDL.                              |
| common.PARAM_ERROR.target_component                        | common.ParamError.targetComponent                     | Member case required by RIDL.                              |
| common.PARAM_ERROR.param_id                                | common.ParamError.paramId                             | Member case required by RIDL.                              |
| common.PARAM_ERROR.param_index                             | common.ParamError.paramIndex                          | Member case required by RIDL.                              |
| common.PARAM_ERROR.error                                   | common.ParamError.errorValue                          | Reserved word suffix required.                             |
| minimal.HEARTBEAT (message interaction)                    | minimal.HeartbeatProtocol.heartbeat                   | Interaction case required by RIDL.                         |
| common.MISSION_ITEM (message interaction)                  | common.MissionProtocol.missionItem                    | Interaction case required by RIDL.                         |
| common.MISSION_REQUEST (message interaction)               | common.MissionProtocol.missionRequest                 | Interaction case required by RIDL.                         |
| common.MISSION_SET_CURRENT (message interaction)           | common.MissionProtocol.missionSetCurrent              | Interaction case required by RIDL.                         |
| common.MISSION_CURRENT (message interaction)               | common.MissionProtocol.missionCurrent                 | Interaction case required by RIDL.                         |
| common.MISSION_REQUEST_LIST (message interaction)          | common.MissionProtocol.missionRequestList             | Interaction case required by RIDL.                         |
| common.MISSION_COUNT (message interaction)                 | common.MissionProtocol.missionCount                   | Interaction case required by RIDL.                         |
| common.MISSION_CLEAR_ALL (message interaction)             | common.MissionProtocol.missionClearAll                | Interaction case required by RIDL.                         |
| common.MISSION_ITEM_REACHED (message interaction)          | common.MissionProtocol.missionItemReached             | Interaction case required by RIDL.                         |
| common.MISSION_ACK (message interaction)                   | common.MissionProtocol.missionAck                     | Interaction case required by RIDL.                         |
| common.MISSION_REQUEST_INT (message interaction)           | common.MissionProtocol.missionRequestInt              | Interaction case required by RIDL.                         |
| common.MISSION_ITEM_INT (message interaction)              | common.MissionProtocol.missionItemInt                 | Interaction case required by RIDL.                         |
| common.COMMAND_INT (message interaction)                   | common.CommandProtocol.commandInt                     | Interaction case required by RIDL.                         |
| common.COMMAND_LONG (message interaction)                  | common.CommandProtocol.commandLong                    | Interaction case required by RIDL.                         |
| common.COMMAND_ACK (message interaction)                   | common.CommandProtocol.commandAck                     | Interaction case required by RIDL.                         |
| common.COMMAND_CANCEL (message interaction)                | common.CommandProtocol.commandCancel                  | Interaction case required by RIDL.                         |
| common.PARAM_REQUEST_READ (message interaction)            | common.ParameterProtocol.paramRequestRead             | Interaction case required by RIDL.                         |
| common.PARAM_REQUEST_LIST (message interaction)            | common.ParameterProtocol.paramRequestList             | Interaction case required by RIDL.                         |
| common.PARAM_VALUE (message interaction)                   | common.ParameterProtocol.paramValue                   | Interaction case required by RIDL.                         |
| common.PARAM_SET (message interaction)                     | common.ParameterProtocol.paramSet                     | Interaction case required by RIDL.                         |
| common.PARAM_ERROR (message interaction)                   | common.ParameterProtocol.paramError                   | Interaction case required by RIDL.                         |
| common.SYS_STATUS (message interaction)                    | common.Telemetry.sysStatus                            | Interaction case required by RIDL.                         |
| common.GPS_RAW_INT (message interaction)                   | common.Telemetry.gpsRawInt                            | Interaction case required by RIDL.                         |
| common.ATTITUDE (message interaction)                      | common.Telemetry.attitude                             | Interaction case required by RIDL.                         |
| common.LOCAL_POSITION_NED (message interaction)            | common.Telemetry.localPositionNed                     | Interaction case required by RIDL.                         |
| standard.GLOBAL_POSITION_INT (message interaction)         | common.Telemetry.globalPositionInt                    | Interaction case required by RIDL.                         |
| common.VFR_HUD (message interaction)                       | common.Telemetry.vfrHud                               | Interaction case required by RIDL.                         |

## Comments, extensions, defaults and metadata

Text between XML description/field/param/deprecated/superseded tags becomes line
comments with only a fixed comment prefix added and XML entities decoded. Each
text line's internal wording, punctuation, spacing, tabs, leading/trailing
whitespace and line breaks are retained; XML layout whitespace between elements
is excluded as markup. Empty text lines become empty comment lines. XML comments
inside selected enum blocks are retained with comment delimiters changed.
MAV_CMD comments pertaining to omitted entries are outside the selected entry
blocks and omitted. Message/enum/entry/field/param attributes remain exact
key/value metadata comments; JSON string quoting represents XML attribute
strings without changing their values. Original attribute names are metadata,
not new RIDL identifiers or unrecognized attributes. XML markup tags are
replaced by the comment labels `XML message`, `XML enum`, `XML entry`,
`XML field`, `XML param`, and annotation tag names. Deprecation on message
containers also uses the existing @deprecated doc tag without inventing a
reason, so its expected warning is preserved.

The <extensions/> boundary is retained in place in each of these messages:

`common.SYS_STATUS`, `common.GPS_RAW_INT`, `common.MISSION_ITEM`,
`common.MISSION_REQUEST`, `common.MISSION_CURRENT`,
`common.MISSION_REQUEST_LIST`, `common.MISSION_COUNT`,
`common.MISSION_CLEAR_ALL`, `common.MISSION_ACK`, `common.MISSION_REQUEST_INT`,
`common.MISSION_ITEM_INT`, `common.COMMAND_ACK`.

Extension fields remain required and in source order. RIDL does not represent
MAVLink 1 exclusion, MAVLink 2 zero-truncation/padding or packet-length/version
presence. No extension default or optional marker is introduced. XML field order
is preserved; MAVLink generator wire reordering, IDs, CRC extra, payload byte
layout and endian encoding are not RIDL struct ordinals or a new wire backend.

XML invalid, multiplier, print_format, default, param
labels/index/ranges/increments/reserved flags, hasLocation, isDestination,
mission, command, wip, superseded and deprecation dates/replacements have no
exact consumed RIDL attribute for their MAVLink semantics here. They remain
literal metadata/comments. Parameter units and MavBool references remain
command-entry metadata because command payloads have generic parameters.
Enum/entry deprecation and superseded/wip annotations remain comments rather
than unsupported entry attributes. The metadata sites below list the exact
unrepresented semantics; descriptions retain additional literal magic/sentinel
values.

| Source site                                                | Metadata kind | Original attributes                                                         |
| ---------------------------------------------------------- | ------------- | --------------------------------------------------------------------------- |
| standard.GLOBAL_POSITION_INT.hdg                           | invalid       | invalid="UINT16_MAX"                                                        |
| common.MAV_FRAME.MAV_FRAME_GLOBAL_INT                      | superseded    | since="2024-03" replaced_by="MAV_FRAME_GLOBAL"                              |
| common.MAV_FRAME.MAV_FRAME_GLOBAL_RELATIVE_ALT_INT         | superseded    | since="2024-03" replaced_by="MAV_FRAME_GLOBAL_RELATIVE_ALT"                 |
| common.MAV_FRAME.MAV_FRAME_BODY_NED                        | superseded    | since="2019-08" replaced_by="MAV_FRAME_BODY_FRD"                            |
| common.MAV_FRAME.MAV_FRAME_BODY_OFFSET_NED                 | superseded    | since="2019-08" replaced_by="MAV_FRAME_BODY_FRD"                            |
| common.MAV_FRAME.MAV_FRAME_GLOBAL_TERRAIN_ALT_INT          | superseded    | since="2024-03" replaced_by="MAV_FRAME_GLOBAL_TERRAIN_ALT"                  |
| common.MAV_FRAME.MAV_FRAME_RESERVED_13                     | deprecated    | since="2019-04" replaced_by=""                                              |
| common.MAV_FRAME.MAV_FRAME_RESERVED_14                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FRD"                           |
| common.MAV_FRAME.MAV_FRAME_RESERVED_15                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FLU"                           |
| common.MAV_FRAME.MAV_FRAME_RESERVED_16                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FRD"                           |
| common.MAV_FRAME.MAV_FRAME_RESERVED_17                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FLU"                           |
| common.MAV_FRAME.MAV_FRAME_RESERVED_18                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FRD"                           |
| common.MAV_FRAME.MAV_FRAME_RESERVED_19                     | deprecated    | since="2019-04" replaced_by="MAV_FRAME_LOCAL_FLU"                           |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param1                 | param         | index="1" label="Hold" units="s" minValue="0"                               |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param2                 | param         | index="2" label="Accept Radius" units="m" minValue="0"                      |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param3                 | param         | index="3" label="Pass Radius" units="m"                                     |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param4                 | param         | index="4" label="Yaw" units="deg"                                           |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param5                 | param         | index="5" label="Latitude"                                                  |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param6                 | param         | index="6" label="Longitude"                                                 |
| common.MAV_CMD.MAV_CMD_NAV_WAYPOINT.param7                 | param         | index="7" label="Altitude" units="m"                                        |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param1       | param         | index="1" label="Number" minValue="-1" increment="1"                        |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param2       | param         | index="2" label="Reset Mission" enum="MAV_BOOL"                             |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param3       | param         | index="3"                                                                   |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param4       | param         | index="4"                                                                   |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param5       | param         | index="5"                                                                   |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param6       | param         | index="6"                                                                   |
| common.MAV_CMD.MAV_CMD_DO_SET_MISSION_CURRENT.param7       | param         | index="7"                                                                   |
| common.MAV_CMD.MAV_CMD_COMPONENT_ARM_DISARM.param1         | param         | index="1" label="Arm" enum="MAV_BOOL"                                       |
| common.MAV_CMD.MAV_CMD_COMPONENT_ARM_DISARM.param2         | param         | index="2" label="Force" minValue="0" maxValue="21196" increment="21196"     |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param1         | param         | index="1" label="Message ID" minValue="0" maxValue="16777215" increment="1" |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param2         | param         | index="2" label="Interval" units="us" minValue="-1" increment="1"           |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param3         | param         | index="3" label="Req Param 3"                                               |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param4         | param         | index="4" label="Req Param 4"                                               |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param5         | param         | index="5" label="Req Param 5"                                               |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param6         | param         | index="6" label="Req Param 6"                                               |
| common.MAV_CMD.MAV_CMD_SET_MESSAGE_INTERVAL.param7         | param         | index="7" label="Response Target" minValue="0" maxValue="2" increment="1"   |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param1              | param         | index="1" label="Message ID" minValue="0" maxValue="16777215" increment="1" |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param2              | param         | index="2" label="Req Param 1"                                               |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param3              | param         | index="3" label="Req Param 2"                                               |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param4              | param         | index="4" label="Req Param 3"                                               |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param5              | param         | index="5" label="Req Param 4"                                               |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param6              | param         | index="6" label="Req Param 5"                                               |
| common.MAV_CMD.MAV_CMD_REQUEST_MESSAGE.param7              | param         | index="7" label="Response Target" minValue="0" maxValue="2" increment="1"   |
| common.MAV_RESULT.MAV_RESULT_NOT_IN_CONTROL                | wip           |                                                                             |
| common.SYS_STATUS.onboard_control_sensors_present          | print_format  | print_format="0x%04x"                                                       |
| common.SYS_STATUS.onboard_control_sensors_enabled          | print_format  | print_format="0x%04x"                                                       |
| common.SYS_STATUS.onboard_control_sensors_health           | print_format  | print_format="0x%04x"                                                       |
| common.SYS_STATUS.voltage_battery                          | invalid       | invalid="UINT16_MAX"                                                        |
| common.SYS_STATUS.current_battery                          | invalid       | invalid="-1"                                                                |
| common.SYS_STATUS.battery_remaining                        | invalid       | invalid="-1"                                                                |
| common.SYS_STATUS.onboard_control_sensors_present_extended | print_format  | print_format="0x%04x"                                                       |
| common.SYS_STATUS.onboard_control_sensors_enabled_extended | print_format  | print_format="0x%04x"                                                       |
| common.SYS_STATUS.onboard_control_sensors_health_extended  | print_format  | print_format="0x%04x"                                                       |
| common.PARAM_REQUEST_READ.param_index                      | invalid       | invalid="-1"                                                                |
| common.GPS_RAW_INT.eph                                     | invalid       | invalid="UINT16_MAX"                                                        |
| common.GPS_RAW_INT.eph                                     | multiplier    | multiplier="1E-2"                                                           |
| common.GPS_RAW_INT.epv                                     | invalid       | invalid="UINT16_MAX"                                                        |
| common.GPS_RAW_INT.epv                                     | multiplier    | multiplier="1E-2"                                                           |
| common.GPS_RAW_INT.vel                                     | invalid       | invalid="UINT16_MAX"                                                        |
| common.GPS_RAW_INT.cog                                     | invalid       | invalid="UINT16_MAX"                                                        |
| common.GPS_RAW_INT.satellites_visible                      | invalid       | invalid="UINT8_MAX"                                                         |
| common.GPS_RAW_INT.yaw                                     | invalid       | invalid="0"                                                                 |
| common.MISSION_ITEM                                        | deprecated    | since="2020-06" replaced_by="MISSION_ITEM_INT"                              |
| common.MISSION_REQUEST                                     | deprecated    | since="2020-06" replaced_by="MISSION_REQUEST_INT"                           |
| common.MISSION_SET_CURRENT                                 | superseded    | since="2022-08" replaced_by="MAV_CMD_DO_SET_MISSION_CURRENT"                |
| common.MISSION_CURRENT.total                               | invalid       | invalid="UINT16_MAX"                                                        |
| common.MISSION_CURRENT.mission_state                       | invalid       | invalid="0"                                                                 |
| common.MISSION_CURRENT.mission_mode                        | invalid       | invalid="0"                                                                 |
| common.MISSION_CURRENT.mission_id                          | invalid       | invalid="0"                                                                 |
| common.MISSION_CURRENT.fence_id                            | invalid       | invalid="0"                                                                 |
| common.MISSION_CURRENT.rally_points_id                     | invalid       | invalid="0"                                                                 |
| common.MISSION_COUNT.opaque_id                             | invalid       | invalid="0"                                                                 |
| common.MISSION_ACK.opaque_id                               | invalid       | invalid="0"                                                                 |
| common.COMMAND_INT.param1                                  | invalid       | invalid="NaN"                                                               |
| common.COMMAND_INT.param2                                  | invalid       | invalid="NaN"                                                               |
| common.COMMAND_INT.param3                                  | invalid       | invalid="NaN"                                                               |
| common.COMMAND_INT.param4                                  | invalid       | invalid="NaN"                                                               |
| common.COMMAND_INT.x                                       | invalid       | invalid="INT32_MAX"                                                         |
| common.COMMAND_INT.y                                       | invalid       | invalid="INT32_MAX"                                                         |
| common.COMMAND_INT.z                                       | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param1                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param2                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param3                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param4                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param5                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param6                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_LONG.param7                                 | invalid       | invalid="NaN"                                                               |
| common.COMMAND_ACK.progress                                | invalid       | invalid="UINT8_MAX"                                                         |
| common.COMMAND_CANCEL                                      | wip           |                                                                             |

No XML invalid value is converted to a RIDL default, absence marker or narrower
range. Numeric magic values, descriptive no-data values, NaN/INT32_MAX
optional-parameter conventions, 0/-1 disable/default-rate choices, force-arm
value 21196, ignored/reserved parameters and broadcast target IDs remain source
data/comments, not newly enforced domain assumptions. No `= 0`, first-enum init,
empty return, placeholder type or stub is added. The no-default behavior is
preserved syntactically; RIDL's own implicit signal/type initialisation is not
an upstream MAVLink startup guarantee.

Warnings are expected and are not suppressed or repaired. Candidate design
emitters and calibration checks are not run. Ordinary compilation and physical
line counting are the permitted verification for this task; The controller
reports that independent Claude review passed all 43 declarations and 20
detailed samples; the advisory corrections are recorded below.

## Verification and handoff

`cargo test -p ridl-cli --test eval_corpus --locked` passes all three tests:
ordinary no-Error compilation, physical source budgets and provenance presence.
A direct ordinary `ridl check --format json evals/corpus/mavlink` exits 0. Both
commands were rerun after the correction; the diagnostic JSON is identical to
its pre-correction result. No candidate design metric or calibration check was
run.

The final physical source count is 1,695 lines: common/enums.typl 565,
common/interactions.ridl 41, common/messages.typl 716, minimal/enums.typl 288,
minimal/interactions.ridl 5, minimal/messages.typl 28, standard/enums.typl 12
and standard/messages.typl 40. The combined corpus currently contains 4,086
physical source lines. These counts include comments and blank lines; manifests,
provenance and licence text are excluded from source budgets.

The declaration/field comparison against the actual pinned XML and frozen
inventory verifies all 27 messages, 16 enums, 186 fields and 223 selected enum
entries. It compares every decoded description text line, attribute value,
field/entry order, numeric value and extension marker. Only bitmask values are
converted to the documented bit positions.

`cmp` confirms LICENSE is byte-identical to upstream COPYING. Both SHA-256
digests are `81133ba4d3343a0e627dd0d9582134f956bc9dc82322d369c561249cc2910224`.

Ordinary diagnostics are 48 warnings and 3 info diagnostics, with no Error:
RIDL-100 (13), TYPL-102 (18), TYPL-007 (1), RIDL-112 (14), TYPL-405 (2),
TYPL-115 (1 info) and RIDL-406 (2 info). Warnings and info diagnostics are
retained. Undeclared signal timing invokes RIDL's own default timing; this is a
translation limitation and not an upstream period promise. No timing is declared
to eliminate the warnings.

The original port created the 14 allowlisted MAVLink files. This correction
changes only `common/messages.typl` and `PROVENANCE.md`. Nothing was staged,
committed or pushed. The controller reports that independent Claude review
passed all 43 declarations and 20 detailed samples.

The second generated WIP comment inside `common.CommandCancel` was removed; the
first copy before the struct and all upstream content remain. This removes one
physical source line without changing declarations, fields or output semantics.
No precise source-line references in this provenance are affected.

An independent exact-name comparison parsed the pinned `common.xml` MAV_CMD
entries and subtracted the frozen five: 170 upstream entries, five selected
entries and 165 omitted entries. The provenance list contains exactly those 165
names in upstream order, with no missing, extra or duplicate names; no list
correction was needed. Only this provenance was formatted with
`prim fmt evals/corpus/mavlink/PROVENANCE.md`; source files and other corpus
metadata were not formatted.

`prim fmt --check evals/corpus/mavlink/PROVENANCE.md` exits 0 after formatting.
The four literal header paragraphs remain separate, and all original facts and
links are retained except the corrected physical source counts and review
status.
