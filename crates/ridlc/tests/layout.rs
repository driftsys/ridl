//! A backend plugin can compute a deployment's shared-memory layouts and its
//! socket message sizes from the codegen request alone.
//!
//! `LayoutBackend` below is a test plugin. It implements
//! `ridl_ir::codegen::Backend` and reads nothing but the request it is given:
//! no raw IR and no file. Its output for `examples/cabin` is compared with a
//! fixture whose every number is derived by hand, in a comment beside it, from
//! the request (`tests/fixtures/cabin-layout.json`).
//!
//! **The layout rule** is the plugin's own; the toolchain supplies the inputs
//! and does not prescribe a layout.
//!
//! - A region is one catalog of the deployment. Its slots are the channels of
//!   that catalog in the order the deployment section lists them, which is
//!   interface number order and then member ordinal order. A slot's `member`
//!   is `<Interface>.<member>`, its `offset` is the sum of the sizes of the
//!   slots before it, and the region's `bytes` is the sum of all its slot
//!   sizes.
//! - A slot's base is a FlatBuffers size rounded up to a multiple of 8 bytes:
//!   the bounded size of the payload for a signal and an event, and the
//!   member's FlatBuffers reservation (the sum of its request and reply
//!   bounds) for a command and a query.
//! - A signal slot is its base. An event slot is its base times the channel's
//!   ring depth, which the toolchain states as the largest depth over the
//!   channel's consumer links and leaves absent when any link's depth is
//!   absent. A call slot is its base times `slots`, the largest `slots` value
//!   over the channel's consumer links that state one; a link with no `slots`
//!   is skipped.
//! - A message is one consumer link whose encoding is proto3. Its
//!   `proto3_bound` is the proto3 bounded size of the payload: the payload of
//!   a signal or an event, the request of a command, and the larger of the
//!   request and the reply of a query; `null` when any of those is not
//!   bounded. Its `max_message_bytes` is `frame_header_max_bytes +
//!   envelope_bytes + proto3_bound` from the `websocket` binding row, and
//!   `null` when that row is absent or a term is absent.
//!
//! A size the rule needs and the request does not state — an unsized
//! payload, an absent ring depth, a call none of whose links states `slots` —
//! makes `generate` return an error diagnostic and no file.
//!
//! **Limits.** The plugin is a test of the request's inputs, not a layout for
//! every deployment:
//!
//! - Every channel of a catalog goes into every region whose catalog has that
//!   name. A deployment with two regions of one catalog would get the same
//!   slots twice.
//! - A slot's `member` label names no producer. Two instances that offer one
//!   interface give two slots with the same label.

use std::fs;
use std::path::Path;

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::codegen::{self, Backend, v1};
use ridl_ir::projection::flatbuffers::{Packages, max_size};
use ridl_ir::v2;
use serde_json::{Value, json};

/// The test plugin. See the module documentation for its layout rule.
struct LayoutBackend;

impl Backend for LayoutBackend {
    fn language(&self) -> &str {
        "layout"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        match layout(request) {
            Ok(document) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    "layout.json".to_string(),
                    serde_json::to_string_pretty(&document).expect("a JSON value renders"),
                )],
                diagnostics: Vec::new(),
            },
            Err(message) => v1::CodegenResponse {
                files: Vec::new(),
                diagnostics: vec![codegen::error(message)],
            },
        }
    }
}

