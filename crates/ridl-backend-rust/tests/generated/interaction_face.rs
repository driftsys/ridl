/// Cabin temperature, in degrees.
///
/// # Rules
///
/// - range -40 ..= 85
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
///
/// # Rules
///
/// - range 0 ..= 100
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
///
/// # Rules
///
/// - range 0 ..= 100000
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
///
/// # Rules
///
/// - range 0 ..= 1000
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
    /// # Rules
    ///
    /// - range 0 ..= 100
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
    /// The value the bytes hold. `Temperature` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Temperature {
        __ridl_fb_decode_Temperature(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type Buffer = [::core::primitive::u8; 43usize];
    const BUFFER: Self::Buffer = [0u8; 43usize];
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
    /// The value the bytes hold. `Level` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Level {
        __ridl_fb_decode_Level(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type Buffer = [::core::primitive::u8; 43usize];
    const BUFFER: Self::Buffer = [0u8; 43usize];
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
    /// The value the bytes hold. `Window` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Window {
        __ridl_fb_decode_Window(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
    const MAX_SIZE: ::core::primitive::usize = 46usize;
    type Buffer = [::core::primitive::u8; 46usize];
    const BUFFER: Self::Buffer = [0u8; 46usize];
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
    /// The value the bytes hold. `Average` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Average {
        __ridl_fb_decode_Average(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
    const MAX_SIZE: ::core::primitive::usize = 44usize;
    type Buffer = [::core::primitive::u8; 44usize];
    const BUFFER: Self::Buffer = [0u8; 44usize];
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
    /// The value the bytes hold. `Health` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Health {
        __ridl_fb_decode_Health(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
#[allow(deprecated, non_snake_case)]
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
    const MAX_SIZE: ::core::primitive::usize = 50usize;
    type Buffer = [::core::primitive::u8; 50usize];
    const BUFFER: Self::Buffer = [0u8; 50usize];
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
    const MAX_SIZE: ::core::primitive::usize = 60usize;
    type Buffer = [::core::primitive::u8; 60usize];
    const BUFFER: Self::Buffer = [0u8; 60usize];
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
/**Descriptor for interface `Cabin`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Cabin as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Cabin;
impl ::ridl_rt::contract::Interface for Cabin {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
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
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Cabin {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Temperature as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Warning as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64
            + <Average as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
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
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = {
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
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct CabinTemperature;
impl ::ridl_rt::contract::Interaction for CabinTemperature {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Signal for CabinTemperature {
    type Payload = Temperature;
    fn init() -> Self::Payload {
        <Temperature as ::core::default::Default>::default()
    }
}
#[doc(hidden)]
pub struct CabinWarning;
impl ::ridl_rt::contract::Interaction for CabinWarning {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Event for CabinWarning {
    type Payload = Warning;
}
#[doc(hidden)]
pub struct CabinSetLevel;
impl ::ridl_rt::contract::Interaction for CabinSetLevel {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[2];
    const ROW: ::core::primitive::usize = 2;
}
impl ::ridl_rt::contract::Command for CabinSetLevel {
    type Args = Level;
    fn require(args: &Self::Args) -> ::core::result::Result<(), ()> {
        if args.0 < 100 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
}
#[doc(hidden)]
pub struct CabinAverage;
impl ::ridl_rt::contract::Interaction for CabinAverage {
    type Iface = Cabin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Cabin as ::ridl_rt::contract::Interface>::MEMBERS[3];
    const ROW: ::core::primitive::usize = 3;
}
impl ::ridl_rt::contract::Query for CabinAverage {
    type Args = Window;
    type Reply = Average;
    fn require(args: &Self::Args) -> ::core::result::Result<(), ()> {
        if args.0 > 0 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
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
/**Descriptor for interface `Horn`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Horn as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Horn;
impl ::ridl_rt::contract::Interface for Horn {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
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
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Horn {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Health as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct HornActive;
impl ::ridl_rt::contract::Interaction for HornActive {
    type Iface = Horn;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Horn as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Signal for HornActive {
    type Payload = Health;
    fn init() -> Self::Payload {
        <Health as ::core::default::Default>::default()
    }
}
/**Descriptor for interface `Siren`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Siren as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Siren;
impl ::ridl_rt::contract::Interface for Siren {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
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
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Siren {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Warning as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = {
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
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct SirenTripped;
impl ::ridl_rt::contract::Interaction for SirenTripped {
    type Iface = Siren;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Siren as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Event for SirenTripped {
    type Payload = Warning;
}
/**Descriptor for interface `Valve`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Valve as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Valve;
impl ::ridl_rt::contract::Interface for Valve {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(4);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Valve";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "open",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "pressure",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(3000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Window",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Valve {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64
            + <Average as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
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
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct ValveOpen;
impl ::ridl_rt::contract::Interaction for ValveOpen {
    type Iface = Valve;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Valve as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Command for ValveOpen {
    type Args = Level;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
#[doc(hidden)]
pub struct ValvePressure;
impl ::ridl_rt::contract::Interaction for ValvePressure {
    type Iface = Valve;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Valve as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Query for ValvePressure {
    type Args = Window;
    type Reply = Average;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(
        _args: &Self::Args,
        _reply: &Self::Reply,
    ) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
/**Descriptor for interface `Wiper`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Wiper as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Wiper;
impl ::ridl_rt::contract::Interface for Wiper {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(5);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Wiper";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Fixed,
            name: "blades",
            timing: ::core::option::Option::None,
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "speed",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Wiper {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        0u64,
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct WiperBlades;
impl ::ridl_rt::contract::Interaction for WiperBlades {
    type Iface = Wiper;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Wiper as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Fixed for WiperBlades {
    type Payload = Level;
}
#[doc(hidden)]
pub struct WiperSpeed;
impl ::ridl_rt::contract::Interaction for WiperSpeed {
    type Iface = Wiper;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Wiper as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Signal for WiperSpeed {
    type Payload = Level;
    fn init() -> Self::Payload {
        <Level as ::core::default::Default>::default()
    }
}
/**Descriptor for interface `Zone`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.

Its size table in FlatBuffers is `<Zone as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.*/
pub struct Zone;
impl ::ridl_rt::contract::Interface for Zone {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            43, 15, 166, 38, 196, 139, 213, 90, 75, 7, 103, 237, 140, 214, 219, 12, 127,
            233, 1, 103, 97, 104, 101, 27, 130, 196, 155, 191, 49, 217, 231, 223,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(6);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Zone";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Signal,
            name: "level",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::StrictPeriodic,
                min: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(10000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "setLevel",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(3),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "average",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(3000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Window",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Average",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Zone {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64
            + <Average as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
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
    const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = 0usize;
    type ClaimBuffer = [::core::primitive::u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
#[doc(hidden)]
pub struct ZoneLevel;
impl ::ridl_rt::contract::Interaction for ZoneLevel {
    type Iface = Zone;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Zone as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Signal for ZoneLevel {
    type Payload = Level;
    fn init() -> Self::Payload {
        <Level as ::core::default::Default>::default()
    }
}
#[doc(hidden)]
pub struct ZoneSetLevel;
impl ::ridl_rt::contract::Interaction for ZoneSetLevel {
    type Iface = Zone;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Zone as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Command for ZoneSetLevel {
    type Args = Level;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
#[doc(hidden)]
pub struct ZoneAverage;
impl ::ridl_rt::contract::Interaction for ZoneAverage {
    type Iface = Zone;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Zone as ::ridl_rt::contract::Interface>::MEMBERS[2];
    const ROW: ::core::primitive::usize = 2;
}
impl ::ridl_rt::contract::Query for ZoneAverage {
    type Args = Window;
    type Reply = Average;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(
        _args: &Self::Args,
        _reply: &Self::Reply,
    ) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
/// The cabin climate controller. It publishes the cabin [Temperature](crate::face::demo::Temperature) and
/// takes a control [Level](crate::face::demo::Level).
///
/// # See also
///
/// - [Horn](crate::face::demo::horn)
///
/// Available since 1.0.
///
///The generated interaction face of interface `Cabin`.
pub mod cabin {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct SetLevelCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct AverageCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Cabin as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Cabin` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
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
        /// The cabin temperature, read from [Cabin.average](crate::face::demo::cabin::Client::average)'s source sensor.
        ///
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Reads signal `temperature` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn temperature(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Temperature>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = <super::Temperature as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
        /// Sets the control level.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        /// - require `level < 100`
        ///
        /**Sends command `setLevel` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `level`: The level to hold.
        ///   - range 0 ..= 100
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
        /// The average cabin temperature over a window of samples.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        /// - require `window > 0`
        /// - ensure `result >= 0`
        ///
        /// Available since 1.1.
        ///
        /**Sends query `average` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Cabin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
    enum SetLevelPhase {
        Unsent(super::Level),
        Waiting(SetLevelCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > SetLevelCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
    enum AveragePhase {
        Unsent(super::Window),
        Waiting(AverageCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > AverageCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
        let mut buf = <super::Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                ::core::option::Option::None,
            )
            .map(SetLevelCorrelation)
    }
    pub(crate) fn poll_set_level_ack<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: SetLevelCorrelation,
    ) -> ::core::option::Option<
        ::core::result::Result<(), ::ridl_rt::error::CallError>,
    > {
        port.ack(correlation.0)
    }
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
        let mut buf = <super::Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                ::core::option::Option::None,
            )
            .map(AverageCorrelation)
    }
    pub(crate) fn poll_average_reply<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: AverageCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = <super::Average as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
    pub(crate) fn poll_next_event<P: ::ridl_rt::port::EventSource>(
        port: &mut P,
    ) -> ::core::result::Result<
        ::core::option::Option<Event>,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = <super::Cabin as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::FlatBuffers,
        >>::EVENT_BUFFER;
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
        /// The cabin temperature, read from [Cabin.average](crate::face::demo::cabin::Client::average)'s source sensor.
        ///
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Stages a new value for signal `temperature`. It is published by `commit`.
        pub fn temperature(
            &mut self,
            value: super::Temperature,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = <super::Temperature as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
        /// A warning the controller raises.
        ///
        /// # Rules
        ///
        /// - interval 100000 ..= 1000000 us
        ///
        ///Raises one occurrence of event `warning`.
        pub fn warning(
            &mut self,
            value: super::Warning,
        ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
            let mut buf = <super::Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
                    ::core::option::Option::None,
                )
        }
    }
    impl<
        W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink,
    > ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Cabin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: W) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
        /// Sets the control level.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        /// - require `level < 100`
        ///
        ///Serves command `setLevel`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        ///
        /// # Parameters
        ///
        /// - `level`: The level to hold.
        ///   - range 0 ..= 100
        fn set_level(&mut self, level: &super::Level);
        /// The average cabin temperature over a window of samples.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        /// - require `window > 0`
        /// - ensure `result >= 0`
        ///
        /// Available since 1.1.
        ///
        ///Serves query `average`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
        fn average(&mut self, window: &super::Window) -> super::Average;
    }
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
        if buf.len()
            < <super::Cabin as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_BUFFER_SIZE
        {
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
    const SERVE_BUDGET: usize = 32;
    /**Serves interface `Cabin`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Cabin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
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
            buf: <super::Cabin as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::CLAIM_BUFFER,
            state,
        }
    }
    ///The future `serve` returns over interface `Cabin`. It holds the handler, the provider, and the claim buffer of `<Cabin as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > {
        handler: H,
        provider: &'a mut P,
        buf: <super::Cabin as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::FlatBuffers,
        >>::ClaimBuffer,
        state: ServeState,
    }
    #[derive(Clone, Copy)]
    enum ServeState {
        Refused(::ridl_rt::port::ServeError),
        Serving,
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
            /// The cabin temperature, read from [Cabin.average](crate::face::demo::cabin::Client::average)'s source sensor.
            ///
            /// # Rules
            ///
            /// - period 10000 us
            ///
            ///Reads signal `temperature`, as `Client::temperature` does: a read never waits.
            pub fn temperature(
                &self,
            ) -> ::core::result::Result<
                ::ridl_rt::sample::Sample<super::super::Temperature>,
                ::ridl_rt::port::ReadError,
            > {
                self.inner.temperature()
            }
            /// Sets the control level.
            ///
            /// # Rules
            ///
            /// - response within 50000 us
            /// - require `level < 100`
            ///
            ///Sends command `setLevel` and waits for its outcome, as `block_on` over `Client::set_level`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `level`: The level to hold.
            ///   - range 0 ..= 100
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
            /// The average cabin temperature over a window of samples.
            ///
            /// # Rules
            ///
            /// - response within 200000 us
            /// - require `window > 0`
            /// - ensure `result >= 0`
            ///
            /// Available since 1.1.
            ///
            ///Sends query `average` and waits for its outcome, as `block_on` over `Client::average`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `window`
            ///   - range 0 ..= 100000
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
            /**Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Cabin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
            #[track_caller]
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
        /**Serves interface `Cabin`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Cabin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Cabin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
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
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Horn as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Horn` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Horn`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<P: ::ridl_rt::port::SignalReader> {
        port: P,
    }
    impl<P: ::ridl_rt::port::SignalReader> Client<P> {
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Reads signal `active` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn active(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Health>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = <super::Health as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Horn as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Horn` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Stages a new value for signal `active`. It is published by `commit`.
        pub fn active(
            &mut self,
            value: super::Health,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = <super::Health as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Horn as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Horn` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: W) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Siren as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Siren` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Siren as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Siren` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
    pub(crate) fn poll_next_event<P: ::ridl_rt::port::EventSource>(
        port: &mut P,
    ) -> ::core::result::Result<
        ::core::option::Option<Event>,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = <super::Siren as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::FlatBuffers,
        >>::EVENT_BUFFER;
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
        /// # Rules
        ///
        /// - interval 100000 ..= 1000000 us
        ///
        ///Raises one occurrence of event `tripped`.
        pub fn tripped(
            &mut self,
            value: super::Warning,
        ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
            let mut buf = <super::Warning as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
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
                    ::core::option::Option::None,
                )
        }
    }
    impl<W: ::ridl_rt::port::EventSink> ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Siren as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Siren` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: W) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
            /**Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Siren as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Siren` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
            #[track_caller]
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct OpenCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct PressureCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Valve as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Valve` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Valve`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > Client<P> {
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /**Sends command `open` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
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
        /// # Rules
        ///
        /// - response within 3000000 us
        ///
        /**Sends query `pressure` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Valve as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Valve` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
    enum OpenPhase {
        Unsent(super::Level),
        Waiting(OpenCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > OpenCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
    enum PressurePhase {
        Unsent(super::Window),
        Waiting(PressureCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > PressureCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
        let mut buf = <super::Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                ::core::option::Option::None,
            )
            .map(OpenCorrelation)
    }
    pub(crate) fn poll_open_ack<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: OpenCorrelation,
    ) -> ::core::option::Option<
        ::core::result::Result<(), ::ridl_rt::error::CallError>,
    > {
        port.ack(correlation.0)
    }
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
        let mut buf = <super::Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                ::core::option::Option::None,
            )
            .map(PressureCorrelation)
    }
    pub(crate) fn poll_pressure_reply<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: PressureCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = <super::Average as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        ///Serves command `open`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
        fn open(&mut self, level: &super::Level);
        /// # Rules
        ///
        /// - response within 3000000 us
        ///
        ///Serves query `pressure`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
        fn pressure(&mut self, window: &super::Window) -> super::Average;
    }
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
        if buf.len()
            < <super::Valve as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_BUFFER_SIZE
        {
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
    const SERVE_BUDGET: usize = 32;
    /**Serves interface `Valve`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Valve as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Valve` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
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
            buf: <super::Valve as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::CLAIM_BUFFER,
            state,
        }
    }
    ///The future `serve` returns over interface `Valve`. It holds the handler, the provider, and the claim buffer of `<Valve as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > {
        handler: H,
        provider: &'a mut P,
        buf: <super::Valve as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::FlatBuffers,
        >>::ClaimBuffer,
        state: ServeState,
    }
    #[derive(Clone, Copy)]
    enum ServeState {
        Refused(::ridl_rt::port::ServeError),
        Serving,
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
            /// # Rules
            ///
            /// - response within 1000000 us
            ///
            ///Sends command `open` and waits for its outcome, as `block_on` over `Client::open`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `level`
            ///   - range 0 ..= 100
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
            /// # Rules
            ///
            /// - response within 3000000 us
            ///
            ///Sends query `pressure` and waits for its outcome, as `block_on` over `Client::pressure`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `window`
            ///   - range 0 ..= 100000
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
            /**Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Valve as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Valve` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
            #[track_caller]
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
        /**Serves interface `Valve`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Valve as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Valve` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
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
///The generated interaction face of interface `Wiper`.
pub mod wiper {
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Wiper as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Wiper` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Wiper`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<P: ::ridl_rt::port::SignalReader> {
        port: P,
    }
    impl<P: ::ridl_rt::port::SignalReader> Client<P> {
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Reads signal `speed` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        pub fn speed(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Level>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = <super::Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
            let raw = self
                .port
                .read(
                    <super::Wiper as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(2u32),
                    &mut buf,
                )?;
            let (value, provenance) = match raw.provenance {
                ::ridl_rt::sample::Provenance::Init
                | ::ridl_rt::sample::Provenance::Invalid(
                    ::ridl_rt::sample::Cause::Declared,
                ) if raw.len == 0 => {
                    (
                        <super::WiperSpeed as ::ridl_rt::contract::Signal>::init(),
                        raw.provenance,
                    )
                }
                _ => {
                    match ::ridl_rt::payload::Ref::<
                        super::Level,
                        ::ridl_rt::encoding::FlatBuffers,
                    >::verify(&buf[..raw.len]) {
                        Ok(checked) => (checked.decode(), raw.provenance),
                        Err(error) => {
                            (
                                <super::WiperSpeed as ::ridl_rt::contract::Signal>::init(),
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
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Wiper as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Wiper` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client { port }
        }
    }
    ///Stages the invalid state of one signal of interface `Wiper`: one method per signal, implemented by `Publisher`. A trait rather than inherent methods so that a member of the interface may be named `invalidate<Signal>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Invalidate {
        ///Stages the invalid state for signal `speed`, with `Cause::Declared`. It is published by `commit`.
        fn invalidate_speed(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError>;
    }
    ///The provider face of interface `Wiper`'s signals and events. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `commit` is `ridl_rt::face::Publish`'s and `invalidate_<signal>` is this module's `Invalidate`'s, all in scope through `prelude`.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter> Publisher<W> {
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Stages a new value for signal `speed`. It is published by `commit`.
        pub fn speed(
            &mut self,
            value: super::Level,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = <super::Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Level,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Level` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Level as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Level` failed"),
            };
            self.port
                .set(
                    <super::Wiper as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(2u32),
                    bytes,
                )
        }
    }
    impl<W: ::ridl_rt::port::SignalWriter> ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Wiper as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Wiper` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: W) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
        fn invalidate_speed(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Wiper as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(2u32),
                )
        }
    }
    ///The traits a consumer of interface `Wiper`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `commit`, `invalidate_<signal>` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        pub use ::ridl_rt::face::Publish;
        pub use super::Invalidate as _;
    }
}
///The generated interaction face of interface `Zone`.
pub mod zone {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct SetLevelCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct AverageCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Zone as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Zone` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Zone`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > Client<P> {
        /// The zone level.
        ///
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Reads signal `level` and returns its value with the provenance, the freshness and the envelope the runtime resolved. Before the first publication, and when the channel is invalidated with no prior publication, there is no payload to check and the value is the channel's init value, under `Provenance::Init` or `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, §4.5). A payload that fails its check is reported as `Provenance::Invalid` with the detection, and the value is the channel's init value.
        #[deprecated(note = "use Cabin.temperature")]
        pub fn level(
            &self,
        ) -> ::core::result::Result<
            ::ridl_rt::sample::Sample<super::Level>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = <super::Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
            let raw = self
                .port
                .read(
                    <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    &mut buf,
                )?;
            let (value, provenance) = match raw.provenance {
                ::ridl_rt::sample::Provenance::Init
                | ::ridl_rt::sample::Provenance::Invalid(
                    ::ridl_rt::sample::Cause::Declared,
                ) if raw.len == 0 => {
                    (
                        <super::ZoneLevel as ::ridl_rt::contract::Signal>::init(),
                        raw.provenance,
                    )
                }
                _ => {
                    match ::ridl_rt::payload::Ref::<
                        super::Level,
                        ::ridl_rt::encoding::FlatBuffers,
                    >::verify(&buf[..raw.len]) {
                        Ok(checked) => (checked.decode(), raw.provenance),
                        Err(error) => {
                            (
                                <super::ZoneLevel as ::ridl_rt::contract::Signal>::init(),
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
        /// Sets the zone level.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /**Sends command `setLevel` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
        #[deprecated(note = "use Cabin.setLevel")]
        pub fn set_level(&mut self, level: super::Level) -> SetLevelCall<'_, P> {
            let __arg = level;
            let deadline = <super::ZoneSetLevel as ::ridl_rt::contract::Interaction>::MEMBER
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
        /// The average zone level over a window of samples.
        ///
        /// # Rules
        ///
        /// - response within 3000000 us
        ///
        /**Sends query `average` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.

The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.*/
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
        #[deprecated(note = "use Cabin.average")]
        pub fn average(&mut self, window: super::Window) -> AverageCall<'_, P> {
            let __arg = window;
            let deadline = <super::ZoneAverage as ::ridl_rt::contract::Interaction>::MEMBER
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
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P> {
        type Port = P;
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Zone as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Zone` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client { port }
        }
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
    enum SetLevelPhase {
        Unsent(super::Level),
        Waiting(SetLevelCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > SetLevelCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
    enum AveragePhase {
        Unsent(super::Window),
        Waiting(AverageCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > AverageCall<'_, P> {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
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
    pub(crate) fn send_set_level<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        level: &super::Level,
    ) -> ::core::result::Result<SetLevelCorrelation, ::ridl_rt::port::SendError> {
        let __arg = level;
        <super::ZoneSetLevel as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = <super::Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(2u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(SetLevelCorrelation)
    }
    pub(crate) fn poll_set_level_ack<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: SetLevelCorrelation,
    ) -> ::core::option::Option<
        ::core::result::Result<(), ::ridl_rt::error::CallError>,
    > {
        port.ack(correlation.0)
    }
    pub(crate) fn send_average<P: ::ridl_rt::port::Caller>(
        __port: &mut P,
        window: &super::Window,
    ) -> ::core::result::Result<AverageCorrelation, ::ridl_rt::port::SendError> {
        let __arg = window;
        <super::ZoneAverage as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut buf = <super::Window as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
                <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(3u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(AverageCorrelation)
    }
    pub(crate) fn poll_average_reply<P: ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: AverageCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Average, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    > {
        let mut buf = <super::Average as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::BUFFER;
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
    ///Stages the invalid state of one signal of interface `Zone`: one method per signal, implemented by `Publisher`. A trait rather than inherent methods so that a member of the interface may be named `invalidate<Signal>` (ADR-0023 decision 7); `prelude` brings it into scope anonymously.
    pub trait Invalidate {
        ///Stages the invalid state for signal `level`, with `Cause::Declared`. It is published by `commit`.
        fn invalidate_level(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError>;
    }
    ///The provider face of interface `Zone`'s signals and events. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `commit` is `ridl_rt::face::Publish`'s and `invalidate_<signal>` is this module's `Invalidate`'s, all in scope through `prelude`.
    pub struct Publisher<W: ::ridl_rt::port::SignalWriter> {
        port: W,
    }
    impl<W: ::ridl_rt::port::SignalWriter> Publisher<W> {
        /// The zone level.
        ///
        /// # Rules
        ///
        /// - period 10000 us
        ///
        ///Stages a new value for signal `level`. It is published by `commit`.
        pub fn level(
            &mut self,
            value: super::Level,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            let mut buf = <super::Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::BUFFER;
            let bytes = match ::ridl_rt::payload::Ref::<
                super::Level,
                ::ridl_rt::encoding::FlatBuffers,
            >::encode(&value, &mut buf) {
                Ok(encoded) => encoded.bytes(),
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding `Level` needs {} bytes and the payload buffer has {}; a legal value cannot exceed `<Level as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside its own type's range or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding `Level` failed"),
            };
            self.port
                .set(
                    <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                    bytes,
                )
        }
    }
    impl<W: ::ridl_rt::port::SignalWriter> ::ridl_rt::face::Bind for Publisher<W> {
        type Port = W;
        /**Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Zone as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Zone` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
        fn new(port: W) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
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
        fn invalidate_level(
            &mut self,
        ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
            self.port
                .invalidate(
                    <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                    ::ridl_rt::contract::Ordinal(1u32),
                )
        }
    }
    /**What an application implements to serve interface `Zone`'s calls.

An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses, and the generated payload types implement neither `Copy` nor `Clone`.*/
    pub trait Provider {
        /// Sets the zone level.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        ///Serves command `setLevel`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
        #[deprecated(note = "use Cabin.setLevel")]
        fn set_level(&mut self, level: &super::Level);
        /// The average zone level over a window of samples.
        ///
        /// # Rules
        ///
        /// - response within 3000000 us
        ///
        ///Serves query `average`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        ///
        /// # Parameters
        ///
        /// - `window`
        ///   - range 0 ..= 100000
        #[deprecated(note = "use Cabin.average")]
        fn average(&mut self, window: &super::Window) -> super::Average;
    }
    #[allow(deprecated)]
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
        if buf.len()
            < <super::Zone as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_BUFFER_SIZE
        {
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
                        != <super::Zone as ::ridl_rt::contract::Interface>::NUMBER
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
                            ::ridl_rt::contract::Ordinal(2u32) => {
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
                                        match <super::ZoneSetLevel as ::ridl_rt::contract::Command>::require(
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
                            ::ridl_rt::contract::Ordinal(3u32) => {
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
                                        match <super::ZoneAverage as ::ridl_rt::contract::Query>::require(
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
                                                match <super::ZoneAverage as ::ridl_rt::contract::Query>::ensure(
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
    const SERVE_BUDGET: usize = 32;
    /**Serves interface `Zone`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Zone as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Zone` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(2u32), ::ridl_rt::contract::Ordinal(3u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Zone as ::ridl_rt::contract::Sizes<
                ::ridl_rt::encoding::FlatBuffers,
            >>::CLAIM_BUFFER,
            state,
        }
    }
    ///The future `serve` returns over interface `Zone`. It holds the handler, the provider, and the claim buffer of `<Zone as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
        P: Provider,
    > {
        handler: H,
        provider: &'a mut P,
        buf: <super::Zone as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::FlatBuffers,
        >>::ClaimBuffer,
        state: ServeState,
    }
    #[derive(Clone, Copy)]
    enum ServeState {
        Refused(::ridl_rt::port::ServeError),
        Serving,
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
                                <super::Zone as ::ridl_rt::contract::Interface>::NUMBER,
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
    ///The traits a consumer of interface `Zone`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `commit`, `invalidate_<signal>`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        pub use ::ridl_rt::face::Publish;
        pub use super::Invalidate as _;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Zone`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Zone`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, all in scope through the parent module's `prelude`.
        pub struct Client<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > Client<P> {
            /// The zone level.
            ///
            /// # Rules
            ///
            /// - period 10000 us
            ///
            ///Reads signal `level`, as `Client::level` does: a read never waits.
            #[deprecated(note = "use Cabin.temperature")]
            #[allow(deprecated)]
            pub fn level(
                &self,
            ) -> ::core::result::Result<
                ::ridl_rt::sample::Sample<super::super::Level>,
                ::ridl_rt::port::ReadError,
            > {
                self.inner.level()
            }
            /// Sets the zone level.
            ///
            /// # Rules
            ///
            /// - response within 1000000 us
            ///
            ///Sends command `setLevel` and waits for its outcome, as `block_on` over `Client::set_level`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `level`
            ///   - range 0 ..= 100
            #[deprecated(note = "use Cabin.setLevel")]
            #[allow(deprecated)]
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
            /// The average zone level over a window of samples.
            ///
            /// # Rules
            ///
            /// - response within 3000000 us
            ///
            ///Sends query `average` and waits for its outcome, as `block_on` over `Client::average`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            ///
            /// # Parameters
            ///
            /// - `window`
            ///   - range 0 ..= 100000
            #[deprecated(note = "use Cabin.average")]
            #[allow(deprecated)]
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
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P> {
            type Port = P;
            /**Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.

# Panics

Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Zone as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Zone` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
            #[track_caller]
            fn new(port: P) -> Self {
                Client {
                    inner: <super::Client<P> as ::ridl_rt::face::Bind>::new(port),
                    timeout: None,
                }
            }
        }
        impl<
            P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
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
        /**Serves interface `Zone`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.

# Panics

Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Zone as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Zone` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.*/
        #[track_caller]
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
