/// A count with a lower bound of 0 and an upper bound of 200.
///
/// # Rules
///
/// - range 0 ..= 200
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Count(::core::primitive::i64);
impl Count {
    /// Constructs the value, or returns an error when the value breaks a
    /// constraint that the generated code checks.
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
                type_name: ::core::option::Option::Some("Count"),
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 200 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Count"),
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
impl ::core::convert::TryFrom<::core::primitive::i64> for Count {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Count> for ::core::primitive::i64 {
    fn from(value: Count) -> Self {
        value.0
    }
}
impl ::core::default::Default for Count {
    fn default() -> Self {
        Count::new_unchecked(0)
    }
}
/// A level whose range is negative to positive and includes 0.
///
/// # Rules
///
/// - range -5 ..= 10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Level(::core::primitive::i64);
impl Level {
    /// Constructs the value, or returns an error when the value breaks a
    /// constraint that the generated code checks.
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
        if value < -5 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Level"),
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 10 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Level"),
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
/// A label.
///
/// # Rules
///
/// - length 0 ..= 16
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Label(::std::string::String);
impl Label {
    /// Constructs the value, or returns an error when the value breaks a
    /// constraint that the generated code checks.
    pub fn new(
        value: ::std::string::String,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        Self::check(&value)?;
        ::core::result::Result::Ok(Self::new_unchecked(value))
    }
    pub(crate) fn check(
        value: &::core::primitive::str,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
        if (value.chars().count() as ::core::primitive::u64) > 16 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Label"),
                rule: ::ridl_rt::payload::Rule::Length,
            });
        }
        ::core::result::Result::Ok(())
    }
    /// Constructs the value without checking its constraints.
    ///
    /// Safe: nothing here relies on the invariant for memory
    /// soundness. Use it only for a value already known to satisfy
    /// the contract.
    pub const fn new_unchecked(value: ::std::string::String) -> Self {
        Self(value)
    }
    pub fn get(&self) -> &::core::primitive::str {
        &self.0
    }
    pub fn into_inner(self) -> ::std::string::String {
        self.0
    }
}
impl ::core::convert::TryFrom<::std::string::String> for Label {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::std::string::String,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Label> for ::std::string::String {
    fn from(value: Label) -> Self {
        value.0
    }
}
impl ::core::default::Default for Label {
    fn default() -> Self {
        Label::new_unchecked(::std::string::String::new())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum Mode {
    Idle = 0,
    Active = 1,
    Fault = 2,
}
impl ::core::convert::TryFrom<::core::primitive::i64> for Mode {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {
        match value {
            0 => ::core::result::Result::Ok(Self::Idle),
            1 => ::core::result::Result::Ok(Self::Active),
            2 => ::core::result::Result::Ok(Self::Fault),
            _ => {
                ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Mode"),
                    rule: ::ridl_rt::payload::Rule::Variant,
                })
            }
        }
    }
}
impl ::core::convert::From<Mode> for ::core::primitive::i64 {
    fn from(value: Mode) -> Self {
        value as ::core::primitive::i64
    }
}
impl ::core::default::Default for Mode {
    fn default() -> Self {
        Mode::Idle
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// # Rules
    ///
    /// - range 0 ..= 200
    pub count: Count,
    /// # Rules
    ///
    /// - range -5 ..= 10
    pub level: Level,
    /// # Rules
    ///
    /// - length 0 ..= 16
    pub label: Label,
    pub mode: Mode,
    /// # Rules
    ///
    /// - range -1 ..= 1
    pub ratio: ::core::primitive::f64,
    pub flag: ::core::primitive::bool,
}
impl ::core::default::Default for Reading {
    fn default() -> Self {
        Reading {
            count: <Count as ::core::default::Default>::default(),
            level: <Level as ::core::default::Default>::default(),
            label: <Label as ::core::default::Default>::default(),
            mode: <Mode as ::core::default::Default>::default(),
            ratio: 0.0,
            flag: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Number(Count),
    Text(Label),
    Mode(Mode),
    Reading(Reading),
}
impl ::core::default::Default for Outcome {
    fn default() -> Self {
        Outcome::Number(<Count as ::core::default::Default>::default())
    }
}
/// A `Count` value in FlatBuffers bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct CountFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for CountFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> CountFbView<'a> {
    /// The value the bytes hold. `Count` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Count {
        __ridl_fb_decode_Count(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Count(
    value: &Count,
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
pub(crate) fn __ridl_fb_verify_Count(
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
        Count::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Count(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Count {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Count::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                ),
            )
        }
        _ => Count::new_unchecked(0i64),
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Count {
    /// The largest FlatBuffers buffer any legal `Count` encodes to: 43 bytes.
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type Buffer = [::core::primitive::u8; 43usize];
    const BUFFER: Self::Buffer = [0u8; 43usize];
    type View<'a> = CountFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, CountFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Count(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: CountFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<CountFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Count(buf, table)?;
        ::core::result::Result::Ok(CountFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Count(__view.buf, __view.table)
    }
}
/// A `Level` value in FlatBuffers bytes.
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
                    ::ridl_rt::flatbuffers::Field::I8(__s.get() as ::core::primitive::i8)
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
        let __raw = ::ridl_rt::flatbuffers::read_i8(buf, __p)
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
                    ::ridl_rt::flatbuffers::read_i8(buf, __p).unwrap_or(0i8),
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
/// A `Label` value in FlatBuffers bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct LabelFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for LabelFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> LabelFbView<'a> {
    /// The value the bytes hold. `Label` owns its bytes, so this allocates — unlike a struct field of the same type, which a view borrows in place.
    pub fn value(&self) -> Label {
        __ridl_fb_decode_Label(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Label(
    value: &Label,
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
                value: ::ridl_rt::flatbuffers::Field::Offset(
                    builder.push_string(value.get())?,
                ),
            },
        ];
        builder.push_table(8usize, 4usize, 1u16, &__box)?
    })
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Label(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __s = ::ridl_rt::flatbuffers::string(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
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
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Label(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Label {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Label::new_unchecked(
            ::std::string::String::from(
                ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
            ),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Label {
    /// The largest FlatBuffers buffer any legal `Label` encodes to: 122 bytes.
    const MAX_SIZE: ::core::primitive::usize = 122usize;
    type Buffer = [::core::primitive::u8; 122usize];
    const BUFFER: Self::Buffer = [0u8; 122usize];
    type View<'a> = LabelFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, LabelFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Label(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LabelFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LabelFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Label(buf, table)?;
        ::core::result::Result::Ok(LabelFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Label(__view.buf, __view.table)
    }
}
/// A `Mode` value in FlatBuffers bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct ModeFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for ModeFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> ModeFbView<'a> {
    /// The value the bytes hold. `Mode` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Mode {
        __ridl_fb_decode_Mode(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Mode(
    value: &Mode,
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
pub(crate) fn __ridl_fb_verify_Mode(
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
        <Mode as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(__raw)
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Mode(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Mode {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 8usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            <Mode as ::core::convert::TryFrom<
                ::core::primitive::i64,
            >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
                .unwrap_or(Mode::Idle)
        }
        _ => Mode::Idle,
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Mode {
    /// The largest FlatBuffers buffer any legal `Mode` encodes to: 50 bytes.
    const MAX_SIZE: ::core::primitive::usize = 50usize;
    type Buffer = [::core::primitive::u8; 50usize];
    const BUFFER: Self::Buffer = [0u8; 50usize];
    type View<'a> = ModeFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ModeFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Mode(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ModeFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ModeFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Mode(buf, table)?;
        ::core::result::Result::Ok(ModeFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Mode(__view.buf, __view.table)
    }
}
/// A `Reading` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct ReadingFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for ReadingFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> ReadingFbView<'a> {
    /// Reads `Reading`'s `count` field in place.
    pub fn count(&self) -> Count {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Count::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(self.buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Count::new_unchecked(0i64),
        }
    }
    /// Reads `Reading`'s `level` field in place.
    pub fn level(&self) -> Level {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_i8(self.buf, __p).unwrap_or(0i8),
                    ),
                )
            }
            _ => Level::new_unchecked(0i64),
        }
    }
    /// Reads `Reading`'s `label` field in place.
    pub fn label(&self) -> &'a ::core::primitive::str {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 2u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        ::ridl_rt::flatbuffers::string(self.buf, __p).unwrap_or("")
    }
    /// Reads `Reading`'s `mode` field in place.
    pub fn mode(&self) -> Mode {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 3u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                <Mode as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(
                        ::ridl_rt::flatbuffers::read_i64(self.buf, __p).unwrap_or(0i64),
                    )
                    .unwrap_or(Mode::Idle)
            }
            _ => Mode::Idle,
        }
    }
    /// Reads `Reading`'s `ratio` field in place.
    pub fn ratio(&self) -> ::core::primitive::f64 {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 4u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::ridl_rt::flatbuffers::read_f64(self.buf, __p).unwrap_or(0.0f64)
            }
            _ => 0.0f64,
        }
    }
    /// Reads `Reading`'s `flag` field in place.
    pub fn flag(&self) -> ::core::primitive::bool {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 5u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::ridl_rt::flatbuffers::read_bool(self.buf, __p).unwrap_or(false)
            }
            _ => false,
        }
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Reading(
    value: &Reading,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    let mut __fields = [::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: ::ridl_rt::flatbuffers::Field::Bool(false),
    }; 6usize];
    let mut __n = 0usize;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: {
            let __s = value.count;
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 5u16,
        value: {
            let __s = value.level;
            ::ridl_rt::flatbuffers::Field::I8(__s.get() as ::core::primitive::i8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 2u16,
        offset: 8u16,
        value: ::ridl_rt::flatbuffers::Field::Offset(
            builder.push_string(value.label.get())?,
        ),
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 3u16,
        offset: 16u16,
        value: {
            let __s = value.mode;
            ::ridl_rt::flatbuffers::Field::I64(::core::primitive::i64::from(__s))
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 4u16,
        offset: 24u16,
        value: {
            let __s = value.ratio;
            ::ridl_rt::flatbuffers::Field::F64(__s)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 5u16,
        offset: 32u16,
        value: {
            let __s = value.flag;
            ::ridl_rt::flatbuffers::Field::Bool(__s)
        },
    };
    __n += 1;
    builder.push_table(33usize, 8usize, 6u16, &__fields[..__n])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Reading(
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
        Count::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            1u16,
            1usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_i8(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Level::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    match ::ridl_rt::flatbuffers::field(buf, table, 2u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __s = ::ridl_rt::flatbuffers::string(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            3u16,
            8usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        <Mode as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(__raw)
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            4u16,
            8usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_f64(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        (|| -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
            let __value = __raw;
            if !__value.is_finite() {
                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Reading"),
                    rule: ::ridl_rt::payload::Rule::Range,
                });
            }
            if __value < -1.0 {
                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Reading"),
                    rule: ::ridl_rt::payload::Rule::Range,
                });
            }
            if __value > 1.0 {
                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Reading"),
                    rule: ::ridl_rt::payload::Rule::Range,
                });
            }
            ::core::result::Result::Ok(())
        })()
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
            buf,
            table,
            5u16,
            1usize,
        )
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        let __raw = ::ridl_rt::flatbuffers::read_bool(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Reading(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Reading {
    Reading {
        count: match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Count::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Count::new_unchecked(0i64),
        },
        level: match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_i8(buf, __p).unwrap_or(0i8),
                    ),
                )
            }
            _ => Level::new_unchecked(0i64),
        },
        label: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 2u16, 4usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            Label::new_unchecked(
                ::std::string::String::from(
                    ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
                ),
            )
        },
        mode: match ::ridl_rt::flatbuffers::field(buf, table, 3u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                <Mode as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64))
                    .unwrap_or(Mode::Idle)
            }
            _ => Mode::Idle,
        },
        ratio: match ::ridl_rt::flatbuffers::field(buf, table, 4u16, 8usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::ridl_rt::flatbuffers::read_f64(buf, __p).unwrap_or(0.0f64)
            }
            _ => 0.0f64,
        },
        flag: match ::ridl_rt::flatbuffers::field(buf, table, 5u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::ridl_rt::flatbuffers::read_bool(buf, __p).unwrap_or(false)
            }
            _ => false,
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Reading {
    /// The largest FlatBuffers buffer any legal `Reading` encodes to: 186 bytes.
    const MAX_SIZE: ::core::primitive::usize = 186usize;
    type Buffer = [::core::primitive::u8; 186usize];
    const BUFFER: Self::Buffer = [0u8; 186usize];
    type View<'a> = ReadingFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ReadingFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Reading(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ReadingFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ReadingFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Reading(buf, table)?;
        ::core::result::Result::Ok(ReadingFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Reading(__view.buf, __view.table)
    }
}
/// A `Outcome` value in FlatBuffers bytes, read in place without copying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct OutcomeFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for OutcomeFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> OutcomeFbView<'a> {
    /// The value the bytes hold. A union's arms have no one view type, so this decodes `Outcome` rather than borrowing it.
    pub fn value(&self) -> Outcome {
        __ridl_fb_decode_Outcome(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Outcome(
    value: &Outcome,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    let (__d, __v) = match value {
        Outcome::Number(__a) => {
            (
                1u8,
                {
                    let __box = [
                        ::ridl_rt::flatbuffers::TableField {
                            slot: 0u16,
                            offset: 4u16,
                            value: {
                                let __s = *__a;
                                ::ridl_rt::flatbuffers::Field::U8(
                                    __s.get() as ::core::primitive::u8,
                                )
                            },
                        },
                    ];
                    builder.push_table(5usize, 4usize, 1u16, &__box)?
                },
            )
        }
        Outcome::Text(__a) => {
            (
                2u8,
                {
                    let __box = [
                        ::ridl_rt::flatbuffers::TableField {
                            slot: 0u16,
                            offset: 4u16,
                            value: ::ridl_rt::flatbuffers::Field::Offset(
                                builder.push_string(__a.get())?,
                            ),
                        },
                    ];
                    builder.push_table(8usize, 4usize, 1u16, &__box)?
                },
            )
        }
        Outcome::Mode(__a) => {
            (
                3u8,
                {
                    let __box = [
                        ::ridl_rt::flatbuffers::TableField {
                            slot: 0u16,
                            offset: 8u16,
                            value: {
                                let __s = *__a;
                                ::ridl_rt::flatbuffers::Field::I64(
                                    ::core::primitive::i64::from(__s),
                                )
                            },
                        },
                    ];
                    builder.push_table(16usize, 8usize, 1u16, &__box)?
                },
            )
        }
        Outcome::Reading(__a) => (4u8, __ridl_fb_encode_Reading(__a, builder)?),
    };
    let __fields = [
        ::ridl_rt::flatbuffers::TableField {
            slot: 0u16,
            offset: 4u16,
            value: ::ridl_rt::flatbuffers::Field::U8(__d),
        },
        ::ridl_rt::flatbuffers::TableField {
            slot: 1u16,
            offset: 8u16,
            value: ::ridl_rt::flatbuffers::Field::Offset(__v),
        },
    ];
    builder.push_table(12usize, 4usize, 2u16, &__fields)
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Outcome(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let __dp = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
    let __vp = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
    let (__d, __value) = match (__dp, __vp) {
        (::core::option::Option::Some(__d), ::core::option::Option::Some(__value)) => {
            (
                ::ridl_rt::flatbuffers::read_u8(buf, __d)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                __value,
            )
        }
        _ => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::Union,
                ),
            );
        }
    };
    match __d {
        1u8 => {
            let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
                    buf,
                    __t,
                    0u16,
                    1usize,
                )
                .map_err(::ridl_rt::payload::VerifyError::Structure)?
            {
                let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                Count::check(&(::core::primitive::i64::from(__raw)))
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
        }
        2u8 => {
            let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            match ::ridl_rt::flatbuffers::field(buf, __t, 0u16, 4usize)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?
            {
                ::core::option::Option::Some(__p) => {
                    let __s = ::ridl_rt::flatbuffers::string(buf, __p)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    Label::check(__s)
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
        }
        3u8 => {
            let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
                    buf,
                    __t,
                    0u16,
                    8usize,
                )
                .map_err(::ridl_rt::payload::VerifyError::Structure)?
            {
                let __raw = ::ridl_rt::flatbuffers::read_i64(buf, __p)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                <Mode as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(__raw)
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
        }
        4u8 => {
            let __t = ::ridl_rt::flatbuffers::follow(buf, __value)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            __ridl_fb_verify_Reading(buf, __t)?;
        }
        _ => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::Union,
                ),
            );
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Outcome(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Outcome {
    let __d = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize)
        .unwrap_or(::core::option::Option::None)
        .map(|__p| ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8))
        .unwrap_or(0u8);
    let __value = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
        .unwrap_or(::core::option::Option::None)
        .unwrap_or(0usize);
    match __d {
        1u8 => {
            Outcome::Number({
                let __t = ::ridl_rt::flatbuffers::follow(buf, __value).unwrap_or(0usize);
                match ::ridl_rt::flatbuffers::field(buf, __t, 0u16, 1usize) {
                    ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                        Count::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                            ),
                        )
                    }
                    _ => Count::new_unchecked(0i64),
                }
            })
        }
        2u8 => {
            Outcome::Text({
                let __t = ::ridl_rt::flatbuffers::follow(buf, __value).unwrap_or(0usize);
                {
                    let __p = ::ridl_rt::flatbuffers::field(buf, __t, 0u16, 4usize)
                        .unwrap_or(::core::option::Option::None)
                        .unwrap_or(0usize);
                    Label::new_unchecked(
                        ::std::string::String::from(
                            ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
                        ),
                    )
                }
            })
        }
        3u8 => {
            Outcome::Mode({
                let __t = ::ridl_rt::flatbuffers::follow(buf, __value).unwrap_or(0usize);
                match ::ridl_rt::flatbuffers::field(buf, __t, 0u16, 8usize) {
                    ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                        <Mode as ::core::convert::TryFrom<
                            ::core::primitive::i64,
                        >>::try_from(
                                ::ridl_rt::flatbuffers::read_i64(buf, __p).unwrap_or(0i64),
                            )
                            .unwrap_or(Mode::Idle)
                    }
                    _ => Mode::Idle,
                }
            })
        }
        4u8 => {
            Outcome::Reading(
                __ridl_fb_decode_Reading(
                    buf,
                    ::ridl_rt::flatbuffers::follow(buf, __value).unwrap_or(0usize),
                ),
            )
        }
        _ => {
            Outcome::Number({
                let __t = ::ridl_rt::flatbuffers::follow(buf, __value).unwrap_or(0usize);
                match ::ridl_rt::flatbuffers::field(buf, __t, 0u16, 1usize) {
                    ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                        Count::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                            ),
                        )
                    }
                    _ => Count::new_unchecked(0i64),
                }
            })
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Outcome {
    /// The largest FlatBuffers buffer any legal `Outcome` encodes to: 231 bytes.
    const MAX_SIZE: ::core::primitive::usize = 231usize;
    type Buffer = [::core::primitive::u8; 231usize];
    const BUFFER: Self::Buffer = [0u8; 231usize];
    type View<'a> = OutcomeFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, OutcomeFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Outcome(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: OutcomeFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<OutcomeFbView<'_>, ::ridl_rt::payload::VerifyError> {
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
        __ridl_fb_verify_Outcome(buf, table)?;
        ::core::result::Result::Ok(OutcomeFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Outcome(__view.buf, __view.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Count(value: &Count) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = *value;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Count(
    value: &Count,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = *value;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Count(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 1usize];
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        if __field.number == 1u32 {
            if __seen[0usize] {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Structure(
                        ::ridl_rt::payload::Malformed::Duplicate,
                    ),
                );
            }
            __seen[0usize] = true;
            match __field.value {
                ::ridl_rt::proto3::Value::Varint(__raw) => {
                    let __v = ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_u32(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    );
                    Count::check(&__v)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                _ => {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::WireType,
                        ),
                    );
                }
            }
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Count(buf: &[::core::primitive::u8]) -> Count {
    let mut __f0 = Count::new_unchecked(0i64);
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                __f0 = Count::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                    ),
                );
            }
        }
    }
    __f0
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Count {
    /// The largest proto3 message any legal `Count` encodes to: 6 bytes.
    const MAX_SIZE: ::core::primitive::usize = 6usize;
    type Buffer = [::core::primitive::u8; 6usize];
    const BUFFER: Self::Buffer = [0u8; 6usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Count(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Count(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Count(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Count(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Level(value: &Level) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = *value;
        let __r = ::core::primitive::u64::from(
            ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
        );
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Level(
    value: &Level,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = *value;
        let __r = ::core::primitive::u64::from(
            ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
        );
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Level(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 1usize];
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        if __field.number == 1u32 {
            if __seen[0usize] {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Structure(
                        ::ridl_rt::payload::Malformed::Duplicate,
                    ),
                );
            }
            __seen[0usize] = true;
            match __field.value {
                ::ridl_rt::proto3::Value::Varint(__raw) => {
                    let __v = ::core::primitive::i64::from(
                        ::ridl_rt::proto3::unzigzag32_checked(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    );
                    Level::check(&__v)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                _ => {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::WireType,
                        ),
                    );
                }
            }
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Level(buf: &[::core::primitive::u8]) -> Level {
    let mut __f0 = Level::new_unchecked(0i64);
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                __f0 = Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::unzigzag32_checked(__raw).unwrap_or(0i32),
                    ),
                );
            }
        }
    }
    __f0
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Level {
    /// The largest proto3 message any legal `Level` encodes to: 6 bytes.
    const MAX_SIZE: ::core::primitive::usize = 6usize;
    type Buffer = [::core::primitive::u8; 6usize];
    const BUFFER: Self::Buffer = [0u8; 6usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Level(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Level(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Level(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Level(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Label(value: &Label) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.get();
        if !__s.is_empty() {
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64),
                )
                .saturating_add(__s.len());
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Label(
    value: &Label,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = value.get();
        if !__s.is_empty() {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__s.len() as ::core::primitive::u64);
            writer.bytes(__s.as_bytes());
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Label(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 1usize];
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        if __field.number == 1u32 {
            if __seen[0usize] {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Structure(
                        ::ridl_rt::payload::Malformed::Duplicate,
                    ),
                );
            }
            __seen[0usize] = true;
            match __field.value {
                ::ridl_rt::proto3::Value::Len(__b) => {
                    let __s = ::ridl_rt::proto3::utf8(__b)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    Label::check(__s)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                _ => {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::WireType,
                        ),
                    );
                }
            }
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Label(buf: &[::core::primitive::u8]) -> Label {
    let mut __f0 = Label::new_unchecked(::std::string::String::new());
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                __f0 = Label::new_unchecked(
                    ::std::string::String::from(
                        ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                    ),
                );
            }
        }
    }
    __f0
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Label {
    /// The largest proto3 message any legal `Label` encodes to: 66 bytes.
    const MAX_SIZE: ::core::primitive::usize = 66usize;
    type Buffer = [::core::primitive::u8; 66usize];
    const BUFFER: Self::Buffer = [0u8; 66usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Label(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Label(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Label(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Label(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Mode(value: &Mode) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = *value;
        let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Mode(
    value: &Mode,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = *value;
        let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Mode(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 1usize];
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        if __field.number == 1u32 {
            if __seen[0usize] {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Structure(
                        ::ridl_rt::payload::Malformed::Duplicate,
                    ),
                );
            }
            __seen[0usize] = true;
            match __field.value {
                ::ridl_rt::proto3::Value::Varint(__raw) => {
                    let __v = ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_i32(__raw)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                    );
                    <Mode as ::core::convert::TryFrom<
                        ::core::primitive::i64,
                    >>::try_from(__v)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                _ => {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::WireType,
                        ),
                    );
                }
            }
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Mode(buf: &[::core::primitive::u8]) -> Mode {
    let mut __f0 = Mode::Idle;
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                __f0 = <Mode as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(
                        ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_i32(__raw).unwrap_or(0i32),
                        ),
                    )
                    .unwrap_or(Mode::Idle);
            }
        }
    }
    __f0
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Mode {
    /// The largest proto3 message any legal `Mode` encodes to: 2 bytes.
    const MAX_SIZE: ::core::primitive::usize = 2usize;
    type Buffer = [::core::primitive::u8; 2usize];
    const BUFFER: Self::Buffer = [0u8; 2usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Mode(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Mode(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Mode(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Mode(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Reading(value: &Reading) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.count;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    {
        let __s = value.level;
        let __r = ::core::primitive::u64::from(
            ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
        );
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    {
        let __s = value.label.get();
        if !__s.is_empty() {
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64),
                )
                .saturating_add(__s.len());
        }
    }
    {
        let __s = value.mode;
        let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    {
        let __s = value.ratio;
        let __r = __s.to_bits();
        if __r != 0 {
            __n = __n.saturating_add(1usize + 8usize);
        }
    }
    {
        let __s = value.flag;
        let __r = ::core::primitive::u64::from(__s);
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Reading(
    value: &Reading,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = value.count;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    {
        let __s = value.level;
        let __r = ::core::primitive::u64::from(
            ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
        );
        if __r != 0 {
            writer.tag(2u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    {
        let __s = value.label.get();
        if !__s.is_empty() {
            writer.tag(3u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__s.len() as ::core::primitive::u64);
            writer.bytes(__s.as_bytes());
        }
    }
    {
        let __s = value.mode;
        let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(4u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    {
        let __s = value.ratio;
        let __r = __s.to_bits();
        if __r != 0 {
            writer.tag(5u32, ::ridl_rt::proto3::WireType::I64);
            writer.fixed64(__r);
        }
    }
    {
        let __s = value.flag;
        let __r = ::core::primitive::u64::from(__s);
        if __r != 0 {
            writer.tag(6u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Reading(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 6usize];
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        match __field.number {
            1u32 => {
                if __seen[0usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[0usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_u32(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        Count::check(&__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            2u32 => {
                if __seen[1usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[1usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::unzigzag32_checked(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        Level::check(&__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            3u32 => {
                if __seen[2usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[2usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__b) => {
                        let __s = ::ridl_rt::proto3::utf8(__b)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        Label::check(__s)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            4u32 => {
                if __seen[3usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[3usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_i32(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        <Mode as ::core::convert::TryFrom<
                            ::core::primitive::i64,
                        >>::try_from(__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            5u32 => {
                if __seen[4usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[4usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::I64(__raw) => {
                        let __v = ::core::primitive::f64::from_bits(__raw);
                        (|| -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
                            let __value = __v;
                            if !__value.is_finite() {
                                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                                    type_name: ::core::option::Option::Some("Reading"),
                                    rule: ::ridl_rt::payload::Rule::Range,
                                });
                            }
                            if __value < -1.0 {
                                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                                    type_name: ::core::option::Option::Some("Reading"),
                                    rule: ::ridl_rt::payload::Rule::Range,
                                });
                            }
                            if __value > 1.0 {
                                return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                                    type_name: ::core::option::Option::Some("Reading"),
                                    rule: ::ridl_rt::payload::Rule::Range,
                                });
                            }
                            ::core::result::Result::Ok(())
                        })()
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            6u32 => {
                if __seen[5usize] {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __seen[5usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(_) => {}
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Reading(buf: &[::core::primitive::u8]) -> Reading {
    let mut __f0 = Count::new_unchecked(0i64);
    let mut __f1 = Level::new_unchecked(0i64);
    let mut __f2 = Label::new_unchecked(::std::string::String::new());
    let mut __f3 = Mode::Idle;
    let mut __f4 = 0.0f64;
    let mut __f5 = false;
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        match __field.number {
            1u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f0 = Count::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                        ),
                    );
                }
            }
            2u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f1 = Level::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::proto3::unzigzag32_checked(__raw).unwrap_or(0i32),
                        ),
                    );
                }
            }
            3u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __f2 = Label::new_unchecked(
                        ::std::string::String::from(
                            ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                        ),
                    );
                }
            }
            4u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f3 = <Mode as ::core::convert::TryFrom<
                        ::core::primitive::i64,
                    >>::try_from(
                            ::core::primitive::i64::from(
                                ::ridl_rt::proto3::narrow_i32(__raw).unwrap_or(0i32),
                            ),
                        )
                        .unwrap_or(Mode::Idle);
                }
            }
            5u32 => {
                if let ::ridl_rt::proto3::Value::I64(__raw) = __field.value {
                    __f4 = ::core::primitive::f64::from_bits(__raw);
                }
            }
            6u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f5 = __raw != 0u64;
                }
            }
            _ => {}
        }
    }
    Reading {
        count: __f0,
        level: __f1,
        label: __f2,
        mode: __f3,
        ratio: __f4,
        flag: __f5,
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Reading {
    /// The largest proto3 message any legal `Reading` encodes to: 91 bytes.
    const MAX_SIZE: ::core::primitive::usize = 91usize;
    type Buffer = [::core::primitive::u8; 91usize];
    const BUFFER: Self::Buffer = [0u8; 91usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Reading(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Reading(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Reading(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Reading(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Outcome(value: &Outcome) -> ::core::primitive::usize {
    let mut __n = 0usize;
    match value {
        Outcome::Number(__a) => {
            let __s = *__a;
            let __r = __s.get() as ::core::primitive::u64;
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
        Outcome::Text(__a) => {
            let __s = __a.get();
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64),
                )
                .saturating_add(__s.len());
        }
        Outcome::Mode(__a) => {
            let __s = *__a;
            let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
        Outcome::Reading(__a) => {
            let __l = __ridl_pb_len_Reading(__a);
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__l as ::core::primitive::u64),
                )
                .saturating_add(__l);
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Outcome(
    value: &Outcome,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    match value {
        Outcome::Number(__a) => {
            let __s = *__a;
            let __r = __s.get() as ::core::primitive::u64;
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
        Outcome::Text(__a) => {
            let __s = __a.get();
            writer.tag(2u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__s.len() as ::core::primitive::u64);
            writer.bytes(__s.as_bytes());
        }
        Outcome::Mode(__a) => {
            let __s = *__a;
            let __r = ::core::primitive::i64::from(__s) as ::core::primitive::u64;
            writer.tag(3u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
        Outcome::Reading(__a) => {
            let __l = __ridl_pb_len_Reading(__a);
            writer.tag(4u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__l as ::core::primitive::u64);
            __ridl_pb_encode_Reading(__a, writer)?;
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_verify_Outcome(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __arm = false;
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        let __field = __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
        match __field.number {
            1u32 => {
                if __arm {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __arm = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_u32(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        Count::check(&__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            2u32 => {
                if __arm {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __arm = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__b) => {
                        let __s = ::ridl_rt::proto3::utf8(__b)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        Label::check(__s)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            3u32 => {
                if __arm {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __arm = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_i32(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        <Mode as ::core::convert::TryFrom<
                            ::core::primitive::i64,
                        >>::try_from(__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            4u32 => {
                if __arm {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Duplicate,
                        ),
                    );
                }
                __arm = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__b) => {
                        __ridl_pb_verify_Reading(__b)?;
                    }
                    _ => {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::WireType,
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    if !__arm {
        return ::core::result::Result::Err(
            ::ridl_rt::payload::VerifyError::Structure(
                ::ridl_rt::payload::Malformed::MissingRequired,
            ),
        );
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_decode_Outcome(buf: &[::core::primitive::u8]) -> Outcome {
    let mut __out = Outcome::Number(Count::new_unchecked(0i64));
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        match __field.number {
            1u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __out = Outcome::Number(
                        Count::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                            ),
                        ),
                    );
                }
            }
            2u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __out = Outcome::Text(
                        Label::new_unchecked(
                            ::std::string::String::from(
                                ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                            ),
                        ),
                    );
                }
            }
            3u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __out = Outcome::Mode(
                        <Mode as ::core::convert::TryFrom<
                            ::core::primitive::i64,
                        >>::try_from(
                                ::core::primitive::i64::from(
                                    ::ridl_rt::proto3::narrow_i32(__raw).unwrap_or(0i32),
                                ),
                            )
                            .unwrap_or(Mode::Idle),
                    );
                }
            }
            4u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __out = Outcome::Reading(__ridl_pb_decode_Reading(__b));
                }
            }
            _ => {}
        }
    }
    __out
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Outcome {
    /// The largest proto3 message any legal `Outcome` encodes to: 93 bytes.
    const MAX_SIZE: ::core::primitive::usize = 93usize;
    type Buffer = [::core::primitive::u8; 93usize];
    const BUFFER: Self::Buffer = [0u8; 93usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Outcome(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Outcome(self, &mut __writer)?;
        let bytes = __writer.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ::ridl_rt::proto3::Verified::new(bytes),
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::proto3::Verified<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        __ridl_pb_verify_Outcome(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Outcome(::ridl_rt::payload::View::bytes(r.view()))
    }
}
impl Count {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
impl Level {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
impl Label {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
impl Mode {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
impl Reading {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
impl Outcome {
    /** Re-encodes a flatbuffers payload as a proto3 payload.

 The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_flatbuffers_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
    /** Re-encodes a proto3 payload as a flatbuffers payload.

 The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`. The body is [`::ridl_rt::payload::transcode`].*/
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
}
