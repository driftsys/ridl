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
#[allow(clippy::derivable_impls)]
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
#[allow(clippy::derivable_impls)]
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
#[allow(clippy::derivable_impls)]
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
#[allow(clippy::derivable_impls)]
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
#[allow(clippy::derivable_impls)]
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
/// A key of the map field.
///
/// # Rules
///
/// - length 1 ..= 8
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Key(::std::string::String);
impl Key {
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
        if (value.chars().count() as ::core::primitive::u64) < 1 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Key"),
                rule: ::ridl_rt::payload::Rule::Length,
            });
        }
        if (value.chars().count() as ::core::primitive::u64) > 8 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Key"),
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
impl ::core::convert::TryFrom<::std::string::String> for Key {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::std::string::String,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Key> for ::std::string::String {
    fn from(value: Key) -> Self {
        value.0
    }
}
/// A struct that nests inside `Panel`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Inner {
    /// # Rules
    ///
    /// - range 0 ..= 200
    pub count: Count,
    /// # Rules
    ///
    /// - length 0 ..= 16
    pub label: Label,
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Inner {
    fn default() -> Self {
        Inner {
            count: <Count as ::core::default::Default>::default(),
            label: <Label as ::core::default::Default>::default(),
        }
    }
}
/// An optional field, an array, a map and a nested struct.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Panel {
    /// # Rules
    ///
    /// - length 0 ..= 16
    pub note: ::core::option::Option<Label>,
    /// # Rules
    ///
    /// - count 0 ..= 4
    pub levels: ::std::vec::Vec<Level>,
    /// # Rules
    ///
    /// - count 0 ..= 3
    pub by_key: ::std::vec::Vec<(Key, Count)>,
    pub inner: Inner,
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Panel {
    fn default() -> Self {
        Panel {
            note: ::core::option::Option::None,
            levels: ::std::vec::Vec::new(),
            by_key: ::std::vec::Vec::new(),
            inner: <Inner as ::core::default::Default>::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Outcome {
    Number(Count),
    Text(Label),
    Mode(Mode),
    Reading(Reading),
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Outcome {
    fn default() -> Self {
        Outcome::Number(<Count as ::core::default::Default>::default())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GaugeSetParams {
    pub count: Count,
    pub level: Level,
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for GaugeSetParams {
    fn default() -> Self {
        GaugeSetParams {
            count: <Count as ::core::default::Default>::default(),
            level: <Level as ::core::default::Default>::default(),
        }
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
/// A `Key` value in FlatBuffers bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct KeyFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for KeyFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> KeyFbView<'a> {
    /// The value the bytes hold. `Key` owns its bytes, so this allocates — unlike a struct field of the same type, which a view borrows in place.
    pub fn value(&self) -> Key {
        __ridl_fb_decode_Key(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Key(
    value: &Key,
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
pub(crate) fn __ridl_fb_verify_Key(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __s = ::ridl_rt::flatbuffers::string(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            Key::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
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
pub(crate) fn __ridl_fb_decode_Key(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Key {
    {
        let __p = ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        Key::new_unchecked(
            ::std::string::String::from(
                ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
            ),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Key {
    /// The largest FlatBuffers buffer any legal `Key` encodes to: 90 bytes.
    const MAX_SIZE: ::core::primitive::usize = 90usize;
    type Buffer = [::core::primitive::u8; 90usize];
    const BUFFER: Self::Buffer = [0u8; 90usize];
    type View<'a> = KeyFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, KeyFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Key(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: KeyFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<KeyFbView<'_>, ::ridl_rt::payload::VerifyError> {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_Key(buf, table)?;
        ::core::result::Result::Ok(KeyFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Key(__view.buf, __view.table)
    }
}
/// A `Inner` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct InnerFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for InnerFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> InnerFbView<'a> {
    /// Reads `Inner`'s `count` field in place.
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
    /// Reads `Inner`'s `label` field in place.
    pub fn label(&self) -> &'a ::core::primitive::str {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        ::ridl_rt::flatbuffers::string(self.buf, __p).unwrap_or("")
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Inner(
    value: &Inner,
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
            let __s = value.count;
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 8u16,
        value: ::ridl_rt::flatbuffers::Field::Offset(
            builder.push_string(value.label.get())?,
        ),
    };
    __n += 1;
    builder.push_table(12usize, 4usize, 2u16, &__fields[..__n])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Inner(
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
    match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
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
pub(crate) fn __ridl_fb_decode_Inner(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Inner {
    Inner {
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
        label: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            Label::new_unchecked(
                ::std::string::String::from(
                    ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
                ),
            )
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Inner {
    /// The largest FlatBuffers buffer any legal `Inner` encodes to: 132 bytes.
    const MAX_SIZE: ::core::primitive::usize = 132usize;
    type Buffer = [::core::primitive::u8; 132usize];
    const BUFFER: Self::Buffer = [0u8; 132usize];
    type View<'a> = InnerFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, InnerFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Inner(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: InnerFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<InnerFbView<'_>, ::ridl_rt::payload::VerifyError> {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_Inner(buf, table)?;
        ::core::result::Result::Ok(InnerFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Inner(__view.buf, __view.table)
    }
}
/// A `Panel` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct PanelFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for PanelFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> PanelFbView<'a> {
    /// Reads `Panel`'s `note` field in place.
    pub fn note(&self) -> ::core::option::Option<&'a ::core::primitive::str> {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 0u16, 4usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::core::option::Option::Some(
                    ::ridl_rt::flatbuffers::string(self.buf, __p).unwrap_or(""),
                )
            }
            _ => ::core::option::Option::None,
        }
    }
    /// Reads `Panel`'s `levels` field in place.
    pub fn levels(&self) -> ::std::vec::Vec<Level> {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        {
            let __v = ::ridl_rt::flatbuffers::vector(self.buf, __p, 1usize)
                .unwrap_or(::ridl_rt::flatbuffers::Vector {
                    len: 0,
                    first: 0,
                });
            let mut __out = ::std::vec::Vec::with_capacity(__v.len);
            for __i in 0..__v.len {
                let __at = __v.element(__i, 1usize);
                __out
                    .push(
                        Level::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::flatbuffers::read_i8(self.buf, __at)
                                    .unwrap_or(0i8),
                            ),
                        ),
                    );
            }
            __out
        }
    }
    /// Reads `Panel`'s `by_key` field in place.
    pub fn by_key(&self) -> ::std::vec::Vec<(Key, Count)> {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 2u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        {
            let __v = ::ridl_rt::flatbuffers::vector(self.buf, __p, 4usize)
                .unwrap_or(::ridl_rt::flatbuffers::Vector {
                    len: 0,
                    first: 0,
                });
            let mut __out = ::std::vec::Vec::with_capacity(__v.len);
            for __i in 0..__v.len {
                let __t = ::ridl_rt::flatbuffers::follow(
                        self.buf,
                        __v.element(__i, 4usize),
                    )
                    .unwrap_or(0usize);
                __out
                    .push((
                        {
                            let __p = ::ridl_rt::flatbuffers::field(
                                    self.buf,
                                    __t,
                                    0u16,
                                    4usize,
                                )
                                .unwrap_or(::core::option::Option::None)
                                .unwrap_or(0usize);
                            Key::new_unchecked(
                                ::std::string::String::from(
                                    ::ridl_rt::flatbuffers::string(self.buf, __p).unwrap_or(""),
                                ),
                            )
                        },
                        match ::ridl_rt::flatbuffers::field(
                            self.buf,
                            __t,
                            1u16,
                            1usize,
                        ) {
                            ::core::result::Result::Ok(
                                ::core::option::Option::Some(__p),
                            ) => {
                                Count::new_unchecked(
                                    ::core::primitive::i64::from(
                                        ::ridl_rt::flatbuffers::read_u8(self.buf, __p)
                                            .unwrap_or(0u8),
                                    ),
                                )
                            }
                            _ => Count::new_unchecked(0i64),
                        },
                    ));
            }
            __out
        }
    }
    /// Reads `Panel`'s `inner` field in place.
    pub fn inner(&self) -> InnerFbView<'a> {
        let __p = ::ridl_rt::flatbuffers::field(self.buf, self.table, 3u16, 4usize)
            .unwrap_or(::core::option::Option::None)
            .unwrap_or(0usize);
        InnerFbView {
            buf: self.buf,
            table: ::ridl_rt::flatbuffers::follow(self.buf, __p).unwrap_or(0usize),
        }
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Panel(
    value: &Panel,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    let mut __fields = [::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: ::ridl_rt::flatbuffers::Field::Bool(false),
    }; 4usize];
    let mut __n = 0usize;
    if let ::core::option::Option::Some(__v) = &value.note {
        __fields[__n] = ::ridl_rt::flatbuffers::TableField {
            slot: 0u16,
            offset: 4u16,
            value: ::ridl_rt::flatbuffers::Field::Offset(builder.push_string(__v.get())?),
        };
        __n += 1;
    }
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 8u16,
        value: ::ridl_rt::flatbuffers::Field::Offset({
            let __c = &value.levels;
            if __c.len() > 4usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            let mut __bytes: ::std::vec::Vec<::core::primitive::u8> = ::std::vec::Vec::with_capacity(
                __c.len(),
            );
            for __e in __c.iter() {
                let __s = *__e;
                let __r = __s.get() as ::core::primitive::i8;
                __bytes.extend_from_slice(&__r.to_le_bytes());
            }
            builder.push_vector(&__bytes, 1usize)?
        }),
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 2u16,
        offset: 12u16,
        value: ::ridl_rt::flatbuffers::Field::Offset({
            let __c = &value.by_key;
            if __c.len() > 3usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            let mut __offsets: ::std::vec::Vec<::ridl_rt::flatbuffers::Pos> = ::std::vec::Vec::with_capacity(
                __c.len(),
            );
            for __e in __c.iter() {
                __offsets
                    .push(
                        {
                            let mut __fields = [::ridl_rt::flatbuffers::TableField {
                                slot: 0u16,
                                offset: 4u16,
                                value: ::ridl_rt::flatbuffers::Field::Bool(false),
                            }; 2usize];
                            let mut __n = 0usize;
                            __fields[__n] = ::ridl_rt::flatbuffers::TableField {
                                slot: 0u16,
                                offset: 4u16,
                                value: ::ridl_rt::flatbuffers::Field::Offset(
                                    builder.push_string(__e.0.get())?,
                                ),
                            };
                            __n += 1;
                            __fields[__n] = ::ridl_rt::flatbuffers::TableField {
                                slot: 1u16,
                                offset: 8u16,
                                value: {
                                    let __s = __e.1;
                                    ::ridl_rt::flatbuffers::Field::U8(
                                        __s.get() as ::core::primitive::u8,
                                    )
                                },
                            };
                            __n += 1;
                            builder.push_table(9usize, 4usize, 2u16, &__fields[..__n])
                        }?,
                    );
            }
            builder.push_offset_vector(&__offsets)?
        }),
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 3u16,
        offset: 16u16,
        value: ::ridl_rt::flatbuffers::Field::Offset(
            __ridl_fb_encode_Inner(&value.inner, builder)?,
        ),
    };
    __n += 1;
    builder.push_table(20usize, 4usize, 4u16, &__fields[..__n])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Panel(
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
        let __s = ::ridl_rt::flatbuffers::string(buf, __p)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __v = ::ridl_rt::flatbuffers::vector(buf, __p, 1usize)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            if __v.len > 4usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            for __i in 0..__v.len {
                let __at = __v.element(__i, 1usize);
                let __raw = ::ridl_rt::flatbuffers::read_i8(buf, __at)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                Level::check(&(::core::primitive::i64::from(__raw)))
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    match ::ridl_rt::flatbuffers::field(buf, table, 2u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __v = ::ridl_rt::flatbuffers::vector(buf, __p, 4usize)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            if __v.len > 3usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            for __i in 0..__v.len {
                let table = ::ridl_rt::flatbuffers::follow(buf, __v.element(__i, 4usize))
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?
                {
                    ::core::option::Option::Some(__p) => {
                        let __s = ::ridl_rt::flatbuffers::string(buf, __p)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        Key::check(__s)
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
                if let ::core::option::Option::Some(__p) = ::ridl_rt::flatbuffers::field(
                        buf,
                        table,
                        1u16,
                        1usize,
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?
                {
                    let __raw = ::ridl_rt::flatbuffers::read_u8(buf, __p)
                        .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                    Count::check(&(::core::primitive::i64::from(__raw)))
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                let __key = ::ridl_rt::flatbuffers::string(
                        buf,
                        ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?
                            .unwrap_or(0usize),
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                for __previous in 0..__i {
                    let __previous_key = {
                        let table = ::ridl_rt::flatbuffers::follow(
                                buf,
                                __v.element(__previous, 4usize),
                            )
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                        ::ridl_rt::flatbuffers::string(
                                buf,
                                ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize)
                                    .map_err(::ridl_rt::payload::VerifyError::Structure)?
                                    .unwrap_or(0usize),
                            )
                            .map_err(::ridl_rt::payload::VerifyError::Structure)?
                    };
                    if __previous_key == __key {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                                type_name: ::core::option::Option::Some("Panel"),
                                rule: ::ridl_rt::payload::Rule::Unique,
                            }),
                        );
                    }
                }
            }
        }
        ::core::option::Option::None => {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::MissingRequired,
                ),
            );
        }
    }
    match ::ridl_rt::flatbuffers::field(buf, table, 3u16, 4usize)
        .map_err(::ridl_rt::payload::VerifyError::Structure)?
    {
        ::core::option::Option::Some(__p) => {
            let __t = ::ridl_rt::flatbuffers::follow(buf, __p)
                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
            __ridl_fb_verify_Inner(buf, __t)?;
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
pub(crate) fn __ridl_fb_decode_Panel(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Panel {
    Panel {
        note: match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 4usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::core::option::Option::Some(
                    Label::new_unchecked(
                        ::std::string::String::from(
                            ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
                        ),
                    ),
                )
            }
            _ => ::core::option::Option::None,
        },
        levels: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 1u16, 4usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            {
                let __v = ::ridl_rt::flatbuffers::vector(buf, __p, 1usize)
                    .unwrap_or(::ridl_rt::flatbuffers::Vector {
                        len: 0,
                        first: 0,
                    });
                let mut __out = ::std::vec::Vec::with_capacity(__v.len);
                for __i in 0..__v.len {
                    let __at = __v.element(__i, 1usize);
                    __out
                        .push(
                            Level::new_unchecked(
                                ::core::primitive::i64::from(
                                    ::ridl_rt::flatbuffers::read_i8(buf, __at).unwrap_or(0i8),
                                ),
                            ),
                        );
                }
                __out
            }
        },
        by_key: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 2u16, 4usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            {
                let __v = ::ridl_rt::flatbuffers::vector(buf, __p, 4usize)
                    .unwrap_or(::ridl_rt::flatbuffers::Vector {
                        len: 0,
                        first: 0,
                    });
                let mut __out = ::std::vec::Vec::with_capacity(__v.len);
                for __i in 0..__v.len {
                    let __t = ::ridl_rt::flatbuffers::follow(
                            buf,
                            __v.element(__i, 4usize),
                        )
                        .unwrap_or(0usize);
                    __out
                        .push((
                            {
                                let __p = ::ridl_rt::flatbuffers::field(
                                        buf,
                                        __t,
                                        0u16,
                                        4usize,
                                    )
                                    .unwrap_or(::core::option::Option::None)
                                    .unwrap_or(0usize);
                                Key::new_unchecked(
                                    ::std::string::String::from(
                                        ::ridl_rt::flatbuffers::string(buf, __p).unwrap_or(""),
                                    ),
                                )
                            },
                            match ::ridl_rt::flatbuffers::field(buf, __t, 1u16, 1usize) {
                                ::core::result::Result::Ok(
                                    ::core::option::Option::Some(__p),
                                ) => {
                                    Count::new_unchecked(
                                        ::core::primitive::i64::from(
                                            ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                                        ),
                                    )
                                }
                                _ => Count::new_unchecked(0i64),
                            },
                        ));
                }
                __out
            }
        },
        inner: {
            let __p = ::ridl_rt::flatbuffers::field(buf, table, 3u16, 4usize)
                .unwrap_or(::core::option::Option::None)
                .unwrap_or(0usize);
            __ridl_fb_decode_Inner(
                buf,
                ::ridl_rt::flatbuffers::follow(buf, __p).unwrap_or(0usize),
            )
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Panel {
    /// The largest FlatBuffers buffer any legal `Panel` encodes to: 587 bytes.
    const MAX_SIZE: ::core::primitive::usize = 587usize;
    type Buffer = [::core::primitive::u8; 587usize];
    const BUFFER: Self::Buffer = [0u8; 587usize];
    type View<'a> = PanelFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, PanelFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Panel(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: PanelFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<PanelFbView<'_>, ::ridl_rt::payload::VerifyError> {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_Panel(buf, table)?;
        ::core::result::Result::Ok(PanelFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Panel(__view.buf, __view.table)
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
/// A `GaugeSetParams` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct GaugeSetParamsFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for GaugeSetParamsFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> GaugeSetParamsFbView<'a> {
    /// Reads `GaugeSetParams`'s `count` field in place.
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
    /// Reads `GaugeSetParams`'s `level` field in place.
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
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_GaugeSetParams(
    value: &GaugeSetParams,
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
    builder.push_table(6usize, 4usize, 2u16, &__fields[..__n])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_GaugeSetParams(
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
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_GaugeSetParams(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> GaugeSetParams {
    GaugeSetParams {
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
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for GaugeSetParams {
    /// The largest FlatBuffers buffer any legal `GaugeSetParams` encodes to: 53 bytes.
    const MAX_SIZE: ::core::primitive::usize = 53usize;
    type Buffer = [::core::primitive::u8; 53usize];
    const BUFFER: Self::Buffer = [0u8; 53usize];
    type View<'a> = GaugeSetParamsFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, GaugeSetParamsFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_GaugeSetParams(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: GaugeSetParamsFbView {
                buf: bytes,
                table,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        GaugeSetParamsFbView<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_GaugeSetParams(buf, table)?;
        ::core::result::Result::Ok(GaugeSetParamsFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_GaugeSetParams(__view.buf, __view.table)
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Reading(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Key(value: &Key) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.get();
        __n = __n
            .saturating_add(1usize)
            .saturating_add(
                ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64),
            )
            .saturating_add(__s.len());
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Key(
    value: &Key,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = value.get();
        writer.tag(1u32, ::ridl_rt::proto3::WireType::Len);
        writer.varint(__s.len() as ::core::primitive::u64);
        writer.bytes(__s.as_bytes());
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_Key(
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
                    Key::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
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
    if !__seen[0usize] {
        return ::core::result::Result::Err(
            ::ridl_rt::payload::VerifyError::Structure(
                ::ridl_rt::payload::Malformed::MissingRequired,
            ),
        );
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_Key(buf: &[::core::primitive::u8]) -> Key {
    let mut __f0 = Key::new_unchecked(::std::string::String::new());
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                __f0 = Key::new_unchecked(
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
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Key {
    /// The largest proto3 message any legal `Key` encodes to: 34 bytes.
    const MAX_SIZE: ::core::primitive::usize = 34usize;
    type Buffer = [::core::primitive::u8; 34usize];
    const BUFFER: Self::Buffer = [0u8; 34usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Key(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Key(self, &mut __writer)?;
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
        __ridl_pb_verify_Key(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Key(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Inner(value: &Inner) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.count;
        let __r = __s.get() as ::core::primitive::u64;
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
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Inner(
    value: &Inner,
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
        let __s = value.label.get();
        if !__s.is_empty() {
            writer.tag(2u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__s.len() as ::core::primitive::u64);
            writer.bytes(__s.as_bytes());
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_Inner(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 2usize];
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
            _ => {}
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_Inner(buf: &[::core::primitive::u8]) -> Inner {
    let mut __f0 = Count::new_unchecked(0i64);
    let mut __f1 = Label::new_unchecked(::std::string::String::new());
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
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __f1 = Label::new_unchecked(
                        ::std::string::String::from(
                            ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                        ),
                    );
                }
            }
            _ => {}
        }
    }
    Inner { count: __f0, label: __f1 }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Inner {
    /// The largest proto3 message any legal `Inner` encodes to: 72 bytes.
    const MAX_SIZE: ::core::primitive::usize = 72usize;
    type Buffer = [::core::primitive::u8; 72usize];
    const BUFFER: Self::Buffer = [0u8; 72usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Inner(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Inner(self, &mut __writer)?;
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
        __ridl_pb_verify_Inner(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Inner(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Panel(value: &Panel) -> ::core::primitive::usize {
    let mut __n = 0usize;
    if let ::core::option::Option::Some(__v) = &value.note {
        {
            let __s = __v.get();
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__s.len() as ::core::primitive::u64),
                )
                .saturating_add(__s.len());
        }
    }
    {
        let __c = &value.levels;
        if !__c.is_empty() {
            let mut __l = 0usize;
            for __e in __c.iter() {
                let __s = *__e;
                let __r = ::core::primitive::u64::from(
                    ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
                );
                __l = __l.saturating_add(::ridl_rt::proto3::varint_len(__r));
            }
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__l as ::core::primitive::u64),
                )
                .saturating_add(__l);
        }
    }
    {
        let __c = &value.by_key;
        for __e in __c.iter() {
            let __l = {
                let mut __n = 0usize;
                {
                    let __s = __e.0.get();
                    if !__s.is_empty() {
                        __n = __n
                            .saturating_add(1usize)
                            .saturating_add(
                                ::ridl_rt::proto3::varint_len(
                                    __s.len() as ::core::primitive::u64,
                                ),
                            )
                            .saturating_add(__s.len());
                    }
                }
                {
                    let __s = __e.1;
                    let __r = __s.get() as ::core::primitive::u64;
                    if __r != 0 {
                        __n = __n
                            .saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
                    }
                }
                __n
            };
            __n = __n
                .saturating_add(1usize)
                .saturating_add(
                    ::ridl_rt::proto3::varint_len(__l as ::core::primitive::u64),
                )
                .saturating_add(__l);
        }
    }
    {
        let __l = __ridl_pb_len_Inner(&value.inner);
        __n = __n
            .saturating_add(1usize)
            .saturating_add(::ridl_rt::proto3::varint_len(__l as ::core::primitive::u64))
            .saturating_add(__l);
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Panel(
    value: &Panel,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    if let ::core::option::Option::Some(__v) = &value.note {
        {
            let __s = __v.get();
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__s.len() as ::core::primitive::u64);
            writer.bytes(__s.as_bytes());
        }
    }
    {
        let __c = &value.levels;
        if __c.len() > 4usize {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Panel"),
                    rule: ::ridl_rt::payload::Rule::Length,
                }),
            );
        }
        if !__c.is_empty() {
            let mut __l = 0usize;
            for __e in __c.iter() {
                let __s = *__e;
                let __r = ::core::primitive::u64::from(
                    ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
                );
                __l = __l.saturating_add(::ridl_rt::proto3::varint_len(__r));
            }
            writer.tag(2u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__l as ::core::primitive::u64);
            for __e in __c.iter() {
                let __s = *__e;
                let __r = ::core::primitive::u64::from(
                    ::ridl_rt::proto3::zigzag32(__s.get() as ::core::primitive::i32),
                );
                writer.varint(__r);
            }
        }
    }
    {
        let __c = &value.by_key;
        if __c.len() > 3usize {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Panel"),
                    rule: ::ridl_rt::payload::Rule::Length,
                }),
            );
        }
        for (__i, __e) in __c.iter().enumerate() {
            for __p in __c[..__i].iter() {
                if __p.0 == __e.0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                            type_name: ::core::option::Option::Some("Panel"),
                            rule: ::ridl_rt::payload::Rule::Unique,
                        }),
                    );
                }
            }
        }
        for __e in __c.iter() {
            let __l = {
                let mut __n = 0usize;
                {
                    let __s = __e.0.get();
                    if !__s.is_empty() {
                        __n = __n
                            .saturating_add(1usize)
                            .saturating_add(
                                ::ridl_rt::proto3::varint_len(
                                    __s.len() as ::core::primitive::u64,
                                ),
                            )
                            .saturating_add(__s.len());
                    }
                }
                {
                    let __s = __e.1;
                    let __r = __s.get() as ::core::primitive::u64;
                    if __r != 0 {
                        __n = __n
                            .saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
                    }
                }
                __n
            };
            writer.tag(3u32, ::ridl_rt::proto3::WireType::Len);
            writer.varint(__l as ::core::primitive::u64);
            {
                let __s = __e.0.get();
                if !__s.is_empty() {
                    writer.tag(1u32, ::ridl_rt::proto3::WireType::Len);
                    writer.varint(__s.len() as ::core::primitive::u64);
                    writer.bytes(__s.as_bytes());
                }
            }
            {
                let __s = __e.1;
                let __r = __s.get() as ::core::primitive::u64;
                if __r != 0 {
                    writer.tag(2u32, ::ridl_rt::proto3::WireType::Varint);
                    writer.varint(__r);
                }
            }
        }
    }
    {
        let __l = __ridl_pb_len_Inner(&value.inner);
        writer.tag(4u32, ::ridl_rt::proto3::WireType::Len);
        writer.varint(__l as ::core::primitive::u64);
        __ridl_pb_encode_Inner(&value.inner, writer)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_Panel(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 4usize];
    let mut __c1 = 0usize;
    let mut __c2 = 0usize;
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
            2u32 => {
                __seen[1usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__run) => {
                        let mut __at = 0usize;
                        while __at < __run.len() {
                            let __raw = ::ridl_rt::proto3::read_varint(__run, &mut __at)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                            let __v = ::core::primitive::i64::from(
                                ::ridl_rt::proto3::unzigzag32_checked(__raw)
                                    .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                            );
                            Level::check(&__v)
                                .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                            __c1 += 1usize;
                        }
                    }
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        let __v = ::core::primitive::i64::from(
                            ::ridl_rt::proto3::unzigzag32_checked(__raw)
                                .map_err(::ridl_rt::payload::VerifyError::Structure)?,
                        );
                        Level::check(&__v)
                            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                        __c1 += 1usize;
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
                __seen[2usize] = true;
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__b) => {
                        {
                            let mut __seen = [false; 2usize];
                            let mut __reader = ::ridl_rt::proto3::Reader::new(__b);
                            while let ::core::option::Option::Some(__field) = __reader
                                .next()
                            {
                                let __field = __field
                                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
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
                                            ::ridl_rt::proto3::Value::Len(__b) => {
                                                let __s = ::ridl_rt::proto3::utf8(__b)
                                                    .map_err(::ridl_rt::payload::VerifyError::Structure)?;
                                                Key::check(__s)
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
                                    _ => {}
                                }
                            }
                            if !__seen[0usize] {
                                return ::core::result::Result::Err(
                                    ::ridl_rt::payload::VerifyError::Structure(
                                        ::ridl_rt::payload::Malformed::MissingRequired,
                                    ),
                                );
                            }
                        }
                        if __c2 >= 3usize {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                                    type_name: ::core::option::Option::Some("Panel"),
                                    rule: ::ridl_rt::payload::Rule::Length,
                                }),
                            );
                        }
                        let __k = {
                            let mut __k = &[] as &[::core::primitive::u8];
                            let mut __reader = ::ridl_rt::proto3::Reader::new(__b);
                            while let ::core::option::Option::Some(
                                ::core::result::Result::Ok(__field),
                            ) = __reader.next()
                            {
                                if __field.number == 1u32 {
                                    if let ::ridl_rt::proto3::Value::Len(__kb) = __field.value {
                                        __k = __kb;
                                    }
                                }
                            }
                            __k
                        };
                        let mut __left = __c2;
                        let mut __prev = ::ridl_rt::proto3::Reader::new(buf);
                        while __left > 0usize {
                            match __prev.next() {
                                ::core::option::Option::Some(
                                    ::core::result::Result::Ok(__p),
                                ) => {
                                    if __p.number == 3u32 {
                                        __left -= 1usize;
                                        if let ::ridl_rt::proto3::Value::Len(__pb) = __p.value {
                                            let __pk = {
                                                let mut __k = &[] as &[::core::primitive::u8];
                                                let mut __reader = ::ridl_rt::proto3::Reader::new(__pb);
                                                while let ::core::option::Option::Some(
                                                    ::core::result::Result::Ok(__field),
                                                ) = __reader.next()
                                                {
                                                    if __field.number == 1u32 {
                                                        if let ::ridl_rt::proto3::Value::Len(__kb) = __field.value {
                                                            __k = __kb;
                                                        }
                                                    }
                                                }
                                                __k
                                            };
                                            if __pk == __k {
                                                return ::core::result::Result::Err(
                                                    ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                                                        type_name: ::core::option::Option::Some("Panel"),
                                                        rule: ::ridl_rt::payload::Rule::Unique,
                                                    }),
                                                );
                                            }
                                        }
                                    }
                                }
                                _ => break,
                            }
                        }
                        __c2 += 1usize;
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
                    ::ridl_rt::proto3::Value::Len(__b) => {
                        __ridl_pb_verify_Inner(__b)?;
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
    if __c1 > 4usize {
        return ::core::result::Result::Err(
            ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Panel"),
                rule: ::ridl_rt::payload::Rule::Length,
            }),
        );
    }
    if __c2 > 3usize {
        return ::core::result::Result::Err(
            ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Panel"),
                rule: ::ridl_rt::payload::Rule::Length,
            }),
        );
    }
    if !__seen[3usize] {
        __ridl_pb_verify_Inner(&[])?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_Panel(buf: &[::core::primitive::u8]) -> Panel {
    let mut __f0 = ::core::option::Option::None;
    let mut __f1 = ::std::vec::Vec::new();
    let mut __f2 = ::std::vec::Vec::new();
    let mut __f3 = __ridl_pb_decode_Inner(&[]);
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        match __field.number {
            1u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __f0 = ::core::option::Option::Some(
                        Label::new_unchecked(
                            ::std::string::String::from(
                                ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                            ),
                        ),
                    );
                }
            }
            2u32 => {
                match __field.value {
                    ::ridl_rt::proto3::Value::Len(__run) => {
                        let mut __at = 0usize;
                        while __at < __run.len() {
                            match ::ridl_rt::proto3::read_varint(__run, &mut __at) {
                                ::core::result::Result::Ok(__raw) => {
                                    __f1.push(
                                        Level::new_unchecked(
                                            ::core::primitive::i64::from(
                                                ::ridl_rt::proto3::unzigzag32_checked(__raw).unwrap_or(0i32),
                                            ),
                                        ),
                                    );
                                }
                                ::core::result::Result::Err(_) => break,
                            }
                        }
                    }
                    ::ridl_rt::proto3::Value::Varint(__raw) => {
                        __f1.push(
                            Level::new_unchecked(
                                ::core::primitive::i64::from(
                                    ::ridl_rt::proto3::unzigzag32_checked(__raw).unwrap_or(0i32),
                                ),
                            ),
                        );
                    }
                    _ => {}
                }
            }
            3u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    let mut __k = Key::new_unchecked(::std::string::String::new());
                    let mut __v = Count::new_unchecked(0i64);
                    let mut __reader = ::ridl_rt::proto3::Reader::new(__b);
                    while let ::core::option::Option::Some(
                        ::core::result::Result::Ok(__field),
                    ) = __reader.next()
                    {
                        match __field.number {
                            1u32 => {
                                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                                    __k = Key::new_unchecked(
                                        ::std::string::String::from(
                                            ::ridl_rt::proto3::utf8(__b).unwrap_or(""),
                                        ),
                                    );
                                }
                            }
                            2u32 => {
                                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field
                                    .value
                                {
                                    __v = Count::new_unchecked(
                                        ::core::primitive::i64::from(
                                            ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                                        ),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    __f2.push((__k, __v));
                }
            }
            4u32 => {
                if let ::ridl_rt::proto3::Value::Len(__b) = __field.value {
                    __f3 = __ridl_pb_decode_Inner(__b);
                }
            }
            _ => {}
        }
    }
    Panel {
        note: __f0,
        levels: __f1,
        by_key: __f2,
        inner: __f3,
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Panel {
    /// The largest proto3 message any legal `Panel` encodes to: 288 bytes.
    const MAX_SIZE: ::core::primitive::usize = 288usize;
    type Buffer = [::core::primitive::u8; 288usize];
    const BUFFER: Self::Buffer = [0u8; 288usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_Panel(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Panel(self, &mut __writer)?;
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
        __ridl_pb_verify_Panel(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Panel(::ridl_rt::payload::View::bytes(r.view()))
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
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
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Outcome(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_GaugeSetParams(
    value: &GaugeSetParams,
) -> ::core::primitive::usize {
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
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_GaugeSetParams(
    value: &GaugeSetParams,
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
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_GaugeSetParams(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __seen = [false; 2usize];
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
            _ => {}
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_GaugeSetParams(
    buf: &[::core::primitive::u8],
) -> GaugeSetParams {
    let mut __f0 = Count::new_unchecked(0i64);
    let mut __f1 = Level::new_unchecked(0i64);
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
            _ => {}
        }
    }
    GaugeSetParams {
        count: __f0,
        level: __f1,
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for GaugeSetParams {
    /// The largest proto3 message any legal `GaugeSetParams` encodes to: 12 bytes.
    const MAX_SIZE: ::core::primitive::usize = 12usize;
    type Buffer = [::core::primitive::u8; 12usize];
    const BUFFER: Self::Buffer = [0u8; 12usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_GaugeSetParams(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_GaugeSetParams(self, &mut __writer)?;
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
        __ridl_pb_verify_GaugeSetParams(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_GaugeSetParams(::ridl_rt::payload::View::bytes(r.view()))
    }
}
/// The podbuf head of `Reading`: every field at the offset the first-fit law gives it, with each unused byte run written out as a pad. A buffer holds it after the 8-byte preamble; its size is 24 bytes.
#[allow(deprecated)]
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct ReadingPodbufHead {
    pub count: ::core::primitive::u8,
    pub level: ::core::primitive::i8,
    pub mode: ::core::primitive::u8,
    pub flag: ::core::primitive::u8,
    pub label: ::ridl_rt::podbuf::Desc,
    pub _pad12: [::core::primitive::u8; 4],
    pub ratio: ::core::primitive::f64,
}
const _: () = {
    assert!(::core::mem::size_of:: < ReadingPodbufHead > () == 24usize);
    assert!(::core::mem::align_of:: < ReadingPodbufHead > () == 8usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, count) == 0usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, level) == 1usize);
    assert!(::core::mem::size_of:: < ::core::primitive::i8 > () == 1usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, mode) == 2usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, flag) == 3usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, label) == 4usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::Desc > () == 8usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, _pad12) == 12usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 4] > () == 4usize);
    assert!(::core::mem::offset_of!(ReadingPodbufHead, ratio) == 16usize);
    assert!(::core::mem::size_of:: < ::core::primitive::f64 > () == 8usize);
};
#[allow(deprecated)]
impl Reading {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(24u32)?;
        self.__ridl_podbuf_encode_at(w, __at)
    }
    pub(crate) fn __ridl_podbuf_encode_at(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
        __at: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        w.put_u8(__at, self.count.get() as ::core::primitive::u8);
        w.put_i8(__at + 1usize, self.level.get() as ::core::primitive::i8);
        w.put_u8(
            __at + 2usize,
            ::core::primitive::i64::from(self.mode) as ::core::primitive::u8,
        );
        w.put_u8(__at + 3usize, ::core::primitive::u8::from(self.flag));
        {
            let __s: &::core::primitive::str = self.label.get();
            if __s.as_bytes().contains(&0u8) {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Label"),
                        rule: ::ridl_rt::payload::Rule::Pattern,
                    }),
                );
            }
            if !__s.is_empty() {
                let __n = __s.len();
                let __data = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                w.put_bytes(__data, __s.as_bytes());
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __data as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    },
                );
            }
        }
        w.put_f64(__at + 16usize, self.ratio);
        ::core::result::Result::Ok(())
    }
}
/// A `Reading` value in podbuf bytes, read in place without copying it. Each accessor is a bounds-checked load at the field's offset; a field past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct ReadingPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for ReadingPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> ReadingPodbufView<'a> {
    /// Reads `Reading`'s `count` field in place.
    pub fn count(&self) -> Count {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Count::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
            ),
        )
    }
    /// Reads `Reading`'s `level` field in place.
    pub fn level(&self) -> Level {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Level::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(1usize),
            ),
        )
    }
    /// Reads `Reading`'s `mode` field in place.
    pub fn mode(&self) -> Mode {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        <Mode as ::core::convert::TryFrom<
            ::core::primitive::i64,
        >>::try_from(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(2usize),
                ),
            )
            .unwrap_or(Mode::Idle)
    }
    /// Reads `Reading`'s `flag` field in place.
    pub fn flag(&self) -> ::core::primitive::bool {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(3usize) != 0u8
    }
    /// Reads `Reading`'s `label` field in place.
    pub fn label(&self) -> &'a ::core::primitive::str {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                4usize,
            );
            ::core::str::from_utf8(
                    __b
                        .get(
                            (__d.off
                                as ::core::primitive::usize)..(__d.off
                                as ::core::primitive::usize)
                                .saturating_add(__d.len as ::core::primitive::usize),
                        )
                        .unwrap_or(&[]),
                )
                .unwrap_or("")
        }
    }
    /// Reads `Reading`'s `ratio` field in place.
    pub fn ratio(&self) -> ::core::primitive::f64 {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        ::ridl_rt::podbuf::Region::new(__b, __a, __z).f64_at(16usize)
    }
}
#[allow(deprecated)]
impl Reading {
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ReadingPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __size = __p.head_size as ::core::primitive::usize;
        let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
        Self::__ridl_podbuf_verify_at(
            buf,
            ::ridl_rt::podbuf::PREAMBLE_SIZE,
            __size,
            __tail,
        )?;
        ::core::result::Result::Ok(ReadingPodbufView {
            buf,
            at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_verify_at(
        buf: &[::core::primitive::u8],
        at: ::core::primitive::usize,
        size: ::core::primitive::usize,
        __tail: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
        Count::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(0usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        Level::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).i8_at(1usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        <Mode as ::core::convert::TryFrom<
            ::core::primitive::i64,
        >>::try_from(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(2usize),
                ),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        if ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(3usize) > 1u8 {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Reading"),
                    rule: ::ridl_rt::payload::Rule::Range,
                }),
            );
        }
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(buf, at, size),
                4usize,
            );
            let __s: &::core::primitive::str = if __d.len == 0 {
                if __d.off != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                ""
            } else {
                if ::core::primitive::u64::from(__d.len) > 64u64 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                let __g = ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len) + 1u64,
                    buf.len(),
                    __tail,
                    1usize,
                )?;
                ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
            };
            Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        (|| -> ::core::result::Result<(), ::ridl_rt::payload::Violation> {
            let __value = ::ridl_rt::podbuf::Region::new(buf, at, size).f64_at(16usize);
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
        ::core::result::Result::Ok(())
    }
}
#[allow(deprecated)]
impl Reading {
    pub(crate) fn __ridl_podbuf_decode(view: ReadingPodbufView<'_>) -> Self {
        #[allow(unused_variables)]
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Reading {
            count: Count::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
                ),
            ),
            level: Level::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(1usize),
                ),
            ),
            mode: <Mode as ::core::convert::TryFrom<
                ::core::primitive::i64,
            >>::try_from(
                    ::core::primitive::i64::from(
                        ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(2usize),
                    ),
                )
                .unwrap_or(Mode::Idle),
            flag: ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(3usize) != 0u8,
            label: Label::new_unchecked(
                ::std::string::String::from({
                    let __d = ::ridl_rt::podbuf::Desc::read(
                        &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                        4usize,
                    );
                    ::core::str::from_utf8(
                            __b
                                .get(
                                    (__d.off
                                        as ::core::primitive::usize)..(__d.off
                                        as ::core::primitive::usize)
                                        .saturating_add(__d.len as ::core::primitive::usize),
                                )
                                .unwrap_or(&[]),
                        )
                        .unwrap_or("")
                }),
            ),
            ratio: ::ridl_rt::podbuf::Region::new(__b, __a, __z).f64_at(16usize),
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Reading {
    /// The largest podbuf buffer any legal `Reading` encodes to: 97 bytes.
    const MAX_SIZE: ::core::primitive::usize = 97usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 97usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 97usize],
    >::zeroed();
    type View<'a> = ReadingPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ReadingPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ReadingPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 24usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ReadingPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// The podbuf head of `Inner`: every field at the offset the first-fit law gives it, with each unused byte run written out as a pad. A buffer holds it after the 8-byte preamble; its size is 16 bytes.
#[allow(deprecated)]
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct InnerPodbufHead {
    pub count: ::core::primitive::u8,
    pub _pad1: [::core::primitive::u8; 3],
    pub label: ::ridl_rt::podbuf::Desc,
    pub _pad12: [::core::primitive::u8; 4],
}
const _: () = {
    assert!(::core::mem::size_of:: < InnerPodbufHead > () == 16usize);
    assert!(::core::mem::align_of:: < InnerPodbufHead > () == 8usize);
    assert!(::core::mem::offset_of!(InnerPodbufHead, count) == 0usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(InnerPodbufHead, _pad1) == 1usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 3] > () == 3usize);
    assert!(::core::mem::offset_of!(InnerPodbufHead, label) == 4usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::Desc > () == 8usize);
    assert!(::core::mem::offset_of!(InnerPodbufHead, _pad12) == 12usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 4] > () == 4usize);
};
#[allow(deprecated)]
impl Inner {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(16u32)?;
        self.__ridl_podbuf_encode_at(w, __at)
    }
    pub(crate) fn __ridl_podbuf_encode_at(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
        __at: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        w.put_u8(__at, self.count.get() as ::core::primitive::u8);
        {
            let __s: &::core::primitive::str = self.label.get();
            if __s.as_bytes().contains(&0u8) {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Label"),
                        rule: ::ridl_rt::payload::Rule::Pattern,
                    }),
                );
            }
            if !__s.is_empty() {
                let __n = __s.len();
                let __data = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                w.put_bytes(__data, __s.as_bytes());
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __data as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    },
                );
            }
        }
        ::core::result::Result::Ok(())
    }
}
/// A `Inner` value in podbuf bytes, read in place without copying it. Each accessor is a bounds-checked load at the field's offset; a field past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct InnerPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for InnerPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> InnerPodbufView<'a> {
    /// Reads `Inner`'s `count` field in place.
    pub fn count(&self) -> Count {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Count::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
            ),
        )
    }
    /// Reads `Inner`'s `label` field in place.
    pub fn label(&self) -> &'a ::core::primitive::str {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                4usize,
            );
            ::core::str::from_utf8(
                    __b
                        .get(
                            (__d.off
                                as ::core::primitive::usize)..(__d.off
                                as ::core::primitive::usize)
                                .saturating_add(__d.len as ::core::primitive::usize),
                        )
                        .unwrap_or(&[]),
                )
                .unwrap_or("")
        }
    }
}
#[allow(deprecated)]
impl Inner {
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<InnerPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __size = __p.head_size as ::core::primitive::usize;
        let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
        Self::__ridl_podbuf_verify_at(
            buf,
            ::ridl_rt::podbuf::PREAMBLE_SIZE,
            __size,
            __tail,
        )?;
        ::core::result::Result::Ok(InnerPodbufView {
            buf,
            at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_verify_at(
        buf: &[::core::primitive::u8],
        at: ::core::primitive::usize,
        size: ::core::primitive::usize,
        __tail: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
        Count::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(0usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(buf, at, size),
                4usize,
            );
            let __s: &::core::primitive::str = if __d.len == 0 {
                if __d.off != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                ""
            } else {
                if ::core::primitive::u64::from(__d.len) > 64u64 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                let __g = ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len) + 1u64,
                    buf.len(),
                    __tail,
                    1usize,
                )?;
                ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
            };
            Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::result::Result::Ok(())
    }
}
#[allow(deprecated)]
impl Inner {
    pub(crate) fn __ridl_podbuf_decode(view: InnerPodbufView<'_>) -> Self {
        #[allow(unused_variables)]
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Inner {
            count: Count::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
                ),
            ),
            label: Label::new_unchecked(
                ::std::string::String::from({
                    let __d = ::ridl_rt::podbuf::Desc::read(
                        &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                        4usize,
                    );
                    ::core::str::from_utf8(
                            __b
                                .get(
                                    (__d.off
                                        as ::core::primitive::usize)..(__d.off
                                        as ::core::primitive::usize)
                                        .saturating_add(__d.len as ::core::primitive::usize),
                                )
                                .unwrap_or(&[]),
                        )
                        .unwrap_or("")
                }),
            ),
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Inner {
    /// The largest podbuf buffer any legal `Inner` encodes to: 89 bytes.
    const MAX_SIZE: ::core::primitive::usize = 89usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 89usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 89usize],
    >::zeroed();
    type View<'a> = InnerPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, InnerPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: InnerPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 16usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<InnerPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// The podbuf head of `Panel`: every field at the offset the first-fit law gives it, with each unused byte run written out as a pad. A buffer holds it after the 8-byte preamble; its size is 48 bytes.
#[allow(deprecated)]
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct PanelPodbufHead {
    pub _flags0: ::core::primitive::u8,
    pub _pad1: [::core::primitive::u8; 3],
    pub note: ::ridl_rt::podbuf::Desc,
    pub levels: ::ridl_rt::podbuf::ArrayDesc,
    pub by_key: ::ridl_rt::podbuf::ArrayDesc,
    pub _pad28: [::core::primitive::u8; 4],
    pub inner: InnerPodbufHead,
}
const _: () = {
    assert!(::core::mem::size_of:: < PanelPodbufHead > () == 48usize);
    assert!(::core::mem::align_of:: < PanelPodbufHead > () == 8usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, _flags0) == 0usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, _pad1) == 1usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 3] > () == 3usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, note) == 4usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::Desc > () == 8usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, levels) == 12usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::ArrayDesc > () == 8usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, by_key) == 20usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::ArrayDesc > () == 8usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, _pad28) == 28usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 4] > () == 4usize);
    assert!(::core::mem::offset_of!(PanelPodbufHead, inner) == 32usize);
    assert!(::core::mem::size_of:: < InnerPodbufHead > () == 16usize);
};
#[allow(deprecated)]
impl Panel {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(48u32)?;
        self.__ridl_podbuf_encode_at(w, __at)
    }
    pub(crate) fn __ridl_podbuf_encode_at(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
        __at: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        {
            let mut __f = 0u8;
            if self.note.is_some() {
                __f |= 1u8;
            }
            w.put_u8(__at, __f);
        }
        if let ::core::option::Option::Some(__o) = &self.note {
            {
                let __s: &::core::primitive::str = __o.get();
                if __s.as_bytes().contains(&0u8) {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                            type_name: ::core::option::Option::Some("Label"),
                            rule: ::ridl_rt::payload::Rule::Pattern,
                        }),
                    );
                }
                if !__s.is_empty() {
                    let __n = __s.len();
                    let __data = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                    w.put_bytes(__data, __s.as_bytes());
                    w.put_desc(
                        __at + 4usize,
                        ::ridl_rt::podbuf::Desc {
                            off: __data as ::core::primitive::u32,
                            len: __n as ::core::primitive::u32,
                        },
                    );
                }
            }
        }
        {
            let __c = &self.levels;
            if __c.len() > 4usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            if !__c.is_empty() {
                let __off = w.tail(__c.len() as ::core::primitive::u64, 1usize)?;
                w.put_array_desc(
                    __at + 12usize,
                    ::ridl_rt::podbuf::ArrayDesc {
                        off: __off as ::core::primitive::u32,
                        len: __c.len() as ::core::primitive::u16,
                        elem_size: 1u16,
                    },
                );
                for (__i, __e) in __c.iter().enumerate() {
                    let __ea = __off + __i;
                    w.put_i8(__ea, __e.get() as ::core::primitive::i8);
                }
            }
        }
        {
            let __c = &self.by_key;
            if __c.len() > 3usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            let mut __s: ::std::vec::Vec<_> = __c.iter().collect();
            __s.sort_by(|__a, __b| ::core::cmp::Ord::cmp(
                __a.0.get().as_bytes(),
                __b.0.get().as_bytes(),
            ));
            for __p in __s.windows(2) {
                if __p[0].0.get() == __p[1].0.get() {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                            type_name: ::core::option::Option::Some("Panel"),
                            rule: ::ridl_rt::payload::Rule::Unique,
                        }),
                    );
                }
            }
            if !__s.is_empty() {
                let __off = w
                    .tail((__s.len() * 16usize) as ::core::primitive::u64, 8usize)?;
                w.put_array_desc(
                    __at + 20usize,
                    ::ridl_rt::podbuf::ArrayDesc {
                        off: __off as ::core::primitive::u32,
                        len: __s.len() as ::core::primitive::u16,
                        elem_size: 16u16,
                    },
                );
                for (__i, __e) in __s.iter().enumerate() {
                    let __ea = __off + __i * 16usize;
                    {
                        let __s: &::core::primitive::str = __e.0.get();
                        if __s.as_bytes().contains(&0u8) {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                                    type_name: ::core::option::Option::Some("Key"),
                                    rule: ::ridl_rt::payload::Rule::Pattern,
                                }),
                            );
                        }
                        if !__s.is_empty() {
                            let __n = __s.len();
                            let __data = w
                                .tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                            w.put_bytes(__data, __s.as_bytes());
                            w.put_desc(
                                __ea,
                                ::ridl_rt::podbuf::Desc {
                                    off: __data as ::core::primitive::u32,
                                    len: __n as ::core::primitive::u32,
                                },
                            );
                        }
                    }
                    w.put_u8(__ea + 8usize, __e.1.get() as ::core::primitive::u8);
                }
            }
        }
        Inner::__ridl_podbuf_encode_at(&self.inner, w, __at + 32usize)?;
        ::core::result::Result::Ok(())
    }
}
/// A `Panel` value in podbuf bytes, read in place without copying it. Each accessor is a bounds-checked load at the field's offset; a field past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct PanelPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for PanelPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> PanelPodbufView<'a> {
    /// Reads `Panel`'s `note` field in place.
    pub fn note(&self) -> ::core::option::Option<&'a ::core::primitive::str> {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        if ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize) & 1u8 != 0 {
            ::core::option::Option::Some({
                let __d = ::ridl_rt::podbuf::Desc::read(
                    &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                    4usize,
                );
                ::core::str::from_utf8(
                        __b
                            .get(
                                (__d.off
                                    as ::core::primitive::usize)..(__d.off
                                    as ::core::primitive::usize)
                                    .saturating_add(__d.len as ::core::primitive::usize),
                            )
                            .unwrap_or(&[]),
                    )
                    .unwrap_or("")
            })
        } else {
            ::core::option::Option::None
        }
    }
    /// Reads `Panel`'s `levels` field in place.
    pub fn levels(&self) -> impl ::core::iter::Iterator<Item = Level> + 'a {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                12usize,
            );
            (0..::core::primitive::usize::from(__d.len))
                .map(move |__i| {
                    let __ez = ::core::primitive::usize::from(__d.elem_size);
                    let __es = (__d.off as ::core::primitive::usize)
                        .saturating_add(__i.saturating_mul(__ez));
                    Level::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(__b, __es, __ez).i8_at(0usize),
                        ),
                    )
                })
        }
    }
    /// Reads `Panel`'s `by_key` field in place.
    pub fn by_key(
        &self,
    ) -> impl ::core::iter::Iterator<Item = (&'a ::core::primitive::str, Count)> + 'a {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                20usize,
            );
            (0..::core::primitive::usize::from(__d.len))
                .map(move |__i| {
                    let __ez = ::core::primitive::usize::from(__d.elem_size);
                    let __es = (__d.off as ::core::primitive::usize)
                        .saturating_add(__i.saturating_mul(__ez));
                    (
                        {
                            let __d = ::ridl_rt::podbuf::Desc::read(
                                &::ridl_rt::podbuf::Region::new(__b, __es, __ez),
                                0usize,
                            );
                            ::core::str::from_utf8(
                                    __b
                                        .get(
                                            (__d.off
                                                as ::core::primitive::usize)..(__d.off
                                                as ::core::primitive::usize)
                                                .saturating_add(__d.len as ::core::primitive::usize),
                                        )
                                        .unwrap_or(&[]),
                                )
                                .unwrap_or("")
                        },
                        Count::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::podbuf::Region::new(__b, __es, __ez)
                                    .u8_at(8usize),
                            ),
                        ),
                    )
                })
        }
    }
    /// Reads `Panel`'s `inner` field in place.
    pub fn inner(&self) -> InnerPodbufView<'a> {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        InnerPodbufView {
            buf: __b,
            at: __a + 32usize,
            size: __z.saturating_sub(32usize),
        }
    }
}
#[allow(deprecated)]
impl Panel {
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<PanelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __size = __p.head_size as ::core::primitive::usize;
        let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
        Self::__ridl_podbuf_verify_at(
            buf,
            ::ridl_rt::podbuf::PREAMBLE_SIZE,
            __size,
            __tail,
        )?;
        ::core::result::Result::Ok(PanelPodbufView {
            buf,
            at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_verify_at(
        buf: &[::core::primitive::u8],
        at: ::core::primitive::usize,
        size: ::core::primitive::usize,
        __tail: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
        if ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(0usize) & 1u8 != 0 {
            {
                let __d = ::ridl_rt::podbuf::Desc::read(
                    &::ridl_rt::podbuf::Region::new(buf, at, size),
                    4usize,
                );
                let __s: &::core::primitive::str = if __d.len == 0 {
                    if __d.off != 0 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                    ""
                } else {
                    if ::core::primitive::u64::from(__d.len) > 64u64 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                    let __g = ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len) + 1u64,
                        buf.len(),
                        __tail,
                        1usize,
                    )?;
                    ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
                };
                Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
        } else if ::ridl_rt::podbuf::Region::new(buf, at, size)
            .bytes_at(4usize, 8usize)
            .any(|__byte| __byte != 0u8)
        {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                    type_name: ::core::option::Option::Some("Panel"),
                    rule: ::ridl_rt::payload::Rule::Variant,
                }),
            );
        }
        {
            let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                &::ridl_rt::podbuf::Region::new(buf, at, size),
                12usize,
            );
            if __d.len == 0 {
                if __d.off != 0 || __d.elem_size != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
            } else {
                if ::core::primitive::u32::from(__d.elem_size) != 1u32 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Stride,
                        ),
                    );
                }
                ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len)
                        * ::core::primitive::u64::from(__d.elem_size),
                    buf.len(),
                    __tail,
                    1usize,
                )?;
            }
            if ::core::primitive::usize::from(__d.len) > 4usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            for __i in 0..::core::primitive::usize::from(__d.len) {
                let __ez = ::core::primitive::usize::from(__d.elem_size);
                let __es = (__d.off as ::core::primitive::usize)
                    .saturating_add(__i.saturating_mul(__ez));
                Level::check(
                        &(::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(buf, __es, __ez).i8_at(0usize),
                        )),
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
        }
        {
            let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                &::ridl_rt::podbuf::Region::new(buf, at, size),
                20usize,
            );
            if __d.len == 0 {
                if __d.off != 0 || __d.elem_size != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
            } else {
                if !__d.elem_size.is_multiple_of(8u16) {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::Stride,
                        ),
                    );
                }
                ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len)
                        * ::core::primitive::u64::from(__d.elem_size),
                    buf.len(),
                    __tail,
                    8usize,
                )?;
            }
            if ::core::primitive::usize::from(__d.len) > 3usize {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::VerifyError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Panel"),
                        rule: ::ridl_rt::payload::Rule::Length,
                    }),
                );
            }
            for __i in 0..::core::primitive::usize::from(__d.len) {
                let __ez = ::core::primitive::usize::from(__d.elem_size);
                let __es = (__d.off as ::core::primitive::usize)
                    .saturating_add(__i.saturating_mul(__ez));
                {
                    let __d = ::ridl_rt::podbuf::Desc::read(
                        &::ridl_rt::podbuf::Region::new(buf, __es, __ez),
                        0usize,
                    );
                    let __s: &::core::primitive::str = if __d.len == 0 {
                        if __d.off != 0 {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Structure(
                                    ::ridl_rt::payload::Malformed::OutOfBounds,
                                ),
                            );
                        }
                        ""
                    } else {
                        if ::core::primitive::u64::from(__d.len) > 32u64 {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Structure(
                                    ::ridl_rt::payload::Malformed::OutOfBounds,
                                ),
                            );
                        }
                        let __g = ::ridl_rt::podbuf::tail_range(
                            __d.off,
                            ::core::primitive::u64::from(__d.len) + 1u64,
                            buf.len(),
                            __tail,
                            1usize,
                        )?;
                        ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
                    };
                    Key::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
                Count::check(
                        &(::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(buf, __es, __ez).u8_at(8usize),
                        )),
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                let __r = ::ridl_rt::podbuf::Region::new(buf, __es, __ez);
                if __i > 0 {
                    let __q = ::ridl_rt::podbuf::Region::new(
                        buf,
                        __es.saturating_sub(__ez),
                        __ez,
                    );
                    {
                        let __ka: &[::core::primitive::u8] = {
                            let __d = ::ridl_rt::podbuf::Desc::read(&__q, 0usize);
                            buf.get(
                                    (__d.off
                                        as ::core::primitive::usize)..(__d.off
                                        as ::core::primitive::usize)
                                        .saturating_add(__d.len as ::core::primitive::usize),
                                )
                                .unwrap_or(&[])
                        };
                        let __kb: &[::core::primitive::u8] = {
                            let __d = ::ridl_rt::podbuf::Desc::read(&__r, 0usize);
                            buf.get(
                                    (__d.off
                                        as ::core::primitive::usize)..(__d.off
                                        as ::core::primitive::usize)
                                        .saturating_add(__d.len as ::core::primitive::usize),
                                )
                                .unwrap_or(&[])
                        };
                        if __ka >= __kb {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Structure(
                                    ::ridl_rt::payload::Malformed::KeyOrder,
                                ),
                            );
                        }
                    }
                }
            }
        }
        Inner::__ridl_podbuf_verify_at(
            buf,
            at + 32usize,
            size.saturating_sub(32usize),
            __tail,
        )?;
        ::core::result::Result::Ok(())
    }
}
#[allow(deprecated)]
impl Panel {
    pub(crate) fn __ridl_podbuf_decode(view: PanelPodbufView<'_>) -> Self {
        #[allow(unused_variables)]
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Panel {
            note: if ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize) & 1u8
                != 0
            {
                ::core::option::Option::Some(
                    Label::new_unchecked(
                        ::std::string::String::from({
                            let __d = ::ridl_rt::podbuf::Desc::read(
                                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                                4usize,
                            );
                            ::core::str::from_utf8(
                                    __b
                                        .get(
                                            (__d.off
                                                as ::core::primitive::usize)..(__d.off
                                                as ::core::primitive::usize)
                                                .saturating_add(__d.len as ::core::primitive::usize),
                                        )
                                        .unwrap_or(&[]),
                                )
                                .unwrap_or("")
                        }),
                    ),
                )
            } else {
                ::core::option::Option::None
            },
            levels: {
                let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                    &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                    12usize,
                );
                (0..::core::primitive::usize::from(__d.len))
                    .map(move |__i| {
                        let __ez = ::core::primitive::usize::from(__d.elem_size);
                        let __es = (__d.off as ::core::primitive::usize)
                            .saturating_add(__i.saturating_mul(__ez));
                        Level::new_unchecked(
                            ::core::primitive::i64::from(
                                ::ridl_rt::podbuf::Region::new(__b, __es, __ez)
                                    .i8_at(0usize),
                            ),
                        )
                    })
            }
                .collect::<::std::vec::Vec<Level>>(),
            by_key: {
                let __d = ::ridl_rt::podbuf::ArrayDesc::read(
                    &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                    20usize,
                );
                (0..::core::primitive::usize::from(__d.len))
                    .map(move |__i| {
                        let __ez = ::core::primitive::usize::from(__d.elem_size);
                        let __es = (__d.off as ::core::primitive::usize)
                            .saturating_add(__i.saturating_mul(__ez));
                        (
                            Key::new_unchecked(
                                ::std::string::String::from({
                                    let __d = ::ridl_rt::podbuf::Desc::read(
                                        &::ridl_rt::podbuf::Region::new(__b, __es, __ez),
                                        0usize,
                                    );
                                    ::core::str::from_utf8(
                                            __b
                                                .get(
                                                    (__d.off
                                                        as ::core::primitive::usize)..(__d.off
                                                        as ::core::primitive::usize)
                                                        .saturating_add(__d.len as ::core::primitive::usize),
                                                )
                                                .unwrap_or(&[]),
                                        )
                                        .unwrap_or("")
                                }),
                            ),
                            Count::new_unchecked(
                                ::core::primitive::i64::from(
                                    ::ridl_rt::podbuf::Region::new(__b, __es, __ez)
                                        .u8_at(8usize),
                                ),
                            ),
                        )
                    })
            }
                .collect::<::std::vec::Vec<(Key, Count)>>(),
            inner: Inner::__ridl_podbuf_decode(InnerPodbufView {
                buf: __b,
                at: __a + 32usize,
                size: __z.saturating_sub(32usize),
            }),
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Panel {
    /// The largest podbuf buffer any legal `Panel` encodes to: 344 bytes.
    const MAX_SIZE: ::core::primitive::usize = 344usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 344usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 344usize],
    >::zeroed();
    type View<'a> = PanelPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, PanelPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: PanelPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 48usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<PanelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// The podbuf head of `Outcome`: every field at the offset the first-fit law gives it, with each unused byte run written out as a pad. A buffer holds it after the 8-byte preamble; its size is 16 bytes.
#[allow(deprecated)]
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct OutcomePodbufHead {
    pub tag: ::core::primitive::u8,
    pub _pad1: [::core::primitive::u8; 3],
    pub arm: ::ridl_rt::podbuf::Desc,
    pub _pad12: [::core::primitive::u8; 4],
}
const _: () = {
    assert!(::core::mem::size_of:: < OutcomePodbufHead > () == 16usize);
    assert!(::core::mem::align_of:: < OutcomePodbufHead > () == 8usize);
    assert!(::core::mem::offset_of!(OutcomePodbufHead, tag) == 0usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(OutcomePodbufHead, _pad1) == 1usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 3] > () == 3usize);
    assert!(::core::mem::offset_of!(OutcomePodbufHead, arm) == 4usize);
    assert!(::core::mem::size_of:: < ::ridl_rt::podbuf::Desc > () == 8usize);
    assert!(::core::mem::offset_of!(OutcomePodbufHead, _pad12) == 12usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 4] > () == 4usize);
};
#[allow(deprecated)]
impl Outcome {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(16u32)?;
        self.__ridl_podbuf_encode_at(w, __at)
    }
    pub(crate) fn __ridl_podbuf_encode_at(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
        __at: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        match self {
            Outcome::Number(__a) => {
                w.put_u8(__at, 1u8);
                let __off = w.tail(8u64, 8usize)?;
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: 8u32,
                    },
                );
                w.put_u8(__off, __a.get() as ::core::primitive::u8);
            }
            Outcome::Text(__a) => {
                w.put_u8(__at, 2u8);
                let __off = w.tail(8u64, 8usize)?;
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: 8u32,
                    },
                );
                {
                    let __s: &::core::primitive::str = __a.get();
                    if __s.as_bytes().contains(&0u8) {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                                type_name: ::core::option::Option::Some("Label"),
                                rule: ::ridl_rt::payload::Rule::Pattern,
                            }),
                        );
                    }
                    if !__s.is_empty() {
                        let __n = __s.len();
                        let __data = w
                            .tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                        w.put_bytes(__data, __s.as_bytes());
                        w.put_desc(
                            __off,
                            ::ridl_rt::podbuf::Desc {
                                off: __data as ::core::primitive::u32,
                                len: __n as ::core::primitive::u32,
                            },
                        );
                    }
                }
            }
            Outcome::Mode(__a) => {
                w.put_u8(__at, 3u8);
                let __off = w.tail(8u64, 8usize)?;
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: 8u32,
                    },
                );
                w.put_u8(
                    __off,
                    ::core::primitive::i64::from(*__a) as ::core::primitive::u8,
                );
            }
            Outcome::Reading(__a) => {
                w.put_u8(__at, 4u8);
                let __off = w.tail(24u64, 8usize)?;
                w.put_desc(
                    __at + 4usize,
                    ::ridl_rt::podbuf::Desc {
                        off: __off as ::core::primitive::u32,
                        len: 24u32,
                    },
                );
                Reading::__ridl_podbuf_encode_at(__a, w, __off)?;
            }
        }
        ::core::result::Result::Ok(())
    }
}
/// A `Outcome` value in podbuf bytes, read in place without copying it. Each accessor is a bounds-checked load at the field's offset; a field past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct OutcomePodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for OutcomePodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> OutcomePodbufView<'a> {
    /// Reads `Outcome`'s `arm` field in place.
    pub fn arm(&self) -> OutcomePodbufArm<'a> {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        let __d = ::ridl_rt::podbuf::Desc::read(
            &::ridl_rt::podbuf::Region::new(__b, __a, __z),
            4usize,
        );
        match ::core::primitive::i64::from(
            ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
        ) {
            1i64 => {
                OutcomePodbufArm::Number(
                    Count::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    __b,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        ),
                    ),
                )
            }
            2i64 => {
                OutcomePodbufArm::Text({
                    let __d = ::ridl_rt::podbuf::Desc::read(
                        &::ridl_rt::podbuf::Region::new(
                            __b,
                            __d.off as ::core::primitive::usize,
                            __d.len as ::core::primitive::usize,
                        ),
                        0usize,
                    );
                    ::core::str::from_utf8(
                            __b
                                .get(
                                    (__d.off
                                        as ::core::primitive::usize)..(__d.off
                                        as ::core::primitive::usize)
                                        .saturating_add(__d.len as ::core::primitive::usize),
                                )
                                .unwrap_or(&[]),
                        )
                        .unwrap_or("")
                })
            }
            3i64 => {
                OutcomePodbufArm::Mode(
                    <Mode as ::core::convert::TryFrom<
                        ::core::primitive::i64,
                    >>::try_from(
                            ::core::primitive::i64::from(
                                ::ridl_rt::podbuf::Region::new(
                                        __b,
                                        __d.off as ::core::primitive::usize,
                                        __d.len as ::core::primitive::usize,
                                    )
                                    .u8_at(0usize),
                            ),
                        )
                        .unwrap_or(Mode::Idle),
                )
            }
            4i64 => {
                OutcomePodbufArm::Reading(ReadingPodbufView {
                    buf: __b,
                    at: __d.off as ::core::primitive::usize,
                    size: __d.len as ::core::primitive::usize,
                })
            }
            _ => {
                OutcomePodbufArm::Number(
                    Count::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    __b,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        ),
                    ),
                )
            }
        }
    }
}
/// The arms of `Outcome` as a view reads them: the active arm's view, or its value for a scalar, enum or enum set arm.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub enum OutcomePodbufArm<'a> {
    /// The `number` arm.
    Number(Count),
    /// The `text` arm.
    Text(&'a ::core::primitive::str),
    /// The `mode` arm.
    Mode(Mode),
    /// The `reading` arm.
    Reading(ReadingPodbufView<'a>),
}
#[allow(deprecated)]
impl Outcome {
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<OutcomePodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __size = __p.head_size as ::core::primitive::usize;
        let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
        Self::__ridl_podbuf_verify_at(
            buf,
            ::ridl_rt::podbuf::PREAMBLE_SIZE,
            __size,
            __tail,
        )?;
        ::core::result::Result::Ok(OutcomePodbufView {
            buf,
            at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_verify_at(
        buf: &[::core::primitive::u8],
        at: ::core::primitive::usize,
        size: ::core::primitive::usize,
        __tail: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
        let __d = ::ridl_rt::podbuf::Desc::read(
            &::ridl_rt::podbuf::Region::new(buf, at, size),
            4usize,
        );
        if !__d.len.is_multiple_of(8u32) {
            return ::core::result::Result::Err(
                ::ridl_rt::payload::VerifyError::Structure(
                    ::ridl_rt::payload::Malformed::Union,
                ),
            );
        }
        match ::core::primitive::i64::from(
            ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(0usize),
        ) {
            1i64 => {
                if __d.len == 0 {
                    if __d.off != 0 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                } else {
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len),
                        buf.len(),
                        __tail,
                        8usize,
                    )?;
                }
                Count::check(
                        &(::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    buf,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        )),
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
            2i64 => {
                if __d.len == 0 {
                    if __d.off != 0 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                } else {
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len),
                        buf.len(),
                        __tail,
                        8usize,
                    )?;
                }
                {
                    let __d = ::ridl_rt::podbuf::Desc::read(
                        &::ridl_rt::podbuf::Region::new(
                            buf,
                            __d.off as ::core::primitive::usize,
                            __d.len as ::core::primitive::usize,
                        ),
                        0usize,
                    );
                    let __s: &::core::primitive::str = if __d.len == 0 {
                        if __d.off != 0 {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Structure(
                                    ::ridl_rt::payload::Malformed::OutOfBounds,
                                ),
                            );
                        }
                        ""
                    } else {
                        if ::core::primitive::u64::from(__d.len) > 64u64 {
                            return ::core::result::Result::Err(
                                ::ridl_rt::payload::VerifyError::Structure(
                                    ::ridl_rt::payload::Malformed::OutOfBounds,
                                ),
                            );
                        }
                        let __g = ::ridl_rt::podbuf::tail_range(
                            __d.off,
                            ::core::primitive::u64::from(__d.len) + 1u64,
                            buf.len(),
                            __tail,
                            1usize,
                        )?;
                        ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
                    };
                    Label::check(__s)
                        .map_err(::ridl_rt::payload::VerifyError::Contract)?;
                }
            }
            3i64 => {
                if __d.len == 0 {
                    if __d.off != 0 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                } else {
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len),
                        buf.len(),
                        __tail,
                        8usize,
                    )?;
                }
                <Mode as ::core::convert::TryFrom<
                    ::core::primitive::i64,
                >>::try_from(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    buf,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        ),
                    )
                    .map_err(::ridl_rt::payload::VerifyError::Contract)?;
            }
            4i64 => {
                if __d.len == 0 {
                    if __d.off != 0 {
                        return ::core::result::Result::Err(
                            ::ridl_rt::payload::VerifyError::Structure(
                                ::ridl_rt::payload::Malformed::OutOfBounds,
                            ),
                        );
                    }
                } else {
                    ::ridl_rt::podbuf::tail_range(
                        __d.off,
                        ::core::primitive::u64::from(__d.len),
                        buf.len(),
                        __tail,
                        8usize,
                    )?;
                }
                Reading::__ridl_podbuf_verify_at(
                    buf,
                    __d.off as ::core::primitive::usize,
                    __d.len as ::core::primitive::usize,
                    __tail,
                )?;
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
}
#[allow(deprecated)]
impl Outcome {
    pub(crate) fn __ridl_podbuf_decode(view: OutcomePodbufView<'_>) -> Self {
        #[allow(unused_variables)]
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        let __d = ::ridl_rt::podbuf::Desc::read(
            &::ridl_rt::podbuf::Region::new(__b, __a, __z),
            4usize,
        );
        match ::core::primitive::i64::from(
            ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
        ) {
            1i64 => {
                Outcome::Number(
                    Count::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    __b,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        ),
                    ),
                )
            }
            2i64 => {
                Outcome::Text(
                    Label::new_unchecked(
                        ::std::string::String::from({
                            let __d = ::ridl_rt::podbuf::Desc::read(
                                &::ridl_rt::podbuf::Region::new(
                                    __b,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                ),
                                0usize,
                            );
                            ::core::str::from_utf8(
                                    __b
                                        .get(
                                            (__d.off
                                                as ::core::primitive::usize)..(__d.off
                                                as ::core::primitive::usize)
                                                .saturating_add(__d.len as ::core::primitive::usize),
                                        )
                                        .unwrap_or(&[]),
                                )
                                .unwrap_or("")
                        }),
                    ),
                )
            }
            3i64 => {
                Outcome::Mode(
                    <Mode as ::core::convert::TryFrom<
                        ::core::primitive::i64,
                    >>::try_from(
                            ::core::primitive::i64::from(
                                ::ridl_rt::podbuf::Region::new(
                                        __b,
                                        __d.off as ::core::primitive::usize,
                                        __d.len as ::core::primitive::usize,
                                    )
                                    .u8_at(0usize),
                            ),
                        )
                        .unwrap_or(Mode::Idle),
                )
            }
            4i64 => {
                Outcome::Reading(
                    Reading::__ridl_podbuf_decode(ReadingPodbufView {
                        buf: __b,
                        at: __d.off as ::core::primitive::usize,
                        size: __d.len as ::core::primitive::usize,
                    }),
                )
            }
            _ => {
                Outcome::Number(
                    Count::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::podbuf::Region::new(
                                    __b,
                                    __d.off as ::core::primitive::usize,
                                    __d.len as ::core::primitive::usize,
                                )
                                .u8_at(0usize),
                        ),
                    ),
                )
            }
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Outcome {
    /// The largest podbuf buffer any legal `Outcome` encodes to: 113 bytes.
    const MAX_SIZE: ::core::primitive::usize = 113usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 113usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 113usize],
    >::zeroed();
    type View<'a> = OutcomePodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, OutcomePodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: OutcomePodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 16usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<OutcomePodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// The podbuf head of `GaugeSetParams`: every field at the offset the first-fit law gives it, with each unused byte run written out as a pad. A buffer holds it after the 8-byte preamble; its size is 8 bytes.