/// The layout of the request's package: its regions and its messages.
fn layout(request: &v1::CodegenRequest) -> Result<Value, String> {
    let model = request
        .model
        .as_ref()
        .ok_or("the request carries no model")?;
    let deployment = request
        .deployment
        .as_ref()
        .ok_or("the request carries no deployment")?;
    let catalog = model
        .name
        .as_ref()
        .map(|name| name.dotted.as_str())
        .ok_or("the model has no name")?;
    let channels: Vec<&v1::Channel> = deployment
        .channels
        .iter()
        .filter(|channel| channel.catalog == catalog)
        .collect();

    let mut regions = Vec::new();
    for region in deployment
        .regions
        .iter()
        .filter(|region| region.catalog == catalog)
    {
        let mut slots = Vec::new();
        let mut offset = 0u64;
        for channel in &channels {
            let interaction = interaction(model, channel)?;
            let size = slot_size(channel, interaction)?;
            slots.push(json!({
                "member": format!("{}.{}", channel.interface, channel.member),
                "offset": offset,
                "size": size,
            }));
            offset += size;
        }
        regions.push(json!({ "catalog": region.catalog, "slots": slots, "bytes": offset }));
    }

    let websocket = deployment
        .bindings
        .iter()
        .find(|binding| binding.name == "websocket");
    let mut messages = Vec::new();
    for channel in &channels {
        let interaction = interaction(model, channel)?;
        for consumer in &channel.consumers {
            if consumer.encoding != v1::Encoding::Proto3 as i32 {
                continue;
            }
            let bound = proto3_bound(interaction);
            let max_message_bytes = websocket.and_then(|binding| {
                Some(
                    u64::from(binding.frame_header_max_bytes?)
                        + u64::from(binding.envelope_bytes?)
                        + bound?,
                )
            });
            messages.push(json!({
                "interface": channel.interface,
                "member": channel.member,
                "consumer": format!("{}.{}", consumer.component, consumer.instance),
                "proto3_bound": bound,
                "max_message_bytes": max_message_bytes,
            }));
        }
    }
    Ok(json!({ "regions": regions, "messages": messages }))
}

/// The model's interaction a channel carries: the slot of the channel's
/// member ordinal in the interface of the channel's number.
fn interaction<'m>(
    model: &'m v1::Model,
    channel: &v1::Channel,
) -> Result<&'m v1::Interaction, String> {
    let interface = model
        .interfaces
        .iter()
        .find(|interface| interface.number == channel.interface_number)
        .ok_or_else(|| format!("the model has no interface {}", channel.interface))?;
    interface
        .slots
        .iter()
        .find(|slot| slot.ordinal == channel.member_ordinal)
        .and_then(|slot| match &slot.occupant {
            Some(v1::interaction_slot::Occupant::Interaction(interaction)) => Some(&**interaction),
            _ => None,
        })
        .ok_or_else(|| {
            format!(
                "the model has no live member {}.{}",
                channel.interface, channel.member
            )
        })
}

/// A FlatBuffers bound rounded up to a multiple of 8.
fn round8(bytes: u64) -> u64 {
    bytes.div_ceil(8) * 8
}

/// The bytes of one channel's slot under the layout rule.
fn slot_size(channel: &v1::Channel, interaction: &v1::Interaction) -> Result<u64, String> {
    let name = format!("{}.{}", channel.interface, channel.member);
    let payload_bound = |payload: &Option<v1::Payload>| {
        bounded(
            payload
                .as_ref()
                .and_then(|payload| payload.sizes.as_ref())
                .and_then(|sizes| sizes.flatbuffers.as_ref()),
        )
        .ok_or_else(|| format!("{name}: no FlatBuffers bound"))
    };
    match &interaction.shape {
        Some(v1::interaction::Shape::Signal(signal)) => Ok(round8(payload_bound(&signal.payload)?)),
        Some(v1::interaction::Shape::Event(event)) => {
            let depth = channel
                .depth
                .as_ref()
                .and_then(|depth| depth.value)
                .ok_or_else(|| format!("{name}: no ring depth"))?;
            Ok(round8(payload_bound(&event.payload)?) * u64::from(depth))
        }
        Some(v1::interaction::Shape::Command(_) | v1::interaction::Shape::Query(_)) => {
            let reservation = match interaction
                .reservation
                .as_ref()
                .and_then(|reservation| reservation.flatbuffers.as_ref())
                .and_then(|state| state.state.as_ref())
            {
                Some(v1::reservation_state::State::Bytes(bytes)) => *bytes,
                _ => return Err(format!("{name}: no FlatBuffers reservation")),
            };
            let slots = channel
                .consumers
                .iter()
                .filter_map(|consumer| consumer.slots)
                .max()
                .ok_or_else(|| format!("{name}: no slots"))?;
            Ok(round8(reservation) * u64::from(slots))
        }
        _ => Err(format!("{name}: a member this layout does not place")),
    }
}

/// The bounded value of a size state, if it is bounded.
fn bounded(state: Option<&v1::SizeState>) -> Option<u64> {
    match state?.state.as_ref()? {
        v1::size_state::State::Bounded(bytes) => Some(u64::from(*bytes)),
        _ => None,
    }
}

