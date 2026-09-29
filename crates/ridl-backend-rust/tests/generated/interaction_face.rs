/// Cabin temperature, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Temperature(::core::primitive::i64);
impl Temperature {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: ::core::primitive::i64,
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
        value: &::core::primitive::i64,
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
    pub const fn new_unchecked(value: ::core::primitive::i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> ::core::primitive::i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Temperature {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Temperature> for ::core::primitive::i64 {
    fn from(value: Temperature) -> Self {
        value.0
    }
}
impl ::core::default::Default for Temperature {
    fn default() -> Self {
        Temperature::new_unchecked(0)
    }
}
/// A control level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Level(::core::primitive::i64);
impl Level {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: ::core::primitive::i64,
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
        value: &::core::primitive::i64,
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
    pub const fn new_unchecked(value: ::core::primitive::i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> ::core::primitive::i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Level {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Level> for ::core::primitive::i64 {
    fn from(value: Level) -> Self {
        value.0
    }
}
impl ::core::default::Default for Level {
    fn default() -> Self {
        Level::new_unchecked(0)
    }
}
/// A window length, in samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Window(::core::primitive::i64);
impl Window {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: ::core::primitive::i64,
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
        value: &::core::primitive::i64,
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
    pub const fn new_unchecked(value: ::core::primitive::i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> ::core::primitive::i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Window {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Window> for ::core::primitive::i64 {
    fn from(value: Window) -> Self {
        value.0
    }
}
impl ::core::default::Default for Window {
    fn default() -> Self {
        Window::new_unchecked(0)
    }
}
/// An averaged reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Average(::core::primitive::i64);
impl Average {
    /// Constructs the value, enforcing its typl constraints.
    pub fn new(
        value: ::core::primitive::i64,
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
        value: &::core::primitive::i64,
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
    pub const fn new_unchecked(value: ::core::primitive::i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> ::core::primitive::i64 {
        self.0
    }
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Average {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Average> for ::core::primitive::i64 {
    fn from(value: Average) -> Self {
        value.0
    }
}
impl ::core::default::Default for Average {
    fn default() -> Self {
        Average::new_unchecked(0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum Health {
    Ok = 0,
    Warn = 1,
    Fail = 2,
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Health {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        match value {
            0 => ::core::result::Result::Ok(Self::Ok),
            1 => ::core::result::Result::Ok(Self::Warn),
            2 => ::core::result::Result::Ok(Self::Fail),
            _ => {
                ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: "Health",
                    rule: ::ridl_rt::payload::Rule::Variant,
                })
            }
        }
    }
}
impl ::core::convert::From<Health> for ::core::primitive::i64 {
    fn from(value: Health) -> Self {
        value as ::core::primitive::i64
    }
}
impl ::core::default::Default for Health {
    fn default() -> Self {
        Health::Ok
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Warning {
    pub code: Level,
    pub health: Health,
}
impl ::core::default::Default for Warning {
    fn default() -> Self {
        Warning {
            code: <Level as ::core::default::Default>::default(),
            health: <Health as ::core::default::Default>::default(),
        }
    }
}
/// An accessor over FlatBuffers bytes `Temperature`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one non-optional `value` field. A
/// buffer carrying no slot for it holds the FlatBuffers
/// default, 0, when 0 is a legal value of this type, and is
/// `MissingRequired` when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct TemperatureFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for TemperatureFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> TemperatureFbView<'a> {
    /// The value the box carries. `Temperature` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Temperature {
        __ridl_fb_decode_Temperature(self.buf, self.table)
    }
}
/// Writes `Temperature` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_encode_Temperature(
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
                    ::ridl_rt::flatbuffers::Field::I8(__s.get() as ::core::primitive::i8)
                },
            },
        ];
        builder.push_table(5usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_verify_Temperature(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            1usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_i8(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Temperature::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_decode_Temperature(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Temperature {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Temperature::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_i8(buf, __p).unwrap_or(0i8),
                ),
            )
        }
        _ => Temperature::new_unchecked(0i64),
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
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type View<'a> = TemperatureFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, TemperatureFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Temperature(self, &mut builder)?;
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
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<TemperatureFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Temperature(buf, table)?;
        ::core::result::Result::Ok(TemperatureFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Temperature(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Level`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one non-optional `value` field. A
/// buffer carrying no slot for it holds the FlatBuffers
/// default, 0, when 0 is a legal value of this type, and is
/// `MissingRequired` when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct LevelFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for LevelFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> LevelFbView<'a> {
    /// The value the box carries. `Level` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Level {
        __ridl_fb_decode_Level(self.buf, self.table)
    }
}
/// Writes `Level` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_encode_Level(
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
                    ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
                },
            },
        ];
        builder.push_table(5usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_verify_Level(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            1usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Level::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_decode_Level(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Level {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Level::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                ),
            )
        }
        _ => Level::new_unchecked(0i64),
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
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type View<'a> = LevelFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, LevelFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Level(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LevelFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LevelFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Level(buf, table)?;
        ::core::result::Result::Ok(LevelFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Level(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Window`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one non-optional `value` field. A
/// buffer carrying no slot for it holds the FlatBuffers
/// default, 0, when 0 is a legal value of this type, and is
/// `MissingRequired` when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct WindowFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for WindowFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> WindowFbView<'a> {
    /// The value the box carries. `Window` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Window {
        __ridl_fb_decode_Window(self.buf, self.table)
    }
}
/// Writes `Window` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_encode_Window(
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
                    ::ridl_rt::flatbuffers::Field::U32(
                        __s.get() as ::core::primitive::u32,
                    )
                },
            },
        ];
        builder.push_table(8usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_verify_Window(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            4usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_u32(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Window::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_decode_Window(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Window {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Window::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_u32(buf, __p).unwrap_or(0u32),
                ),
            )
        }
        _ => Window::new_unchecked(0i64),
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
    const MAX_SIZE: ::core::primitive::usize = 46usize;
    type View<'a> = WindowFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, WindowFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Window(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: WindowFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<WindowFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Window(buf, table)?;
        ::core::result::Result::Ok(WindowFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Window(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Average`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one non-optional `value` field. A
/// buffer carrying no slot for it holds the FlatBuffers
/// default, 0, when 0 is a legal value of this type, and is
/// `MissingRequired` when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct AverageFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for AverageFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> AverageFbView<'a> {
    /// The value the box carries. `Average` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Average {
        __ridl_fb_decode_Average(self.buf, self.table)
    }
}
/// Writes `Average` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_encode_Average(
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
                    ::ridl_rt::flatbuffers::Field::U16(
                        __s.get() as ::core::primitive::u16,
                    )
                },
            },
        ];
        builder.push_table(6usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_verify_Average(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            2usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_u16(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Average::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_decode_Average(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Average {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 2usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Average::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_u16(buf, __p).unwrap_or(0u16),
                ),
            )
        }
        _ => Average::new_unchecked(0i64),
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
    const MAX_SIZE: ::core::primitive::usize = 44usize;
    type View<'a> = AverageFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, AverageFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Average(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: AverageFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<AverageFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Average(buf, table)?;
        ::core::result::Result::Ok(AverageFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Average(__view.buf, __view.table)
    }
}
/// An accessor over FlatBuffers bytes `Health`'s `verify` accepted.
///
/// The buffer's root is the box table ADR-0019 decision 8
/// gives this declaration: one non-optional `value` field. A
/// buffer carrying no slot for it holds the FlatBuffers
/// default, 0, when 0 is a legal value of this type, and is
/// `MissingRequired` when it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct HealthFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for HealthFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> HealthFbView<'a> {
    /// The value the box carries. `Health` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Health {
        __ridl_fb_decode_Health(self.buf, self.table)
    }
}
/// Writes `Health` as its box table and returns its position (ADR-0019 decision 8).
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_encode_Health(
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
                    ::ridl_rt::flatbuffers::Field::I64(::core::primitive::i64::from(__s))
                },
            },
        ];
        builder.push_table(16usize, 8usize, 1u16, &__box)?
    })
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_verify_Health(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            8usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        <Health as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(__raw)
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated)]
#[allow(non_snake_case)]
pub(crate) fn __ridl_fb_decode_Health(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Health {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 8usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            <Health as ::core::convert::TryFrom<
                ::core::primitive::i64,
            >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
                .unwrap_or(Health::Ok)
        }
        _ => Health::Ok,
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
    const MAX_SIZE: ::core::primitive::usize = 50usize;
    type View<'a> = HealthFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, HealthFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Health(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: HealthFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<HealthFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Health(buf, table)?;
        ::core::result::Result::Ok(HealthFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Health(__view.buf, __view.table)
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
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for WarningFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> WarningFbView<'a> {
    /// Reads `Warning`'s `code` field in place.
    pub fn code(&self) -> Level {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(self.buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Level::new_unchecked(0i64),
        }
    }
    /// Reads `Warning`'s `health` field in place.
    pub fn health(&self) -> Health {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                <Health as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(
                        ::ridl_rt::flatbuffers::read_i64(self.buf, __p).unwrap_or(0i64),
                    )
                    .unwrap_or(Health::Ok)
            }
            _ => Health::Ok,
        }
    }
}
/// Writes `Warning` as a FlatBuffers table and returns its position.
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Warning(
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
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 8u16,
        value: {
            let __s = value.health;
            ::ridl_rt::flatbuffers::Field::I64(::core::primitive::i64::from(__s))
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
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Warning(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            0u16,
            1usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Level::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            1u16,
            8usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        <Health as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(__raw)
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
/// Builds `Warning` from the FlatBuffers table at `table`.
///
/// It cannot fail. An absent non-optional scalar or enum field
/// reads as its FlatBuffers default — 0, or the enum's zero
/// member — which is a value `verify` accepted
/// (driftsys/ridl#472). Any other read that could fail is
/// discharged with the neutral value of its own type — zero, the
/// empty string or collection, the first declared enum variant —
/// and `verify` is what makes those branches unreachable. A named scalar is
/// built with its unchecked constructor (`new_unchecked`) over a
/// value `verify` has already range-checked (`check`), so this
/// never re-checks and never fails.
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Warning(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Warning {
    Warning {
        code: match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Level::new_unchecked(0i64),
        },
        health: match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                <Health as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
                    .unwrap_or(Health::Ok)
            }
            _ => Health::Ok,
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
    const MAX_SIZE: ::core::primitive::usize = 60usize;
    type View<'a> = WarningFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, WarningFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Warning(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: WarningFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<WarningFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Warning(buf, table)?;
        ::core::result::Result::Ok(WarningFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Warning(__view.buf, __view.table)
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
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Cabin";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "temperature",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Temperature",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Event,
            name: "warning",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(100000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Warning",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(3),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "setLevel",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(50000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(4),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "average",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Window",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
    ];
}
impl Cabin {
    ///The largest argument or reply payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. The claim buffer `serve` holds is this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Window as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Average as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
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
    ///The largest event payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
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
}
pub struct CabinTemperature;
impl ::ridl_rt::contract::Interaction for CabinTemperature {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Signal for CabinTemperature {
    type Payload = Temperature;
    fn init() -> Self::Payload {
        <Temperature as ::core::default::Default>::default()
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
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Horn";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "active",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Health",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
    ];
}
impl Horn {
    ///The largest argument or reply payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. The claim buffer `serve` holds is this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    ///The largest event payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
}
pub struct HornActive;
impl ::ridl_rt::contract::Interaction for HornActive {
    type Iface = Horn;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Horn as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Signal for HornActive {
    type Payload = Health;
    fn init() -> Self::Payload {
        <Health as ::core::default::Default>::default()
    }
}
/**Descriptor for interface `Siren`.

`CATALOG.hash` is the placeholder `CatalogHash([0u8; 32])` until E16.2 (driftsys/ridl#378) computes the real catalog hash.

Every `PayloadInfo.max_size` field is `None`. The reading here is that the toolchain cannot size the payload yet, not that the encoding cannot carry it. `ridl-rt`'s own doc comment states the other reading; E16.2 reconciles the two.*/
pub struct Siren;
impl ::ridl_rt::contract::Interface for Siren {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([0u8; 32]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(3);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Siren";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Event,
            name: "tripped",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(100000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Warning",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
    ];
}
impl Siren {
    ///The largest argument or reply payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. The claim buffer `serve` holds is this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    ///The largest event payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
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
}
pub struct SirenTripped;
impl ::ridl_rt::contract::Interaction for SirenTripped {
    type Iface = Siren;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Siren as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Event for SirenTripped {
    type Payload = Warning;
}
/**Descriptor for interface `Valve`.

`CATALOG.hash` is the placeholder `CatalogHash([0u8; 32])` until E16.2 (driftsys/ridl#378) computes the real catalog hash.

Every `PayloadInfo.max_size` field is `None`. The reading here is that the toolchain cannot size the payload yet, not that the encoding cannot carry it. `ridl-rt`'s own doc comment states the other reading; E16.2 reconciles the two.*/
pub struct Valve;
impl ::ridl_rt::contract::Interface for Valve {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([0u8; 32]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(4);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Valve";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "open",
            timing: ::core::option::Option::None,
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "pressure",
            timing: ::core::option::Option::None,
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Window",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                    max_size: ::ridl_rt::contract::EncodedSizes {
                        proto3: ::core::option::Option::None,
                        flatbuffers: ::core::option::Option::None,
                        repr_c: ::core::option::Option::None,
                    },
                },
            ],
        },
    ];
}
impl Valve {
    ///The largest argument or reply payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. The claim buffer `serve` holds is this large, because a reply is encoded into the same buffer as the arguments. `0` when the interface declares no call.
    pub const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Window as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Average as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
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
    ///The largest event payload of this interface, over `<T as Payload<FlatBuffers>>::MAX_SIZE`. `0` when the interface declares no event.
    pub const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
}
pub struct ValveOpen;
impl ::ridl_rt::contract::Interaction for ValveOpen {
    type Iface = Valve;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Valve as ::ridl_rt::contract::Interface>::MEMBERS[0];
}
impl ::ridl_rt::contract::Command for ValveOpen {
    type Args = Level;
    ///Evaluates the `require` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
pub struct ValvePressure;
impl ::ridl_rt::contract::Interaction for ValvePressure {
    type Iface = Valve;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Valve as ::ridl_rt::contract::Interface>::MEMBERS[1];
}
impl ::ridl_rt::contract::Query for ValvePressure {
    type Args = Window;
    type Reply = Average;
    ///Evaluates the `require` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    ///Evaluates the `ensure` clauses. This translation covers one clause form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces it with one driven by the structured expression tree.
    fn ensure(
        _args: &Self::Args,
        _reply: &Self::Reply,
    ) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
///The generated interaction face of interface `Cabin`.
pub mod cabin {
    ///Identifies one sent command `setLevel` to its caller. It is returned by the internal send and accepted by that call's own outcome read, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct SetLevelCorrelation(pub ::ridl_rt::port::Correlation);
    ///Identifies one sent query `average` to its caller. It is returned by the internal send and accepted by that call's own outcome read, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct AverageCorrelation(pub ::ridl_rt::port::Correlation);
    ///Starts delivery of one event of interface `Cabin`: one method per event, implemented by `Client` and by `blocking::Client`. A trait rather than inherent methods so that a member of the interface may be named `subscribe<Event>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Subscribe {
        ///Starts delivery of event `warning`.
        fn subscribe_warning(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError>;
    }
    ///The consumer face of interface `Cabin`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `next_event` is `ridl_rt::face::Events`'s and `subscribe_<event>` is this module's `Subscribe`'s, all in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P> {
        ///Reads signal `temperature` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn temperature(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Temperature>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; <super::Temperature as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let raw = self
                .port
                .read(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    &mut buf,
                )?;
            let (value, provenance) = match raw.provenance {
                ::ridl_rt::sample::Provenance::Init
                | ::ridl_rt::sample::Provenance::Invalid(
                    ::ridl_rt::sample::Cause::Declared,
                ) if raw.len == 0 => {
                    (
                        <super::CabinTemperature as ::ridl_rt::contract::Signal>::init(),
                        raw.provenance,
                    )
                }
                _ => {
                    match ::ridl_rt::payload::Ref::<
                        super::Temperature,
                        ::ridl_rt::encoding::FlatBuffers,
                    >::verify(&buf[..raw.len]) {
                        Ok(checked) => (checked.decode(), raw.provenance),
                        Err(error) => {
                            (
                                <super::CabinTemperature as ::ridl_rt::contract::Signal>::init(),
                                ::ridl_rt::sample::Provenance::Invalid(
                                    ::ridl_rt::sample::Cause::Detected(
                                        match error {
                                            ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                                ::ridl_rt::sample::Detection::InvalidValue(violation)
                                            }
                                            _ => ::ridl_rt::sample::Detection::Corrupt,
                                        },
                                    ),
                                ),
                            )
                        }
                    }
                }
            };
            Ok(::ridl_rt::sample::Sample {
                value,
                provenance,
                freshness: raw.freshness,
                envelope: raw.envelope,
            })
        }
        /**Sends command `setLevel` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        pub fn set_level(&mut self, level: super::Level) -> SetLevelCall<'_, P> {
            let __arg = level;
            let deadline = <super::CabinSetLevel as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_set_level(&mut self.port, &__arg) {
                Ok(correlation) => SetLevelPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => SetLevelPhase::Unsent(__arg),
                Err(error) => SetLevelPhase::Failed(error),
            };
            SetLevelCall {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
        /**Sends query `average` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        pub fn average(&mut self, window: super::Window) -> AverageCall<'_, P> {
            let __arg = window;
            let deadline = <super::CabinAverage as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_average(&mut self.port, &__arg) {
                Ok(correlation) => AveragePhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => AveragePhase::Unsent(__arg),
                Err(error) => AveragePhase::Failed(error),
            };
            AverageCall {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P> {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: P) -> Self {
            Client { port }
        }
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Events for Client<P> {
        type Next<'a> = NextEvent<'a, P> where Self: 'a;
        /**Takes the next occurrence of any subscribed event of interface `Cabin`, routed to its variant by ordinal, as a future: it resolves when an occurrence is waiting and is `Pending` while none is. One method serves every event, because the payload type is not known until the occurrence's ordinal is read. The future holds this client's port until it is dropped.

The interface number is checked before the ordinal, for the reason `serve` checks it: a port is attached to a whole catalog, ordinals restart at 1 in each interface, and an occurrence of a sibling interface at the same ordinal would otherwise be decoded as this interface's payload. Such an occurrence is reported as `Contract::UnknownInteraction`; `EventSource::next` has already consumed it, so this face cannot hand it back to the interface it belongs to. Subscribe on a port this interface owns.*/
        fn next_event(&mut self) -> NextEvent<'_, P> {
            NextEvent { port: &mut self.port }
        }
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Subscribe for Client<P> {
        fn subscribe_warning(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
            self.port
                .subscribe(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    &[::ridl_rt::contract::Ordinal(2u32)],
                )
        }
    }
    ///One occurrence of an event of interface `Cabin`.
    pub enum Event {
        ///An occurrence of event `warning`.
        Warning(::ridl_rt::sample::Occurrence<super::Warning>),
    }
    ///The future of command `setLevel`, returned by `Client::set_level`. It resolves to the delivery acknowledgment: `Ok(())`, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the command was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct SetLevelCall<
        'a,
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: SetLevelPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    ///Where command `setLevel` is, as its future's `poll` moves it.
    enum SetLevelPhase {
        /// The port answered `SendError::Busy`; the argument is kept for
        /// the retry.
        Unsent(super::Level),
        /// Sent, and waiting for the outcome under this correlation.
        Waiting(SetLevelCorrelation),
        /// The send failed before anything was sent; the first poll
        /// reports it.
        Failed(::ridl_rt::port::SendError),
        /// The output was taken.
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > SetLevelCall<'_, P> {
        /// Whether the call's bound has passed on the port's clock. A
        /// call with no bound never expires here.
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        /// Whether the call was sent and waits for its outcome. The
        /// blocking client asks this when `block_on` gives up, to answer
        /// as the future would at its own deadline; it is under `std`
        /// with that client, so a build without the feature has no
        /// unused item.
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, SetLevelPhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for SetLevelCall<'_, P> {
        type Output = ::core::result::Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, SetLevelPhase::Done) {
                    SetLevelPhase::Done => {
                        panic!("`SetLevelCall` polled after completion")
                    }
                    SetLevelPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    SetLevelPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        this.port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_set_level(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = SetLevelPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = SetLevelPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    SetLevelPhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_set_level_ack(
                            &mut *this.port,
                            correlation,
                        ) {
                            Some(outcome) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            None => {
                                if !this.expired() {
                                    this.phase = SetLevelPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                Err(
                                    ::ridl_rt::error::ClientError::Call(
                                        ::ridl_rt::error::CallError::Transport(
                                            ::ridl_rt::error::Transport::Undelivered,
                                        ),
                                    ),
                                )
                            }
                        };
                        this.port.forget(correlation.0);
                        return ::core::task::Poll::Ready(outcome);
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for SetLevelCall<'_, P> {
        fn drop(&mut self) {
            if let SetLevelPhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    ///The future of query `average`, returned by `Client::average`. It resolves to the decoded reply, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct AverageCall<
        'a,
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: AveragePhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    ///Where query `average` is, as its future's `poll` moves it.
    enum AveragePhase {
        /// The port answered `SendError::Busy`; the argument is kept for
        /// the retry.
        Unsent(super::Window),
        /// Sent, and waiting for the outcome under this correlation.
        Waiting(AverageCorrelation),
        /// The send failed before anything was sent; the first poll
        /// reports it.
        Failed(::ridl_rt::port::SendError),
        /// The output was taken.
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > AverageCall<'_, P> {
        /// Whether the call's bound has passed on the port's clock. A
        /// call with no bound never expires here.
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        /// Whether the call was sent and waits for its outcome. The
        /// blocking client asks this when `block_on` gives up, to answer
        /// as the future would at its own deadline; it is under `std`
        /// with that client, so a build without the feature has no
        /// unused item.
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, AveragePhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for AverageCall<'_, P> {
        type Output = ::core::result::Result<
            super::Average,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, AveragePhase::Done) {
                    AveragePhase::Done => panic!("`AverageCall` polled after completion"),
                    AveragePhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    AveragePhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        this.port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_average(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = AveragePhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = AveragePhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    AveragePhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_average_reply(
                            &mut *this.port,
                            correlation,
                        ) {
                            Ok(Some(outcome)) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            Err(error) => Err(::ridl_rt::error::ClientError::Read(error)),
                            Ok(None) => {
                                if !this.expired() {
                                    this.phase = AveragePhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                Err(
                                    ::ridl_rt::error::ClientError::Call(
                                        ::ridl_rt::error::CallError::Transport(
                                            ::ridl_rt::error::Transport::Timeout,
                                        ),
                                    ),
                                )
                            }
                        };
                        this.port.forget(correlation.0);
                        return ::core::task::Poll::Ready(outcome);
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for AverageCall<'_, P> {
        fn drop(&mut self) {
            if let AveragePhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    ///The future of `ridl_rt::face::Events::next_event` on the `Client` of interface `Cabin`. Each poll registers its interest in the interface's events, reads the queue once, and returns: an occurrence resolves it, and a read failure resolves it with that failure. It can be polled again after it resolved, for the next occurrence.
    #[must_use = "an occurrence is taken only when the future is polled"]
    pub struct NextEvent<
        'a,
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
    }
    impl<
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for NextEvent<'_, P> {
        type Output = ::core::result::Result<Event, ::ridl_rt::port::ReadError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            this.port
                .wake_on(
                    ::ridl_rt::port::Interest::Event(
                        <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ),
                    cx.waker(),
                );
            match poll_next_event(&mut *this.port) {
                Ok(Some(event)) => ::core::task::Poll::Ready(Ok(event)),
                Ok(None) => ::core::task::Poll::Pending,
                Err(error) => ::core::task::Poll::Ready(Err(error)),
            }
        }
    }
    ///Sends command `setLevel` once and returns the correlation of its outcome, or the send's failure. A `require` clause that fails is `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent. `Client::set_level` calls it when the method runs, and `SetLevelCall` calls it again on each poll while the port answers `SendError::Busy`.
    pub(crate) fn send_set_level<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        level: &super::Level,
    ) -> ::core::result::Result<SetLevelCorrelation, ::ridl_rt::port::SendError> {
        let __arg = level;
        <super::CabinSetLevel as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = [0u8; <super::Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
            ::ridl_rt::encoding::FlatBuffers,
        >::encode(__arg, &mut buf) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Level as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .command(
                <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(3u32),
                bytes,
            )
            .map(SetLevelCorrelation)
    }
    ///Reads command `setLevel`'s delivery acknowledgment: `Some` once it is known, `None` while it is not. It does not wait; `SetLevelCall` polls it.
    pub(crate) fn poll_set_level_ack<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: SetLevelCorrelation,
    ) -> ::core::option::Option<
        ::core::result::Result<(), ::ridl_rt::error::CallError>,
    > {
        port.ack(correlation.0)
    }
    ///Sends query `average` once and returns the correlation of its outcome, or the send's failure. A `require` clause that fails is `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent. `Client::average` calls it when the method runs, and `AverageCall` calls it again on each poll while the port answers `SendError::Busy`.
    pub(crate) fn send_average<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        window: &super::Window,
    ) -> ::core::result::Result<AverageCorrelation, ::ridl_rt::port::SendError> {
        let __arg = window;
        <super::CabinAverage as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = [0u8; <super::Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Window,
            ::ridl_rt::encoding::FlatBuffers,
        >::encode(__arg, &mut buf) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding `Window` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Window as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Window` failed"),
        };
        __port
            .query(
                <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(4u32),
                bytes,
            )
            .map(AverageCorrelation)
    }
    ///Reads query `average`'s reply: `Ok(Some)` once it is known, `Ok(None)` while it is not, and the port's own failure when the read itself fails. It does not wait; `AverageCall` polls it.
    pub(crate) fn poll_average_reply<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: AverageCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = [0u8; <super::Average as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        match port.reply(correlation.0, &mut buf)? {
            None => Ok(None),
            Some(Err(error)) => Ok(Some(Err(error))),
            Some(Ok(len)) => {
                Ok(
                    Some(
                        match ::ridl_rt::payload::Ref::<
                            super::Average,
                            ::ridl_rt::encoding::FlatBuffers,
                        >::verify(&buf[..len]) {
                            Ok(checked) => Ok(checked.decode()),
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
                        },
                    ),
                )
            }
        }
    }
    ///Reads the next occurrence of any subscribed event of interface `Cabin`, routed to its variant by ordinal: `Ok(None)` when none is waiting. It does not wait; `NextEvent` polls it. An occurrence of another interface is reported as `Contract::UnknownInteraction`, for the reason the `Client`'s `ridl_rt::face::Events::next_event` gives.
    pub(crate) fn poll_next_event<P: ::ridl_rt::port::EventSource>(
        port: &mut P,
    ) -> ::core::result::Result<
        ::core::option::Option<Event>,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = [0u8; super::Cabin::EVENT_SOURCE_BUFFER_SIZE];
        let Some(occurrence) = port.next(&mut buf)? else {
            return Ok(None);
        };
        if occurrence.iface != <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER {
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
                                ::ridl_rt::encoding::FlatBuffers,
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
    ///Stages the invalid state of one signal of interface `Cabin`: one method per signal, implemented by `Publisher`. A trait rather than inherent methods so that a member of the interface may be named `invalidate<Signal>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Invalidate {
        ///Stages the invalid state for signal `temperature`, with `Cause::Declared`. It is published by `commit`.
        fn invalidate_temperature(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError>;
    }
    ///The provider face of interface `Cabin`'s signals and events. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `commit` is `ridl_rt::face::Publish`'s and `invalidate_<signal>` is this module's `Invalidate`'s, all in scope through `prelude`.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink> Publisher<W> {
        ///Stages a new value for signal `temperature`. It is published by `commit`.
        pub fn temperature(
            &mut self,
            value: super::Temperature,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = [0u8; <super::Temperature as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Temperature,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Temperature` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Temperature as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
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
        ///Raises one occurrence of event `warning`.
        pub fn warning(
            &mut self,
            value: super::Warning,
        ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
            let mut buf = [0u8; <super::Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Warning,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Warning` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Warning as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
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
    }
    impl<
        W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink,
    > ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: W) -> Self {
            Publisher { port }
        }
    }
    impl<
        W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink,
    > ::ridl_rt::face::Publish for Publisher<W> {
        /// Publishes every staged signal change.
        fn commit(&mut self) {
            self.port.commit()
        }
    }
    impl<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink> Invalidate
    for Publisher<W> {
        fn invalidate_temperature(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                )
        }
    }
    /**What an application implements to serve interface `Cabin`'s calls.

An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses, and the generated payload types implement neither `Copy` nor `Clone`.*/
    pub trait Provider {
        ///Serves command `setLevel`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        fn set_level(&mut self, level: &super::Level);
        ///Serves query `average`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        fn average(&mut self, window: &super::Window) -> super::Average;
    }
    /**Settles the claims of interface `Cabin` that are waiting, up to `budget` of them, and returns how many were settled, or the handler port's failure. It is the one-pass step `serve` calls on each poll.

It does not wait: it makes one pass over the claims the handler already has and returns. `budget` is decreased by one for each claim taken from `Handler::next_claim`, whether or not its settlement is accepted — except an oversized claim whose settlement the handler refused, below — and the pass stops when it reaches 0 without asking for another claim. With a buffer of at least `Cabin::MAX_BUFFER_SIZE` bytes, `Ok` with `budget` above 0 means the handler has no claim waiting, or refused the settlement of an oversized claim that stays waiting; `Ok` with `budget` at 0 means claims may still be waiting; `Err` means `Handler::next_claim` failed, and every claim settled before the failure stays settled. `ReadError::ShortClaim` is not a failure: the claim's arguments do not fit `Cabin::MAX_BUFFER_SIZE`, the interface's largest argument or reply payload, so they are larger than any valid encoding of this interface's members, and the claim is settled `Transport::Corrupt` by its id without being read, whichever interface or member it names — a claim naming another interface may be validly larger, and is settled `Corrupt` too, because this step cannot read it; it counts toward `budget`, and the pass continues. When the handler refuses that settlement the pass ends at once, returning the count so far with the budget unspent, as if no claim were waiting, because the runtime keeps the unsettled claim the next one; the claims behind it wait until the handler can settle it (driftsys/ridl#569).

`buf` must be at least `Cabin::MAX_BUFFER_SIZE` bytes, because a reply is encoded into the same buffer as the arguments. A shorter buffer returns `Ok(0)` without consuming a claim or changing `budget`.

Every claim that is taken is settled, including one whose interface number or ordinal this interface does not recognise, which settles `Contract::UnknownInteraction`. A claim is counted only once `Handler::settle` has accepted it; a `SettleError` is left to the handler, which already owns that claim's settlement, and the pass continues with the next claim, except for an oversized claim, whose refused settlement ends the pass as stated above.

A command is settled `Ok(&[])` once its arguments and its `require` clauses pass and **before** the application's method runs, because a command's acknowledgment is a delivery acknowledgment and not a completion one (ridl §6.1, and `Handler`'s own contract). A query is settled after the application returns, because its settlement carries the reply.*/
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Handler,
        P: Provider,
    {
        if buf.len() < super::Cabin::MAX_BUFFER_SIZE {
            return Ok(0);
        }
        let mut settled = 0usize;
        while *budget > 0 {
            let settlement = match h.next_claim(buf) {
                Ok(None) => break,
                Err(::ridl_rt::port::ReadError::ShortClaim { claim, .. }) => {
                    let settlement = h
                        .settle(
                            claim,
                            Err(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                ),
                            ),
                        );
                    if settlement.is_err() {
                        return Ok(settled);
                    }
                    settlement
                }
                Err(error) => return Err(error),
                Ok(Some(claim)) => {
                    if claim.iface
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
                                    ::ridl_rt::encoding::FlatBuffers,
                                >::verify(&buf[..claim.len]) {
                                    Ok(checked) => Ok(checked.decode()),
                                    Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                        Err(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Corrupt,
                                            ),
                                        )
                                    }
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
                                };
                                match decoded {
                                    Err(error) => h.settle(claim.id, Err(error)),
                                    Ok(__arg) => {
                                        match <super::CabinSetLevel as ::ridl_rt::contract::Command>::require(
                                            &__arg,
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
                                                p.set_level(&__arg);
                                                accepted
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(4u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Window,
                                    ::ridl_rt::encoding::FlatBuffers,
                                >::verify(&buf[..claim.len]) {
                                    Ok(checked) => Ok(checked.decode()),
                                    Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                        Err(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Corrupt,
                                            ),
                                        )
                                    }
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
                                };
                                match decoded {
                                    Err(error) => h.settle(claim.id, Err(error)),
                                    Ok(__arg) => {
                                        match <super::CabinAverage as ::ridl_rt::contract::Query>::require(
                                            &__arg,
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
                                                let reply = p.average(&__arg);
                                                match <super::CabinAverage as ::ridl_rt::contract::Query>::ensure(
                                                    &__arg,
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
                                                            ::ridl_rt::encoding::FlatBuffers,
                                                        >::encode(&reply, buf) {
                                                            Ok(encoded) => encoded.bytes(),
                                                            Err(
                                                                ::ridl_rt::payload::EncodeError::Capacity {
                                                                    needed,
                                                                    available,
                                                                },
                                                            ) => {
                                                                unreachable!(
                                                                    "encoding `Average` needs {} bytes and the dispatch buffer has {}; a legal value cannot exceed `<Average as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
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
                    }
                }
            };
            *budget -= 1;
            if settlement.is_ok() {
                settled += 1;
            }
        }
        Ok(settled)
    }
    ///The most claims one poll of `Serve` takes (driftsys/ridl#568). A future that holds one poll for an unbounded time blocks every other task on a single-threaded executor; 32 bounds one poll and keeps the cost of registering the claim interest, paid once per poll, small beside the claims the poll settles.
    const SERVE_BUDGET: usize = 32;
    ///Serves interface `Cabin`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        let state = match h
            .serve(
                <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(3u32), ::ridl_rt::contract::Ordinal(4u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: [0u8; super::Cabin::MAX_BUFFER_SIZE],
            state,
        }
    }
    ///The future `serve` returns over interface `Cabin`. It holds the handler, the provider, and the claim buffer of `Cabin::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > {
        handler: H,
        provider: &'a mut P,
        buf: [u8; super::Cabin::MAX_BUFFER_SIZE],
        state: ServeState,
    }
    /// Where `serve` is, as its future's `poll` moves it.
    #[derive(Clone, Copy)]
    enum ServeState {
        /// `Handler::serve` refused the members; the first poll reports
        /// it.
        Refused(::ridl_rt::port::ServeError),
        /// Each poll registers the claim interest and settles at most
        /// `SERVE_BUDGET` claims.
        Serving,
        /// The failure was reported.
        Done,
    }
    /// `Unpin` whatever `H` is: nothing in the future is pinned, and a
    /// frame loop that stores it in its own state polls it through
    /// `Pin::new`.
    impl<
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P> {}
    impl<
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P> {
        type Output = ::core::result::Result<
            ::core::convert::Infallible,
            ::ridl_rt::error::ProviderError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            match this.state {
                ServeState::Done => panic!("`Serve` polled after completion"),
                ServeState::Refused(error) => {
                    this.state = ServeState::Done;
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ProviderError::Serve(error)),
                    )
                }
                ServeState::Serving => {
                    this.handler
                        .wake_on(
                            ::ridl_rt::port::Interest::Claim(
                                <super::Cabin as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        &mut this.buf,
                        &mut budget,
                    ) {
                        Ok(_) => {
                            if budget == 0 {
                                cx.waker().wake_by_ref();
                            }
                            ::core::task::Poll::Pending
                        }
                        Err(error) => {
                            this.state = ServeState::Done;
                            ::core::task::Poll::Ready(
                                Err(::ridl_rt::error::ProviderError::Claim(error)),
                            )
                        }
                    }
                }
            }
        }
    }
    ///The traits a consumer of interface `Cabin`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `next_event`, `subscribe_<event>`, `commit`, `invalidate_<signal>`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        pub use ::ridl_rt::face::Events;
        pub use super::Subscribe as _;
        pub use ::ridl_rt::face::Publish;
        pub use super::Invalidate as _;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Cabin`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        /// The instant `timeout` ends for a wait that starts now. `None`
        /// with no timeout, and with a timeout so large that the instant
        /// cannot be represented, which is then a wait with no bound.
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Cabin`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, `next_event` is `ridl_rt::face::Events`'s and `subscribe_<event>` is the parent module's `Subscribe`'s, all in scope through the parent module's `prelude`.
        pub struct Client<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > Client<P> {
            ///Reads signal `temperature`, as `Client::temperature` does: a read never waits.
            pub fn temperature(
                &self,
            ) -> ::core::result::Result<
                ::ridl_rt::sample::Sample<super::super::Temperature>,
                ::ridl_rt::port::ReadError,
            > {
                self.inner.temperature()
            }
            ///Sends command `setLevel` and waits for its outcome, as `block_on` over `Client::set_level`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            pub fn set_level(
                &mut self,
                level: super::super::Level,
            ) -> ::core::result::Result<(), ::ridl_rt::error::ClientError> {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.set_level(level);
                match ::ridl_rt::task::block_on(&mut __call, __deadline) {
                    Some(outcome) => outcome,
                    None if __call.sent() => {
                        Err(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Undelivered,
                                ),
                            ),
                        )
                    }
                    None => {
                        Err(
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            ),
                        )
                    }
                }
            }
            ///Sends query `average` and waits for its outcome, as `block_on` over `Client::average`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            pub fn average(
                &mut self,
                window: super::super::Window,
            ) -> ::core::result::Result<
                super::super::Average,
                ::ridl_rt::error::ClientError,
            > {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.average(window);
                match ::ridl_rt::task::block_on(&mut __call, __deadline) {
                    Some(outcome) => outcome,
                    None if __call.sent() => {
                        Err(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Timeout,
                                ),
                            ),
                        )
                    }
                    None => {
                        Err(
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            ),
                        )
                    }
                }
            }
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P> {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held
            /// by value: pass a handle, or a `&mut` borrow of one.
            fn new(port: P) -> Self {
                Client {
                    inner: <super::Client<P> as ::ridl_rt::face::Bind>::new(port),
                    timeout: None,
                }
            }
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Timeout for Client<P> {
            /// Sets the timeout every waiting method of this client is
            /// bounded by, and returns the client. A timeout shorter than a
            /// member's `max` is accepted, and ends the call first; a longer
            /// one ends an unserved call at `max` only on a runtime that
            /// wakes the call when its bound passes.
            fn with_timeout(mut self, timeout: ::std::time::Duration) -> Self {
                self.timeout = Some(timeout);
                self
            }
            /// Sets or clears the timeout every waiting method of this
            /// client is bounded by.
            fn set_timeout(
                &mut self,
                timeout: ::core::option::Option<::std::time::Duration>,
            ) {
                self.timeout = timeout;
            }
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Events for Client<P> {
            type Next<'a> = ::core::result::Result<
                ::core::option::Option<super::Event>,
                ::ridl_rt::port::ReadError,
            >
            where
                Self: 'a;
            ///Waits for the next occurrence of any subscribed event of interface `Cabin` and returns it, routed to its variant by ordinal, or `Ok(None)` when this client's timeout passes first. With no timeout it returns only with an occurrence or a read failure. It is `block_on` over the async client's `ridl_rt::face::Events::next_event`.
            fn next_event(
                &mut self,
            ) -> ::core::result::Result<
                ::core::option::Option<super::Event>,
                ::ridl_rt::port::ReadError,
            > {
                let __deadline = __deadline_after(self.timeout);
                let mut __next = ::ridl_rt::face::Events::next_event(&mut self.inner);
                match ::ridl_rt::task::block_on(&mut __next, __deadline) {
                    Some(Ok(event)) => Ok(Some(event)),
                    Some(Err(error)) => Err(error),
                    None => Ok(None),
                }
            }
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource
                + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > super::Subscribe for Client<P> {
            ///Starts delivery of event `warning`, as the async client's `Subscribe::subscribe_warning` does.
            fn subscribe_warning(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
                super::Subscribe::subscribe_warning(&mut self.inner)
            }
        }
        ///Serves interface `Cabin`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
            P: super::Provider,
        {
            let __deadline = __deadline_after(timeout);
            let mut __serve = super::serve(h, p);
            match ::ridl_rt::task::block_on(&mut __serve, __deadline) {
                Some(Ok(never)) => match never {}
                Some(Err(error)) => Err(error),
                None => Ok(()),
            }
        }
    }
}
///The generated interaction face of interface `Horn`.
pub mod horn {
    ///The consumer face of interface `Horn`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<P: ::ridl_rt::port::SignalReader> {
        port: P,
    }
    impl<P: ::ridl_rt::port::SignalReader> Client<P> {
        ///Reads signal `active` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn active(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Health>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = [0u8; <super::Health as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let raw = self
                .port
                .read(
                    <super::Horn as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    &mut buf,
                )?;
            let (value, provenance) = match raw.provenance {
                ::ridl_rt::sample::Provenance::Init
                | ::ridl_rt::sample::Provenance::Invalid(
                    ::ridl_rt::sample::Cause::Declared,
                ) if raw.len == 0 => {
                    (
                        <super::HornActive as ::ridl_rt::contract::Signal>::init(),
                        raw.provenance,
                    )
                }
                _ => {
                    match ::ridl_rt::payload::Ref::<
                        super::Health,
                        ::ridl_rt::encoding::FlatBuffers,
                    >::verify(&buf[..raw.len]) {
                        Ok(checked) => (checked.decode(), raw.provenance),
                        Err(error) => {
                            (
                                <super::HornActive as ::ridl_rt::contract::Signal>::init(),
                                ::ridl_rt::sample::Provenance::Invalid(
                                    ::ridl_rt::sample::Cause::Detected(
                                        match error {
                                            ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                                ::ridl_rt::sample::Detection::InvalidValue(violation)
                                            }
                                            _ => ::ridl_rt::sample::Detection::Corrupt,
                                        },
                                    ),
                                ),
                            )
                        }
                    }
                }
            };
            Ok(::ridl_rt::sample::Sample {
                value,
                provenance,
                freshness: raw.freshness,
                envelope: raw.envelope,
            })
        }
    }
    impl<P: ::ridl_rt::port::SignalReader> ::ridl_rt::face::Bind for Client<P> {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: P) -> Self {
            Client { port }
        }
    }
    ///Stages the invalid state of one signal of interface `Horn`: one method per signal, implemented by `Publisher`. A trait rather than inherent methods so that a member of the interface may be named `invalidate<Signal>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Invalidate {
        ///Stages the invalid state for signal `active`, with `Cause::Declared`. It is published by `commit`.
        fn invalidate_active(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError>;
    }
    ///The provider face of interface `Horn`'s signals and events. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `commit` is `ridl_rt::face::Publish`'s and `invalidate_<signal>` is this module's `Invalidate`'s, all in scope through `prelude`.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter> Publisher<W> {
        ///Stages a new value for signal `active`. It is published by `commit`.
        pub fn active(
            &mut self,
            value: super::Health,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = [0u8; <super::Health as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Health,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Health` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Health as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
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
    }
    impl<W: ::ridl_rt::port::SignalWriter> ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: W) -> Self {
            Publisher { port }
        }
    }
    impl<W: ::ridl_rt::port::SignalWriter> ::ridl_rt::face::Publish for Publisher<W> {
        /// Publishes every staged signal change.
        fn commit(&mut self) {
            self.port.commit()
        }
    }
    impl<W: ::ridl_rt::port::SignalWriter> Invalidate for Publisher<W> {
        fn invalidate_active(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Horn as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                )
        }
    }
    ///The traits a consumer of interface `Horn`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `commit`, `invalidate_<signal>` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        pub use ::ridl_rt::face::Publish;
        pub use super::Invalidate as _;
    }
}
///The generated interaction face of interface `Siren`.
pub mod siren {
    ///Starts delivery of one event of interface `Siren`: one method per event, implemented by `Client` and by `blocking::Client`. A trait rather than inherent methods so that a member of the interface may be named `subscribe<Event>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Subscribe {
        ///Starts delivery of event `tripped`.
        fn subscribe_tripped(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError>;
    }
    ///The consumer face of interface `Siren`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `next_event` is `ridl_rt::face::Events`'s and `subscribe_<event>` is this module's `Subscribe`'s, all in scope through `prelude`.
    pub struct Client<P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable> {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P> {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: P) -> Self {
            Client { port }
        }
    }
    impl<
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Events for Client<P> {
        type Next<'a> = NextEvent<'a, P> where Self: 'a;
        /**Takes the next occurrence of any subscribed event of interface `Siren`, routed to its variant by ordinal, as a future: it resolves when an occurrence is waiting and is `Pending` while none is. One method serves every event, because the payload type is not known until the occurrence's ordinal is read. The future holds this client's port until it is dropped.

The interface number is checked before the ordinal, for the reason `serve` checks it: a port is attached to a whole catalog, ordinals restart at 1 in each interface, and an occurrence of a sibling interface at the same ordinal would otherwise be decoded as this interface's payload. Such an occurrence is reported as `Contract::UnknownInteraction`; `EventSource::next` has already consumed it, so this face cannot hand it back to the interface it belongs to. Subscribe on a port this interface owns.*/
        fn next_event(&mut self) -> NextEvent<'_, P> {
            NextEvent { port: &mut self.port }
        }
    }
    impl<P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable> Subscribe
    for Client<P> {
        fn subscribe_tripped(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
            self.port
                .subscribe(
                    <super::Siren as ::ridl_rt::contract::Interface>::NUMBER,
                    &[::ridl_rt::contract::Ordinal(1u32)],
                )
        }
    }
    ///One occurrence of an event of interface `Siren`.
    pub enum Event {
        ///An occurrence of event `tripped`.
        Tripped(::ridl_rt::sample::Occurrence<super::Warning>),
    }
    ///The future of `ridl_rt::face::Events::next_event` on the `Client` of interface `Siren`. Each poll registers its interest in the interface's events, reads the queue once, and returns: an occurrence resolves it, and a read failure resolves it with that failure. It can be polled again after it resolved, for the next occurrence.
    #[must_use = "an occurrence is taken only when the future is polled"]
    pub struct NextEvent<
        'a,
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
    }
    impl<
        P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for NextEvent<'_, P> {
        type Output = ::core::result::Result<Event, ::ridl_rt::port::ReadError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            this.port
                .wake_on(
                    ::ridl_rt::port::Interest::Event(
                        <super::Siren as ::ridl_rt::contract::Interface>::NUMBER,
                    ),
                    cx.waker(),
                );
            match poll_next_event(&mut *this.port) {
                Ok(Some(event)) => ::core::task::Poll::Ready(Ok(event)),
                Ok(None) => ::core::task::Poll::Pending,
                Err(error) => ::core::task::Poll::Ready(Err(error)),
            }
        }
    }
    ///Reads the next occurrence of any subscribed event of interface `Siren`, routed to its variant by ordinal: `Ok(None)` when none is waiting. It does not wait; `NextEvent` polls it. An occurrence of another interface is reported as `Contract::UnknownInteraction`, for the reason the `Client`'s `ridl_rt::face::Events::next_event` gives.
    pub(crate) fn poll_next_event<P: ::ridl_rt::port::EventSource>(
        port: &mut P,
    ) -> ::core::result::Result<
        ::core::option::Option<Event>,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = [0u8; super::Siren::EVENT_SOURCE_BUFFER_SIZE];
        let Some(occurrence) = port.next(&mut buf)? else {
            return Ok(None);
        };
        if occurrence.iface != <super::Siren as ::ridl_rt::contract::Interface>::NUMBER {
            return Err(
                ::ridl_rt::port::ReadError::Contract(
                    ::ridl_rt::error::Contract::UnknownInteraction,
                ),
            );
        }
        match occurrence.ord {
            ::ridl_rt::contract::Ordinal(1u32) => {
                Ok(
                    Some(
                        Event::Tripped(::ridl_rt::sample::Occurrence {
                            payload: match ::ridl_rt::payload::Ref::<
                                super::Warning,
                                ::ridl_rt::encoding::FlatBuffers,
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
    ///The provider face of interface `Siren`'s signals and events. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Publisher<W: ::ridl_rt::port::EventSink> {
        port: W,
    }
    impl<W: ::ridl_rt::port::EventSink> Publisher<W> {
        ///Raises one occurrence of event `tripped`.
        pub fn tripped(
            &mut self,
            value: super::Warning,
        ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
            let mut buf = [0u8; <super::Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE];
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Warning,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Warning` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Warning as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Warning` failed"),
            };
            self.port
                .raise(
                    <super::Siren as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    bytes,
                )
        }
    }
    impl<W: ::ridl_rt::port::EventSink> ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: W) -> Self {
            Publisher { port }
        }
    }
    ///The traits a consumer of interface `Siren`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `next_event`, `subscribe_<event>`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        pub use ::ridl_rt::face::Events;
        pub use super::Subscribe as _;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Siren`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        /// The instant `timeout` ends for a wait that starts now. `None`
        /// with no timeout, and with a timeout so large that the instant
        /// cannot be represented, which is then a wait with no bound.
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Siren`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, `next_event` is `ridl_rt::face::Events`'s and `subscribe_<event>` is the parent module's `Subscribe`'s, all in scope through the parent module's `prelude`.
        pub struct Client<P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable> {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P> {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held
            /// by value: pass a handle, or a `&mut` borrow of one.
            fn new(port: P) -> Self {
                Client {
                    inner: <super::Client<P> as ::ridl_rt::face::Bind>::new(port),
                    timeout: None,
                }
            }
        }
        impl<
            P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Timeout for Client<P> {
            /// Sets the timeout every waiting method of this client is
            /// bounded by, and returns the client. A timeout shorter than a
            /// member's `max` is accepted, and ends the call first; a longer
            /// one ends an unserved call at `max` only on a runtime that
            /// wakes the call when its bound passes.
            fn with_timeout(mut self, timeout: ::std::time::Duration) -> Self {
                self.timeout = Some(timeout);
                self
            }
            /// Sets or clears the timeout every waiting method of this
            /// client is bounded by.
            fn set_timeout(
                &mut self,
                timeout: ::core::option::Option<::std::time::Duration>,
            ) {
                self.timeout = timeout;
            }
        }
        impl<
            P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Events for Client<P> {
            type Next<'a> = ::core::result::Result<
                ::core::option::Option<super::Event>,
                ::ridl_rt::port::ReadError,
            >
            where
                Self: 'a;
            ///Waits for the next occurrence of any subscribed event of interface `Siren` and returns it, routed to its variant by ordinal, or `Ok(None)` when this client's timeout passes first. With no timeout it returns only with an occurrence or a read failure. It is `block_on` over the async client's `ridl_rt::face::Events::next_event`.
            fn next_event(
                &mut self,
            ) -> ::core::result::Result<
                ::core::option::Option<super::Event>,
                ::ridl_rt::port::ReadError,
            > {
                let __deadline = __deadline_after(self.timeout);
                let mut __next = ::ridl_rt::face::Events::next_event(&mut self.inner);
                match ::ridl_rt::task::block_on(&mut __next, __deadline) {
                    Some(Ok(event)) => Ok(Some(event)),
                    Some(Err(error)) => Err(error),
                    None => Ok(None),
                }
            }
        }
        impl<
            P: ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
        > super::Subscribe for Client<P> {
            ///Starts delivery of event `tripped`, as the async client's `Subscribe::subscribe_tripped` does.
            fn subscribe_tripped(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
                super::Subscribe::subscribe_tripped(&mut self.inner)
            }
        }
    }
}
///The generated interaction face of interface `Valve`.
pub mod valve {
    ///Identifies one sent command `open` to its caller. It is returned by the internal send and accepted by that call's own outcome read, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct OpenCorrelation(pub ::ridl_rt::port::Correlation);
    ///Identifies one sent query `pressure` to its caller. It is returned by the internal send and accepted by that call's own outcome read, and by no other.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct PressureCorrelation(pub ::ridl_rt::port::Correlation);
    ///The consumer face of interface `Valve`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > Client<P> {
        /**Sends command `open` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        pub fn open(&mut self, level: super::Level) -> OpenCall<'_, P> {
            let __arg = level;
            let deadline = <super::ValveOpen as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_open(&mut self.port, &__arg) {
                Ok(correlation) => OpenPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => OpenPhase::Unsent(__arg),
                Err(error) => OpenPhase::Failed(error),
            };
            OpenCall {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
        /**Sends query `pressure` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        pub fn pressure(&mut self, window: super::Window) -> PressureCall<'_, P> {
            let __arg = window;
            let deadline = <super::ValvePressure as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_pressure(&mut self.port, &__arg) {
                Ok(correlation) => PressurePhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => PressurePhase::Unsent(__arg),
                Err(error) => PressurePhase::Failed(error),
            };
            PressureCall {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P> {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a
        /// handle, or a `&mut` borrow of one.
        fn new(port: P) -> Self {
            Client { port }
        }
    }
    ///The future of command `open`, returned by `Client::open`. It resolves to the delivery acknowledgment: `Ok(())`, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the command was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct OpenCall<
        'a,
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: OpenPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    ///Where command `open` is, as its future's `poll` moves it.
    enum OpenPhase {
        /// The port answered `SendError::Busy`; the argument is kept for
        /// the retry.
        Unsent(super::Level),
        /// Sent, and waiting for the outcome under this correlation.
        Waiting(OpenCorrelation),
        /// The send failed before anything was sent; the first poll
        /// reports it.
        Failed(::ridl_rt::port::SendError),
        /// The output was taken.
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > OpenCall<'_, P> {
        /// Whether the call's bound has passed on the port's clock. A
        /// call with no bound never expires here.
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        /// Whether the call was sent and waits for its outcome. The
        /// blocking client asks this when `block_on` gives up, to answer
        /// as the future would at its own deadline; it is under `std`
        /// with that client, so a build without the feature has no
        /// unused item.
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, OpenPhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for OpenCall<'_, P> {
        type Output = ::core::result::Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, OpenPhase::Done) {
                    OpenPhase::Done => panic!("`OpenCall` polled after completion"),
                    OpenPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    OpenPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        this.port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_open(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = OpenPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = OpenPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    OpenPhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_open_ack(&mut *this.port, correlation) {
                            Some(outcome) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            None => {
                                if !this.expired() {
                                    this.phase = OpenPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                Err(
                                    ::ridl_rt::error::ClientError::Call(
                                        ::ridl_rt::error::CallError::Transport(
                                            ::ridl_rt::error::Transport::Undelivered,
                                        ),
                                    ),
                                )
                            }
                        };
                        this.port.forget(correlation.0);
                        return ::core::task::Poll::Ready(outcome);
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for OpenCall<'_, P> {
        fn drop(&mut self) {
            if let OpenPhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    ///The future of query `pressure`, returned by `Client::pressure`. It resolves to the decoded reply, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct PressureCall<
        'a,
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: PressurePhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    ///Where query `pressure` is, as its future's `poll` moves it.
    enum PressurePhase {
        /// The port answered `SendError::Busy`; the argument is kept for
        /// the retry.
        Unsent(super::Window),
        /// Sent, and waiting for the outcome under this correlation.
        Waiting(PressureCorrelation),
        /// The send failed before anything was sent; the first poll
        /// reports it.
        Failed(::ridl_rt::port::SendError),
        /// The output was taken.
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > PressureCall<'_, P> {
        /// Whether the call's bound has passed on the port's clock. A
        /// call with no bound never expires here.
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        /// Whether the call was sent and waits for its outcome. The
        /// blocking client asks this when `block_on` gives up, to answer
        /// as the future would at its own deadline; it is under `std`
        /// with that client, so a build without the feature has no
        /// unused item.
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, PressurePhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for PressureCall<'_, P> {
        type Output = ::core::result::Result<
            super::Average,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, PressurePhase::Done) {
                    PressurePhase::Done => {
                        panic!("`PressureCall` polled after completion")
                    }
                    PressurePhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    PressurePhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        this.port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_pressure(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = PressurePhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = PressurePhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    PressurePhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_pressure_reply(
                            &mut *this.port,
                            correlation,
                        ) {
                            Ok(Some(outcome)) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            Err(error) => Err(::ridl_rt::error::ClientError::Read(error)),
                            Ok(None) => {
                                if !this.expired() {
                                    this.phase = PressurePhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                Err(
                                    ::ridl_rt::error::ClientError::Call(
                                        ::ridl_rt::error::CallError::Transport(
                                            ::ridl_rt::error::Transport::Timeout,
                                        ),
                                    ),
                                )
                            }
                        };
                        this.port.forget(correlation.0);
                        return ::core::task::Poll::Ready(outcome);
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for PressureCall<'_, P> {
        fn drop(&mut self) {
            if let PressurePhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    ///Sends command `open` once and returns the correlation of its outcome, or the send's failure. A `require` clause that fails is `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent. `Client::open` calls it when the method runs, and `OpenCall` calls it again on each poll while the port answers `SendError::Busy`.
    pub(crate) fn send_open<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        level: &super::Level,
    ) -> ::core::result::Result<OpenCorrelation, ::ridl_rt::port::SendError> {
        let __arg = level;
        <super::ValveOpen as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = [0u8; <super::Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
            ::ridl_rt::encoding::FlatBuffers,
        >::encode(__arg, &mut buf) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Level as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .command(
                <super::Valve as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
            )
            .map(OpenCorrelation)
    }
    ///Reads command `open`'s delivery acknowledgment: `Some` once it is known, `None` while it is not. It does not wait; `OpenCall` polls it.
    pub(crate) fn poll_open_ack<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: OpenCorrelation,
    ) -> ::core::option::Option<
        ::core::result::Result<(), ::ridl_rt::error::CallError>,
    > {
        port.ack(correlation.0)
    }
    ///Sends query `pressure` once and returns the correlation of its outcome, or the send's failure. A `require` clause that fails is `SendError::Contract(Contract::PreconditionFailed)` and nothing is sent. `Client::pressure` calls it when the method runs, and `PressureCall` calls it again on each poll while the port answers `SendError::Busy`.
    pub(crate) fn send_pressure<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        window: &super::Window,
    ) -> ::core::result::Result<PressureCorrelation, ::ridl_rt::port::SendError> {
        let __arg = window;
        <super::ValvePressure as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = [0u8; <super::Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Window,
            ::ridl_rt::encoding::FlatBuffers,
        >::encode(__arg, &mut buf) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding `Window` needs {} bytes and the argument buffer has {}; a legal value cannot exceed `<Window as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Window` failed"),
        };
        __port
            .query(
                <super::Valve as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(2u32),
                bytes,
            )
            .map(PressureCorrelation)
    }
    ///Reads query `pressure`'s reply: `Ok(Some)` once it is known, `Ok(None)` while it is not, and the port's own failure when the read itself fails. It does not wait; `PressureCall` polls it.
    pub(crate) fn poll_pressure_reply<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: PressureCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = [0u8; <super::Average as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE];
        match port.reply(correlation.0, &mut buf)? {
            None => Ok(None),
            Some(Err(error)) => Ok(Some(Err(error))),
            Some(Ok(len)) => {
                Ok(
                    Some(
                        match ::ridl_rt::payload::Ref::<
                            super::Average,
                            ::ridl_rt::encoding::FlatBuffers,
                        >::verify(&buf[..len]) {
                            Ok(checked) => Ok(checked.decode()),
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
                        },
                    ),
                )
            }
        }
    }
    /**What an application implements to serve interface `Valve`'s calls.

An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses, and the generated payload types implement neither `Copy` nor `Clone`.*/
    pub trait Provider {
        ///Serves command `open`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        fn open(&mut self, level: &super::Level);
        ///Serves query `pressure`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        fn pressure(&mut self, window: &super::Window) -> super::Average;
    }
    /**Settles the claims of interface `Valve` that are waiting, up to `budget` of them, and returns how many were settled, or the handler port's failure. It is the one-pass step `serve` calls on each poll.

It does not wait: it makes one pass over the claims the handler already has and returns. `budget` is decreased by one for each claim taken from `Handler::next_claim`, whether or not its settlement is accepted — except an oversized claim whose settlement the handler refused, below — and the pass stops when it reaches 0 without asking for another claim. With a buffer of at least `Valve::MAX_BUFFER_SIZE` bytes, `Ok` with `budget` above 0 means the handler has no claim waiting, or refused the settlement of an oversized claim that stays waiting; `Ok` with `budget` at 0 means claims may still be waiting; `Err` means `Handler::next_claim` failed, and every claim settled before the failure stays settled. `ReadError::ShortClaim` is not a failure: the claim's arguments do not fit `Valve::MAX_BUFFER_SIZE`, the interface's largest argument or reply payload, so they are larger than any valid encoding of this interface's members, and the claim is settled `Transport::Corrupt` by its id without being read, whichever interface or member it names — a claim naming another interface may be validly larger, and is settled `Corrupt` too, because this step cannot read it; it counts toward `budget`, and the pass continues. When the handler refuses that settlement the pass ends at once, returning the count so far with the budget unspent, as if no claim were waiting, because the runtime keeps the unsettled claim the next one; the claims behind it wait until the handler can settle it (driftsys/ridl#569).

`buf` must be at least `Valve::MAX_BUFFER_SIZE` bytes, because a reply is encoded into the same buffer as the arguments. A shorter buffer returns `Ok(0)` without consuming a claim or changing `budget`.

Every claim that is taken is settled, including one whose interface number or ordinal this interface does not recognise, which settles `Contract::UnknownInteraction`. A claim is counted only once `Handler::settle` has accepted it; a `SettleError` is left to the handler, which already owns that claim's settlement, and the pass continues with the next claim, except for an oversized claim, whose refused settlement ends the pass as stated above.

A command is settled `Ok(&[])` once its arguments and its `require` clauses pass and **before** the application's method runs, because a command's acknowledgment is a delivery acknowledgment and not a completion one (ridl §6.1, and `Handler`'s own contract). A query is settled after the application returns, because its settlement carries the reply.*/
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Handler,
        P: Provider,
    {
        if buf.len() < super::Valve::MAX_BUFFER_SIZE {
            return Ok(0);
        }
        let mut settled = 0usize;
        while *budget > 0 {
            let settlement = match h.next_claim(buf) {
                Ok(None) => break,
                Err(::ridl_rt::port::ReadError::ShortClaim { claim, .. }) => {
                    let settlement = h
                        .settle(
                            claim,
                            Err(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                ),
                            ),
                        );
                    if settlement.is_err() {
                        return Ok(settled);
                    }
                    settlement
                }
                Err(error) => return Err(error),
                Ok(Some(claim)) => {
                    if claim.iface
                        != <super::Valve as ::ridl_rt::contract::Interface>::NUMBER
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
                            ::ridl_rt::contract::Ordinal(1u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Level,
                                    ::ridl_rt::encoding::FlatBuffers,
                                >::verify(&buf[..claim.len]) {
                                    Ok(checked) => Ok(checked.decode()),
                                    Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                        Err(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Corrupt,
                                            ),
                                        )
                                    }
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
                                };
                                match decoded {
                                    Err(error) => h.settle(claim.id, Err(error)),
                                    Ok(__arg) => {
                                        match <super::ValveOpen as ::ridl_rt::contract::Command>::require(
                                            &__arg,
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
                                                p.open(&__arg);
                                                accepted
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(2u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Window,
                                    ::ridl_rt::encoding::FlatBuffers,
                                >::verify(&buf[..claim.len]) {
                                    Ok(checked) => Ok(checked.decode()),
                                    Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                                        Err(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Corrupt,
                                            ),
                                        )
                                    }
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
                                };
                                match decoded {
                                    Err(error) => h.settle(claim.id, Err(error)),
                                    Ok(__arg) => {
                                        match <super::ValvePressure as ::ridl_rt::contract::Query>::require(
                                            &__arg,
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
                                                let reply = p.pressure(&__arg);
                                                match <super::ValvePressure as ::ridl_rt::contract::Query>::ensure(
                                                    &__arg,
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
                                                            ::ridl_rt::encoding::FlatBuffers,
                                                        >::encode(&reply, buf) {
                                                            Ok(encoded) => encoded.bytes(),
                                                            Err(
                                                                ::ridl_rt::payload::EncodeError::Capacity {
                                                                    needed,
                                                                    available,
                                                                },
                                                            ) => {
                                                                unreachable!(
                                                                    "encoding `Average` needs {} bytes and the dispatch buffer has {}; a legal value cannot exceed `<Average as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
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
                    }
                }
            };
            *budget -= 1;
            if settlement.is_ok() {
                settled += 1;
            }
        }
        Ok(settled)
    }
    ///The most claims one poll of `Serve` takes (driftsys/ridl#568). A future that holds one poll for an unbounded time blocks every other task on a single-threaded executor; 32 bounds one poll and keeps the cost of registering the claim interest, paid once per poll, small beside the claims the poll settles.
    const SERVE_BUDGET: usize = 32;
    ///Serves interface `Valve`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        let state = match h
            .serve(
                <super::Valve as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(1u32), ::ridl_rt::contract::Ordinal(2u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: [0u8; super::Valve::MAX_BUFFER_SIZE],
            state,
        }
    }
    ///The future `serve` returns over interface `Valve`. It holds the handler, the provider, and the claim buffer of `Valve::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > {
        handler: H,
        provider: &'a mut P,
        buf: [u8; super::Valve::MAX_BUFFER_SIZE],
        state: ServeState,
    }
    /// Where `serve` is, as its future's `poll` moves it.
    #[derive(Clone, Copy)]
    enum ServeState {
        /// `Handler::serve` refused the members; the first poll reports
        /// it.
        Refused(::ridl_rt::port::ServeError),
        /// Each poll registers the claim interest and settles at most
        /// `SERVE_BUDGET` claims.
        Serving,
        /// The failure was reported.
        Done,
    }
    /// `Unpin` whatever `H` is: nothing in the future is pinned, and a
    /// frame loop that stores it in its own state polls it through
    /// `Pin::new`.
    impl<
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P> {}
    impl<
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P> {
        type Output = ::core::result::Result<
            ::core::convert::Infallible,
            ::ridl_rt::error::ProviderError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            match this.state {
                ServeState::Done => panic!("`Serve` polled after completion"),
                ServeState::Refused(error) => {
                    this.state = ServeState::Done;
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ProviderError::Serve(error)),
                    )
                }
                ServeState::Serving => {
                    this.handler
                        .wake_on(
                            ::ridl_rt::port::Interest::Claim(
                                <super::Valve as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        &mut this.buf,
                        &mut budget,
                    ) {
                        Ok(_) => {
                            if budget == 0 {
                                cx.waker().wake_by_ref();
                            }
                            ::core::task::Poll::Pending
                        }
                        Err(error) => {
                            this.state = ServeState::Done;
                            ::core::task::Poll::Ready(
                                Err(::ridl_rt::error::ProviderError::Claim(error)),
                            )
                        }
                    }
                }
            }
        }
    }
    ///The traits a consumer of interface `Valve`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Valve`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        /// The instant `timeout` ends for a wait that starts now. `None`
        /// with no timeout, and with a timeout so large that the instant
        /// cannot be represented, which is then a wait with no bound.
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Valve`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, all in scope through the parent module's `prelude`.
        pub struct Client<
            P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > Client<P> {
            ///Sends command `open` and waits for its outcome, as `block_on` over `Client::open`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            pub fn open(
                &mut self,
                level: super::super::Level,
            ) -> ::core::result::Result<(), ::ridl_rt::error::ClientError> {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.open(level);
                match ::ridl_rt::task::block_on(&mut __call, __deadline) {
                    Some(outcome) => outcome,
                    None if __call.sent() => {
                        Err(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Undelivered,
                                ),
                            ),
                        )
                    }
                    None => {
                        Err(
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            ),
                        )
                    }
                }
            }
            ///Sends query `pressure` and waits for its outcome, as `block_on` over `Client::pressure`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            pub fn pressure(
                &mut self,
                window: super::super::Window,
            ) -> ::core::result::Result<
                super::super::Average,
                ::ridl_rt::error::ClientError,
            > {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.pressure(window);
                match ::ridl_rt::task::block_on(&mut __call, __deadline) {
                    Some(outcome) => outcome,
                    None if __call.sent() => {
                        Err(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Timeout,
                                ),
                            ),
                        )
                    }
                    None => {
                        Err(
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            ),
                        )
                    }
                }
            }
        }
        impl<
            P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P> {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held
            /// by value: pass a handle, or a `&mut` borrow of one.
            fn new(port: P) -> Self {
                Client {
                    inner: <super::Client<P> as ::ridl_rt::face::Bind>::new(port),
                    timeout: None,
                }
            }
        }
        impl<
            P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Timeout for Client<P> {
            /// Sets the timeout every waiting method of this client is
            /// bounded by, and returns the client. A timeout shorter than a
            /// member's `max` is accepted, and ends the call first; a longer
            /// one ends an unserved call at `max` only on a runtime that
            /// wakes the call when its bound passes.
            fn with_timeout(mut self, timeout: ::std::time::Duration) -> Self {
                self.timeout = Some(timeout);
                self
            }
            /// Sets or clears the timeout every waiting method of this
            /// client is bounded by.
            fn set_timeout(
                &mut self,
                timeout: ::core::option::Option<::std::time::Duration>,
            ) {
                self.timeout = timeout;
            }
        }
        ///Serves interface `Valve`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
            P: super::Provider,
        {
            let __deadline = __deadline_after(timeout);
            let mut __serve = super::serve(h, p);
            match ::ridl_rt::task::block_on(&mut __serve, __deadline) {
                Some(Ok(never)) => match never {}
                Some(Err(error)) => Err(error),
                None => Ok(()),
            }
        }
    }
}
