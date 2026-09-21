///The payload encoding this package's generated interaction face encodes and verifies over, and the one the `Payload` implementations below implement: FlatBuffers (ADR-0019, ADR-0020 decision 2). It is named once here rather than repeated at every buffer and every `Ref` the face builds, so the package's encoding is one line to read and one line to change.
pub type Wire = ::ridl_rt::encoding::FlatBuffers;
/// Cabin temperature, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Temperature(i64);
impl Temperature {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        Self::check(&value)?;
        ::core::result::Result::Ok(Self::new_unchecked(value))
    }
    /// Checks `value` against this type's typl constraints, without
    /// constructing it. `pub(crate)` rather than `pub`: a caller
    /// outside `new` is a function generated into this crate — since
    /// driftsys/ridl#467 that includes the codec of *another* package
    /// of the same build, which reaches this type through the module
    /// tree and so cannot see a private item here. The emitted crate
    /// is one crate per build, so `pub(crate)` reaches every such
    /// caller while adding nothing to the crate's public surface.
    /// Whether this becomes `pub` is Epic 10's call, still open.
    /// `new` is the composition of this and `new_unchecked`.
    pub(crate) fn check(
        value: &i64,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
        let value = *value;
        if value < -40 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Temperature",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 85 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Temperature",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        ::core::result::Result::Ok(())
    }
    /// Constructs the value without checking its constraints.
    ///
    /// Safe: nothing here relies on the invariant for memory
    /// soundness. Use it only for a value already known to satisfy
    /// the contract.
    pub const fn new_unchecked(value: i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<i64> for Temperature {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Temperature> for i64 {
    fn from(value: Temperature) -> Self {
        value.0
    }
}
impl Default for Temperature {
    fn default() -> Self {
        Temperature::new_unchecked(0)
    }
}
/// A control level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Level(i64);
impl Level {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        Self::check(&value)?;
        ::core::result::Result::Ok(Self::new_unchecked(value))
    }
    /// Checks `value` against this type's typl constraints, without
    /// constructing it. `pub(crate)` rather than `pub`: a caller
    /// outside `new` is a function generated into this crate — since
    /// driftsys/ridl#467 that includes the codec of *another* package
    /// of the same build, which reaches this type through the module
    /// tree and so cannot see a private item here. The emitted crate
    /// is one crate per build, so `pub(crate)` reaches every such
    /// caller while adding nothing to the crate's public surface.
    /// Whether this becomes `pub` is Epic 10's call, still open.
    /// `new` is the composition of this and `new_unchecked`.
    pub(crate) fn check(
        value: &i64,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
        let value = *value;
        if value < 0 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Level",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 100 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Level",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        ::core::result::Result::Ok(())
    }
    /// Constructs the value without checking its constraints.
    ///
    /// Safe: nothing here relies on the invariant for memory
    /// soundness. Use it only for a value already known to satisfy
    /// the contract.
    pub const fn new_unchecked(value: i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<i64> for Level {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Level> for i64 {
    fn from(value: Level) -> Self {
        value.0
    }
}
impl Default for Level {
    fn default() -> Self {
        Level::new_unchecked(0)
    }
}
/// A window length, in samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Window(i64);
impl Window {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        Self::check(&value)?;
        ::core::result::Result::Ok(Self::new_unchecked(value))
    }
    /// Checks `value` against this type's typl constraints, without
    /// constructing it. `pub(crate)` rather than `pub`: a caller
    /// outside `new` is a function generated into this crate — since
    /// driftsys/ridl#467 that includes the codec of *another* package
    /// of the same build, which reaches this type through the module
    /// tree and so cannot see a private item here. The emitted crate
    /// is one crate per build, so `pub(crate)` reaches every such
    /// caller while adding nothing to the crate's public surface.
    /// Whether this becomes `pub` is Epic 10's call, still open.
    /// `new` is the composition of this and `new_unchecked`.
    pub(crate) fn check(
        value: &i64,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
        let value = *value;
        if value < 0 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Window",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 100000 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Window",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        ::core::result::Result::Ok(())
    }
    /// Constructs the value without checking its constraints.
    ///
    /// Safe: nothing here relies on the invariant for memory
    /// soundness. Use it only for a value already known to satisfy
    /// the contract.
    pub const fn new_unchecked(value: i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<i64> for Window {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Window> for i64 {
    fn from(value: Window) -> Self {
        value.0
    }
}
impl Default for Window {
    fn default() -> Self {
        Window::new_unchecked(0)
    }
}
/// An averaged reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Average(i64);
impl Average {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        Self::check(&value)?;
        ::core::result::Result::Ok(Self::new_unchecked(value))
    }
    /// Checks `value` against this type's typl constraints, without
    /// constructing it. `pub(crate)` rather than `pub`: a caller
    /// outside `new` is a function generated into this crate — since
    /// driftsys/ridl#467 that includes the codec of *another* package
    /// of the same build, which reaches this type through the module
    /// tree and so cannot see a private item here. The emitted crate
    /// is one crate per build, so `pub(crate)` reaches every such
    /// caller while adding nothing to the crate's public surface.
    /// Whether this becomes `pub` is Epic 10's call, still open.
    /// `new` is the composition of this and `new_unchecked`.
    pub(crate) fn check(
        value: &i64,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
        let value = *value;
        if value < 0 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Average",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 1000 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: "Average",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        ::core::result::Result::Ok(())
    }
    /// Constructs the value without checking its constraints.
    ///
    /// Safe: nothing here relies on the invariant for memory
    /// soundness. Use it only for a value already known to satisfy
    /// the contract.
    pub const fn new_unchecked(value: i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<i64> for Average {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Average> for i64 {
    fn from(value: Average) -> Self {
        value.0
    }
}
impl Default for Average {
    fn default() -> Self {
        Average::new_unchecked(0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum Health {
    OK = 0,
    WARN = 1,
    FAIL = 2,
}
impl ::core::convert::TryFrom<i64> for Health {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(value: i64) -> ::core::result::Result<Self, Self::Error> {
        match value {
            0 => ::core::result::Result::Ok(Self::OK),
            1 => ::core::result::Result::Ok(Self::WARN),
            2 => ::core::result::Result::Ok(Self::FAIL),
            _ => {
                ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: "Health",
                    rule: ::ridl_rt::payload::Rule::Variant,
                })
            }
        }
    }
}
impl ::core::convert::From<Health> for i64 {
    fn from(value: Health) -> Self {
        value as i64
    }
}
impl Default for Health {
    fn default() -> Self {
        Health::OK
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Warning {
    pub code: Level,
    pub health: Health,
}
impl Default for Warning {
    fn default() -> Self {
        Warning {
            code: Level::default(),
            health: Health::default(),
        }
    }
}
/// An accessor over FlatBuffers bytes `Temperature`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one required `value` field. A
/// buffer carrying no slot for it is `MissingRequired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct TemperatureFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> TemperatureFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// The value the box carries. `Temperature` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Temperature {
        __ridl_fb_decode_temperature(self.buf, self.table)
    }
}
/// Writes `Temperature` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_temperature(
    value: &Temperature,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    ::core::result::Result::Ok({
        let __box = [
            ::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 4u16,
                value: {
                    let __s = *value;
                    ::ridl_rt::flatbuffers::Field::I8(__s.get() as i8)
                },
            },
        ];
        builder.push_table(5usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_temperature(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_i8(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Temperature::check(&(i64::from(__raw)))
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_temperature(buf: &[u8], table: usize) -> Temperature {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Temperature::new_unchecked(
            i64::from(::ridl_rt::flatbuffers::read_i8(buf, __p).unwrap_or(0i8)),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Temperature {
    /// The largest FlatBuffers buffer any legal `Temperature` encodes to: 43 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 43usize;
    type View<'a> = TemperatureFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_temperature(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: TemperatureFbView {
                buf: bytes,
                table,
            },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_temperature(buf, table)?;
        ::core::result::Result::Ok(TemperatureFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_temperature(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Level`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one required `value` field. A
/// buffer carrying no slot for it is `MissingRequired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct LevelFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> LevelFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// The value the box carries. `Level` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Level {
        __ridl_fb_decode_level(self.buf, self.table)
    }
}
/// Writes `Level` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_level(
    value: &Level,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    ::core::result::Result::Ok({
        let __box = [
            ::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 4u16,
                value: {
                    let __s = *value;
                    ::ridl_rt::flatbuffers::Field::U8(__s.get() as u8)
                },
            },
        ];
        builder.push_table(5usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_level(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Level::check(&(i64::from(__raw)))
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_level(buf: &[u8], table: usize) -> Level {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Level::new_unchecked(
            i64::from(::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8)),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Level {
    /// The largest FlatBuffers buffer any legal `Level` encodes to: 43 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 43usize;
    type View<'a> = LevelFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_level(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LevelFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_level(buf, table)?;
        ::core::result::Result::Ok(LevelFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_level(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Window`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one required `value` field. A
/// buffer carrying no slot for it is `MissingRequired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct WindowFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> WindowFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// The value the box carries. `Window` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Window {
        __ridl_fb_decode_window(self.buf, self.table)
    }
}
/// Writes `Window` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_window(
    value: &Window,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    ::core::result::Result::Ok({
        let __box = [
            ::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 4u16,
                value: {
                    let __s = *value;
                    ::ridl_rt::flatbuffers::Field::U32(__s.get() as u32)
                },
            },
        ];
        builder.push_table(8usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_window(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_u32(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Window::check(&(i64::from(__raw)))
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_window(buf: &[u8], table: usize) -> Window {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Window::new_unchecked(
            i64::from(::ridl_rt::flatbuffers::read_u32(buf, __p).unwrap_or(0u32)),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Window {
    /// The largest FlatBuffers buffer any legal `Window` encodes to: 46 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 46usize;
    type View<'a> = WindowFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_window(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: WindowFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_window(buf, table)?;
        ::core::result::Result::Ok(WindowFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_window(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Average`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one required `value` field. A
/// buffer carrying no slot for it is `MissingRequired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct AverageFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> AverageFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// The value the box carries. `Average` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Average {
        __ridl_fb_decode_average(self.buf, self.table)
    }
}
/// Writes `Average` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_average(
    value: &Average,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    ::core::result::Result::Ok({
        let __box = [
            ::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 4u16,
                value: {
                    let __s = *value;
                    ::ridl_rt::flatbuffers::Field::U16(__s.get() as u16)
                },
            },
        ];
        builder.push_table(6usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_average(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 2usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_u16(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Average::check(&(i64::from(__raw)))
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_average(buf: &[u8], table: usize) -> Average {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 2usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Average::new_unchecked(
            i64::from(::ridl_rt::flatbuffers::read_u16(buf, __p).unwrap_or(0u16)),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Average {
    /// The largest FlatBuffers buffer any legal `Average` encodes to: 44 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 44usize;
    type View<'a> = AverageFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_average(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: AverageFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_average(buf, table)?;
        ::core::result::Result::Ok(AverageFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_average(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Health`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one required `value` field. A
/// buffer carrying no slot for it is `MissingRequired`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct HealthFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> HealthFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// The value the box carries. `Health` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Health {
        __ridl_fb_decode_health(self.buf, self.table)
    }
}
/// Writes `Health` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_health(
    value: &Health,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    ::core::result::Result::Ok({
        let __box = [
            ::ridl_rt::flatbuffers::TableField {
                slot: 0u16,
                offset: 8u16,
                value: {
                    let __s = *value;
                    ::ridl_rt::flatbuffers::Field::I64(i64::from(__s))
                },
            },
        ];
        builder.push_table(16usize, 8usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_health(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 8usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            <Health as ::core::convert::TryFrom<i64>>::try_from(__raw)
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_health(buf: &[u8], table: usize) -> Health {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 8usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        <Health as ::core::convert::TryFrom<
            i64,
        >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
            .unwrap_or(Health::OK)
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Health {
    /// The largest FlatBuffers buffer any legal `Health` encodes to: 50 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 50usize;
    type View<'a> = HealthFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_health(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: HealthFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_health(buf, table)?;
        ::core::result::Result::Ok(HealthFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_health(__view.buf, __view.table)
    }
}
/// A zero-copy accessor over FlatBuffers bytes `Warning`'s `verify` accepted.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct WarningFbView<'a> {
    pub(crate) buf: &'a [u8],
    pub(crate) table: usize,
}
#[allow(deprecated)]
impl<'a> WarningFbView<'a> {
    /// The verified bytes this view reads.
    pub fn bytes(&self) -> &'a [u8] {
        self.buf
    }
    /// Reads `Warning`'s `code` field in place.
    pub fn code(&self) -> Level {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 0u16, 1usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Level::new_unchecked(
            i64::from(::ridl_rt::flatbuffers::read_u8(self.buf, __p).unwrap_or(0u8)),
        )
    }
    /// Reads `Warning`'s `health` field in place.
    pub fn health(&self) -> Health {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 8usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        <Health as ::core::convert::TryFrom<
            i64,
        >>::try_from(::ridl_rt::flatbuffers::read_i64(self.buf, __p).unwrap_or(0i64))
            .unwrap_or(Health::OK)
    }
}
/// Writes `Warning` as a FlatBuffers table and returns its position.
#[allow(deprecated)]
pub(crate) fn __ridl_fb_encode_warning(
    value: &Warning,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    let mut __fields = [::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: ::ridl_rt::flatbuffers::Field::Bool(false),
    }; 2usize];
    let mut __n = 0usize;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: {
            let __s = value.code;
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as u8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 8u16,
        value: {
            let __s = value.health;
            ::ridl_rt::flatbuffers::Field::I64(i64::from(__s))
        },
    };
    __n += 1;
    builder.push_table(16usize, 8usize, 2u16, &__fields[..__n])
}
/// Checks the FlatBuffers table at `table` against `Warning`'s shape.
///
/// A total walk of the type's own shape: the structure in full,
/// an enum and an enum-set discriminant, a collection's declared
/// element count, and every **named** scalar's own declared
/// range, length and pattern, checked over a borrow (`check`,
/// beside `new` on the type itself) against its declared range,
/// length and pattern.
///
/// This does not make every value `decode` builds satisfy every
/// typl constraint. Three gaps:
///
/// - a `step` constraint is checked nowhere — not by `new`, by
///   `check`, or here (driftsys/ridl#469);
/// - the pattern check is behind the `validate-pattern` feature,
///   so a value violating a `match` pattern passes when that
///   feature is off;
/// - an anonymous inline constraint (a field's own `[..]` or
///   `match` written at the field, not through a named scalar)
///   is not checked here at all (driftsys/ridl#469).
#[allow(deprecated)]
pub(crate) fn __ridl_fb_verify_warning(
    buf: &[u8],
    table: usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Level::check(&(i64::from(__raw)))
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 8usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            <Health as ::core::convert::TryFrom<i64>>::try_from(__raw)
                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
/// Builds `Warning` from the FlatBuffers table at `table`.
///
/// It cannot fail. A read that could is discharged with the
/// neutral value of its own type — zero, the empty string or
/// collection, the first declared enum variant — and `verify` is
/// what makes those branches unreachable. A named scalar is
/// built with its unchecked constructor (`new_unchecked`) over a
/// value `verify` has already range-checked (`check`), so this
/// never re-checks and never fails.
#[allow(deprecated)]
pub(crate) fn __ridl_fb_decode_warning(buf: &[u8], table: usize) -> Warning {
    Warning {
        code: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            Level::new_unchecked(
                i64::from(::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8)),
            )
        },
        health: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 8usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            <Health as ::core::convert::TryFrom<
                i64,
            >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
                .unwrap_or(Health::OK)
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Warning {
    /// The largest FlatBuffers buffer any legal `Warning` encodes to: 60 bytes.
    ///
    /// `ridl_ir::projection::flatbuffers::max_size` computed it,
    /// which is the one implementation of the bound (design note
    /// D-6): each table is charged its `soffset`, its inline
    /// fields, its vtable and one alignment event per slot; a
    /// string four bytes per declared character plus a
    /// terminator; a collection its declared maximum. It is a
    /// literal rather than an expression over the field types
    /// because that slack is not expressible in Rust's type
    /// system.
    const MAX_SIZE: usize = 60usize;
    type View<'a> = WarningFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, Self::View<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_warning(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: WarningFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[u8],
    ) -> ::core::result::Result<Self::View<'_>, ::ridl_rt::payload::VerifyError> {
        if buf.len()
            > <Self as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::TooLarge,
                ),
            );
        }
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_warning(buf, table)?;
        ::core::result::Result::Ok(WarningFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_warning(__view.buf, __view.table)
    }
}
/**Descriptor for interface `Cabin`.

`CATALOG.hash` is the placeholder `CatalogHash([0u8; 32])` until E16.2 (driftsys/ridl#378) computes the real catalog hash.

Every `PayloadInfo.max_size` field is `None`. The reading here is that the toolchain cannot size the payload yet, not that the encoding cannot carry it. `ridl-rt`'s own doc comment states the other reading; E16.2 reconciles the two.*/
pub struct Cabin;
impl ::ridl_rt::contract::Interface for Cabin {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([0u8; 32]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(1);
    const PROVISIONAL: bool = true;
    const NAME: &'static str = "Cabin";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "temperature",
            timing: Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: Some(::ridl_rt::sample::Duration(10000)),
                max: Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Temperature",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Event,
            name: "warning",
            timing: Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: Some(::ridl_rt::sample::Duration(100000)),
                max: Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Warning",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(3),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "setLevel",
            timing: Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: None,
                max: Some(::ridl_rt::sample::Duration(50000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(4),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "average",
            timing: Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: None,
                max: Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Window",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
            ],
        },
    ];
}
impl Cabin {
    ///The largest argument or reply payload of this interface, over `<T as Payload<Wire>>::MAX_SIZE`. A dispatch buffer must be at least this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<Wire>>::MAX_SIZE,
            <Window as ::ridl_rt::payload::Payload<Wire>>::MAX_SIZE,
            <Average as ::ridl_rt::payload::Payload<Wire>>::MAX_SIZE,
        ];
        let mut max = 0usize;
        let mut index = 0usize;
        while index < sizes.len() {
            if sizes[index] > max {
                max = sizes[index];
            }
            index += 1;
        }
        max
    };
    ///The largest event payload of this interface, over `<T as Payload<Wire>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: usize = {
        let sizes = [<Warning as ::ridl_rt::payload::Payload<Wire>>::MAX_SIZE];
        let mut max = 0usize;
        let mut index = 0usize;
        while index < sizes.len() {
            if sizes[index] > max {
                max = sizes[index];
            }
            index += 1;
        }
        max
    };
}
pub struct CabinTemperature;
impl ::ridl_rt::contract::Interaction for CabinTemperature {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Signal for CabinTemperature {
    type Payload = Temperature;
    fn init() -> Self::Payload {
        Temperature::default()
    }
}
pub struct CabinWarning;
impl ::ridl_rt::contract::Interaction for CabinWarning {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[1];
}
impl ::ridl_rt::contract::Event for CabinWarning {
    type Payload = Warning;
}
pub struct CabinSetLevel;
impl ::ridl_rt::contract::Interaction for CabinSetLevel {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[2];
}
impl ::ridl_rt::contract::Command for CabinSetLevel {
    type Args = Level;
    ///Evaluates the `require` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn require(args: &Self::Args) -> ::core::result::Result<(), ()> {
        if args.0 < 100 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
}
pub struct CabinAverage;
impl ::ridl_rt::contract::Interaction for CabinAverage {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[3];
}
impl ::ridl_rt::contract::Query for CabinAverage {
    type Args = Window;
    type Reply = Average;
    ///Evaluates the `require` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn require(args: &Self::Args) -> ::core::result::Result<(), ()> {
        if args.0 > 0 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
    ///Evaluates the `ensure` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn ensure(
        _args: &Self::Args,
        reply: &Self::Reply,
    ) -> ::core::result::Result<(), ()> {
        if reply.0 >= 0 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
}
/**Descriptor for interface `Horn`.

`CATALOG.hash` is the placeholder `CatalogHash([0u8; 32])` until E16.2 (driftsys/ridl#378) computes the real catalog hash.

Every `PayloadInfo.max_size` field is `None`. The reading here is that the toolchain cannot size the payload yet, not that the encoding cannot carry it. `ridl-rt`'s own doc comment states the other reading; E16.2 reconciles the two.*/
pub struct Horn;
impl ::ridl_rt::contract::Interface for Horn {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([0u8; 32]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(2);
    const PROVISIONAL: bool = true;
    const NAME: &'static str = "Horn";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "active",
            timing: Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: Some(::ridl_rt::sample::Duration(10000)),
                max: Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Health",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: None,
                        flatbuffers: None,
                        repr_c: None,
                    },
                },
            ],
        },
    ];
}
impl Horn {
    ///The largest argument or reply payload of this interface, over `<T as Payload<Wire>>::MAX_SIZE`. A dispatch buffer must be at least this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: usize = 0usize;
    ///The largest event payload of this interface, over `<T as Payload<Wire>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: usize = 0usize;
}
pub struct HornActive;
impl ::ridl_rt::contract::Interaction for HornActive {
    type Iface = Horn;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Horn as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Signal for HornActive {
    type Payload = Health;
    fn init() -> Self::Payload {
        Health::default()
    }
}
///The generated interaction face of interface `Cabin`.
pub mod cabin {
    ///Identifies one sent command `setLevel` to its caller. It is returned by the send method and accepted by that call's own outcome method, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct SetLevelCorrelation(pub ::ridl_rt::port::Correlation);
    ///Identifies one sent query `average` to its caller. It is returned by the send method and accepted by that call's own outcome method, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct AverageCorrelation(pub ::ridl_rt::port::Correlation);
    ///The consumer face of interface `Cabin`, generic over exactly the ports the interface's interactions need.
    pub struct Client<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller,
    > Client<P> {
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        pub fn new(port: P) -> Self {
            Client { port }
        }
        ///Reads signal `temperature` and returns its value with the provenance, the freshness and the envelope the runtime resolved. A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn temperature(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Temperature>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; <super::Temperature as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let raw = self
                .port
                .read(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    &mut buf,
                )?;
            match ::ridl_rt::payload::Ref::<
                super::Temperature,
                super::Wire,
            >::verify(&buf[..raw.len]) {
                Ok(checked) => {
                    Ok(::ridl_rt::sample::Sample {
                        value: checked.decode(),
                        provenance: raw.provenance,
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    })
                }
                Err(error) => {
                    Ok(::ridl_rt::sample::Sample {
                        value: <super::CabinTemperature as ::ridl_rt::contract::Signal>::init(),
                        provenance: ::ridl_rt::sample::Provenance::Invalid(
                            ::ridl_rt::sample::Cause::Detected(
                                match error {
                                    ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                        ::ridl_rt::sample::Detection::InvalidValue(violation)
                                    }
                                    _ => ::ridl_rt::sample::Detection::Corrupt,
                                },
                            ),
                        ),
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    })
                }
            }
        }
        ///Starts delivery of event `warning`.
        pub fn subscribe_warning(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
            self.port
                .subscribe(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    &[::ridl_rt::contract::Ordinal(2u32)],
                )
        }
        /**Takes the next occurrence of any subscribed event of interface `Cabin`, routed to its variant by ordinal. `Ok(None)` when none is waiting. One method serves every event, because the payload type is not known until the occurrence's ordinal is read.

The interface number is checked before the ordinal, for the reason `dispatch` checks it: a port is attached to a whole catalog, ordinals restart at 1 in each interface, and an occurrence of a sibling interface at the same ordinal would otherwise be decoded as this interface's payload. Such an occurrence is reported as `Contract::UnknownInteraction`; `EventSource::next` has already consumed it, so this face cannot hand it back to the interface it belongs to. Subscribe on a port this interface owns.*/
        pub fn next_event(
            &mut self,
        ) -> ::core::result::Result<
            ::core::option::Option<Event>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; super::Cabin::EVENT_SOURCE_BUFFER_SIZE];
            let Some(occurrence) = self.port.next(&mut buf)? else {
                return Ok(None);
            };
            if occurrence.iface
                != <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER
            {
                return Err(
                    ::ridl_rt::port::ReadError::Contract(
                        ::ridl_rt::error::Contract::UnknownInteraction,
                    ),
                );
            }
            match occurrence.ord {
                ::ridl_rt::contract::Ordinal(2u32) => {
                    Ok(
                        Some(
                            Event::Warning(::ridl_rt::sample::Occurrence {
                                payload: match ::ridl_rt::payload::Ref::<
                                    super::Warning,
                                    super::Wire,
                                >::verify(&buf[..occurrence.len]) {
                                    Ok(checked) => Ok(checked.decode()),
                                    Err(
                                        ::ridl_rt::payload::VerifyError::Contract(violation),
                                    ) => {
                                        Err(::ridl_rt::sample::Detection::InvalidValue(violation))
                                    }
                                    Err(_) => Err(::ridl_rt::sample::Detection::Corrupt),
                                },
                                envelope: occurrence.envelope,
                            }),
                        ),
                    )
                }
                _ => {
                    Err(
                        ::ridl_rt::port::ReadError::Contract(
                            ::ridl_rt::error::Contract::UnknownInteraction,
                        ),
                    )
                }
            }
        }
        ///Sends command `setLevel` and returns the correlation of its outcome. A `require` clause that fails is reported as `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent.
        pub fn set_level(
            &mut self,
            level: super::Level,
        ) -> ::core::result::Result<SetLevelCorrelation, ::ridl_rt::port::SendError> {
            <super::CabinSetLevel as ::ridl_rt::contract::Command>::require(&level)
                .map_err(|()| {
                    ::ridl_rt::port::SendError::Contract(
                        ::ridl_rt::error::Contract::PreconditionFailed,
                    )
                })?;
            let mut buf = [0u8; <super::Level as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Level,
                super::Wire,
            >::encode(&level, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Level as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Level` failed"),
            };
            self.port
                .command(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(3u32),
                    bytes,
                )
                .map(SetLevelCorrelation)
        }
        ///Sends query `average` and returns the correlation of its outcome. A `require` clause that fails is reported as `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent.
        pub fn average(
            &mut self,
            window: super::Window,
        ) -> ::core::result::Result<AverageCorrelation, ::ridl_rt::port::SendError> {
            <super::CabinAverage as ::ridl_rt::contract::Query>::require(&window)
                .map_err(|()| {
                    ::ridl_rt::port::SendError::Contract(
                        ::ridl_rt::error::Contract::PreconditionFailed,
                    )
                })?;
            let mut buf = [0u8; <super::Window as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Window,
                super::Wire,
            >::encode(&window, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Window` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Window as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Window` failed"),
            };
            self.port
                .query(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(4u32),
                    bytes,
                )
                .map(AverageCorrelation)
        }
        ///Takes query `average`'s reply once it is known, or `Ok(None)` while it is not. It does not wait.
        pub fn average_reply(
            &mut self,
            correlation: AverageCorrelation,
        ) -> ::core::result::Result<
            ::core::option::Option<
                ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
            >,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; <super::Average as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            match self.port.reply(correlation.0, &mut buf)? {
                None => Ok(None),
                Some(Err(error)) => Ok(Some(Err(error))),
                Some(Ok(len)) => {
                    Ok(
                        Some(
                            match ::ridl_rt::payload::Ref::<
                                super::Average,
                                super::Wire,
                            >::verify(&buf[..len]) {
                                Ok(checked) => Ok(checked.decode()),
                                Err(
                                    ::ridl_rt::payload::VerifyError::Contract(violation),
                                ) => {
                                    Err(
                                        ::ridl_rt::error::CallError::Contract(
                                            ::ridl_rt::error::Contract::InvalidValue(violation),
                                        ),
                                    )
                                }
                                Err(_) => {
                                    Err(
                                        ::ridl_rt::error::CallError::Transport(
                                            ::ridl_rt::error::Transport::Corrupt,
                                        ),
                                    )
                                }
                            },
                        ),
                    )
                }
            }
        }
        ///Takes command `setLevel`'s delivery acknowledgment once it is known, or `None` while it is not. It does not wait.
        pub fn set_level_ack(
            &mut self,
            correlation: SetLevelCorrelation,
        ) -> ::core::option::Option<
            ::core::result::Result<(), ::ridl_rt::error::CallError>,
        > {
            self.port.ack(correlation.0)
        }
    }
    ///One occurrence of an event of interface `Cabin`.
    pub enum Event {
        ///An occurrence of event `warning`.
        Warning(::ridl_rt::sample::Occurrence<super::Warning>),
    }
    ///The provider face of interface `Cabin`'s signals and events.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink> Publisher<W> {
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        pub fn new(port: W) -> Self {
            Publisher { port }
        }
        ///Stages a new value for signal `temperature`. It is published by `commit`.
        pub fn temperature(
            &mut self,
            value: super::Temperature,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = [0u8; <super::Temperature as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Temperature,
                super::Wire,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Temperature` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Temperature as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Temperature` failed"),
            };
            self.port
                .set(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    bytes,
                )
        }
        ///Stages the invalid state for signal `temperature`, with `Cause::Declared`. It is published by `commit`.
        pub fn invalidate_temperature(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                )
        }
        ///Raises one occurrence of event `warning`.
        pub fn warning(
            &mut self,
            value: super::Warning,
        ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
            let mut buf = [0u8; <super::Warning as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Warning,
                super::Wire,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Warning` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Warning as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Warning` failed"),
            };
            self.port
                .raise(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(2u32),
                    bytes,
                )
        }
        /// Publishes every staged signal change.
        pub fn commit(&mut self) {
            self.port.commit()
        }
    }
    /**What an application implements to serve interface `Cabin`'s calls.

An argument is taken by reference because `dispatch` reads it again when it evaluates a query's `ensure` clauses, and the generated payload types implement neither `Copy` nor `Clone`.*/
    pub trait Provider {
        ///Serves command `setLevel`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        fn set_level(&mut self, level: &super::Level);
        ///Serves query `average`. A reply that breaks an `ensure` clause is discarded by `dispatch`, which settles `ContractBroken` instead.
        fn average(&mut self, window: &super::Window) -> super::Average;
    }
    /**Settles every claim of interface `Cabin` that is waiting, and returns how many were settled.

It does not wait: it makes one pass over the claims the handler already has and returns. The loop that calls it belongs to the application or to the runtime.

`buf` is caller-owned and must be at least `Cabin::MAX_BUFFER_SIZE` bytes, because a reply is encoded into the same buffer as the arguments. A shorter buffer returns `0` without consuming a claim, so the caller can retry with a correctly sized one.

Every claim that is taken is settled, including one whose interface number or ordinal this interface does not recognise, which settles `Contract::UnknownInteraction`. A claim is counted only once `Handler::settle` has accepted it; a `SettleError` is left to the handler, which already owns that claim's settlement, and the pass continues with the next claim.

A command is settled `Ok(&[])` once its arguments and its `require` clauses pass and **before** the application's method runs, because a command's acknowledgment is a delivery acknowledgment and not a completion one (ridl §6.1, and `Handler`'s own contract). A query is settled after the application returns, because its settlement carries the reply.*/
    pub fn dispatch<H, P>(h: &mut H, p: &mut P, buf: &mut [u8]) -> usize
    where
        H: ::ridl_rt::port::Handler,
        P: Provider,
    {
        if buf.len() < super::Cabin::MAX_BUFFER_SIZE {
            return 0;
        }
        let mut settled = 0usize;
        loop {
            let Ok(Some(claim)) = h.next_claim(buf) else {
                return settled;
            };
            let settlement = if claim.iface
                != <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER
            {
                h.settle(
                    claim.id,
                    Err(
                        ::ridl_rt::error::CallError::Contract(
                            ::ridl_rt::error::Contract::UnknownInteraction,
                        ),
                    ),
                )
            } else {
                match claim.ord {
                    ::ridl_rt::contract::Ordinal(3u32) => {
                        let decoded = match ::ridl_rt::payload::Ref::<
                            super::Level,
                            super::Wire,
                        >::verify(&buf[..claim.len]) {
                            Ok(checked) => Ok(checked.decode()),
                            Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                Err(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                )
                            }
                            Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                                Err(
                                    ::ridl_rt::error::CallError::Contract(
                                        ::ridl_rt::error::Contract::InvalidValue(violation),
                                    ),
                                )
                            }
                            Err(_) => {
                                Err(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                )
                            }
                        };
                        match decoded {
                            Err(error) => h.settle(claim.id, Err(error)),
                            Ok(level) => {
                                match <super::CabinSetLevel as ::ridl_rt::contract::Command>::require(
                                    &level,
                                ) {
                                    Err(()) => {
                                        h.settle(
                                            claim.id,
                                            Err(
                                                ::ridl_rt::error::CallError::Contract(
                                                    ::ridl_rt::error::Contract::PreconditionFailed,
                                                ),
                                            ),
                                        )
                                    }
                                    Ok(()) => {
                                        let accepted = h.settle(claim.id, Ok(&[]));
                                        p.set_level(&level);
                                        accepted
                                    }
                                }
                            }
                        }
                    }
                    ::ridl_rt::contract::Ordinal(4u32) => {
                        let decoded = match ::ridl_rt::payload::Ref::<
                            super::Window,
                            super::Wire,
                        >::verify(&buf[..claim.len]) {
                            Ok(checked) => Ok(checked.decode()),
                            Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                Err(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                )
                            }
                            Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                                Err(
                                    ::ridl_rt::error::CallError::Contract(
                                        ::ridl_rt::error::Contract::InvalidValue(violation),
                                    ),
                                )
                            }
                            Err(_) => {
                                Err(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                )
                            }
                        };
                        match decoded {
                            Err(error) => h.settle(claim.id, Err(error)),
                            Ok(window) => {
                                match <super::CabinAverage as ::ridl_rt::contract::Query>::require(
                                    &window,
                                ) {
                                    Err(()) => {
                                        h.settle(
                                            claim.id,
                                            Err(
                                                ::ridl_rt::error::CallError::Contract(
                                                    ::ridl_rt::error::Contract::PreconditionFailed,
                                                ),
                                            ),
                                        )
                                    }
                                    Ok(()) => {
                                        let reply = p.average(&window);
                                        match <super::CabinAverage as ::ridl_rt::contract::Query>::ensure(
                                            &window,
                                            &reply,
                                        ) {
                                            Err(()) => {
                                                h.settle(
                                                    claim.id,
                                                    Err(
                                                        ::ridl_rt::error::CallError::Contract(
                                                            ::ridl_rt::error::Contract::ContractBroken,
                                                        ),
                                                    ),
                                                )
                                            }
                                            Ok(()) => {
                                                let bytes = match ::ridl_rt::payload::Ref::<
                                                    super::Average,
                                                    super::Wire,
                                                >::encode(&reply, buf) {
                                                    Ok(encoded) => encoded.bytes(),
                                                    Err(
                                                        ::ridl_rt::payload::EncodeError::Capacity {
                                                            needed,
                                                            available,
                                                        },
                                                    ) => {
                                                        unreachable!(
                                                            "encoding `Average` needs {} bytes and the dispatch buffer has {}; a legal value cannot exceed `<Average as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                                                            needed, available
                                                        )
                                                    }
                                                    Err(_) => unreachable!("encoding `Average` failed"),
                                                };
                                                h.settle(claim.id, Ok(bytes))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        h.settle(
                            claim.id,
                            Err(
                                ::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::UnknownInteraction,
                                ),
                            ),
                        )
                    }
                }
            };
            if settlement.is_ok() {
                settled += 1;
            }
        }
    }
}
///The generated interaction face of interface `Horn`.
pub mod horn {
    ///The consumer face of interface `Horn`, generic over exactly the ports the interface's interactions need.
    pub struct Client<P: ::ridl_rt::port::SignalReader> {
        port: P,
    }
    impl<P: ::ridl_rt::port::SignalReader> Client<P> {
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        pub fn new(port: P) -> Self {
            Client { port }
        }
        ///Reads signal `active` and returns its value with the provenance, the freshness and the envelope the runtime resolved. A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn active(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Health>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; <super::Health as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let raw = self
                .port
                .read(
                    <super::Horn as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    &mut buf,
                )?;
            match ::ridl_rt::payload::Ref::<
                super::Health,
                super::Wire,
            >::verify(&buf[..raw.len]) {
                Ok(checked) => {
                    Ok(::ridl_rt::sample::Sample {
                        value: checked.decode(),
                        provenance: raw.provenance,
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    })
                }
                Err(error) => {
                    Ok(::ridl_rt::sample::Sample {
                        value: <super::HornActive as ::ridl_rt::contract::Signal>::init(),
                        provenance: ::ridl_rt::sample::Provenance::Invalid(
                            ::ridl_rt::sample::Cause::Detected(
                                match error {
                                    ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                        ::ridl_rt::sample::Detection::InvalidValue(violation)
                                    }
                                    _ => ::ridl_rt::sample::Detection::Corrupt,
                                },
                            ),
                        ),
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    })
                }
            }
        }
    }
    ///The provider face of interface `Horn`'s signals and events.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter> Publisher<W> {
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        pub fn new(port: W) -> Self {
            Publisher { port }
        }
        ///Stages a new value for signal `active`. It is published by `commit`.
        pub fn active(
            &mut self,
            value: super::Health,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = [0u8; <super::Health as ::ridl_rt::payload::Payload<
                super::Wire,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Health,
                super::Wire,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Health` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Health as Payload<Wire>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Health` failed"),
            };
            self.port
                .set(
                    <super::Horn as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    bytes,
                )
        }
        ///Stages the invalid state for signal `active`, with `Cause::Declared`. It is published by `commit`.
        pub fn invalidate_active(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Horn as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                )
        }
        /// Publishes every staged signal change.
        pub fn commit(&mut self) {
            self.port.commit()
        }
    }
}