#[allow(deprecated)]
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct GaugeSetParamsPodbufHead {
    pub count: ::core::primitive::u8,
    pub level: ::core::primitive::i8,
    pub _pad2: [::core::primitive::u8; 6],
}
const _: () = {
    assert!(::core::mem::size_of:: < GaugeSetParamsPodbufHead > () == 8usize);
    assert!(::core::mem::align_of:: < GaugeSetParamsPodbufHead > () == 8usize);
    assert!(::core::mem::offset_of!(GaugeSetParamsPodbufHead, count) == 0usize);
    assert!(::core::mem::size_of:: < ::core::primitive::u8 > () == 1usize);
    assert!(::core::mem::offset_of!(GaugeSetParamsPodbufHead, level) == 1usize);
    assert!(::core::mem::size_of:: < ::core::primitive::i8 > () == 1usize);
    assert!(::core::mem::offset_of!(GaugeSetParamsPodbufHead, _pad2) == 2usize);
    assert!(::core::mem::size_of:: < [::core::primitive::u8; 6] > () == 6usize);
};
#[allow(deprecated)]
impl GaugeSetParams {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        self.__ridl_podbuf_encode_at(w, __at)
    }
    pub(crate) fn __ridl_podbuf_encode_at(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
        __at: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        w.put_u8(__at, self.count.get() as ::core::primitive::u8);
        w.put_i8(__at + 1usize, self.level.get() as ::core::primitive::i8);
        ::core::result::Result::Ok(())
    }
}
/// A `GaugeSetParams` value in podbuf bytes, read in place without copying it. Each accessor is a bounds-checked load at the field's offset; a field past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct GaugeSetParamsPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for GaugeSetParamsPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> GaugeSetParamsPodbufView<'a> {
    /// Reads `GaugeSetParams`'s `count` field in place.
    pub fn count(&self) -> Count {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Count::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
            ),
        )
    }
    /// Reads `GaugeSetParams`'s `level` field in place.
    pub fn level(&self) -> Level {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Level::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(1usize),
            ),
        )
    }
}
#[allow(deprecated)]
impl GaugeSetParams {
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        GaugeSetParamsPodbufView<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __size = __p.head_size as ::core::primitive::usize;
        let __tail = ::ridl_rt::podbuf::PREAMBLE_SIZE + __size;
        Self::__ridl_podbuf_verify_at(
            buf,
            ::ridl_rt::podbuf::PREAMBLE_SIZE,
            __size,
            __tail,
        )?;
        ::core::result::Result::Ok(GaugeSetParamsPodbufView {
            buf,
            at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_verify_at(
        buf: &[::core::primitive::u8],
        at: ::core::primitive::usize,
        size: ::core::primitive::usize,
        _tail: ::core::primitive::usize,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
        Count::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).u8_at(0usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        Level::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, at, size).i8_at(1usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        ::core::result::Result::Ok(())
    }
}
#[allow(deprecated)]
impl GaugeSetParams {
    pub(crate) fn __ridl_podbuf_decode(view: GaugeSetParamsPodbufView<'_>) -> Self {
        #[allow(unused_variables)]
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        GaugeSetParams {
            count: Count::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
                ),
            ),
            level: Level::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(1usize),
                ),
            ),
        }
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for GaugeSetParams {
    /// The largest podbuf buffer any legal `GaugeSetParams` encodes to: 16 bytes.
    const MAX_SIZE: ::core::primitive::usize = 16usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 16usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 16usize],
    >::zeroed();
    type View<'a> = GaugeSetParamsPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, GaugeSetParamsPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: GaugeSetParamsPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        GaugeSetParamsPodbufView<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// A `Count` value in podbuf bytes, read in place without copying it. The value is a bounds-checked load; a value past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct CountPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for CountPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> CountPodbufView<'a> {
    /// Reads the `Count` value in place.
    pub fn value(&self) -> Count {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Count::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
            ),
        )
    }
}
#[allow(deprecated)]
impl Count {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        w.put_u8(__at, self.get() as ::core::primitive::u8);
        ::core::result::Result::Ok(())
    }
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<CountPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __head = ::ridl_rt::podbuf::PREAMBLE_SIZE;
        let __size = __p.head_size as ::core::primitive::usize;
        #[allow(unused_variables)]
        let __tail = __head + __size;
        Count::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, __head, __size).u8_at(0usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        ::core::result::Result::Ok(CountPodbufView {
            buf,
            at: __head,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_decode(view: CountPodbufView<'_>) -> Self {
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Count::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
            ),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Count {
    /// The largest podbuf buffer any legal `Count` encodes to: 16 bytes.
    const MAX_SIZE: ::core::primitive::usize = 16usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 16usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 16usize],
    >::zeroed();
    type View<'a> = CountPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, CountPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: CountPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<CountPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// A `Level` value in podbuf bytes, read in place without copying it. The value is a bounds-checked load; a value past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct LevelPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for LevelPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> LevelPodbufView<'a> {
    /// Reads the `Level` value in place.
    pub fn value(&self) -> Level {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        Level::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(0usize),
            ),
        )
    }
}
#[allow(deprecated)]
impl Level {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        w.put_i8(__at, self.get() as ::core::primitive::i8);
        ::core::result::Result::Ok(())
    }
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LevelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __head = ::ridl_rt::podbuf::PREAMBLE_SIZE;
        let __size = __p.head_size as ::core::primitive::usize;
        #[allow(unused_variables)]
        let __tail = __head + __size;
        Level::check(
                &(::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, __head, __size).i8_at(0usize),
                )),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        ::core::result::Result::Ok(LevelPodbufView {
            buf,
            at: __head,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_decode(view: LevelPodbufView<'_>) -> Self {
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Level::new_unchecked(
            ::core::primitive::i64::from(
                ::ridl_rt::podbuf::Region::new(__b, __a, __z).i8_at(0usize),
            ),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Level {
    /// The largest podbuf buffer any legal `Level` encodes to: 16 bytes.
    const MAX_SIZE: ::core::primitive::usize = 16usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 16usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 16usize],
    >::zeroed();
    type View<'a> = LevelPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, LevelPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LevelPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LevelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// A `Label` value in podbuf bytes, read in place without copying it. The value is a bounds-checked load; a value past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct LabelPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for LabelPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> LabelPodbufView<'a> {
    /// Reads the `Label` value in place.
    pub fn value(&self) -> &'a ::core::primitive::str {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                0usize,
            );
            ::core::str::from_utf8(
                    __b
                        .get(
                            (__d.off
                                as ::core::primitive::usize)..(__d.off
                                as ::core::primitive::usize)
                                .saturating_add(__d.len as ::core::primitive::usize),
                        )
                        .unwrap_or(&[]),
                )
                .unwrap_or("")
        }
    }
}
#[allow(deprecated)]
impl Label {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        {
            let __s: &::core::primitive::str = self.get();
            if __s.as_bytes().contains(&0u8) {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Label"),
                        rule: ::ridl_rt::payload::Rule::Pattern,
                    }),
                );
            }
            if !__s.is_empty() {
                let __n = __s.len();
                let __data = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                w.put_bytes(__data, __s.as_bytes());
                w.put_desc(
                    __at,
                    ::ridl_rt::podbuf::Desc {
                        off: __data as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    },
                );
            }
        }
        ::core::result::Result::Ok(())
    }
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LabelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __head = ::ridl_rt::podbuf::PREAMBLE_SIZE;
        let __size = __p.head_size as ::core::primitive::usize;
        #[allow(unused_variables)]
        let __tail = __head + __size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(buf, __head, __size),
                0usize,
            );
            let __s: &::core::primitive::str = if __d.len == 0 {
                if __d.off != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                ""
            } else {
                if ::core::primitive::u64::from(__d.len) > 64u64 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                let __g = ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len) + 1u64,
                    buf.len(),
                    __tail,
                    1usize,
                )?;
                ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
            };
            Label::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::result::Result::Ok(LabelPodbufView {
            buf,
            at: __head,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_decode(view: LabelPodbufView<'_>) -> Self {
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Label::new_unchecked(
            ::std::string::String::from({
                let __d = ::ridl_rt::podbuf::Desc::read(
                    &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                    0usize,
                );
                ::core::str::from_utf8(
                        __b
                            .get(
                                (__d.off
                                    as ::core::primitive::usize)..(__d.off
                                    as ::core::primitive::usize)
                                    .saturating_add(__d.len as ::core::primitive::usize),
                            )
                            .unwrap_or(&[]),
                    )
                    .unwrap_or("")
            }),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Label {
    /// The largest podbuf buffer any legal `Label` encodes to: 81 bytes.
    const MAX_SIZE: ::core::primitive::usize = 81usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 81usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 81usize],
    >::zeroed();
    type View<'a> = LabelPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, LabelPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LabelPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<LabelPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// A `Mode` value in podbuf bytes, read in place without copying it. The value is a bounds-checked load; a value past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct ModePodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for ModePodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> ModePodbufView<'a> {
    /// Reads the `Mode` value in place.
    pub fn value(&self) -> Mode {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        <Mode as ::core::convert::TryFrom<
            ::core::primitive::i64,
        >>::try_from(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
                ),
            )
            .unwrap_or(Mode::Idle)
    }
}
#[allow(deprecated)]
impl Mode {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        w.put_u8(__at, ::core::primitive::i64::from(*self) as ::core::primitive::u8);
        ::core::result::Result::Ok(())
    }
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ModePodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __head = ::ridl_rt::podbuf::PREAMBLE_SIZE;
        let __size = __p.head_size as ::core::primitive::usize;
        #[allow(unused_variables)]
        let __tail = __head + __size;
        <Mode as ::core::convert::TryFrom<
            ::core::primitive::i64,
        >>::try_from(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(buf, __head, __size).u8_at(0usize),
                ),
            )
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
        ::core::result::Result::Ok(ModePodbufView {
            buf,
            at: __head,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_decode(view: ModePodbufView<'_>) -> Self {
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        <Mode as ::core::convert::TryFrom<
            ::core::primitive::i64,
        >>::try_from(
                ::core::primitive::i64::from(
                    ::ridl_rt::podbuf::Region::new(__b, __a, __z).u8_at(0usize),
                ),
            )
            .unwrap_or(Mode::Idle)
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Mode {
    /// The largest podbuf buffer any legal `Mode` encodes to: 16 bytes.
    const MAX_SIZE: ::core::primitive::usize = 16usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 16usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 16usize],
    >::zeroed();
    type View<'a> = ModePodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ModePodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: ModePodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<ModePodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
/// A `Key` value in podbuf bytes, read in place without copying it. The value is a bounds-checked load; a value past the size the producer wrote reads as 0.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct KeyPodbufView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) at: ::core::primitive::usize,
    pub(crate) size: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for KeyPodbufView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> KeyPodbufView<'a> {
    /// Reads the `Key` value in place.
    pub fn value(&self) -> &'a ::core::primitive::str {
        let __b = self.buf;
        let __a = self.at;
        let __z = self.size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                0usize,
            );
            ::core::str::from_utf8(
                    __b
                        .get(
                            (__d.off
                                as ::core::primitive::usize)..(__d.off
                                as ::core::primitive::usize)
                                .saturating_add(__d.len as ::core::primitive::usize),
                        )
                        .unwrap_or(&[]),
                )
                .unwrap_or("")
        }
    }
}
#[allow(deprecated)]
impl Key {
    pub(crate) fn __ridl_podbuf_encode(
        &self,
        w: &mut ::ridl_rt::podbuf::Writer<'_>,
    ) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
        let __at = w.reserve_head(8u32)?;
        {
            let __s: &::core::primitive::str = self.get();
            if __s.as_bytes().contains(&0u8) {
                return ::core::result::Result::Err(
                    ::ridl_rt::payload::EncodeError::Contract(::ridl_rt::payload::Violation {
                        type_name: ::core::option::Option::Some("Key"),
                        rule: ::ridl_rt::payload::Rule::Pattern,
                    }),
                );
            }
            if !__s.is_empty() {
                let __n = __s.len();
                let __data = w.tail(__n as ::core::primitive::u64 + 1u64, 1usize)?;
                w.put_bytes(__data, __s.as_bytes());
                w.put_desc(
                    __at,
                    ::ridl_rt::podbuf::Desc {
                        off: __data as ::core::primitive::u32,
                        len: __n as ::core::primitive::u32,
                    },
                );
            }
        }
        ::core::result::Result::Ok(())
    }
    pub(crate) fn __ridl_podbuf_verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<KeyPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        let __p = ::ridl_rt::podbuf::Preamble::read(buf)?;
        let __head = ::ridl_rt::podbuf::PREAMBLE_SIZE;
        let __size = __p.head_size as ::core::primitive::usize;
        #[allow(unused_variables)]
        let __tail = __head + __size;
        {
            let __d = ::ridl_rt::podbuf::Desc::read(
                &::ridl_rt::podbuf::Region::new(buf, __head, __size),
                0usize,
            );
            let __s: &::core::primitive::str = if __d.len == 0 {
                if __d.off != 0 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                ""
            } else {
                if ::core::primitive::u64::from(__d.len) > 32u64 {
                    return ::core::result::Result::Err(
                        ::ridl_rt::payload::VerifyError::Structure(
                            ::ridl_rt::payload::Malformed::OutOfBounds,
                        ),
                    );
                }
                let __g = ::ridl_rt::podbuf::tail_range(
                    __d.off,
                    ::core::primitive::u64::from(__d.len) + 1u64,
                    buf.len(),
                    __tail,
                    1usize,
                )?;
                ::ridl_rt::podbuf::text(buf.get(__g).unwrap_or(&[]))?
            };
            Key::check(__s).map_err(::ridl_rt::payload::VerifyError::Contract)?;
        }
        ::core::result::Result::Ok(KeyPodbufView {
            buf,
            at: __head,
            size: __size,
        })
    }
    pub(crate) fn __ridl_podbuf_decode(view: KeyPodbufView<'_>) -> Self {
        let (__b, __a, __z) = (view.buf, view.at, view.size);
        Key::new_unchecked(
            ::std::string::String::from({
                let __d = ::ridl_rt::podbuf::Desc::read(
                    &::ridl_rt::podbuf::Region::new(__b, __a, __z),
                    0usize,
                );
                ::core::str::from_utf8(
                        __b
                            .get(
                                (__d.off
                                    as ::core::primitive::usize)..(__d.off
                                    as ::core::primitive::usize)
                                    .saturating_add(__d.len as ::core::primitive::usize),
                            )
                            .unwrap_or(&[]),
                    )
                    .unwrap_or("")
            }),
        )
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Podbuf> for Key {
    /// The largest podbuf buffer any legal `Key` encodes to: 49 bytes.
    const MAX_SIZE: ::core::primitive::usize = 49usize;
    type Buffer = ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; 49usize]>;
    const BUFFER: Self::Buffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; 49usize],
    >::zeroed();
    type View<'a> = KeyPodbufView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, KeyPodbufView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut __w = ::ridl_rt::podbuf::Writer::new(out);
        self.__ridl_podbuf_encode(&mut __w)?;
        let bytes = __w.finish()?;
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: KeyPodbufView {
                buf: bytes,
                at: ::ridl_rt::podbuf::PREAMBLE_SIZE,
                size: 8usize,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<KeyPodbufView<'_>, ::ridl_rt::payload::VerifyError> {
        Self::__ridl_podbuf_verify(buf)
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Podbuf>,
    ) -> Self {
        Self::__ridl_podbuf_decode(r.into_view())
    }
}
#[allow(deprecated)]
impl Count {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Level {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Label {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Mode {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Reading {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Key {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Inner {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Panel {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl Outcome {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
#[allow(deprecated)]
impl GaugeSetParams {
    /// Re-encodes a flatbuffers payload as a proto3 payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a flatbuffers payload as a podbuf payload.
    ///
    /// The bytes are verified as flatbuffers, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_flatbuffers_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::FlatBuffers,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a proto3 payload as a flatbuffers payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
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
    /// Re-encodes a proto3 payload as a podbuf payload.
    ///
    /// The bytes are verified as proto3, decoded, and encoded as podbuf into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_proto3_to_podbuf<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Podbuf>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Proto3,
            ::ridl_rt::encoding::Podbuf,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a flatbuffers payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as flatbuffers into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_flatbuffers<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::FlatBuffers,
        >(bytes, out)
    }
    /// Re-encodes a podbuf payload as a proto3 payload.
    ///
    /// The bytes are verified as podbuf, decoded, and encoded as proto3 into `out`.
    /// The body is [`::ridl_rt::payload::transcode`].
    pub fn transcode_podbuf_to_proto3<'o>(
        bytes: &[::core::primitive::u8],
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::Proto3>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<
            Self,
            ::ridl_rt::encoding::Podbuf,
            ::ridl_rt::encoding::Proto3,
        >(bytes, out)
    }
}
/// Descriptor for interface `Gauge`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.
///
/// Its size table in FlatBuffers is `<Gauge as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in proto3 is `<Gauge as Sizes<Proto3>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in podbuf is `<Gauge as Sizes<Podbuf>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
pub struct Gauge;
impl ::ridl_rt::contract::Interface for Gauge {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "tx.demo",
        hash: ::ridl_rt::contract::CatalogHash([
            238, 149, 134, 184, 219, 100, 205, 162, 101, 75, 85, 232, 13, 7, 126, 166,
            15, 69, 111, 121, 7, 172, 47, 11, 37, 30, 147, 171, 106, 93, 248, 192,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(1);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Gauge";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "set",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(50000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "GaugeSetParams",
                },
            ],
            streams: &[],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Gauge {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <GaugeSetParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <GaugeSetParams as ::ridl_rt::payload::Payload<
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
    type ClaimBuffer = [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::FlatBuffers,
    >>::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::FlatBuffers,
    >>::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::FlatBuffers,
    >>::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::FlatBuffers,
    >>::EVENT_SOURCE_BUFFER_SIZE];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Gauge {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <GaugeSetParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::Proto3,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <GaugeSetParams as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
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
    type ClaimBuffer = [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::Proto3,
    >>::MAX_BUFFER_SIZE];
    type EventBuffer = [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::Proto3,
    >>::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::Proto3,
    >>::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; <Self as ::ridl_rt::contract::Sizes<
        ::ridl_rt::encoding::Proto3,
    >>::EVENT_SOURCE_BUFFER_SIZE];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Podbuf> for Gauge {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <GaugeSetParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::Podbuf,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <GaugeSetParams as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Podbuf,
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
    type ClaimBuffer = ::ridl_rt::podbuf::Aligned<
        [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::Podbuf,
        >>::MAX_BUFFER_SIZE],
    >;
    type EventBuffer = ::ridl_rt::podbuf::Aligned<
        [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::Podbuf,
        >>::EVENT_SOURCE_BUFFER_SIZE],
    >;
    const CLAIM_BUFFER: Self::ClaimBuffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::Podbuf,
        >>::MAX_BUFFER_SIZE],
    >::zeroed();
    const EVENT_BUFFER: Self::EventBuffer = ::ridl_rt::podbuf::Aligned::<
        [::core::primitive::u8; <Self as ::ridl_rt::contract::Sizes<
            ::ridl_rt::encoding::Podbuf,
        >>::EVENT_SOURCE_BUFFER_SIZE],
    >::zeroed();
}
#[doc(hidden)]
pub struct GaugeSet;
impl ::ridl_rt::contract::Interaction for GaugeSet {
    type Iface = Gauge;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Gauge as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Command for GaugeSet {
    type Args = GaugeSetParams;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
/// An interface whose multi-parameter call induces a payload type, so the
/// induced struct gets transcode functions too.
///
///The generated interaction face of interface `Gauge`.
#[allow(clippy::module_inception)]
pub mod gauge {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct SetCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Gauge as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Gauge` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Gauge`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Two parameters: the request type is an induced struct.
        ///
        /// Sends command `set` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        ///
        /// # Parameters
        ///
        /// - `count`
        ///   - range 0 ..= 200
        /// - `level`
        ///   - range -5 ..= 10
        pub fn set(&mut self, args: super::GaugeSetParams) -> SetCall<'_, P> {
            let __arg = args;
            let deadline = <super::GaugeSet as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_set(&mut self.port, &__arg) {
                Ok(correlation) => SetPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => SetPhase::Unsent(__arg),
                Err(error) => SetPhase::Failed(error),
            };
            SetCall {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.
        ///
        /// # Panics
        ///
        /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Gauge as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Gauge` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client { port }
        }
    }
    ///The future of command `set`, returned by `Client::set`. It resolves to the delivery acknowledgment: `Ok(())`, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the command was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct SetCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: SetPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum SetPhase {
        Unsent(super::GaugeSetParams),
        Waiting(SetCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > SetCall<'_, P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, SetPhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for SetCall<'_, P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, SetPhase::Done) {
                    SetPhase::Done => panic!("`SetCall` polled after completion"),
                    SetPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    SetPhase::Unsent(__arg) => {
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
                        match send_set(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = SetPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = SetPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    SetPhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_set_ack(&mut *this.port, correlation) {
                            Some(outcome) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            None => {
                                if !this.expired() {
                                    this.phase = SetPhase::Waiting(correlation);
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
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for SetCall<'_, P> {
        fn drop(&mut self) {
            if let SetPhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    pub(crate) fn send_set<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        args: &super::GaugeSetParams,
    ) -> ::core::result::Result<SetCorrelation, ::ridl_rt::port::SendError>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = args;
        <super::GaugeSet as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::GaugeSetParams as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::GaugeSetParams,
            P::Encoding,
        >::encode(__arg, buf) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Contract(__violation)) => {
                return ::core::result::Result::Err(
                    ::ridl_rt::port::SendError::Contract(
                        ::ridl_rt::error::Contract::InvalidValue(__violation),
                    ),
                );
            }
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding `GaugeSetParams` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `GaugeSetParams` failed"),
        };
        __port
            .command(
                <super::Gauge as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(SetCorrelation)
    }
    pub(crate) fn poll_set_ack<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: SetCorrelation,
    ) -> ::core::option::Option<::core::result::Result<(), ::ridl_rt::error::CallError>>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port.ack(correlation.0)
    }
    /// What an application implements to serve interface `Gauge`'s calls.
    ///
    /// An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses.
    pub trait Provider {
        /// Two parameters: the request type is an induced struct.
        ///
        ///Serves command `set`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        ///
        /// # Parameters
        ///
        /// - `count`
        ///   - range 0 ..= 200
        /// - `level`
        ///   - range -5 ..= 10
        fn set(&mut self, args: &super::GaugeSetParams);
    }
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler,
        P: Provider,
        super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if buf.len()
            < <super::Gauge as ::ridl_rt::contract::Sizes<H::Encoding>>::MAX_BUFFER_SIZE
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
                        != <super::Gauge as ::ridl_rt::contract::Interface>::NUMBER
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
                                    super::GaugeSetParams,
                                    H::Encoding,
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
                                        match <super::GaugeSet as ::ridl_rt::contract::Command>::require(
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
                                                p.set(&__arg);
                                                accepted
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
    /// Serves interface `Gauge`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Gauge as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Gauge` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
        super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Gauge as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(1u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Gauge as ::ridl_rt::contract::Sizes<H::Encoding>>::CLAIM_BUFFER,
            state,
        }
    }
    ///The future `serve` returns over interface `Gauge`. It holds the handler, the provider, and the claim buffer of `<Gauge as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the encoding the handler port carries. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    >
    where
        super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
    {
        handler: H,
        provider: &'a mut P,
        buf: <super::Gauge as ::ridl_rt::contract::Sizes<H::Encoding>>::ClaimBuffer,
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
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<H::Encoding>,
    {}
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P>
    where
        super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::GaugeSetParams: ::ridl_rt::payload::Payload<H::Encoding>,
    {
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
                                <super::Gauge as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        ::core::convert::AsMut::as_mut(&mut this.buf),
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
    ///The traits a consumer of interface `Gauge`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Gauge`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Gauge`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, all in scope through the parent module's `prelude`.
        pub struct Client<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > Client<P>
        where
            super::super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            /// Two parameters: the request type is an induced struct.
            ///
            ///Sends command `set` and waits for its outcome, as `block_on` over `Client::set`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            ///
            /// # Rules
            ///
            /// - response within 50000 us
            ///
            /// # Parameters
            ///
            /// - `count`
            ///   - range 0 ..= 200
            /// - `level`
            ///   - range -5 ..= 10
            pub fn set(
                &mut self,
                args: super::super::GaugeSetParams,
            ) -> ::core::result::Result<(), ::ridl_rt::error::ClientError> {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.set(args);
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
        }
        impl<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P>
        where
            super::super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.
            ///
            /// # Panics
            ///
            /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Gauge as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Gauge` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
            #[track_caller]
            fn new(port: P) -> Self {
                Client {
                    inner: <super::Client<P> as ::ridl_rt::face::Bind>::new(port),
                    timeout: None,
                }
            }
        }
        impl<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Timeout for Client<P>
        where
            super::super::Gauge: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::GaugeSetParams: ::ridl_rt::payload::Payload<P::Encoding>,
        {
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
        /// Serves interface `Gauge`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        ///
        /// # Panics
        ///
        /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Gauge as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Gauge` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
                + ::ridl_rt::port::Wakeable,
            P: super::Provider,
            super::super::Gauge: ::ridl_rt::contract::Sizes<H::Encoding>,
            super::super::GaugeSetParams: ::ridl_rt::payload::Payload<H::Encoding>,
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