/// The proto3 bound of one message of a member: the payload of a signal or an
/// event, the request of a command, and the larger of the request and the
/// reply of a query.
fn proto3_bound(interaction: &v1::Interaction) -> Option<u64> {
    let proto3 = |sizes: &Option<v1::PayloadSizes>| bounded(sizes.as_ref()?.proto3.as_ref());
    let payload = |payload: &Option<v1::Payload>| proto3(&payload.as_ref()?.sizes);
    match interaction.shape.as_ref()? {
        v1::interaction::Shape::Signal(signal) => payload(&signal.payload),
        v1::interaction::Shape::Event(event) => payload(&event.payload),
        v1::interaction::Shape::Command(command) => proto3(&command.request_sizes),
        v1::interaction::Shape::Query(query) => {
            Some(proto3(&query.request_sizes)?.max(proto3(&query.reply_sizes)?))
        }
        v1::interaction::Shape::Fixed(_) => None,
    }
}

/// Every package of the workspace at `entry` with `ridl.std` last, the
/// lowered system, and the names of the deployments its source declares.
fn system_and_packages(entry: &Path) -> (Option<v2::System>, Vec<String>, Vec<v2::Package>) {
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, entry).expect("the workspace loads");
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    (output.system, output.declared_deployments, packages)
}

/// The request `ridl build` hands a backend for the first package of the
/// workspace at `entry`, with the one deployment the source declares.
fn request_for(entry: &Path, name: &str) -> (v1::CodegenRequest, Option<v2::System>) {
    let (system, declared, packages) = system_and_packages(entry);
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let deployment = ridlc::select_deployment(system.as_ref(), &declared, None, &refs)
        .expect("no name is not an error")
        .expect("the one deployment is selected");
    let request =
        ridlc::codegen_request(name, &packages[0], &refs[1..], Vec::new(), Some(deployment));
    (request, system)
}

fn cabin() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cabin")
}

/// The fixture, with its `//` derivation lines removed.
fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cabin-layout.json");
    let text = fs::read_to_string(&path).expect("the fixture is readable");
    let json: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&json).expect("the fixture is JSON once its comments are removed")
}

/// The test plugin, given only the request for `examples/cabin`'s deployment,
/// computes the region layout and the socket messages the fixture derives by
/// hand.
#[test]
fn a_layout_plugin_computes_cabin_from_the_request_alone() {
    let (request, _) = request_for(&cabin(), "veh.cabin");
    let response = LayoutBackend.generate(&request);
    assert!(
        response.diagnostics.is_empty(),
        "{:#?}",
        response.diagnostics
    );
    assert_eq!(response.files.len(), 1);
    assert_eq!(response.files[0].path, "layout.json");
    let Some(v1::generated_file::Content::Text(text)) = &response.files[0].content else {
        panic!("layout.json is a text file");
    };
    let actual: Value = serde_json::from_str(text).expect("layout.json is JSON");
    assert_eq!(actual, fixture());

    // No binding has a row yet (driftsys/ridl#265), so every
    // `max_message_bytes` in the fixture is null.
    let deployment = request.deployment.as_ref().expect("a deployment");
    assert!(deployment.bindings.is_empty());
}

/// The file `layout.json` the plugin writes for a request, as JSON.
fn layout_json(request: &v1::CodegenRequest) -> Value {
    let response = LayoutBackend.generate(request);
    assert!(
        response.diagnostics.is_empty(),
        "{:#?}",
        response.diagnostics
    );
    let [file] = response.files.as_slice() else {
        panic!("one file: {:#?}", response.files);
    };
    let Some(v1::generated_file::Content::Text(text)) = &file.content else {
        panic!("layout.json is a text file");
    };
    serde_json::from_str(text).expect("layout.json is JSON")
}

/// With a `websocket` binding row, a message's `max_message_bytes` is the
/// row's frame header bound plus its envelope plus the message's proto3
/// bound. The row is added to cabin's request by the test, because no binding
/// has a row yet (driftsys/ridl#265).
#[test]
fn a_websocket_row_adds_its_frame_header_and_envelope_to_the_proto3_bound() {
    let (mut request, _) = request_for(&cabin(), "veh.cabin");
    request
        .deployment
        .as_mut()
        .expect("a deployment")
        .bindings
        .push(v1::Binding {
            name: "websocket".to_string(),
            version: "1".to_string(),
            frame_header_max_bytes: Some(14),
            envelope_bytes: Some(2),
        });
    let actual = layout_json(&request);
    let max_message_bytes = |member: &str| {
        actual["messages"]
            .as_array()
            .expect("a list of messages")
            .iter()
            .find(|message| message["member"] == member)
            .unwrap_or_else(|| panic!("a message for {member}"))["max_message_bytes"]
            .clone()
    };
    // `Warning`'s proto3 bound is 8 (the fixture derives it): 14 + 2 + 8.
    assert_eq!(max_message_bytes("warning"), json!(24));
    // `Temperature` has no proto3 bound, so the sum has no value.
    assert_eq!(max_message_bytes("temperature"), Value::Null);
}

/// The proto3 size states of one payload: `proto3` bounded at `bound`, or
/// absent when `bound` is `None`.
fn proto3_sizes(bound: Option<u32>) -> Option<v1::PayloadSizes> {
    Some(v1::PayloadSizes {
        proto3: Some(v1::SizeState {
            state: Some(match bound {
                Some(bytes) => v1::size_state::State::Bounded(bytes),
                None => v1::size_state::State::Absent(v1::SizeAbsent::default()),
            }),
        }),
        flatbuffers: None,
    })
}

/// The proto3 bound of a command is its request's, and the bound of a query
/// is the larger of its request's and its reply's. A missing bound gives no
/// bound. Cabin's calls carry named scalars, which have no proto3 bound, so
/// these interactions are built by the test.
#[test]
fn a_call_s_proto3_bound_is_its_request_s_or_the_larger_of_a_query_s_two() {
    let command = |request: Option<u32>| v1::Interaction {
        shape: Some(v1::interaction::Shape::Command(v1::CommandShape {
            request_sizes: proto3_sizes(request),
            ..Default::default()
        })),
        ..Default::default()
    };
    let query = |request: Option<u32>, reply: Option<u32>| v1::Interaction {
        shape: Some(v1::interaction::Shape::Query(Box::new(v1::QueryShape {
            request_sizes: proto3_sizes(request),
            reply_sizes: proto3_sizes(reply),
            ..Default::default()
        }))),
        ..Default::default()
    };
    assert_eq!(proto3_bound(&command(Some(5))), Some(5));
    assert_eq!(proto3_bound(&command(None)), None);
    assert_eq!(proto3_bound(&query(Some(5), Some(9))), Some(9));
    assert_eq!(proto3_bound(&query(Some(9), Some(5))), Some(9));
    assert_eq!(proto3_bound(&query(Some(5), None)), None);
    assert_eq!(proto3_bound(&query(None, Some(9))), None);
}

/// A size the layout rule needs and the request does not state gives an
/// error diagnostic and no file. Here the test removes the ring depth of
/// cabin's event channel `Cabin.warning`.
#[test]
fn a_missing_ring_depth_gives_an_error_and_no_file() {
    let (mut request, _) = request_for(&cabin(), "veh.cabin");
    let warning = request
        .deployment
        .as_mut()
        .expect("a deployment")
        .channels
        .iter_mut()
        .find(|channel| channel.member == "warning")
        .expect("a channel for Cabin.warning");
    warning.depth = None;
    let response = LayoutBackend.generate(&request);
    assert!(response.files.is_empty(), "{:#?}", response.files);
    assert_eq!(
        response.diagnostics,
        [codegen::error("Cabin.warning: no ring depth".to_string())]
    );
}

/// The FlatBuffers bounds the fixture cites are the ones
/// `projection::flatbuffers::max_size` computes, so its derivation does not
/// rest on the lowering alone.
#[test]
fn the_fixture_cites_the_projection_s_own_bounds() {
    let (_, _, packages) = system_and_packages(&cabin());
    let refs: Vec<&v2::Package> = packages.iter().collect();
    let cabin = &packages[0];
    let scope = Packages {
        package: cabin,
        others: &refs[1..],
    };
    for (name, bound) in [
        ("Temperature", 43),
        ("Warning", 60),
        ("Level", 43),
        ("Window", 46),
        ("Average", 44),
        ("Health", 50),
    ] {
        let decl = cabin
            .decls
            .iter()
            .find(|decl| decl.name == name)
            .unwrap_or_else(|| panic!("veh.cabin declares {name}"));
        assert_eq!(max_size(scope, decl), Some(bound), "{name}");
    }
}

/// Two builds of one workspace hand a plugin the same request bytes.
#[test]
fn two_builds_write_the_same_request() {
    let render = || {
        let (request, _) = request_for(&cabin(), "veh.cabin");
        codegen::request_to_json(&request).expect("the request renders")
    };
    assert_eq!(render(), render());
}

/// Each channel has one consumer link per link of the system IR that reaches
/// the channel's interface from the channel's producer.
#[test]
fn each_channel_has_a_consumer_per_link_of_its_interface() {
    let (request, system) = request_for(&cabin(), "veh.cabin");
    let section = request.deployment.as_ref().expect("a deployment");
    let system = system.expect("cabin declares a system");
    let placed = system
        .deployments
        .iter()
        .find(|deployment| deployment.name == section.name)
        .expect("the system IR holds the deployment");
    assert_eq!(section.channels.len(), 5);
    for channel in &section.channels {
        let producer = channel.producer.as_ref().expect("a producer");
        let links = placed
            .links
            .iter()
            .filter(|link| {
                link.interface.as_ref().is_some_and(|interface| {
                    interface.catalog == channel.catalog && interface.name == channel.interface
                }) && link.producer.as_ref().is_some_and(|end| {
                    end.component == producer.component && end.instance == producer.instance
                })
            })
            .count();
        assert_eq!(
            channel.consumers.len(),
            links,
            "{}.{}",
            channel.interface,
            channel.member
        );
    }
}

/// A sizing key declared in an rsdl source reaches the consumer links of the
/// request with the declared source: `depth` and `budget` from the
/// `deployment` declaration, and `slots` from the deployment for one consumer
/// and from its placement line for the other.
#[test]
fn declared_sizing_keys_reach_the_request() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let write = |name: &str, text: &str| {
        fs::write(dir.path().join(name), text).expect("the file is written");
    };
    write(
        "ridl.toml",
        "[package]\nname = \"veh.sized\"\nversion = \"1.0.0\"\n",
    );
    write(
        "sized.ridl",
        "package veh.sized

/// A level.
type Level: integer [0..100]

/// The interface.
interface Panel {
  /// An event.
  event changed: Level @[100ms..1s]
  /// A command.
  command set(level: Level) @[..50ms]
}

/// The service.
service veh.sized.panel: Panel
",
    );
    write(
        "system.rsdl",
        "package veh.sized

/// The system.
system Car { Head, Near, Far }

/// The provider.
component Head { offers veh.sized.panel }
/// A consumer with no placement key.
component Near { requires Panel }
/// A consumer whose placement line declares `slots`.
component Far { requires Panel }

/// The deployment.
deployment Rig for Car [ depth = 12, slots = 4, budget = 4096 ] {
  machine Main { Head, Near, Far [ slots = 8 ] }
}
",
    );

    let (request, _) = request_for(dir.path(), "veh.sized");
    let section = request.deployment.as_ref().expect("a deployment");
    assert_eq!(section.name, "Rig");
    let declared = v1::ValueSource::Declared as i32;
    let channel = |member: &str| {
        section
            .channels
            .iter()
            .find(|channel| channel.member == member)
            .unwrap_or_else(|| panic!("a channel for {member}"))
    };
    let consumers = |member: &str| -> Vec<(String, &v1::Consumer)> {
        channel(member)
            .consumers
            .iter()
            .map(|consumer| (consumer.component.clone(), consumer))
            .collect()
    };

    let event = channel("changed");
    let ring = event.depth.as_ref().expect("a ring depth");
    assert_eq!((ring.value, ring.source), (Some(12), declared));
    for (component, consumer) in consumers("changed") {
        let depth = consumer.depth.as_ref().expect("a link depth");
        assert_eq!(
            (depth.value, depth.source),
            (Some(12), declared),
            "{component}"
        );
    }

    let calls = consumers("set");
    let names: Vec<&str> = calls.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["veh.sized.Far", "veh.sized.Near"]);
    for (component, consumer) in &calls {
        let slots = if component == "veh.sized.Far" { 8 } else { 4 };
        assert_eq!(
            (consumer.slots, consumer.slots_source),
            (Some(slots), declared),
            "{component}"
        );
        assert_eq!(
            (consumer.budget, consumer.budget_source),
            (Some(4096), declared),
            "{component}"
        );
    }
}
