/// A control level.
///
/// # Rules
///
/// - range 0 ..= 100
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
        if value < 0 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Level"),
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 100 {
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
/// One reading of the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reading {
    /// # Rules
    ///
    /// - range 0 ..= 100
    pub level: Level,
    pub flag: ::core::primitive::bool,
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Reading {
    fn default() -> Self {
        Reading {
            level: <Level as ::core::default::Default>::default(),
            flag: false,
        }
    }
}
/// A document identifier.
///
/// # Rules
///
/// - range 0 ..= 100
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Id(::core::primitive::i64);
impl Id {
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
                type_name: ::core::option::Option::Some("Id"),
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        if value > 100 {
            return ::core::result::Result::Err(::ridl_rt::payload::Violation {
                type_name: ::core::option::Option::Some("Id"),
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
impl ::core::convert::TryFrom<::core::primitive::i64> for Id {
    type Error = ::ridl_rt::payload::Violation;
    fn try_from(
        value: ::core::primitive::i64,
    ) -> ::core::result::Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl ::core::convert::From<Id> for ::core::primitive::i64 {
    fn from(value: Id) -> Self {
        value.0
    }
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Id {
    fn default() -> Self {
        Id::new_unchecked(0)
    }
}
/// Upload metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Info {
    /// # Rules
    ///
    /// - range 0 ..= 100
    pub id: Id,
}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for Info {
    fn default() -> Self {
        Info {
            id: <Id as ::core::default::Default>::default(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogUploadParams {}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for LogUploadParams {
    fn default() -> Self {
        LogUploadParams {}
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocsNoteParams {}
#[allow(clippy::derivable_impls)]
impl ::core::default::Default for DocsNoteParams {
    fn default() -> Self {
        DocsNoteParams {}
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
    /// Reads `Reading`'s `level` field in place.
    pub fn level(&self) -> Level {
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
    /// Reads `Reading`'s `flag` field in place.
    pub fn flag(&self) -> ::core::primitive::bool {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 1u16, 1usize) {
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
    }; 2usize];
    let mut __n = 0usize;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: {
            let __s = value.level;
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
        },
    };
    __n += 1;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 1u16,
        offset: 5u16,
        value: {
            let __s = value.flag;
            ::ridl_rt::flatbuffers::Field::Bool(__s)
        },
    };
    __n += 1;
    builder.push_table(6usize, 4usize, 2u16, &__fields[..__n])
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
        Level::check(&(::core::primitive::i64::from(__raw)))
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
        level: match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Level::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Level::new_unchecked(0i64),
        },
        flag: match ::ridl_rt::flatbuffers::field(buf, table, 1u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                ::ridl_rt::flatbuffers::read_bool(buf, __p).unwrap_or(false)
            }
            _ => false,
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Reading {
    /// The largest FlatBuffers buffer any legal `Reading` encodes to: 53 bytes.
    const MAX_SIZE: ::core::primitive::usize = 53usize;
    type Buffer = [::core::primitive::u8; 53usize];
    const BUFFER: Self::Buffer = [0u8; 53usize];
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
/// A `Id` value in FlatBuffers bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct IdFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for IdFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> IdFbView<'a> {
    /// The value the bytes hold. `Id` is one value, so this decodes it rather than borrowing it, which costs one read.
    pub fn value(&self) -> Id {
        __ridl_fb_decode_Id(self.buf, self.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Id(
    value: &Id,
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
pub(crate) fn __ridl_fb_verify_Id(
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
        Id::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Id(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Id {
    match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
        ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
            Id::new_unchecked(
                ::core::primitive::i64::from(
                    ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                ),
            )
        }
        _ => Id::new_unchecked(0i64),
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Id {
    /// The largest FlatBuffers buffer any legal `Id` encodes to: 43 bytes.
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type Buffer = [::core::primitive::u8; 43usize];
    const BUFFER: Self::Buffer = [0u8; 43usize];
    type View<'a> = IdFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, IdFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Id(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: IdFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<IdFbView<'_>, ::ridl_rt::payload::VerifyError> {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_Id(buf, table)?;
        ::core::result::Result::Ok(IdFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Id(__view.buf, __view.table)
    }
}
/// A `Info` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct InfoFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for InfoFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> InfoFbView<'a> {
    /// Reads `Info`'s `id` field in place.
    pub fn id(&self) -> Id {
        match ::ridl_rt::flatbuffers::field(self.buf, self.table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Id::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(self.buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Id::new_unchecked(0i64),
        }
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_Info(
    value: &Info,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    let mut __fields = [::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: ::ridl_rt::flatbuffers::Field::Bool(false),
    }; 1usize];
    let mut __n = 0usize;
    __fields[__n] = ::ridl_rt::flatbuffers::TableField {
        slot: 0u16,
        offset: 4u16,
        value: {
            let __s = value.id;
            ::ridl_rt::flatbuffers::Field::U8(__s.get() as ::core::primitive::u8)
        },
    };
    __n += 1;
    builder.push_table(5usize, 4usize, 1u16, &__fields[..__n])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_Info(
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
        Id::check(&(::core::primitive::i64::from(__raw)))
            .map_err(::ridl_rt::payload::VerifyError::Contract)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_Info(
    buf: &[::core::primitive::u8],
    table: ::core::primitive::usize,
) -> Info {
    Info {
        id: match ::ridl_rt::flatbuffers::field(buf, table, 0u16, 1usize) {
            ::core::result::Result::Ok(::core::option::Option::Some(__p)) => {
                Id::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::flatbuffers::read_u8(buf, __p).unwrap_or(0u8),
                    ),
                )
            }
            _ => Id::new_unchecked(0i64),
        },
    }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for Info {
    /// The largest FlatBuffers buffer any legal `Info` encodes to: 43 bytes.
    const MAX_SIZE: ::core::primitive::usize = 43usize;
    type Buffer = [::core::primitive::u8; 43usize];
    const BUFFER: Self::Buffer = [0u8; 43usize];
    type View<'a> = InfoFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, InfoFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_Info(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: InfoFbView { buf: bytes, table },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<InfoFbView<'_>, ::ridl_rt::payload::VerifyError> {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_Info(buf, table)?;
        ::core::result::Result::Ok(InfoFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_Info(__view.buf, __view.table)
    }
}
/// A `LogUploadParams` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct LogUploadParamsFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for LogUploadParamsFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> LogUploadParamsFbView<'a> {}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_LogUploadParams(
    _value: &LogUploadParams,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    builder.push_table(4usize, 4usize, 0u16, &[])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_LogUploadParams(
    _buf: &[::core::primitive::u8],
    _table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_LogUploadParams(
    _buf: &[::core::primitive::u8],
    _table: ::core::primitive::usize,
) -> LogUploadParams {
    LogUploadParams {}
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for LogUploadParams {
    /// The largest FlatBuffers buffer any legal `LogUploadParams` encodes to: 33 bytes.
    const MAX_SIZE: ::core::primitive::usize = 33usize;
    type Buffer = [::core::primitive::u8; 33usize];
    const BUFFER: Self::Buffer = [0u8; 33usize];
    type View<'a> = LogUploadParamsFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, LogUploadParamsFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_LogUploadParams(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: LogUploadParamsFbView {
                buf: bytes,
                table,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        LogUploadParamsFbView<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_LogUploadParams(buf, table)?;
        ::core::result::Result::Ok(LogUploadParamsFbView {
            buf,
            table,
        })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_LogUploadParams(__view.buf, __view.table)
    }
}
/// A `DocsNoteParams` value in FlatBuffers bytes, read in place without copying it.
///
/// A scalar, a string, a byte sequence and a nested table are
/// read in place and allocate nothing. A union and a collection
/// are decoded on access instead: a union's arms and a
/// collection's elements have no one view type to hand back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(deprecated)]
pub struct DocsNoteParamsFbView<'a> {
    pub(crate) buf: &'a [::core::primitive::u8],
    pub(crate) table: ::core::primitive::usize,
}
#[allow(deprecated)]
impl<'a> ::ridl_rt::payload::View<'a> for DocsNoteParamsFbView<'a> {
    fn bytes(&self) -> &'a [::core::primitive::u8] {
        self.buf
    }
}
#[allow(deprecated)]
impl<'a> DocsNoteParamsFbView<'a> {}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_encode_DocsNoteParams(
    _value: &DocsNoteParams,
    builder: &mut ::ridl_rt::flatbuffers::Builder<'_>,
) -> ::core::result::Result<
    ::ridl_rt::flatbuffers::Pos,
    ::ridl_rt::payload::EncodeError,
> {
    builder.push_table(4usize, 4usize, 0u16, &[])
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_verify_DocsNoteParams(
    _buf: &[::core::primitive::u8],
    _table: ::core::primitive::usize,
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_fb_decode_DocsNoteParams(
    _buf: &[::core::primitive::u8],
    _table: ::core::primitive::usize,
) -> DocsNoteParams {
    DocsNoteParams {}
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for DocsNoteParams {
    /// The largest FlatBuffers buffer any legal `DocsNoteParams` encodes to: 33 bytes.
    const MAX_SIZE: ::core::primitive::usize = 33usize;
    type Buffer = [::core::primitive::u8; 33usize];
    const BUFFER: Self::Buffer = [0u8; 33usize];
    type View<'a> = DocsNoteParamsFbView<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, DocsNoteParamsFbView<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let mut builder = ::ridl_rt::flatbuffers::Builder::new(out);
        let __root = __ridl_fb_encode_DocsNoteParams(self, &mut builder)?;
        let bytes = builder.finish(__root, 8usize)?;
        let table = ::ridl_rt::flatbuffers::root(bytes).unwrap_or(0usize);
        ::core::result::Result::Ok(::ridl_rt::payload::Encoded {
            bytes,
            view: DocsNoteParamsFbView {
                buf: bytes,
                table,
            },
        })
    }
    fn verify(
        buf: &[::core::primitive::u8],
    ) -> ::core::result::Result<
        DocsNoteParamsFbView<'_>,
        ::ridl_rt::payload::VerifyError,
    > {
        let table = ::ridl_rt::flatbuffers::root(buf)
            .map_err(::ridl_rt::payload::VerifyError::Structure)?;
        __ridl_fb_verify_DocsNoteParams(buf, table)?;
        ::core::result::Result::Ok(DocsNoteParamsFbView { buf, table })
    }
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::FlatBuffers>,
    ) -> Self {
        let __view = r.view();
        __ridl_fb_decode_DocsNoteParams(__view.buf, __view.table)
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Level(value: &Level) -> ::core::primitive::usize {
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
pub(crate) fn __ridl_pb_encode_Level(
    value: &Level,
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
                        ::ridl_rt::proto3::narrow_u32(__raw)
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
                        ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
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
pub(crate) fn __ridl_pb_len_Reading(value: &Reading) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.level;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
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
        let __s = value.level;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    {
        let __s = value.flag;
        let __r = ::core::primitive::u64::from(__s);
        if __r != 0 {
            writer.tag(2u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_Reading(
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
    let mut __f0 = Level::new_unchecked(0i64);
    let mut __f1 = false;
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        match __field.number {
            1u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f0 = Level::new_unchecked(
                        ::core::primitive::i64::from(
                            ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                        ),
                    );
                }
            }
            2u32 => {
                if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                    __f1 = __raw != 0u64;
                }
            }
            _ => {}
        }
    }
    Reading { level: __f0, flag: __f1 }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Reading {
    /// The largest proto3 message any legal `Reading` encodes to: 8 bytes.
    const MAX_SIZE: ::core::primitive::usize = 8usize;
    type Buffer = [::core::primitive::u8; 8usize];
    const BUFFER: Self::Buffer = [0u8; 8usize];
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
pub(crate) fn __ridl_pb_len_Id(value: &Id) -> ::core::primitive::usize {
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
pub(crate) fn __ridl_pb_encode_Id(
    value: &Id,
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
pub(crate) fn __ridl_pb_verify_Id(
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
                    Id::check(&__v).map_err(::ridl_rt::payload::VerifyError::Contract)?;
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
pub(crate) fn __ridl_pb_decode_Id(buf: &[::core::primitive::u8]) -> Id {
    let mut __f0 = Id::new_unchecked(0i64);
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                __f0 = Id::new_unchecked(
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
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Id {
    /// The largest proto3 message any legal `Id` encodes to: 6 bytes.
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
        let __needed = __ridl_pb_len_Id(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Id(self, &mut __writer)?;
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
        __ridl_pb_verify_Id(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Id(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_Info(value: &Info) -> ::core::primitive::usize {
    let mut __n = 0usize;
    {
        let __s = value.id;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            __n = __n.saturating_add(1usize + ::ridl_rt::proto3::varint_len(__r));
        }
    }
    __n
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_Info(
    value: &Info,
    writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    {
        let __s = value.id;
        let __r = __s.get() as ::core::primitive::u64;
        if __r != 0 {
            writer.tag(1u32, ::ridl_rt::proto3::WireType::Varint);
            writer.varint(__r);
        }
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_Info(
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
                    Id::check(&__v).map_err(::ridl_rt::payload::VerifyError::Contract)?;
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
pub(crate) fn __ridl_pb_decode_Info(buf: &[::core::primitive::u8]) -> Info {
    let mut __f0 = Id::new_unchecked(0i64);
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(::core::result::Result::Ok(__field)) = __reader
        .next()
    {
        if __field.number == 1u32 {
            if let ::ridl_rt::proto3::Value::Varint(__raw) = __field.value {
                __f0 = Id::new_unchecked(
                    ::core::primitive::i64::from(
                        ::ridl_rt::proto3::narrow_u32(__raw).unwrap_or(0u32),
                    ),
                );
            }
        }
    }
    Info { id: __f0 }
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for Info {
    /// The largest proto3 message any legal `Info` encodes to: 6 bytes.
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
        let __needed = __ridl_pb_len_Info(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_Info(self, &mut __writer)?;
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
        __ridl_pb_verify_Info(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_Info(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_LogUploadParams(
    _value: &LogUploadParams,
) -> ::core::primitive::usize {
    0usize
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_LogUploadParams(
    _value: &LogUploadParams,
    _writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_LogUploadParams(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_LogUploadParams(
    _buf: &[::core::primitive::u8],
) -> LogUploadParams {
    LogUploadParams {}
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for LogUploadParams {
    /// The largest proto3 message any legal `LogUploadParams` encodes to: 0 bytes.
    const MAX_SIZE: ::core::primitive::usize = 0usize;
    type Buffer = [::core::primitive::u8; 0usize];
    const BUFFER: Self::Buffer = [0u8; 0usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_LogUploadParams(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_LogUploadParams(self, &mut __writer)?;
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
        __ridl_pb_verify_LogUploadParams(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_LogUploadParams(::ridl_rt::payload::View::bytes(r.view()))
    }
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_len_DocsNoteParams(
    _value: &DocsNoteParams,
) -> ::core::primitive::usize {
    0usize
}
#[allow(deprecated, non_snake_case)]
pub(crate) fn __ridl_pb_encode_DocsNoteParams(
    _value: &DocsNoteParams,
    _writer: &mut ::ridl_rt::proto3::Writer<'_>,
) -> ::core::result::Result<(), ::ridl_rt::payload::EncodeError> {
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_verify_DocsNoteParams(
    buf: &[::core::primitive::u8],
) -> ::core::result::Result<(), ::ridl_rt::payload::VerifyError> {
    let mut __reader = ::ridl_rt::proto3::Reader::new(buf);
    while let ::core::option::Option::Some(__field) = __reader.next() {
        __field.map_err(::ridl_rt::payload::VerifyError::Structure)?;
    }
    ::core::result::Result::Ok(())
}
#[allow(deprecated, non_snake_case, clippy::collapsible_if)]
pub(crate) fn __ridl_pb_decode_DocsNoteParams(
    _buf: &[::core::primitive::u8],
) -> DocsNoteParams {
    DocsNoteParams {}
}
#[allow(deprecated)]
impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3> for DocsNoteParams {
    /// The largest proto3 message any legal `DocsNoteParams` encodes to: 0 bytes.
    const MAX_SIZE: ::core::primitive::usize = 0usize;
    type Buffer = [::core::primitive::u8; 0usize];
    const BUFFER: Self::Buffer = [0u8; 0usize];
    type View<'a> = ::ridl_rt::proto3::Verified<'a>;
    fn encode<'o>(
        &self,
        out: &'o mut [::core::primitive::u8],
    ) -> ::core::result::Result<
        ::ridl_rt::payload::Encoded<'o, ::ridl_rt::proto3::Verified<'o>>,
        ::ridl_rt::payload::EncodeError,
    > {
        let __needed = __ridl_pb_len_DocsNoteParams(self);
        ::ridl_rt::proto3::check_capacity(__needed, out)?;
        let mut __writer = ::ridl_rt::proto3::Writer::new(out);
        __ridl_pb_encode_DocsNoteParams(self, &mut __writer)?;
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
        __ridl_pb_verify_DocsNoteParams(buf)?;
        ::core::result::Result::Ok(::ridl_rt::proto3::Verified::new(buf))
    }
    #[allow(clippy::collapsible_if)]
    fn decode(
        r: ::ridl_rt::payload::Ref<'_, Self, ::ridl_rt::encoding::Proto3>,
    ) -> Self {
        __ridl_pb_decode_DocsNoteParams(::ridl_rt::payload::View::bytes(r.view()))
    }
}
/// Descriptor for interface `Log`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.
///
/// Its size table in FlatBuffers is `<Log as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in proto3 is `<Log as Sizes<Proto3>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
pub struct Log;
impl ::ridl_rt::contract::Interface for Log {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.stream",
        hash: ::ridl_rt::contract::CatalogHash([
            110, 76, 4, 45, 37, 232, 6, 178, 196, 135, 157, 71, 235, 153, 35, 3, 43, 115,
            249, 55, 62, 70, 114, 46, 17, 58, 67, 17, 96, 110, 238, 215,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(2);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Log";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "history",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "<Reading>",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(0),
                    form: ::ridl_rt::contract::StreamForm::Elements,
                    element: "face.stream.Reading",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "clear",
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
            streams: &[],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(3),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "upload",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "LogUploadParams",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(1),
                    form: ::ridl_rt::contract::StreamForm::Elements,
                    element: "face.stream.Reading",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Log {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <LogUploadParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <LogUploadParams as ::ridl_rt::payload::Payload<
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
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Log {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <LogUploadParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::Proto3,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
            >>::MAX_SIZE,
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
            >>::MAX_SIZE,
            <LogUploadParams as ::ridl_rt::payload::Payload<
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
#[doc(hidden)]
pub struct LogHistory;
impl ::ridl_rt::contract::Interaction for LogHistory {
    type Iface = Log;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Log as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Query for LogHistory {
    type Args = Level;
    type Reply = ();
    fn require(args: &Self::Args) -> ::core::result::Result<(), ()> {
        if args.0 > 0 {
            ::core::result::Result::Ok(())
        } else {
            ::core::result::Result::Err(())
        }
    }
    fn ensure(_args: &Self::Args, _reply: &()) -> ::core::result::Result<(), ()> {
        Ok(())
    }
}
#[doc(hidden)]
pub struct LogClear;
impl ::ridl_rt::contract::Interaction for LogClear {
    type Iface = Log;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Log as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Command for LogClear {
    type Args = Level;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
#[doc(hidden)]
pub struct LogUpload;
impl ::ridl_rt::contract::Interaction for LogUpload {
    type Iface = Log;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Log as ::ridl_rt::contract::Interface>::MEMBERS[2];
    const ROW: ::core::primitive::usize = 2;
}
impl ::ridl_rt::contract::Command for LogUpload {
    type Args = LogUploadParams;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
/// Descriptor for interface `Plain`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.
///
/// Its size table in FlatBuffers is `<Plain as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in proto3 is `<Plain as Sizes<Proto3>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
pub struct Plain;
impl ::ridl_rt::contract::Interface for Plain {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.stream",
        hash: ::ridl_rt::contract::CatalogHash([
            110, 76, 4, 45, 37, 232, 6, 178, 196, 135, 157, 71, 235, 153, 35, 3, 43, 115,
            249, 55, 62, 70, 114, 46, 17, 58, 67, 17, 96, 110, 238, 215,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(3);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Plain";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "latest",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Reading",
                },
            ],
            streams: &[],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Plain {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64
            + <Reading as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Reading as ::ridl_rt::payload::Payload<
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
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Plain {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64
            + <Reading as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
            >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
            >>::MAX_SIZE,
            <Reading as ::ridl_rt::payload::Payload<
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
#[doc(hidden)]
pub struct PlainLatest;
impl ::ridl_rt::contract::Interaction for PlainLatest {
    type Iface = Plain;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Plain as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Query for PlainLatest {
    type Args = Level;
    type Reply = Reading;
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
/// Descriptor for interface `Twin`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.
///
/// Its size table in FlatBuffers is `<Twin as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in proto3 is `<Twin as Sizes<Proto3>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
pub struct Twin;
impl ::ridl_rt::contract::Interface for Twin {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.stream",
        hash: ::ridl_rt::contract::CatalogHash([
            110, 76, 4, 45, 37, 232, 6, 178, 196, 135, 157, 71, 235, 153, 35, 3, 43, 115,
            249, 55, 62, 70, 114, 46, 17, 58, 67, 17, 96, 110, 238, 215,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(4);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Twin";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "left",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "<Reading>",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(0),
                    form: ::ridl_rt::contract::StreamForm::Elements,
                    element: "face.stream.Reading",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "right",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(200000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Level",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "<Level>",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(0),
                    form: ::ridl_rt::contract::StreamForm::Elements,
                    element: "face.stream.Level",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Twin {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Level as ::ridl_rt::payload::Payload<
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
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Twin {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Level as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::Proto3,
            >>::MAX_SIZE,
            <Level as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE,
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
#[doc(hidden)]
pub struct TwinLeft;
impl ::ridl_rt::contract::Interaction for TwinLeft {
    type Iface = Twin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Twin as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Query for TwinLeft {
    type Args = Level;
    type Reply = ();
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(_args: &Self::Args, _reply: &()) -> ::core::result::Result<(), ()> {
        Ok(())
    }
}
#[doc(hidden)]
pub struct TwinRight;
impl ::ridl_rt::contract::Interaction for TwinRight {
    type Iface = Twin;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Twin as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Query for TwinRight {
    type Args = Level;
    type Reply = ();
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(_args: &Self::Args, _reply: &()) -> ::core::result::Result<(), ()> {
        Ok(())
    }
}
/// Descriptor for interface `Docs`: its catalog, number, name and members, as `ridl_rt::contract::Interface`.
///
/// Its size table in FlatBuffers is `<Docs as Sizes<FlatBuffers>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
///
/// Its size table in proto3 is `<Docs as Sizes<Proto3>>`: the space each member reserves, and the largest buffer a call or an event of this interface needs.
pub struct Docs;
impl ::ridl_rt::contract::Interface for Docs {
    const CATALOG: &'static ::ridl_rt::contract::CatalogRef = &::ridl_rt::contract::CatalogRef {
        name: "face.stream",
        hash: ::ridl_rt::contract::CatalogHash([
            110, 76, 4, 45, 37, 232, 6, 178, 196, 135, 157, 71, 235, 153, 35, 3, 43, 115,
            249, 55, 62, 70, 114, 46, 17, 58, 67, 17, 96, 110, 238, 215,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(1);
    const PROVISIONAL: ::core::primitive::bool = true;
    const NAME: &'static ::core::primitive::str = "Docs";
    const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(1),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "fetch",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Id",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "<bytes>",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(0),
                    form: ::ridl_rt::contract::StreamForm::Bytes,
                    element: "",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(2),
            kind: ::ridl_rt::contract::Kind::Query,
            name: "text",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Id",
                },
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "<string>",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(0),
                    form: ::ridl_rt::contract::StreamForm::Text,
                    element: "",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(3),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "put",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "Info",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(1),
                    form: ::ridl_rt::contract::StreamForm::Bytes,
                    element: "",
                },
            ],
        },
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(4),
            kind: ::ridl_rt::contract::Kind::Command,
            name: "note",
            timing: ::core::option::Option::Some(::ridl_rt::contract::Timing {
                mode: ::ridl_rt::contract::TimingMode::Range,
                min: ::core::option::Option::None,
                max: ::core::option::Option::Some(::ridl_rt::sample::Duration(1000000)),
            }),
            payloads: &[
                ::ridl_rt::contract::PayloadInfo {
                    type_name: "DocsNoteParams",
                },
            ],
            streams: &[
                ::ridl_rt::contract::StreamInfo {
                    no: ::ridl_rt::contract::StreamNo(1),
                    form: ::ridl_rt::contract::StreamForm::Text,
                    element: "",
                },
            ],
        },
    ];
}
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Docs {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE
            as ::core::primitive::u64,
        <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE
            as ::core::primitive::u64,
        <Info as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE
            as ::core::primitive::u64,
        <DocsNoteParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::FlatBuffers,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Id as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Id as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <Info as ::ridl_rt::payload::Payload<
                ::ridl_rt::encoding::FlatBuffers,
            >>::MAX_SIZE,
            <DocsNoteParams as ::ridl_rt::payload::Payload<
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
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Docs {
    const RESERVATIONS: &'static [::core::primitive::u64] = &[
        <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <Info as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE
            as ::core::primitive::u64,
        <DocsNoteParams as ::ridl_rt::payload::Payload<
            ::ridl_rt::encoding::Proto3,
        >>::MAX_SIZE as ::core::primitive::u64,
    ];
    const MAX_BUFFER_SIZE: ::core::primitive::usize = {
        let sizes = [
            <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE,
            <Id as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE,
            <Info as ::ridl_rt::payload::Payload<::ridl_rt::encoding::Proto3>>::MAX_SIZE,
            <DocsNoteParams as ::ridl_rt::payload::Payload<
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
#[doc(hidden)]
pub struct DocsFetch;
impl ::ridl_rt::contract::Interaction for DocsFetch {
    type Iface = Docs;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Docs as ::ridl_rt::contract::Interface>::MEMBERS[0];
    const ROW: ::core::primitive::usize = 0;
}
impl ::ridl_rt::contract::Query for DocsFetch {
    type Args = Id;
    type Reply = ();
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(_args: &Self::Args, _reply: &()) -> ::core::result::Result<(), ()> {
        Ok(())
    }
}
#[doc(hidden)]
pub struct DocsText;
impl ::ridl_rt::contract::Interaction for DocsText {
    type Iface = Docs;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Docs as ::ridl_rt::contract::Interface>::MEMBERS[1];
    const ROW: ::core::primitive::usize = 1;
}
impl ::ridl_rt::contract::Query for DocsText {
    type Args = Id;
    type Reply = ();
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
    fn ensure(_args: &Self::Args, _reply: &()) -> ::core::result::Result<(), ()> {
        Ok(())
    }
}
#[doc(hidden)]
pub struct DocsPut;
impl ::ridl_rt::contract::Interaction for DocsPut {
    type Iface = Docs;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Docs as ::ridl_rt::contract::Interface>::MEMBERS[2];
    const ROW: ::core::primitive::usize = 2;
}
impl ::ridl_rt::contract::Command for DocsPut {
    type Args = Info;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
#[doc(hidden)]
pub struct DocsNote;
impl ::ridl_rt::contract::Interaction for DocsNote {
    type Iface = Docs;
    const MEMBER: &'static ::ridl_rt::contract::Member = &<Docs as ::ridl_rt::contract::Interface>::MEMBERS[3];
    const ROW: ::core::primitive::usize = 3;
}
impl ::ridl_rt::contract::Command for DocsNote {
    type Args = DocsNoteParams;
    fn require(_args: &Self::Args) -> ::core::result::Result<(), ()> {
        ::core::result::Result::Ok(())
    }
}
/// A log of readings, read as a stream.
///
///The generated interaction face of interface `Log`.
#[allow(clippy::module_inception)]
pub mod log {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct ClearCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct UploadCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct HistoryCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Log as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Log` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Log`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::cell::RefCell<P>,
        upload: UploadState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Clears the log.
        ///
        /// Sends command `clear` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
        pub fn clear(&mut self, level: super::Level) -> ClearCall<'_, P> {
            let __arg = level;
            let deadline = <super::LogClear as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.get_mut().now().0.saturating_add(max.0),
                ));
            let phase = match send_clear(self.port.get_mut(), &__arg) {
                Ok(correlation) => ClearPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => ClearPhase::Unsent(__arg),
                Err(error) => ClearPhase::Failed(error),
            };
            ClearCall {
                port: self.port.get_mut(),
                phase,
                deadline,
            }
        }
        /// Uploads readings.
        ///
        ///Sends the command and returns its acknowledgment future and parameter sink. The values share this client's port until both are dropped. The acknowledgment reports delivery, independently of later stream refusal. Dropping an unfinished sink interrupts the stream. An empty sink is locally ready; peer refusal is read from the next push or end.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `data`
        pub fn upload(
            &mut self,
            args: super::LogUploadParams,
        ) -> (UploadCall<'_, P>, UploadSink<'_, P>) {
            let __arg = args;
            self.upload = UploadState::new();
            let deadline = <super::LogUpload as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.borrow().now().0.saturating_add(max.0),
                ));
            let mut pending = None;
            match send_upload(&mut *self.port.borrow_mut(), &__arg) {
                Ok(c) => self.upload.correlation.set(Some(c.0)),
                Err(::ridl_rt::port::SendError::Busy) => pending = Some(__arg),
                Err(e) => {
                    self.upload.error.set(Some(::ridl_rt::error::ClientError::Send(e)));
                    self.upload.call_error.set(self.upload.error.get());
                }
            }
            (
                UploadCall {
                    port: &self.port,
                    state: &self.upload,
                    pending,
                    deadline,
                    done: false,
                },
                UploadSink {
                    port: &self.port,
                    state: &self.upload,
                    buffer: <super::Reading as ::ridl_rt::payload::Payload<
                        P::Encoding,
                    >>::BUFFER,
                    held: None,
                },
            )
        }
        /// The readings up to `limit`, oldest first.
        ///
        /// Sends query `history` and returns its future, which resolves to the reply stream, `HistoryStream`, once the first element or the end of the stream arrived, and to `Transport::Timeout` when the bound passes before either. The call is sent when this method runs, not when the future is first polled. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        /// - require `limit > 0`
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        pub fn history(&mut self, limit: super::Level) -> HistoryCall<'_, P> {
            let __arg = limit;
            let deadline = <super::LogHistory as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.get_mut().now().0.saturating_add(max.0),
                ));
            let phase = match send_history(self.port.get_mut(), &__arg) {
                Ok(correlation) => HistoryPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => HistoryPhase::Unsent(__arg),
                Err(error) => HistoryPhase::Failed(error),
            };
            HistoryCall {
                port: Some(self.port.get_mut()),
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.
        ///
        /// # Panics
        ///
        /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Log as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Log` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client {
                port: ::core::cell::RefCell::new(port),
                upload: UploadState::new(),
            }
        }
    }
    struct UploadState {
        correlation: ::core::cell::Cell<Option<::ridl_rt::port::Correlation>>,
        ack: ::core::cell::Cell<bool>,
        call_error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
        ended: ::core::cell::Cell<bool>,
        forgotten: ::core::cell::Cell<bool>,
        error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
    }
    impl UploadState {
        fn new() -> Self {
            Self {
                call_error: ::core::cell::Cell::new(None),
                correlation: ::core::cell::Cell::new(None),
                ack: ::core::cell::Cell::new(false),
                ended: ::core::cell::Cell::new(false),
                forgotten: ::core::cell::Cell::new(false),
                error: ::core::cell::Cell::new(None),
            }
        }
        fn forget<P: ::ridl_rt::port::Caller>(&self, port: &::core::cell::RefCell<P>) {
            match self.correlation.get() {
                Some(c) if !self.forgotten.replace(true) => port.borrow_mut().forget(c),
                _ => {}
            }
        }
    }
    fn upload_push_error(
        error: ::ridl_rt::port::PushError,
    ) -> ::ridl_rt::error::ClientError {
        ::ridl_rt::error::ClientError::Call(
            match error {
                ::ridl_rt::port::PushError::Failed(e) => e,
                _ => {
                    ::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Interrupted,
                    )
                }
            },
        )
    }
    ///The future of command `clear`, returned by `Client::clear`. It resolves to the delivery acknowledgment: `Ok(())`, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the command was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct ClearCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: ClearPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum ClearPhase {
        Unsent(super::Level),
        Waiting(ClearCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ClearCall<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, ClearPhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for ClearCall<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, ClearPhase::Done) {
                    ClearPhase::Done => panic!("`ClearCall` polled after completion"),
                    ClearPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    ClearPhase::Unsent(__arg) => {
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
                        match send_clear(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = ClearPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = ClearPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    ClearPhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_clear_ack(
                            &mut *this.port,
                            correlation,
                        ) {
                            Some(outcome) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            None => {
                                if !this.expired() {
                                    this.phase = ClearPhase::Waiting(correlation);
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
    > ::core::ops::Drop for ClearCall<'_, P> {
        fn drop(&mut self) {
            if let ClearPhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    ///The delivery acknowledgment of a command with a parameter stream. Dropping it before acknowledgment forgets the call. Completion leaves an open sink usable.
    #[must_use]
    pub struct UploadCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
        pending: Option<super::LogUploadParams>,
        deadline: Option<::ridl_rt::sample::Timestamp>,
        done: bool,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::marker::Unpin for UploadCall<'_, P> {}
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for UploadCall<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            assert!(! this.done, "call polled after completion");
            if this.pending.is_some()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                let e = ::ridl_rt::error::ClientError::Send(
                    ::ridl_rt::port::SendError::Busy,
                );
                this.done = true;
                this.state.call_error.set(Some(e));
                this.state.error.set(Some(e));
                return ::core::task::Poll::Ready(Err(e));
            }
            if let Some(arg) = this.pending.take() {
                this.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                match send_upload(&mut *this.port.borrow_mut(), &arg) {
                    Ok(c) => this.state.correlation.set(Some(c.0)),
                    Err(::ridl_rt::port::SendError::Busy) => this.pending = Some(arg),
                    Err(e) => {
                        this.state
                            .call_error
                            .set(Some(::ridl_rt::error::ClientError::Send(e)));
                        this.state.error.set(this.state.call_error.get());
                    }
                }
            }
            let mut result = this.state.call_error.get().map(Err);
            if let (true, Some(c)) = (result.is_none(), this.state.correlation.get()) {
                this.port
                    .borrow()
                    .wake_on(::ridl_rt::port::Interest::Outcome(c), cx.waker());
                result = poll_upload_ack(
                        &mut *this.port.borrow_mut(),
                        UploadCorrelation(c),
                    )
                    .map(|v| v.map_err(::ridl_rt::error::ClientError::Call));
            }
            if result.is_none()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                result = Some(
                    Err(
                        if this.pending.is_some() {
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            )
                        } else {
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Undelivered,
                                ),
                            )
                        },
                    ),
                );
            }
            if let Some(result) = result {
                this.done = true;
                this.state.ack.set(true);
                if result.is_err() {
                    this.state.error.set(result.err());
                    this.state.forget(this.port);
                } else if this.state.ended.get() {
                    this.state.forget(this.port);
                }
                ::core::task::Poll::Ready(result)
            } else {
                ::core::task::Poll::Pending
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for UploadCall<'_, P> {
        fn drop(&mut self) {
            if !self.done {
                self.state
                    .error
                    .set(
                        Some(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Interrupted,
                                ),
                            ),
                        ),
                    );
                self.state.forget(self.port);
            }
        }
    }
    ///The command's parameter sink. It shares the call's port without allocating. Empty writes are local no-ops; peer refusal is learned from a push or end. Dropping it before end interrupts the upload.
    pub struct UploadSink<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    >
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
        buffer: <super::Reading as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,
        held: Option<(usize, usize)>,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > UploadSink<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        ///Encodes and verifies one element. Holds at most one element while the window is full; another send then reports Busy. A zero-byte encoding is an accepted element, sent exactly once.
        pub fn send(
            &mut self,
            item: &super::Reading,
        ) -> Result<(), ::ridl_rt::error::ClientError> {
            if let Some(e) = self.state.error.get() {
                return Err(e);
            }
            if self.held.is_some() {
                return Err(
                    ::ridl_rt::error::ClientError::Call(
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Busy,
                        ),
                    ),
                );
            }
            if self.state.ended.get() {
                return Err(upload_push_error(::ridl_rt::port::PushError::Ended));
            }
            let buf = ::core::convert::AsMut::<[u8]>::as_mut(&mut self.buffer);
            let base = buf.as_ptr() as usize;
            let checked = (|| {
                let bytes = match ::ridl_rt::payload::Ref::<
                    super::Reading,
                    P::Encoding,
                >::encode(item, buf) {
                    Ok(encoded) => encoded.bytes(),
                    Err(::ridl_rt::payload::EncodeError::Contract(v)) => {
                        return Err(
                            ::ridl_rt::error::CallError::Contract(
                                ::ridl_rt::error::Contract::InvalidValue(v),
                            ),
                        );
                    }
                    Err(
                        ::ridl_rt::payload::EncodeError::Capacity { needed, available },
                    ) => {
                        unreachable!(
                            "encoding a parameter element needs {} bytes and its buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                            needed, available
                        )
                    }
                    Err(_) => unreachable!("encoding a parameter element failed"),
                };
                match ::ridl_rt::payload::Ref::<
                    super::Reading,
                    P::Encoding,
                >::verify(bytes) {
                    Ok(_) => Ok((bytes.as_ptr() as usize - base, bytes.len())),
                    Err(::ridl_rt::payload::VerifyError::Contract(v)) => {
                        Err(
                            ::ridl_rt::error::CallError::Contract(
                                ::ridl_rt::error::Contract::InvalidValue(v),
                            ),
                        )
                    }
                    _ => {
                        Err(
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Corrupt,
                            ),
                        )
                    }
                }
            })();
            let (start, len) = match checked {
                Ok(range) => range,
                Err(e) => {
                    if let Some(c) = self.state.correlation.get() {
                        let _ = self
                            .port
                            .borrow_mut()
                            .abort(c, ::ridl_rt::contract::StreamNo(1), e);
                    }
                    let error = ::ridl_rt::error::ClientError::Call(e);
                    self.state.error.set(Some(error));
                    return Err(error);
                }
            };
            self.held = Some((start, start + len));
            if let Some(c) = self.state.correlation.get() {
                let bytes = &::core::convert::AsRef::<
                    [u8],
                >::as_ref(&self.buffer)[start..start + len];
                match self
                    .port
                    .borrow_mut()
                    .push(c, ::ridl_rt::contract::StreamNo(1), bytes)
                {
                    Ok(n) if n == len => self.held = None,
                    Ok(_) => {}
                    Err(e) => {
                        let error = upload_push_error(e);
                        self.state.error.set(Some(error));
                        return Err(error);
                    }
                }
            }
            Ok(())
        }
        ///Waits for readiness, then sends one checked element.
        pub async fn feed(
            &mut self,
            item: &super::Reading,
        ) -> Result<(), ::ridl_rt::error::ClientError> {
            self.ready().await?;
            self.send(item)
        }
        ///Flushes a held element. With no held element readiness describes local capacity and does not probe the peer.
        pub fn poll_ready(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if let Some(e) = self.state.error.get() {
                return ::core::task::Poll::Ready(Err(e));
            }
            let Some((start, end)) = self.held else {
                return ::core::task::Poll::Ready(Ok(()));
            };
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            self.port
                .borrow()
                .wake_on(
                    ::ridl_rt::port::Interest::Stream(
                        c,
                        ::ridl_rt::contract::StreamNo(1),
                    ),
                    cx.waker(),
                );
            let bytes = &::core::convert::AsRef::<
                [u8],
            >::as_ref(&self.buffer)[start..end];
            let pushed = self
                .port
                .borrow_mut()
                .push(c, ::ridl_rt::contract::StreamNo(1), bytes);
            match pushed {
                Ok(n) if n == bytes.len() => {
                    self.held = None;
                    ::core::task::Poll::Ready(Ok(()))
                }
                Ok(_) => ::core::task::Poll::Pending,
                Err(e) => {
                    let error = upload_push_error(e);
                    self.state.error.set(Some(error));
                    ::core::task::Poll::Ready(Err(error))
                }
            }
        }
        ///Waits until the sink has no held element.
        pub async fn ready(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_ready(cx)).await
        }
        ///Flushes the held element and ends the parameter stream. The call retains ownership until acknowledgment.
        pub fn poll_end(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if self.state.ended.get() {
                return ::core::task::Poll::Ready(Ok(()));
            }
            match self.poll_ready(cx) {
                ::core::task::Poll::Ready(Ok(())) => {}
                other => return other,
            }
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            let result = self
                .port
                .borrow_mut()
                .end(c, ::ridl_rt::contract::StreamNo(1))
                .map_err(upload_push_error);
            if result.is_ok() {
                self.state.ended.set(true);
                if self.state.ack.get() {
                    self.state.forget(self.port);
                }
            } else {
                self.state.error.set(result.err());
            }
            ::core::task::Poll::Ready(result)
        }
        ///Ends the stream after flushing any held element.
        pub async fn end(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_end(cx)).await
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for UploadSink<'_, P>
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn drop(&mut self) {
            if !self.state.ended.get() {
                if let (true, Some(c)) = (
                    self.state.error.get().is_none() && !self.state.forgotten.get(),
                    self.state.correlation.get(),
                ) {
                    let _ = self
                        .port
                        .borrow_mut()
                        .abort(
                            c,
                            ::ridl_rt::contract::StreamNo(1),
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Interrupted,
                            ),
                        );
                }
                self.state.forget(self.port);
            }
        }
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StreamState {
        First(usize),
        Open,
        Refused(::ridl_rt::error::CallError),
        Stopped,
        Done,
    }
    ///The future of query `history`, returned by `Client::history`. It resolves to `HistoryStream`, the reply stream, once its first element or its end arrived; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; to `Transport::Timeout` when the call was sent and its bound passed before the first element or the end; and to the outcome the provider settled when it failed the call before any element. A first chunk that is no legal element resolves it to the stream, which yields the refusal, `Transport::Corrupt`, as its one error. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits calls `Caller::forget` on the call, which interrupts the stream; a future that resolved to an error has already done so, and one that resolved to the stream handed the call to it. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its stream is taken only when the future is polled"]
    pub struct HistoryCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::option::Option<&'a mut P>,
        phase: HistoryPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum HistoryPhase {
        Unsent(super::Level),
        Waiting(HistoryCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > HistoryCall<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            match (&self.port, self.deadline) {
                (Some(port), Some(deadline)) => port.now() > deadline,
                _ => false,
            }
        }
        fn port(&mut self) -> &mut P {
            match self.port.as_deref_mut() {
                Some(port) => port,
                None => panic!("`HistoryCall` polled after completion"),
            }
        }
    }
    impl<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for HistoryCall<'a, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            HistoryStream<'a, P>,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, HistoryPhase::Done) {
                    HistoryPhase::Done => panic!("`HistoryCall` polled after completion"),
                    HistoryPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    HistoryPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        let port = this.port();
                        port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_history(port, &__arg) {
                            Ok(correlation) => {
                                this.phase = HistoryPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = HistoryPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    HistoryPhase::Waiting(correlation) => {
                        let expired = this.expired();
                        let port = this.port();
                        port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        let mut __buf = <super::Reading as ::ridl_rt::payload::Payload<
                            P::Encoding,
                        >>::BUFFER;
                        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
                            &mut __buf,
                        );
                        let state = match port
                            .pull(correlation.0, ::ridl_rt::port::REPLY_STREAM, buf)
                        {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                                StreamState::First(len)
                            }
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                if !expired {
                                    this.phase = HistoryPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ),
                                );
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                port.forget(correlation.0);
                                StreamState::Done
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Call(error)),
                                );
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                let error = ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                );
                                let _ = port
                                    .abort(correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
                                StreamState::Refused(error)
                            }
                            Err(error) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Read(error)),
                                );
                            }
                        };
                        let port = match this.port.take() {
                            Some(port) => port,
                            None => panic!("`HistoryCall` polled after completion"),
                        };
                        return ::core::task::Poll::Ready(
                            Ok(HistoryStream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                            }),
                        );
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for HistoryCall<'_, P> {
        fn drop(&mut self) {
            if let (Some(port), HistoryPhase::Waiting(correlation)) = (
                &mut self.port,
                &self.phase,
            ) {
                port.forget(correlation.0);
            }
        }
    }
    ///The reply stream of query `history`, which `HistoryCall` resolves to. `poll_next` is the pull API, the whole face on a target without `std`: it registers the stream's interest, pulls one chunk, and verifies and decodes it as one element; `next` awaits it. An element that fails its check refuses the stream with `Contract::InvalidValue`, and a chunk that is no well-formed element, or longer than any legal one, refuses it with `Transport::Corrupt`: the stream is aborted with that outcome, which the provider reads from its next push, and the refusal is the one error the stream yields; the call is kept until the stream is dropped. A stream the provider stopped yields its outcome, `Transport::Interrupted` for a stop on purpose, as its one error, and the call is forgotten at once, as it is after the end. After the end or an error every later `poll_next` is `Ready(None)`. Dropping the stream forgets the call, once, unless the end or the provider's failure already did; before the end that interrupts the stream (ridl §12).
    #[must_use = "dropping the stream before its end interrupts it"]
    pub struct HistoryStream<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    >
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port: &'a mut P,
        correlation: HistoryCorrelation,
        buffer: <super::Reading as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,
        state: StreamState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > HistoryStream<'_, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Pulls the next element: `Ready(Some(Ok(element)))` with the
        /// next element, `Ready(None)` once the stream ended,
        /// `Ready(Some(Err(error)))` once and then `Ready(None)` when
        /// the stream stopped or was refused, and `Pending` while no
        /// chunk is waiting, after the waker of `cx` was registered
        /// under `Interest::Stream`. A refused stream keeps its call
        /// until this value is dropped.
        pub fn poll_next(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<
            ::core::option::Option<
                ::core::result::Result<super::Reading, ::ridl_rt::error::ClientError>,
            >,
        > {
            let len = match self.state {
                StreamState::Done | StreamState::Stopped => {
                    return ::core::task::Poll::Ready(None);
                }
                StreamState::Refused(error) => {
                    self.state = StreamState::Stopped;
                    return ::core::task::Poll::Ready(
                        Some(Err(::ridl_rt::error::ClientError::Call(error))),
                    );
                }
                StreamState::First(len) => {
                    self.state = StreamState::Open;
                    len
                }
                StreamState::Open => {
                    self.port
                        .wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                self.correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                    match self
                        .port
                        .pull(
                            self.correlation.0,
                            ::ridl_rt::port::REPLY_STREAM,
                            ::core::convert::AsMut::as_mut(&mut self.buffer),
                        )
                    {
                        Ok(::ridl_rt::port::Pull::Chunk(len)) => len,
                        Ok(::ridl_rt::port::Pull::Pending) => {
                            return ::core::task::Poll::Pending;
                        }
                        Ok(::ridl_rt::port::Pull::End) => {
                            self.finish();
                            return ::core::task::Poll::Ready(None);
                        }
                        Ok(::ridl_rt::port::Pull::Failed(error)) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Call(error))),
                            );
                        }
                        Err(::ridl_rt::port::ReadError::Short { .. }) => {
                            return ::core::task::Poll::Ready(
                                Some(
                                    Err(
                                        self
                                            .refuse(
                                                ::ridl_rt::error::CallError::Transport(
                                                    ::ridl_rt::error::Transport::Corrupt,
                                                ),
                                            ),
                                    ),
                                ),
                            );
                        }
                        Err(error) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Read(error))),
                            );
                        }
                    }
                }
            };
            let buf: &[::core::primitive::u8] = ::core::convert::AsRef::as_ref(
                &self.buffer,
            );
            let element = match ::ridl_rt::payload::Ref::<
                super::Reading,
                P::Encoding,
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
            };
            ::core::task::Poll::Ready(
                Some(
                    match element {
                        Ok(element) => Ok(element),
                        Err(error) => Err(self.refuse(error)),
                    },
                ),
            )
        }
        /// Awaits the next element: `poll_next` as a future.
        pub async fn next(
            &mut self,
        ) -> ::core::option::Option<
            ::core::result::Result<super::Reading, ::ridl_rt::error::ClientError>,
        > {
            ::core::future::poll_fn(|cx| self.poll_next(cx)).await
        }
        fn finish(&mut self) {
            self.state = StreamState::Done;
            self.port.forget(self.correlation.0);
        }
        fn refuse(
            &mut self,
            error: ::ridl_rt::error::CallError,
        ) -> ::ridl_rt::error::ClientError {
            let _ = self
                .port
                .abort(self.correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
            self.state = StreamState::Stopped;
            ::ridl_rt::error::ClientError::Call(error)
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for HistoryStream<'_, P>
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn drop(&mut self) {
            if self.state != StreamState::Done {
                self.port.forget(self.correlation.0);
            }
        }
    }
    pub(crate) fn send_clear<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        level: &super::Level,
    ) -> ::core::result::Result<ClearCorrelation, ::ridl_rt::port::SendError>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = level;
        <super::LogClear as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
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
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .command(
                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(2u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(ClearCorrelation)
    }
    pub(crate) fn poll_clear_ack<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: ClearCorrelation,
    ) -> ::core::option::Option<::core::result::Result<(), ::ridl_rt::error::CallError>>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port.ack(correlation.0)
    }
    pub(crate) fn send_upload<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        args: &super::LogUploadParams,
    ) -> ::core::result::Result<UploadCorrelation, ::ridl_rt::port::SendError>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = args;
        <super::LogUpload as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::LogUploadParams as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::LogUploadParams,
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
                    "encoding `LogUploadParams` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `LogUploadParams` failed"),
        };
        __port
            .command(
                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(3u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(UploadCorrelation)
    }
    pub(crate) fn poll_upload_ack<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: UploadCorrelation,
    ) -> ::core::option::Option<::core::result::Result<(), ::ridl_rt::error::CallError>>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port.ack(correlation.0)
    }
    pub(crate) fn send_history<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        limit: &super::Level,
    ) -> ::core::result::Result<HistoryCorrelation, ::ridl_rt::port::SendError>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = limit;
        <super::LogHistory as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
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
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .query(
                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(HistoryCorrelation)
    }
    /// What an application implements to serve interface `Log`'s calls.
    ///
    /// An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses.
    pub trait Provider {
        /// Clears the log.
        ///
        ///Serves command `clear`. It returns nothing: a command has no failure the application reports (ridl §6.1). Arguments that break their typl constraints or the `require` clauses never reach it.
        ///
        /// # Rules
        ///
        /// - response within 50000 us
        ///
        /// # Parameters
        ///
        /// - `level`
        ///   - range 0 ..= 100
        fn clear(&mut self, level: &super::Level);
        ///The consumer of this command's parameter stream.
        type Upload: ::ridl_rt::face::Consumer<Item = super::Reading>;
        /// Uploads readings.
        ///
        ///Returns the consumer driven by serve after argument validation and successful delivery acknowledgment. Stream failures are reported to the consumer and the producer without changing that acknowledgment.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `data`
        fn upload(&mut self, args: &super::LogUploadParams) -> Self::Upload;
        ///The source of query `history`'s reply stream, which `serve` polls for each `Reading` it sends.
        type History: ::ridl_rt::face::Source<Item = super::Reading>;
        /// The readings up to `limit`, oldest first.
        ///
        ///Serves query `history`, whose reply is a stream of `Reading`: it returns the source `serve` drives. `serve` polls the source at most once per poll of its own, and only while the stream's window has room; it checks each element against its type before it is sent and ends the stream with `Contract::InvalidValue` on the first one that fails, sends `Ready(None)` as the end of the stream, and an `Interrupted` as `Transport::Interrupted`. The source is dropped once the stream ended, failed, or the caller stopped reading it. `ensure` clauses are not evaluated on a streamed reply.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        /// - require `limit > 0`
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        fn history(&mut self, limit: &super::Level) -> Self::History;
    }
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
        streams: &mut Streams<H, P>,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::StreamHandler,
        P: Provider,
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if buf.len()
            < <super::Log as ::ridl_rt::contract::Sizes<H::Encoding>>::MAX_BUFFER_SIZE
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
                        != <super::Log as ::ridl_rt::contract::Interface>::NUMBER
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
                                        match <super::LogClear as ::ridl_rt::contract::Command>::require(
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
                                                p.clear(&__arg);
                                                accepted
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(3u32) => {
                                match match ::ridl_rt::payload::Ref::<
                                    super::LogUploadParams,
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
                                } {
                                    Err(e) => h.settle(claim.id, Err(e)),
                                    Ok(__arg) => {
                                        match <super::LogUpload as ::ridl_rt::contract::Command>::require(
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
                                                if streams.upload.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    let accepted = h.settle(claim.id, Ok(&[]));
                                                    if accepted.is_ok() {
                                                        streams.upload = Some(InputSlot {
                                                            claim: claim.id,
                                                            consumer: p.upload(&__arg),
                                                            buffer: <super::Reading as ::ridl_rt::payload::Payload<
                                                                H::Encoding,
                                                            >>::BUFFER,
                                                        });
                                                        streams.__started = true;
                                                    }
                                                    accepted
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(1u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Level,
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
                                        match <super::LogHistory as ::ridl_rt::contract::Query>::require(
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
                                                if streams.history.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    streams.history = Some(StreamSlot {
                                                        claim: claim.id,
                                                        source: p.history(&__arg),
                                                        buffer: <super::Reading as ::ridl_rt::payload::Payload<
                                                            H::Encoding,
                                                        >>::BUFFER,
                                                        held: None,
                                                    });
                                                    streams.__started = true;
                                                    Ok(())
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
    type InputBuffer<H, T> = <T as ::ridl_rt::payload::Payload<
        <H as ::ridl_rt::port::Encoded>::Encoding,
    >>::Buffer;
    struct InputSlot<C, B> {
        claim: ::ridl_rt::port::ClaimId,
        consumer: C,
        buffer: B,
    }
    fn drive_input_upload<H, P>(
        h: &mut H,
        slot: &mut Option<InputSlot<P::Upload, InputBuffer<H, super::Reading>>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if slot.is_none() {
            return false;
        }
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let mut stream = slot.take().expect("checked above");
        match ::ridl_rt::face::Consumer::poll_ready(&mut stream.consumer, cx) {
            ::core::task::Poll::Pending => {
                *slot = Some(stream);
                return false;
            }
            ::core::task::Poll::Ready(()) => {}
        }
        h.wake_on(
            ::ridl_rt::port::Interest::ClaimStream(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
            ),
            cx.waker(),
        );
        match h
            .pull_claim(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
                ::core::convert::AsMut::as_mut(&mut stream.buffer),
            )
        {
            Ok(::ridl_rt::port::Pull::Chunk(n)) => {
                let bytes = &::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[..n];
                let decoded = match ::ridl_rt::payload::Ref::<
                    super::Reading,
                    H::Encoding,
                >::verify(bytes) {
                    Ok(v) => Ok(v.decode()),
                    Err(::ridl_rt::payload::VerifyError::Contract(v)) => {
                        Err(
                            ::ridl_rt::error::CallError::Contract(
                                ::ridl_rt::error::Contract::InvalidValue(v),
                            ),
                        )
                    }
                    _ => {
                        Err(
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Corrupt,
                            ),
                        )
                    }
                };
                match decoded {
                    Ok(item) => {
                        ::ridl_rt::face::Consumer::accept(&mut stream.consumer, item)
                    }
                    Err(e) => {
                        let _ = h
                            .abort_claim(
                                stream.claim,
                                ::ridl_rt::contract::StreamNo(1),
                                e,
                            );
                        ::ridl_rt::face::Consumer::failed(&mut stream.consumer, e);
                        return false;
                    }
                }
                *slot = Some(stream);
                true
            }
            Ok(::ridl_rt::port::Pull::End) => {
                ::ridl_rt::face::Consumer::end(&mut stream.consumer);
                false
            }
            Ok(::ridl_rt::port::Pull::Failed(e)) => {
                ::ridl_rt::face::Consumer::failed(&mut stream.consumer, e);
                false
            }
            Ok(::ridl_rt::port::Pull::Pending) => {
                *slot = Some(stream);
                false
            }
            Err(_) => {
                let e = ::ridl_rt::error::CallError::Transport(
                    ::ridl_rt::error::Transport::Corrupt,
                );
                let _ = h.abort_claim(stream.claim, ::ridl_rt::contract::StreamNo(1), e);
                ::ridl_rt::face::Consumer::failed(&mut stream.consumer, e);
                false
            }
        }
    }
    type ElementBuffer<H, T> = <T as ::ridl_rt::payload::Payload<
        <H as ::ridl_rt::port::Encoded>::Encoding,
    >>::Buffer;
    struct StreamSlot<S, B> {
        claim: ::ridl_rt::port::ClaimId,
        source: S,
        buffer: B,
        held: ::core::option::Option<(usize, usize)>,
    }
    pub(crate) struct Streams<H, P>
    where
        H: ::ridl_rt::port::Encoded,
        P: Provider,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        __started: bool,
        __port: ::core::marker::PhantomData<H>,
        history: ::core::option::Option<
            StreamSlot<P::History, ElementBuffer<H, super::Reading>>,
        >,
        upload: Option<InputSlot<P::Upload, InputBuffer<H, super::Reading>>>,
    }
    fn drive_stream<H, T, S>(
        h: &mut H,
        slot: &mut ::core::option::Option<StreamSlot<S, ElementBuffer<H, T>>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        T: ::ridl_rt::payload::Payload<H::Encoding>,
        S: ::ridl_rt::face::Source<Item = T>,
    {
        let Some(stream) = slot.as_mut() else {
            return false;
        };
        let claim = stream.claim;
        let interest = ::ridl_rt::port::Interest::ClaimStream(
            claim,
            ::ridl_rt::port::REPLY_STREAM,
        );
        if let Some((start, end)) = stream.held {
            h.wake_on(interest, cx.waker());
            let bytes = &::core::convert::AsRef::<
                [u8],
            >::as_ref(&stream.buffer)[start..end];
            match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
                Ok(taken) if taken == bytes.len() => stream.held = None,
                Ok(_) => return false,
                Err(error) => {
                    stop_stream(h, claim, error);
                    *slot = None;
                    return false;
                }
            }
        }
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let element = match stream.source.poll_next(cx) {
            ::core::task::Poll::Pending => return false,
            ::core::task::Poll::Ready(None) => {
                let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
                *slot = None;
                return false;
            }
            ::core::task::Poll::Ready(Some(Err(::ridl_rt::face::Interrupted))) => {
                let _ = h
                    .abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                *slot = None;
                return false;
            }
            ::core::task::Poll::Ready(Some(Ok(element))) => element,
        };
        let buf: &mut [u8] = ::core::convert::AsMut::as_mut(&mut stream.buffer);
        let base = buf.as_ptr() as usize;
        let checked = match ::ridl_rt::payload::Ref::<
            T,
            H::Encoding,
        >::encode(&element, buf) {
            Ok(encoded) => {
                let bytes = encoded.bytes();
                match ::ridl_rt::payload::Ref::<T, H::Encoding>::verify(bytes) {
                    Ok(_) => {
                        let start = bytes.as_ptr() as usize - base;
                        Ok((start, start + bytes.len()))
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
                }
            }
            Err(::ridl_rt::payload::EncodeError::Contract(violation)) => {
                Err(
                    ::ridl_rt::error::CallError::Contract(
                        ::ridl_rt::error::Contract::InvalidValue(violation),
                    ),
                )
            }
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding a stream element needs {} bytes and its buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding a stream element failed"),
        };
        let (start, end) = match checked {
            Ok(range) => range,
            Err(error) => {
                let _ = h.abort_claim(claim, ::ridl_rt::port::REPLY_STREAM, error);
                *slot = None;
                return false;
            }
        };
        h.wake_on(interest, cx.waker());
        let bytes = &::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[start..end];
        match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
            Ok(taken) if taken == bytes.len() => true,
            Ok(_) => {
                stream.held = Some((start, end));
                false
            }
            Err(error) => {
                stop_stream(h, claim, error);
                *slot = None;
                false
            }
        }
    }
    fn stop_stream<H: ::ridl_rt::port::StreamHandler>(
        h: &mut H,
        claim: ::ridl_rt::port::ClaimId,
        error: ::ridl_rt::port::PushError,
    ) {
        match error {
            ::ridl_rt::port::PushError::Failed(_) => {
                let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
            }
            ::ridl_rt::port::PushError::TooLarge { .. } => {
                let _ = h
                    .abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            _ => {}
        }
    }
    /// Serves interface `Log`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Log as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Log` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
    ///
    /// # Streamed replies
    ///
    /// A query whose reply is a stream is served from the source its `Provider` method returns, once the arguments and the `require` clauses pass. Each poll of the future first drives every stream it holds, then takes claims: a stream whose last element found the window full pushes that element again, after it registered its interest in the stream, and polls its source only once the element was taken; otherwise the source is polled once, which counts as one of the 32 claims of the poll. An element is encoded into a buffer inline in the future and checked against its type before it is pushed; one that fails stops the stream with `Contract::InvalidValue` and is not sent. `Ready(None)` ends the stream, and the claim is settled `Ok`; `Ready(Some(Err(Interrupted)))` stops it with `Transport::Interrupted`; a push above the member's limit (`PushError::TooLarge`) stops it with `Transport::Interrupted`; a push the caller refused or stopped reading (`PushError::Failed`) ends the claim. The source is dropped once its stream ended or stopped. The future drives one stream per member at a time: a call of a member whose stream is still running is settled `Transport::Busy`. The handler port gives the future no signal when a caller forgets the call, by its bound passing or by dropping its stream: the future reads the forget from its next push. So while a member's source answers `Pending`, a caller that stopped keeps the member busy until the source yields its next element, whose push fails, and only then is the source dropped and the member free for a new call. Dropping the future stops every stream it still drives with `Transport::Interrupted` and drops its source.
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                &[
                    ::ridl_rt::contract::Ordinal(2u32),
                    ::ridl_rt::contract::Ordinal(3u32),
                    ::ridl_rt::contract::Ordinal(1u32),
                ],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Log as ::ridl_rt::contract::Sizes<H::Encoding>>::CLAIM_BUFFER,
            state,
            streams: Streams {
                __started: false,
                __port: ::core::marker::PhantomData,
                history: None,
                upload: None,
            },
        }
    }
    ///The future `serve` returns over interface `Log`. It holds the handler, the provider, and the claim buffer of `<Log as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the encoding the handler port carries. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    >
    where
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        handler: H,
        provider: &'a mut P,
        buf: <super::Log as ::ridl_rt::contract::Sizes<H::Encoding>>::ClaimBuffer,
        state: ServeState,
        streams: Streams<H, P>,
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
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {}
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::LogUploadParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
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
                                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    let mut again = false;
                    again
                        |= drive_stream::<
                            H,
                            super::Reading,
                            P::History,
                        >(&mut this.handler, &mut this.streams.history, cx, &mut budget);
                    again
                        |= drive_input_upload::<
                            H,
                            P,
                        >(&mut this.handler, &mut this.streams.upload, cx, &mut budget);
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        ::core::convert::AsMut::as_mut(&mut this.buf),
                        &mut budget,
                        &mut this.streams,
                    ) {
                        Ok(_) => {
                            if ::core::mem::take(&mut this.streams.__started)
                                || budget == 0 || again
                            {
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
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::ops::Drop for Serve<'_, H, P>
    where
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        fn drop(&mut self) {
            if let Some(stream) = self.streams.history.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            if let Some(mut stream) = self.streams.upload.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::contract::StreamNo(1),
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                ::ridl_rt::face::Consumer::failed(
                    &mut stream.consumer,
                    ::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Interrupted,
                    ),
                );
            }
        }
    }
    ///The traits a consumer of interface `Log`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Log`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Log`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, all in scope through the parent module's `prelude`.
        pub struct Client<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }
        impl<
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > Client<P>
        where
            super::super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            /// Clears the log.
            ///
            ///Sends command `clear` and waits for its outcome, as `block_on` over `Client::clear`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Undelivered`; either way nothing is left waiting at the port.
            ///
            /// # Rules
            ///
            /// - response within 50000 us
            ///
            /// # Parameters
            ///
            /// - `level`
            ///   - range 0 ..= 100
            pub fn clear(
                &mut self,
                level: super::super::Level,
            ) -> ::core::result::Result<(), ::ridl_rt::error::ClientError> {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.clear(level);
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
                + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P>
        where
            super::super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.
            ///
            /// # Panics
            ///
            /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Log as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Log` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
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
                + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
                + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Timeout for Client<P>
        where
            super::super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::LogUploadParams: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
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
        /// Serves interface `Log`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        ///
        /// # Panics
        ///
        /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Log as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Log` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
                + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
            P: super::Provider,
            super::super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
            super::super::LogUploadParams: ::ridl_rt::payload::Payload<H::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
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
/// An interface with no stream, for the port-bound test. Named to sort after
/// `Log`: provisional interface numbers follow the sorted names.
///
///The generated interaction face of interface `Plain`.
#[allow(clippy::module_inception)]
pub mod plain {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct LatestCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Plain as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Plain` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Plain`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
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
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// The latest reading.
        ///
        /// Sends query `latest` and returns its future. The call is sent when this method runs, not when the future is first polled, and the future resolves on the outcome. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        pub fn latest(&mut self, limit: super::Level) -> LatestCall<'_, P> {
            let __arg = limit;
            let deadline = <super::PlainLatest as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_latest(&mut self.port, &__arg) {
                Ok(correlation) => LatestPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => LatestPhase::Unsent(__arg),
                Err(error) => LatestPhase::Failed(error),
            };
            LatestCall {
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
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.
        ///
        /// # Panics
        ///
        /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Plain as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Plain` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client { port }
        }
    }
    ///The future of query `latest`, returned by `Client::latest`. It resolves to the decoded reply, or the outcome the provider settled; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; and to the port's own expired outcome when the call was sent and its bound passed. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits for its outcome calls `Caller::forget` on the call; a future that has taken its outcome has already done so. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its outcome is taken only when the future is polled"]
    pub struct LatestCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        phase: LatestPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum LatestPhase {
        Unsent(super::Level),
        Waiting(LatestCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > LatestCall<'_, P>
    where
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            self.deadline.is_some_and(|deadline| self.port.now() > deadline)
        }
        #[cfg(feature = "std")]
        fn sent(&self) -> bool {
            matches!(self.phase, LatestPhase::Waiting(_))
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for LatestCall<'_, P>
    where
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            super::Reading,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, LatestPhase::Done) {
                    LatestPhase::Done => panic!("`LatestCall` polled after completion"),
                    LatestPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    LatestPhase::Unsent(__arg) => {
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
                        match send_latest(&mut *this.port, &__arg) {
                            Ok(correlation) => {
                                this.phase = LatestPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = LatestPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    LatestPhase::Waiting(correlation) => {
                        this.port
                            .wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                        let outcome = match poll_latest_reply(
                            &mut *this.port,
                            correlation,
                        ) {
                            Ok(Some(outcome)) => {
                                outcome.map_err(::ridl_rt::error::ClientError::Call)
                            }
                            Err(error) => Err(::ridl_rt::error::ClientError::Read(error)),
                            Ok(None) => {
                                if !this.expired() {
                                    this.phase = LatestPhase::Waiting(correlation);
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
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for LatestCall<'_, P> {
        fn drop(&mut self) {
            if let LatestPhase::Waiting(correlation) = &self.phase {
                self.port.forget(correlation.0);
            }
        }
    }
    pub(crate) fn send_latest<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        limit: &super::Level,
    ) -> ::core::result::Result<LatestCorrelation, ::ridl_rt::port::SendError>
    where
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = limit;
        <super::PlainLatest as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
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
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .query(
                <super::Plain as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(LatestCorrelation)
    }
    pub(crate) fn poll_latest_reply<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller,
    >(
        port: &mut P,
        correlation: LatestCorrelation,
    ) -> ::core::result::Result<
        ::core::option::Option<
            ::core::result::Result<super::Reading, ::ridl_rt::error::CallError>,
        >,
        ::ridl_rt::port::ReadError,
    >
    where
        super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let mut __buf = <super::Reading as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        match port.reply(correlation.0, buf)? {
            None => Ok(None),
            Some(Err(error)) => Ok(Some(Err(error))),
            Some(Ok(len)) => {
                Ok(
                    Some(
                        match ::ridl_rt::payload::Ref::<
                            super::Reading,
                            P::Encoding,
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
    /// What an application implements to serve interface `Plain`'s calls.
    ///
    /// An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses.
    pub trait Provider {
        /// The latest reading.
        ///
        ///Serves query `latest`. A reply that breaks an `ensure` clause is discarded by `serve`, which settles `ContractBroken` instead.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        fn latest(&mut self, limit: &super::Level) -> super::Reading;
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
        super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if buf.len()
            < <super::Plain as ::ridl_rt::contract::Sizes<H::Encoding>>::MAX_BUFFER_SIZE
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
                        != <super::Plain as ::ridl_rt::contract::Interface>::NUMBER
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
                                        match <super::PlainLatest as ::ridl_rt::contract::Query>::require(
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
                                                let reply = p.latest(&__arg);
                                                match <super::PlainLatest as ::ridl_rt::contract::Query>::ensure(
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
                                                        match ::ridl_rt::payload::Ref::<
                                                            super::Reading,
                                                            H::Encoding,
                                                        >::encode(&reply, buf) {
                                                            Ok(encoded) => {
                                                                let bytes = encoded.bytes();
                                                                h.settle(claim.id, Ok(bytes))
                                                            }
                                                            Err(
                                                                ::ridl_rt::payload::EncodeError::Contract(__violation),
                                                            ) => {
                                                                h.settle(
                                                                    claim.id,
                                                                    Err(
                                                                        ::ridl_rt::error::CallError::Contract(
                                                                            ::ridl_rt::error::Contract::InvalidValue(__violation),
                                                                        ),
                                                                    ),
                                                                )
                                                            }
                                                            Err(
                                                                ::ridl_rt::payload::EncodeError::Capacity {
                                                                    needed,
                                                                    available,
                                                                },
                                                            ) => {
                                                                unreachable!(
                                                                    "encoding `Reading` needs {} bytes and the dispatch buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                                                                    needed, available
                                                                )
                                                            }
                                                            Err(_) => unreachable!("encoding `Reading` failed"),
                                                        }
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
    /// Serves interface `Plain`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Plain as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Plain` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
        super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Plain as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(1u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Plain as ::ridl_rt::contract::Sizes<H::Encoding>>::CLAIM_BUFFER,
            state,
        }
    }
    ///The future `serve` returns over interface `Plain`. It holds the handler, the provider, and the claim buffer of `<Plain as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the encoding the handler port carries. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    >
    where
        super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
    {
        handler: H,
        provider: &'a mut P,
        buf: <super::Plain as ::ridl_rt::contract::Sizes<H::Encoding>>::ClaimBuffer,
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
        super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {}
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P>
    where
        super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
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
                                <super::Plain as ::ridl_rt::contract::Interface>::NUMBER,
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
    ///The traits a consumer of interface `Plain`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new`, `with_timeout`, `set_timeout` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
        #[cfg(feature = "std")]
        pub use ::ridl_rt::face::Timeout;
    }
    ///The blocking face of interface `Plain`, under the crate's `std` feature: `Client`, and `serve` when the interface declares a command or a query, as `ridl_rt::task::block_on` over the async face's futures, each bounded by a timeout. What a call does is the future's; this module adds the thread's wait and the timeout.
    #[cfg(feature = "std")]
    pub mod blocking {
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        ///The blocking consumer face of interface `Plain`: the async `Client` with a timeout, over the same ports. Every call is `ridl_rt::task::block_on` over the async call's future, so a call parks the calling thread until its outcome or this client's timeout. The member's `max` is measured by the future on the port's clock, and read only when the port wakes the call: a runtime that measures the bound and wakes the waiter when it passes ends the call at `max`; one that does not, `ridl-loopback` among them, leaves an unserved call waiting until this client's timeout. The timeout is `None` until `with_timeout` or `set_timeout` sets it, and with none a call returns only with its outcome, or at `max` on a runtime that wakes at it. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, `with_timeout` and `set_timeout` are `ridl_rt::face::Timeout`'s, all in scope through the parent module's `prelude`.
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
            super::super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            /// The latest reading.
            ///
            ///Sends query `latest` and waits for its outcome, as `block_on` over `Client::latest`. At this client's timeout a call that was never sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, and one that was sent is the port's expired outcome, `Transport::Timeout`; either way nothing is left waiting at the port.
            ///
            /// # Rules
            ///
            /// - response within 200000 us
            ///
            /// # Parameters
            ///
            /// - `limit`
            ///   - range 0 ..= 100
            pub fn latest(
                &mut self,
                limit: super::super::Level,
            ) -> ::core::result::Result<
                super::super::Reading,
                ::ridl_rt::error::ClientError,
            > {
                let __deadline = __deadline_after(self.timeout);
                let mut __call = self.inner.latest(limit);
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
            P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
                + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
        > ::ridl_rt::face::Bind for Client<P>
        where
            super::super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            type Port = P;
            /// Binds the face to a port, with no timeout. The port is held by value: pass a handle, or a `&mut` borrow of one.
            ///
            /// # Panics
            ///
            /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::Client`'s `new`. A program that must not panic makes the same comparison first, `port.catalog() == <Plain as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Plain` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
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
            super::super::Plain: ::ridl_rt::contract::Sizes<P::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
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
        /// Serves interface `Plain`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        ///
        /// # Panics
        ///
        /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Plain as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Plain` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
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
            super::super::Plain: ::ridl_rt::contract::Sizes<H::Encoding>,
            super::super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
            super::super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
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
/// Two streamed replies on one interface, for the test that `serve` drives
/// the source of each. Named to sort after `Plain`.
///
///The generated interaction face of interface `Twin`.
#[allow(clippy::module_inception)]
pub mod twin {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct LeftCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct RightCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Twin as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Twin` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Twin`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: P,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// The readings up to `limit`, oldest first.
        ///
        /// Sends query `left` and returns its future, which resolves to the reply stream, `LeftStream`, once the first element or the end of the stream arrived, and to `Transport::Timeout` when the bound passes before either. The call is sent when this method runs, not when the future is first polled. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        pub fn left(&mut self, limit: super::Level) -> LeftCall<'_, P> {
            let __arg = limit;
            let deadline = <super::TwinLeft as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_left(&mut self.port, &__arg) {
                Ok(correlation) => LeftPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => LeftPhase::Unsent(__arg),
                Err(error) => LeftPhase::Failed(error),
            };
            LeftCall {
                port: Some(&mut self.port),
                phase,
                deadline,
            }
        }
        /// The levels up to `limit`, oldest first.
        ///
        /// Sends query `right` and returns its future, which resolves to the reply stream, `RightStream`, once the first element or the end of the stream arrived, and to `Transport::Timeout` when the bound passes before either. The call is sent when this method runs, not when the future is first polled. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        pub fn right(&mut self, limit: super::Level) -> RightCall<'_, P> {
            let __arg = limit;
            let deadline = <super::TwinRight as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_right(&mut self.port, &__arg) {
                Ok(correlation) => RightPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => RightPhase::Unsent(__arg),
                Err(error) => RightPhase::Failed(error),
            };
            RightCall {
                port: Some(&mut self.port),
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.
        ///
        /// # Panics
        ///
        /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Twin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Twin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client { port }
        }
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StreamState {
        First(usize),
        Open,
        Refused(::ridl_rt::error::CallError),
        Stopped,
        Done,
    }
    ///The future of query `left`, returned by `Client::left`. It resolves to `LeftStream`, the reply stream, once its first element or its end arrived; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; to `Transport::Timeout` when the call was sent and its bound passed before the first element or the end; and to the outcome the provider settled when it failed the call before any element. A first chunk that is no legal element resolves it to the stream, which yields the refusal, `Transport::Corrupt`, as its one error. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits calls `Caller::forget` on the call, which interrupts the stream; a future that resolved to an error has already done so, and one that resolved to the stream handed the call to it. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its stream is taken only when the future is polled"]
    pub struct LeftCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::option::Option<&'a mut P>,
        phase: LeftPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum LeftPhase {
        Unsent(super::Level),
        Waiting(LeftCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > LeftCall<'_, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            match (&self.port, self.deadline) {
                (Some(port), Some(deadline)) => port.now() > deadline,
                _ => false,
            }
        }
        fn port(&mut self) -> &mut P {
            match self.port.as_deref_mut() {
                Some(port) => port,
                None => panic!("`LeftCall` polled after completion"),
            }
        }
    }
    impl<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for LeftCall<'a, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            LeftStream<'a, P>,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, LeftPhase::Done) {
                    LeftPhase::Done => panic!("`LeftCall` polled after completion"),
                    LeftPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    LeftPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        let port = this.port();
                        port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_left(port, &__arg) {
                            Ok(correlation) => {
                                this.phase = LeftPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = LeftPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    LeftPhase::Waiting(correlation) => {
                        let expired = this.expired();
                        let port = this.port();
                        port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        let mut __buf = <super::Reading as ::ridl_rt::payload::Payload<
                            P::Encoding,
                        >>::BUFFER;
                        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
                            &mut __buf,
                        );
                        let state = match port
                            .pull(correlation.0, ::ridl_rt::port::REPLY_STREAM, buf)
                        {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                                StreamState::First(len)
                            }
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                if !expired {
                                    this.phase = LeftPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ),
                                );
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                port.forget(correlation.0);
                                StreamState::Done
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Call(error)),
                                );
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                let error = ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                );
                                let _ = port
                                    .abort(correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
                                StreamState::Refused(error)
                            }
                            Err(error) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Read(error)),
                                );
                            }
                        };
                        let port = match this.port.take() {
                            Some(port) => port,
                            None => panic!("`LeftCall` polled after completion"),
                        };
                        return ::core::task::Poll::Ready(
                            Ok(LeftStream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                            }),
                        );
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for LeftCall<'_, P> {
        fn drop(&mut self) {
            if let (Some(port), LeftPhase::Waiting(correlation)) = (
                &mut self.port,
                &self.phase,
            ) {
                port.forget(correlation.0);
            }
        }
    }
    ///The reply stream of query `left`, which `LeftCall` resolves to. `poll_next` is the pull API, the whole face on a target without `std`: it registers the stream's interest, pulls one chunk, and verifies and decodes it as one element; `next` awaits it. An element that fails its check refuses the stream with `Contract::InvalidValue`, and a chunk that is no well-formed element, or longer than any legal one, refuses it with `Transport::Corrupt`: the stream is aborted with that outcome, which the provider reads from its next push, and the refusal is the one error the stream yields; the call is kept until the stream is dropped. A stream the provider stopped yields its outcome, `Transport::Interrupted` for a stop on purpose, as its one error, and the call is forgotten at once, as it is after the end. After the end or an error every later `poll_next` is `Ready(None)`. Dropping the stream forgets the call, once, unless the end or the provider's failure already did; before the end that interrupts the stream (ridl §12).
    #[must_use = "dropping the stream before its end interrupts it"]
    pub struct LeftStream<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    >
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port: &'a mut P,
        correlation: LeftCorrelation,
        buffer: <super::Reading as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,
        state: StreamState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > LeftStream<'_, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Pulls the next element: `Ready(Some(Ok(element)))` with the
        /// next element, `Ready(None)` once the stream ended,
        /// `Ready(Some(Err(error)))` once and then `Ready(None)` when
        /// the stream stopped or was refused, and `Pending` while no
        /// chunk is waiting, after the waker of `cx` was registered
        /// under `Interest::Stream`. A refused stream keeps its call
        /// until this value is dropped.
        pub fn poll_next(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<
            ::core::option::Option<
                ::core::result::Result<super::Reading, ::ridl_rt::error::ClientError>,
            >,
        > {
            let len = match self.state {
                StreamState::Done | StreamState::Stopped => {
                    return ::core::task::Poll::Ready(None);
                }
                StreamState::Refused(error) => {
                    self.state = StreamState::Stopped;
                    return ::core::task::Poll::Ready(
                        Some(Err(::ridl_rt::error::ClientError::Call(error))),
                    );
                }
                StreamState::First(len) => {
                    self.state = StreamState::Open;
                    len
                }
                StreamState::Open => {
                    self.port
                        .wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                self.correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                    match self
                        .port
                        .pull(
                            self.correlation.0,
                            ::ridl_rt::port::REPLY_STREAM,
                            ::core::convert::AsMut::as_mut(&mut self.buffer),
                        )
                    {
                        Ok(::ridl_rt::port::Pull::Chunk(len)) => len,
                        Ok(::ridl_rt::port::Pull::Pending) => {
                            return ::core::task::Poll::Pending;
                        }
                        Ok(::ridl_rt::port::Pull::End) => {
                            self.finish();
                            return ::core::task::Poll::Ready(None);
                        }
                        Ok(::ridl_rt::port::Pull::Failed(error)) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Call(error))),
                            );
                        }
                        Err(::ridl_rt::port::ReadError::Short { .. }) => {
                            return ::core::task::Poll::Ready(
                                Some(
                                    Err(
                                        self
                                            .refuse(
                                                ::ridl_rt::error::CallError::Transport(
                                                    ::ridl_rt::error::Transport::Corrupt,
                                                ),
                                            ),
                                    ),
                                ),
                            );
                        }
                        Err(error) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Read(error))),
                            );
                        }
                    }
                }
            };
            let buf: &[::core::primitive::u8] = ::core::convert::AsRef::as_ref(
                &self.buffer,
            );
            let element = match ::ridl_rt::payload::Ref::<
                super::Reading,
                P::Encoding,
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
            };
            ::core::task::Poll::Ready(
                Some(
                    match element {
                        Ok(element) => Ok(element),
                        Err(error) => Err(self.refuse(error)),
                    },
                ),
            )
        }
        /// Awaits the next element: `poll_next` as a future.
        pub async fn next(
            &mut self,
        ) -> ::core::option::Option<
            ::core::result::Result<super::Reading, ::ridl_rt::error::ClientError>,
        > {
            ::core::future::poll_fn(|cx| self.poll_next(cx)).await
        }
        fn finish(&mut self) {
            self.state = StreamState::Done;
            self.port.forget(self.correlation.0);
        }
        fn refuse(
            &mut self,
            error: ::ridl_rt::error::CallError,
        ) -> ::ridl_rt::error::ClientError {
            let _ = self
                .port
                .abort(self.correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
            self.state = StreamState::Stopped;
            ::ridl_rt::error::ClientError::Call(error)
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for LeftStream<'_, P>
    where
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn drop(&mut self) {
            if self.state != StreamState::Done {
                self.port.forget(self.correlation.0);
            }
        }
    }
    ///The future of query `right`, returned by `Client::right`. It resolves to `RightStream`, the reply stream, once its first element or its end arrived; to `ClientError::Send` when the call was not sent, including `SendError::Busy` when no slot was free within the call's bound; to `Transport::Timeout` when the call was sent and its bound passed before the first element or the end; and to the outcome the provider settled when it failed the call before any element. A first chunk that is no legal element resolves it to the stream, which yields the refusal, `Transport::Corrupt`, as its one error. Each poll registers its interest, reads the port once, and returns. Dropping the future while it waits calls `Caller::forget` on the call, which interrupts the stream; a future that resolved to an error has already done so, and one that resolved to the stream handed the call to it. Polling it again after it resolved panics.
    #[must_use = "the query was sent, or waits for a slot; its stream is taken only when the future is polled"]
    pub struct RightCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::option::Option<&'a mut P>,
        phase: RightPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum RightPhase {
        Unsent(super::Level),
        Waiting(RightCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > RightCall<'_, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            match (&self.port, self.deadline) {
                (Some(port), Some(deadline)) => port.now() > deadline,
                _ => false,
            }
        }
        fn port(&mut self) -> &mut P {
            match self.port.as_deref_mut() {
                Some(port) => port,
                None => panic!("`RightCall` polled after completion"),
            }
        }
    }
    impl<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for RightCall<'a, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            RightStream<'a, P>,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, RightPhase::Done) {
                    RightPhase::Done => panic!("`RightCall` polled after completion"),
                    RightPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    RightPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        let port = this.port();
                        port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_right(port, &__arg) {
                            Ok(correlation) => {
                                this.phase = RightPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = RightPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    RightPhase::Waiting(correlation) => {
                        let expired = this.expired();
                        let port = this.port();
                        port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
                            P::Encoding,
                        >>::BUFFER;
                        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
                            &mut __buf,
                        );
                        let state = match port
                            .pull(correlation.0, ::ridl_rt::port::REPLY_STREAM, buf)
                        {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                                StreamState::First(len)
                            }
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                if !expired {
                                    this.phase = RightPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ),
                                );
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                port.forget(correlation.0);
                                StreamState::Done
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Call(error)),
                                );
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                let error = ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                );
                                let _ = port
                                    .abort(correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
                                StreamState::Refused(error)
                            }
                            Err(error) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Read(error)),
                                );
                            }
                        };
                        let port = match this.port.take() {
                            Some(port) => port,
                            None => panic!("`RightCall` polled after completion"),
                        };
                        return ::core::task::Poll::Ready(
                            Ok(RightStream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                            }),
                        );
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for RightCall<'_, P> {
        fn drop(&mut self) {
            if let (Some(port), RightPhase::Waiting(correlation)) = (
                &mut self.port,
                &self.phase,
            ) {
                port.forget(correlation.0);
            }
        }
    }
    ///The reply stream of query `right`, which `RightCall` resolves to. `poll_next` is the pull API, the whole face on a target without `std`: it registers the stream's interest, pulls one chunk, and verifies and decodes it as one element; `next` awaits it. An element that fails its check refuses the stream with `Contract::InvalidValue`, and a chunk that is no well-formed element, or longer than any legal one, refuses it with `Transport::Corrupt`: the stream is aborted with that outcome, which the provider reads from its next push, and the refusal is the one error the stream yields; the call is kept until the stream is dropped. A stream the provider stopped yields its outcome, `Transport::Interrupted` for a stop on purpose, as its one error, and the call is forgotten at once, as it is after the end. After the end or an error every later `poll_next` is `Ready(None)`. Dropping the stream forgets the call, once, unless the end or the provider's failure already did; before the end that interrupts the stream (ridl §12).
    #[must_use = "dropping the stream before its end interrupts it"]
    pub struct RightStream<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    >
    where
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port: &'a mut P,
        correlation: RightCorrelation,
        buffer: <super::Level as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,
        state: StreamState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > RightStream<'_, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Pulls the next element: `Ready(Some(Ok(element)))` with the
        /// next element, `Ready(None)` once the stream ended,
        /// `Ready(Some(Err(error)))` once and then `Ready(None)` when
        /// the stream stopped or was refused, and `Pending` while no
        /// chunk is waiting, after the waker of `cx` was registered
        /// under `Interest::Stream`. A refused stream keeps its call
        /// until this value is dropped.
        pub fn poll_next(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<
            ::core::option::Option<
                ::core::result::Result<super::Level, ::ridl_rt::error::ClientError>,
            >,
        > {
            let len = match self.state {
                StreamState::Done | StreamState::Stopped => {
                    return ::core::task::Poll::Ready(None);
                }
                StreamState::Refused(error) => {
                    self.state = StreamState::Stopped;
                    return ::core::task::Poll::Ready(
                        Some(Err(::ridl_rt::error::ClientError::Call(error))),
                    );
                }
                StreamState::First(len) => {
                    self.state = StreamState::Open;
                    len
                }
                StreamState::Open => {
                    self.port
                        .wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                self.correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                    match self
                        .port
                        .pull(
                            self.correlation.0,
                            ::ridl_rt::port::REPLY_STREAM,
                            ::core::convert::AsMut::as_mut(&mut self.buffer),
                        )
                    {
                        Ok(::ridl_rt::port::Pull::Chunk(len)) => len,
                        Ok(::ridl_rt::port::Pull::Pending) => {
                            return ::core::task::Poll::Pending;
                        }
                        Ok(::ridl_rt::port::Pull::End) => {
                            self.finish();
                            return ::core::task::Poll::Ready(None);
                        }
                        Ok(::ridl_rt::port::Pull::Failed(error)) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Call(error))),
                            );
                        }
                        Err(::ridl_rt::port::ReadError::Short { .. }) => {
                            return ::core::task::Poll::Ready(
                                Some(
                                    Err(
                                        self
                                            .refuse(
                                                ::ridl_rt::error::CallError::Transport(
                                                    ::ridl_rt::error::Transport::Corrupt,
                                                ),
                                            ),
                                    ),
                                ),
                            );
                        }
                        Err(error) => {
                            self.finish();
                            return ::core::task::Poll::Ready(
                                Some(Err(::ridl_rt::error::ClientError::Read(error))),
                            );
                        }
                    }
                }
            };
            let buf: &[::core::primitive::u8] = ::core::convert::AsRef::as_ref(
                &self.buffer,
            );
            let element = match ::ridl_rt::payload::Ref::<
                super::Level,
                P::Encoding,
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
            };
            ::core::task::Poll::Ready(
                Some(
                    match element {
                        Ok(element) => Ok(element),
                        Err(error) => Err(self.refuse(error)),
                    },
                ),
            )
        }
        /// Awaits the next element: `poll_next` as a future.
        pub async fn next(
            &mut self,
        ) -> ::core::option::Option<
            ::core::result::Result<super::Level, ::ridl_rt::error::ClientError>,
        > {
            ::core::future::poll_fn(|cx| self.poll_next(cx)).await
        }
        fn finish(&mut self) {
            self.state = StreamState::Done;
            self.port.forget(self.correlation.0);
        }
        fn refuse(
            &mut self,
            error: ::ridl_rt::error::CallError,
        ) -> ::ridl_rt::error::ClientError {
            let _ = self
                .port
                .abort(self.correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
            self.state = StreamState::Stopped;
            ::ridl_rt::error::ClientError::Call(error)
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for RightStream<'_, P>
    where
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn drop(&mut self) {
            if self.state != StreamState::Done {
                self.port.forget(self.correlation.0);
            }
        }
    }
    pub(crate) fn send_left<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        limit: &super::Level,
    ) -> ::core::result::Result<LeftCorrelation, ::ridl_rt::port::SendError>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = limit;
        <super::TwinLeft as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
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
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .query(
                <super::Twin as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(LeftCorrelation)
    }
    pub(crate) fn send_right<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        limit: &super::Level,
    ) -> ::core::result::Result<RightCorrelation, ::ridl_rt::port::SendError>
    where
        super::Twin: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = limit;
        <super::TwinRight as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Level as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Level,
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
                    "encoding `Level` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Level` failed"),
        };
        __port
            .query(
                <super::Twin as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(2u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(RightCorrelation)
    }
    /// What an application implements to serve interface `Twin`'s calls.
    ///
    /// An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses.
    pub trait Provider {
        ///The source of query `left`'s reply stream, which `serve` polls for each `Reading` it sends.
        type Left: ::ridl_rt::face::Source<Item = super::Reading>;
        /// The readings up to `limit`, oldest first.
        ///
        ///Serves query `left`, whose reply is a stream of `Reading`: it returns the source `serve` drives. `serve` polls the source at most once per poll of its own, and only while the stream's window has room; it checks each element against its type before it is sent and ends the stream with `Contract::InvalidValue` on the first one that fails, sends `Ready(None)` as the end of the stream, and an `Interrupted` as `Transport::Interrupted`. The source is dropped once the stream ended, failed, or the caller stopped reading it. `ensure` clauses are not evaluated on a streamed reply.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        fn left(&mut self, limit: &super::Level) -> Self::Left;
        ///The source of query `right`'s reply stream, which `serve` polls for each `Level` it sends.
        type Right: ::ridl_rt::face::Source<Item = super::Level>;
        /// The levels up to `limit`, oldest first.
        ///
        ///Serves query `right`, whose reply is a stream of `Level`: it returns the source `serve` drives. `serve` polls the source at most once per poll of its own, and only while the stream's window has room; it checks each element against its type before it is sent and ends the stream with `Contract::InvalidValue` on the first one that fails, sends `Ready(None)` as the end of the stream, and an `Interrupted` as `Transport::Interrupted`. The source is dropped once the stream ended, failed, or the caller stopped reading it. `ensure` clauses are not evaluated on a streamed reply.
        ///
        /// # Rules
        ///
        /// - response within 200000 us
        ///
        /// # Parameters
        ///
        /// - `limit`
        ///   - range 0 ..= 100
        fn right(&mut self, limit: &super::Level) -> Self::Right;
    }
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
        streams: &mut Streams<H, P>,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler,
        P: Provider,
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if buf.len()
            < <super::Twin as ::ridl_rt::contract::Sizes<H::Encoding>>::MAX_BUFFER_SIZE
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
                        != <super::Twin as ::ridl_rt::contract::Interface>::NUMBER
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
                                        match <super::TwinLeft as ::ridl_rt::contract::Query>::require(
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
                                                if streams.left.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    streams.left = Some(StreamSlot {
                                                        claim: claim.id,
                                                        source: p.left(&__arg),
                                                        buffer: <super::Reading as ::ridl_rt::payload::Payload<
                                                            H::Encoding,
                                                        >>::BUFFER,
                                                        held: None,
                                                    });
                                                    streams.__started = true;
                                                    Ok(())
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(2u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Level,
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
                                        match <super::TwinRight as ::ridl_rt::contract::Query>::require(
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
                                                if streams.right.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    streams.right = Some(StreamSlot {
                                                        claim: claim.id,
                                                        source: p.right(&__arg),
                                                        buffer: <super::Level as ::ridl_rt::payload::Payload<
                                                            H::Encoding,
                                                        >>::BUFFER,
                                                        held: None,
                                                    });
                                                    streams.__started = true;
                                                    Ok(())
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
    type ElementBuffer<H, T> = <T as ::ridl_rt::payload::Payload<
        <H as ::ridl_rt::port::Encoded>::Encoding,
    >>::Buffer;
    struct StreamSlot<S, B> {
        claim: ::ridl_rt::port::ClaimId,
        source: S,
        buffer: B,
        held: ::core::option::Option<(usize, usize)>,
    }
    pub(crate) struct Streams<H, P>
    where
        H: ::ridl_rt::port::Encoded,
        P: Provider,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        __started: bool,
        __port: ::core::marker::PhantomData<H>,
        left: ::core::option::Option<
            StreamSlot<P::Left, ElementBuffer<H, super::Reading>>,
        >,
        right: ::core::option::Option<
            StreamSlot<P::Right, ElementBuffer<H, super::Level>>,
        >,
    }
    fn drive_stream<H, T, S>(
        h: &mut H,
        slot: &mut ::core::option::Option<StreamSlot<S, ElementBuffer<H, T>>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        T: ::ridl_rt::payload::Payload<H::Encoding>,
        S: ::ridl_rt::face::Source<Item = T>,
    {
        let Some(stream) = slot.as_mut() else {
            return false;
        };
        let claim = stream.claim;
        let interest = ::ridl_rt::port::Interest::ClaimStream(
            claim,
            ::ridl_rt::port::REPLY_STREAM,
        );
        if let Some((start, end)) = stream.held {
            h.wake_on(interest, cx.waker());
            let bytes = &::core::convert::AsRef::<
                [u8],
            >::as_ref(&stream.buffer)[start..end];
            match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
                Ok(taken) if taken == bytes.len() => stream.held = None,
                Ok(_) => return false,
                Err(error) => {
                    stop_stream(h, claim, error);
                    *slot = None;
                    return false;
                }
            }
        }
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let element = match stream.source.poll_next(cx) {
            ::core::task::Poll::Pending => return false,
            ::core::task::Poll::Ready(None) => {
                let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
                *slot = None;
                return false;
            }
            ::core::task::Poll::Ready(Some(Err(::ridl_rt::face::Interrupted))) => {
                let _ = h
                    .abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                *slot = None;
                return false;
            }
            ::core::task::Poll::Ready(Some(Ok(element))) => element,
        };
        let buf: &mut [u8] = ::core::convert::AsMut::as_mut(&mut stream.buffer);
        let base = buf.as_ptr() as usize;
        let checked = match ::ridl_rt::payload::Ref::<
            T,
            H::Encoding,
        >::encode(&element, buf) {
            Ok(encoded) => {
                let bytes = encoded.bytes();
                match ::ridl_rt::payload::Ref::<T, H::Encoding>::verify(bytes) {
                    Ok(_) => {
                        let start = bytes.as_ptr() as usize - base;
                        Ok((start, start + bytes.len()))
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
                }
            }
            Err(::ridl_rt::payload::EncodeError::Contract(violation)) => {
                Err(
                    ::ridl_rt::error::CallError::Contract(
                        ::ridl_rt::error::Contract::InvalidValue(violation),
                    ),
                )
            }
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(
                    "encoding a stream element needs {} bytes and its buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding a stream element failed"),
        };
        let (start, end) = match checked {
            Ok(range) => range,
            Err(error) => {
                let _ = h.abort_claim(claim, ::ridl_rt::port::REPLY_STREAM, error);
                *slot = None;
                return false;
            }
        };
        h.wake_on(interest, cx.waker());
        let bytes = &::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[start..end];
        match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
            Ok(taken) if taken == bytes.len() => true,
            Ok(_) => {
                stream.held = Some((start, end));
                false
            }
            Err(error) => {
                stop_stream(h, claim, error);
                *slot = None;
                false
            }
        }
    }
    fn stop_stream<H: ::ridl_rt::port::StreamHandler>(
        h: &mut H,
        claim: ::ridl_rt::port::ClaimId,
        error: ::ridl_rt::port::PushError,
    ) {
        match error {
            ::ridl_rt::port::PushError::Failed(_) => {
                let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
            }
            ::ridl_rt::port::PushError::TooLarge { .. } => {
                let _ = h
                    .abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            _ => {}
        }
    }
    /// Serves interface `Twin`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Twin as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Twin` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
    ///
    /// # Streamed replies
    ///
    /// A query whose reply is a stream is served from the source its `Provider` method returns, once the arguments and the `require` clauses pass. Each poll of the future first drives every stream it holds, then takes claims: a stream whose last element found the window full pushes that element again, after it registered its interest in the stream, and polls its source only once the element was taken; otherwise the source is polled once, which counts as one of the 32 claims of the poll. An element is encoded into a buffer inline in the future and checked against its type before it is pushed; one that fails stops the stream with `Contract::InvalidValue` and is not sent. `Ready(None)` ends the stream, and the claim is settled `Ok`; `Ready(Some(Err(Interrupted)))` stops it with `Transport::Interrupted`; a push above the member's limit (`PushError::TooLarge`) stops it with `Transport::Interrupted`; a push the caller refused or stopped reading (`PushError::Failed`) ends the claim. The source is dropped once its stream ended or stopped. The future drives one stream per member at a time: a call of a member whose stream is still running is settled `Transport::Busy`. The handler port gives the future no signal when a caller forgets the call, by its bound passing or by dropping its stream: the future reads the forget from its next push. So while a member's source answers `Pending`, a caller that stopped keeps the member busy until the source yields its next element, whose push fails, and only then is the source dropped and the member free for a new call. Dropping the future stops every stream it still drives with `Transport::Interrupted` and drops its source.
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Twin as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(1u32), ::ridl_rt::contract::Ordinal(2u32)],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Twin as ::ridl_rt::contract::Sizes<H::Encoding>>::CLAIM_BUFFER,
            state,
            streams: Streams {
                __started: false,
                __port: ::core::marker::PhantomData,
                left: None,
                right: None,
            },
        }
    }
    ///The future `serve` returns over interface `Twin`. It holds the handler, the provider, and the claim buffer of `<Twin as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the encoding the handler port carries. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    >
    where
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        handler: H,
        provider: &'a mut P,
        buf: <super::Twin as ::ridl_rt::contract::Sizes<H::Encoding>>::ClaimBuffer,
        state: ServeState,
        streams: Streams<H, P>,
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
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {}
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
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
                                <super::Twin as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    let mut again = false;
                    again
                        |= drive_stream::<
                            H,
                            super::Reading,
                            P::Left,
                        >(&mut this.handler, &mut this.streams.left, cx, &mut budget);
                    again
                        |= drive_stream::<
                            H,
                            super::Level,
                            P::Right,
                        >(&mut this.handler, &mut this.streams.right, cx, &mut budget);
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        ::core::convert::AsMut::as_mut(&mut this.buf),
                        &mut budget,
                        &mut this.streams,
                    ) {
                        Ok(_) => {
                            if ::core::mem::take(&mut this.streams.__started)
                                || budget == 0 || again
                            {
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
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::ops::Drop for Serve<'_, H, P>
    where
        super::Twin: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        fn drop(&mut self) {
            if let Some(stream) = self.streams.left.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            if let Some(stream) = self.streams.right.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
        }
    }
    ///The traits a consumer of interface `Twin`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
    }
}
/// Documents read as bytes or text.
///
///The generated interaction face of interface `Docs`.
#[allow(clippy::module_inception)]
pub mod docs {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct PutCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct NoteCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct FetchCorrelation(pub ::ridl_rt::port::Correlation);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct TextCorrelation(pub ::ridl_rt::port::Correlation);
    #[track_caller]
    fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
        let expected = <super::Docs as ::ridl_rt::contract::Interface>::CATALOG;
        if *found != *expected {
            ::core::panic!(
                "the face of interface `Docs` was generated from catalog {:?}, but the port is attached to catalog {:?}",
                expected, found
            );
        }
    }
    ///The consumer face of interface `Docs`, generic over exactly the ports the interface's interactions need. Its member methods are inherent; `new` is `ridl_rt::face::Bind`'s, in scope through `prelude`.
    pub struct Client<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::cell::RefCell<P>,
        upload: UploadState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// Stores bytes.
        ///
        ///Sends the command and returns its acknowledgment future and parameter sink. The values share this client's port until both are dropped. The acknowledgment reports delivery, independently of later stream refusal. Dropping an unfinished sink interrupts the stream. An empty sink is locally ready; peer refusal is read from the next push or end.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `info`
        /// - `data`
        pub fn put(&mut self, info: super::Info) -> (PutCall<'_, P>, PutSink<'_, P>) {
            let __arg = info;
            self.upload = UploadState::new();
            let deadline = <super::DocsPut as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.borrow().now().0.saturating_add(max.0),
                ));
            let mut pending = None;
            match send_put(&mut *self.port.borrow_mut(), &__arg) {
                Ok(c) => self.upload.correlation.set(Some(c.0)),
                Err(::ridl_rt::port::SendError::Busy) => pending = Some(__arg),
                Err(e) => {
                    self.upload.error.set(Some(::ridl_rt::error::ClientError::Send(e)));
                    self.upload.call_error.set(self.upload.error.get());
                }
            }
            (
                PutCall {
                    port: &self.port,
                    state: &self.upload,
                    pending,
                    deadline,
                    done: false,
                },
                PutSink {
                    port: &self.port,
                    state: &self.upload,
                },
            )
        }
        /// Stores text.
        ///
        ///Sends the command and returns its acknowledgment future and parameter sink. The values share this client's port until both are dropped. The acknowledgment reports delivery, independently of later stream refusal. Dropping an unfinished sink interrupts the stream. An empty sink is locally ready; peer refusal is read from the next push or end.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `data`
        pub fn note(
            &mut self,
            args: super::DocsNoteParams,
        ) -> (NoteCall<'_, P>, NoteSink<'_, P>) {
            let __arg = args;
            self.upload = UploadState::new();
            let deadline = <super::DocsNote as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.borrow().now().0.saturating_add(max.0),
                ));
            let mut pending = None;
            match send_note(&mut *self.port.borrow_mut(), &__arg) {
                Ok(c) => self.upload.correlation.set(Some(c.0)),
                Err(::ridl_rt::port::SendError::Busy) => pending = Some(__arg),
                Err(e) => {
                    self.upload.error.set(Some(::ridl_rt::error::ClientError::Send(e)));
                    self.upload.call_error.set(self.upload.error.get());
                }
            }
            (
                NoteCall {
                    port: &self.port,
                    state: &self.upload,
                    pending,
                    deadline,
                    done: false,
                },
                NoteSink {
                    port: &self.port,
                    state: &self.upload,
                },
            )
        }
        /// Fetches the document bytes.
        ///
        /// Sends query `fetch` and returns its future, which resolves to the reply stream, `FetchStream`, once the first element or the end of the stream arrived, and to `Transport::Timeout` when the bound passes before either. The call is sent when this method runs, not when the future is first polled. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `id`
        ///   - range 0 ..= 100
        pub fn fetch(&mut self, id: super::Id) -> FetchCall<'_, P> {
            let __arg = id;
            let deadline = <super::DocsFetch as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.get_mut().now().0.saturating_add(max.0),
                ));
            let phase = match send_fetch(self.port.get_mut(), &__arg) {
                Ok(correlation) => FetchPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => FetchPhase::Unsent(__arg),
                Err(error) => FetchPhase::Failed(error),
            };
            FetchCall {
                port: Some(self.port.get_mut()),
                phase,
                deadline,
            }
        }
        /// Fetches the document text.
        ///
        /// Sends query `text` and returns its future, which resolves to the reply stream, `TextStream`, once the first element or the end of the stream arrived, and to `Transport::Timeout` when the bound passes before either. The call is sent when this method runs, not when the future is first polled. A `require` clause that fails, or a send failure other than `SendError::Busy`, is a future that is ready with `ClientError::Send` and sends nothing. `SendError::Busy` is a future that waits for a free slot and sends on a later poll.
        ///
        /// The call's bound is the member's `max`, measured from the port's clock when this method runs; a member with no `max` waits without a bound. The future holds this client's port until it is dropped.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `id`
        ///   - range 0 ..= 100
        pub fn text(&mut self, id: super::Id) -> TextCall<'_, P> {
            let __arg = id;
            let deadline = <super::DocsText as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.get_mut().now().0.saturating_add(max.0),
                ));
            let phase = match send_text(self.port.get_mut(), &__arg) {
                Ok(correlation) => TextPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => TextPhase::Unsent(__arg),
                Err(error) => TextPhase::Failed(error),
            };
            TextCall {
                port: Some(self.port.get_mut()),
                phase,
                deadline,
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::ridl_rt::face::Bind for Client<P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Port = P;
        /// Binds the face to a port. The port is held by value: pass a handle, or a `&mut` borrow of one.
        ///
        /// # Panics
        ///
        /// Panics when `port` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before the port is stored. A program that must not panic makes the same comparison first, `port.catalog() == <Docs as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Docs` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        fn new(port: P) -> Self {
            check_catalog(::ridl_rt::port::Attached::catalog(&port));
            Client {
                port: ::core::cell::RefCell::new(port),
                upload: UploadState::new(),
            }
        }
    }
    struct UploadState {
        correlation: ::core::cell::Cell<Option<::ridl_rt::port::Correlation>>,
        ack: ::core::cell::Cell<bool>,
        call_error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
        ended: ::core::cell::Cell<bool>,
        forgotten: ::core::cell::Cell<bool>,
        error: ::core::cell::Cell<Option<::ridl_rt::error::ClientError>>,
    }
    impl UploadState {
        fn new() -> Self {
            Self {
                call_error: ::core::cell::Cell::new(None),
                correlation: ::core::cell::Cell::new(None),
                ack: ::core::cell::Cell::new(false),
                ended: ::core::cell::Cell::new(false),
                forgotten: ::core::cell::Cell::new(false),
                error: ::core::cell::Cell::new(None),
            }
        }
        fn forget<P: ::ridl_rt::port::Caller>(&self, port: &::core::cell::RefCell<P>) {
            match self.correlation.get() {
                Some(c) if !self.forgotten.replace(true) => port.borrow_mut().forget(c),
                _ => {}
            }
        }
    }
    fn upload_push_error(
        error: ::ridl_rt::port::PushError,
    ) -> ::ridl_rt::error::ClientError {
        ::ridl_rt::error::ClientError::Call(
            match error {
                ::ridl_rt::port::PushError::Failed(e) => e,
                _ => {
                    ::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Interrupted,
                    )
                }
            },
        )
    }
    ///The delivery acknowledgment of a command with a parameter stream. Dropping it before acknowledgment forgets the call. Completion leaves an open sink usable.
    #[must_use]
    pub struct PutCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
        pending: Option<super::Info>,
        deadline: Option<::ridl_rt::sample::Timestamp>,
        done: bool,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::marker::Unpin for PutCall<'_, P> {}
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for PutCall<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            assert!(! this.done, "call polled after completion");
            if this.pending.is_some()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                let e = ::ridl_rt::error::ClientError::Send(
                    ::ridl_rt::port::SendError::Busy,
                );
                this.done = true;
                this.state.call_error.set(Some(e));
                this.state.error.set(Some(e));
                return ::core::task::Poll::Ready(Err(e));
            }
            if let Some(arg) = this.pending.take() {
                this.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                match send_put(&mut *this.port.borrow_mut(), &arg) {
                    Ok(c) => this.state.correlation.set(Some(c.0)),
                    Err(::ridl_rt::port::SendError::Busy) => this.pending = Some(arg),
                    Err(e) => {
                        this.state
                            .call_error
                            .set(Some(::ridl_rt::error::ClientError::Send(e)));
                        this.state.error.set(this.state.call_error.get());
                    }
                }
            }
            let mut result = this.state.call_error.get().map(Err);
            if let (true, Some(c)) = (result.is_none(), this.state.correlation.get()) {
                this.port
                    .borrow()
                    .wake_on(::ridl_rt::port::Interest::Outcome(c), cx.waker());
                result = poll_put_ack(&mut *this.port.borrow_mut(), PutCorrelation(c))
                    .map(|v| v.map_err(::ridl_rt::error::ClientError::Call));
            }
            if result.is_none()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                result = Some(
                    Err(
                        if this.pending.is_some() {
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            )
                        } else {
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Undelivered,
                                ),
                            )
                        },
                    ),
                );
            }
            if let Some(result) = result {
                this.done = true;
                this.state.ack.set(true);
                if result.is_err() {
                    this.state.error.set(result.err());
                    this.state.forget(this.port);
                } else if this.state.ended.get() {
                    this.state.forget(this.port);
                }
                ::core::task::Poll::Ready(result)
            } else {
                ::core::task::Poll::Pending
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for PutCall<'_, P> {
        fn drop(&mut self) {
            if !self.done {
                self.state
                    .error
                    .set(
                        Some(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Interrupted,
                                ),
                            ),
                        ),
                    );
                self.state.forget(self.port);
            }
        }
    }
    ///The command's parameter sink. It shares the call's port without allocating. Empty writes are local no-ops; peer refusal is learned from a push or end. Dropping it before end interrupts the upload.
    pub struct PutSink<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > PutSink<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        ///The maximum chunk length in bytes.
        pub const LIMIT: usize = 16128;
        ///Offers a chunk up to LIMIT. Empty input returns zero without a port call. Pending input registers the stream wake interest before trying the push.
        pub fn poll_write(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
            input: &[u8],
        ) -> ::core::task::Poll<Result<usize, ::ridl_rt::error::ClientError>> {
            if input.is_empty() {
                return ::core::task::Poll::Ready(Ok(0));
            }
            if let Some(e) = self.state.error.get() {
                return ::core::task::Poll::Ready(Err(e));
            }
            if self.state.ended.get() {
                return ::core::task::Poll::Ready(
                    Err(upload_push_error(::ridl_rt::port::PushError::Ended)),
                );
            }
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            let n = input.len().min(Self::LIMIT);
            self.port
                .borrow()
                .wake_on(
                    ::ridl_rt::port::Interest::Stream(
                        c,
                        ::ridl_rt::contract::StreamNo(1),
                    ),
                    cx.waker(),
                );
            let result = self
                .port
                .borrow_mut()
                .push(c, ::ridl_rt::contract::StreamNo(1), &input[..n]);
            match result {
                Ok(0) => ::core::task::Poll::Pending,
                Ok(n) => ::core::task::Poll::Ready(Ok(n)),
                Err(e) => {
                    let error = upload_push_error(e);
                    self.state.error.set(Some(error));
                    ::core::task::Poll::Ready(Err(error))
                }
            }
        }
        ///Waits until a prefix of the input is accepted and returns its byte length.
        pub async fn write(
            &mut self,
            input: &[u8],
        ) -> Result<usize, ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_write(cx, input)).await
        }
        ///Flushes a held element. With no held element readiness describes local capacity and does not probe the peer.
        pub fn poll_ready(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if let Some(e) = self.state.error.get() {
                return ::core::task::Poll::Ready(Err(e));
            }
            let _ = cx;
            ::core::task::Poll::Ready(Ok(()))
        }
        ///Waits until the sink has no held element.
        pub async fn ready(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_ready(cx)).await
        }
        ///Flushes the held element and ends the parameter stream. The call retains ownership until acknowledgment.
        pub fn poll_end(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if self.state.ended.get() {
                return ::core::task::Poll::Ready(Ok(()));
            }
            match self.poll_ready(cx) {
                ::core::task::Poll::Ready(Ok(())) => {}
                other => return other,
            }
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            let result = self
                .port
                .borrow_mut()
                .end(c, ::ridl_rt::contract::StreamNo(1))
                .map_err(upload_push_error);
            if result.is_ok() {
                self.state.ended.set(true);
                if self.state.ack.get() {
                    self.state.forget(self.port);
                }
            } else {
                self.state.error.set(result.err());
            }
            ::core::task::Poll::Ready(result)
        }
        ///Ends the stream after flushing any held element.
        pub async fn end(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_end(cx)).await
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for PutSink<'_, P> {
        fn drop(&mut self) {
            if !self.state.ended.get() {
                if let (true, Some(c)) = (
                    self.state.error.get().is_none() && !self.state.forgotten.get(),
                    self.state.correlation.get(),
                ) {
                    let _ = self
                        .port
                        .borrow_mut()
                        .abort(
                            c,
                            ::ridl_rt::contract::StreamNo(1),
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Interrupted,
                            ),
                        );
                }
                self.state.forget(self.port);
            }
        }
    }
    ///The delivery acknowledgment of a command with a parameter stream. Dropping it before acknowledgment forgets the call. Completion leaves an open sink usable.
    #[must_use]
    pub struct NoteCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
        pending: Option<super::DocsNoteParams>,
        deadline: Option<::ridl_rt::sample::Timestamp>,
        done: bool,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::marker::Unpin for NoteCall<'_, P> {}
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for NoteCall<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = Result<(), ::ridl_rt::error::ClientError>;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            assert!(! this.done, "call polled after completion");
            if this.pending.is_some()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                let e = ::ridl_rt::error::ClientError::Send(
                    ::ridl_rt::port::SendError::Busy,
                );
                this.done = true;
                this.state.call_error.set(Some(e));
                this.state.error.set(Some(e));
                return ::core::task::Poll::Ready(Err(e));
            }
            if let Some(arg) = this.pending.take() {
                this.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                match send_note(&mut *this.port.borrow_mut(), &arg) {
                    Ok(c) => this.state.correlation.set(Some(c.0)),
                    Err(::ridl_rt::port::SendError::Busy) => this.pending = Some(arg),
                    Err(e) => {
                        this.state
                            .call_error
                            .set(Some(::ridl_rt::error::ClientError::Send(e)));
                        this.state.error.set(this.state.call_error.get());
                    }
                }
            }
            let mut result = this.state.call_error.get().map(Err);
            if let (true, Some(c)) = (result.is_none(), this.state.correlation.get()) {
                this.port
                    .borrow()
                    .wake_on(::ridl_rt::port::Interest::Outcome(c), cx.waker());
                result = poll_note_ack(&mut *this.port.borrow_mut(), NoteCorrelation(c))
                    .map(|v| v.map_err(::ridl_rt::error::ClientError::Call));
            }
            if result.is_none()
                && this
                    .deadline
                    .is_some_and(|deadline| this.port.borrow().now() > deadline)
            {
                result = Some(
                    Err(
                        if this.pending.is_some() {
                            ::ridl_rt::error::ClientError::Send(
                                ::ridl_rt::port::SendError::Busy,
                            )
                        } else {
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Undelivered,
                                ),
                            )
                        },
                    ),
                );
            }
            if let Some(result) = result {
                this.done = true;
                this.state.ack.set(true);
                if result.is_err() {
                    this.state.error.set(result.err());
                    this.state.forget(this.port);
                } else if this.state.ended.get() {
                    this.state.forget(this.port);
                }
                ::core::task::Poll::Ready(result)
            } else {
                ::core::task::Poll::Pending
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for NoteCall<'_, P> {
        fn drop(&mut self) {
            if !self.done {
                self.state
                    .error
                    .set(
                        Some(
                            ::ridl_rt::error::ClientError::Call(
                                ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Interrupted,
                                ),
                            ),
                        ),
                    );
                self.state.forget(self.port);
            }
        }
    }
    ///The command's parameter sink. It shares the call's port without allocating. Empty writes are local no-ops; peer refusal is learned from a push or end. Dropping it before end interrupts the upload.
    pub struct NoteSink<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > {
        port: &'a ::core::cell::RefCell<P>,
        state: &'a UploadState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > NoteSink<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        ///The maximum chunk length in bytes.
        pub const LIMIT: usize = 16128;
        ///Offers a chunk up to LIMIT. Empty input returns zero without a port call. Pending input registers the stream wake interest before trying the push.
        pub fn poll_write_str(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
            input: &str,
        ) -> ::core::task::Poll<Result<usize, ::ridl_rt::error::ClientError>> {
            if input.is_empty() {
                return ::core::task::Poll::Ready(Ok(0));
            }
            if let Some(e) = self.state.error.get() {
                return ::core::task::Poll::Ready(Err(e));
            }
            if self.state.ended.get() {
                return ::core::task::Poll::Ready(
                    Err(upload_push_error(::ridl_rt::port::PushError::Ended)),
                );
            }
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            let mut n = input.len().min(Self::LIMIT);
            while !input.is_char_boundary(n) {
                n -= 1;
            }
            self.port
                .borrow()
                .wake_on(
                    ::ridl_rt::port::Interest::Stream(
                        c,
                        ::ridl_rt::contract::StreamNo(1),
                    ),
                    cx.waker(),
                );
            let result = self
                .port
                .borrow_mut()
                .push(c, ::ridl_rt::contract::StreamNo(1), &input.as_bytes()[..n]);
            match result {
                Ok(0) => ::core::task::Poll::Pending,
                Ok(n) => ::core::task::Poll::Ready(Ok(n)),
                Err(e) => {
                    let error = upload_push_error(e);
                    self.state.error.set(Some(error));
                    ::core::task::Poll::Ready(Err(error))
                }
            }
        }
        ///Waits until a prefix of the input is accepted and returns its byte length.
        pub async fn write_str(
            &mut self,
            input: &str,
        ) -> Result<usize, ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_write_str(cx, input)).await
        }
        ///Flushes a held element. With no held element readiness describes local capacity and does not probe the peer.
        pub fn poll_ready(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if let Some(e) = self.state.error.get() {
                return ::core::task::Poll::Ready(Err(e));
            }
            let _ = cx;
            ::core::task::Poll::Ready(Ok(()))
        }
        ///Waits until the sink has no held element.
        pub async fn ready(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_ready(cx)).await
        }
        ///Flushes the held element and ends the parameter stream. The call retains ownership until acknowledgment.
        pub fn poll_end(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<(), ::ridl_rt::error::ClientError>> {
            if self.state.ended.get() {
                return ::core::task::Poll::Ready(Ok(()));
            }
            match self.poll_ready(cx) {
                ::core::task::Poll::Ready(Ok(())) => {}
                other => return other,
            }
            let Some(c) = self.state.correlation.get() else {
                self.port.borrow().wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                return ::core::task::Poll::Pending;
            };
            let result = self
                .port
                .borrow_mut()
                .end(c, ::ridl_rt::contract::StreamNo(1))
                .map_err(upload_push_error);
            if result.is_ok() {
                self.state.ended.set(true);
                if self.state.ack.get() {
                    self.state.forget(self.port);
                }
            } else {
                self.state.error.set(result.err());
            }
            ::core::task::Poll::Ready(result)
        }
        ///Ends the stream after flushing any held element.
        pub async fn end(&mut self) -> Result<(), ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_end(cx)).await
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for NoteSink<'_, P> {
        fn drop(&mut self) {
            if !self.state.ended.get() {
                if let (true, Some(c)) = (
                    self.state.error.get().is_none() && !self.state.forgotten.get(),
                    self.state.correlation.get(),
                ) {
                    let _ = self
                        .port
                        .borrow_mut()
                        .abort(
                            c,
                            ::ridl_rt::contract::StreamNo(1),
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Interrupted,
                            ),
                        );
                }
                self.state.forget(self.port);
            }
        }
    }
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StreamState {
        First(usize),
        Open,
        Refused(::ridl_rt::error::CallError),
        Stopped,
        Done,
    }
    ///The future of query `fetch`, resolving to `FetchStream` when its first chunk or end arrives. It yields a send error if admission fails, or Timeout if the bound passes before the stream starts. The first chunk is retained inline. Dropping a waiting future forgets the call and interrupts the stream. A chunk above LIMIT is refused as Corrupt, and the stream yields that error once; text chunks are checked for UTF-8 before the application sees them. Polling a completed future panics.
    #[must_use = "the query was sent, or waits for a slot; its stream is taken only when the future is polled"]
    pub struct FetchCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::option::Option<&'a mut P>,
        phase: FetchPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum FetchPhase {
        Unsent(super::Id),
        Waiting(FetchCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > FetchCall<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            match (&self.port, self.deadline) {
                (Some(port), Some(deadline)) => port.now() > deadline,
                _ => false,
            }
        }
        fn port(&mut self) -> &mut P {
            match self.port.as_deref_mut() {
                Some(port) => port,
                None => panic!("`FetchCall` polled after completion"),
            }
        }
    }
    impl<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for FetchCall<'a, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            FetchStream<'a, P>,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, FetchPhase::Done) {
                    FetchPhase::Done => panic!("`FetchCall` polled after completion"),
                    FetchPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    FetchPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        let port = this.port();
                        port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_fetch(port, &__arg) {
                            Ok(correlation) => {
                                this.phase = FetchPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = FetchPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    FetchPhase::Waiting(correlation) => {
                        let expired = this.expired();
                        let port = this.port();
                        port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        let mut __buf = [0u8; 16128];
                        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
                            &mut __buf,
                        );
                        let state = match port
                            .pull(correlation.0, ::ridl_rt::port::REPLY_STREAM, buf)
                        {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                                StreamState::First(len)
                            }
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                if !expired {
                                    this.phase = FetchPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ),
                                );
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                port.forget(correlation.0);
                                StreamState::Done
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Call(error)),
                                );
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                let error = ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                );
                                let _ = port
                                    .abort(correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
                                StreamState::Refused(error)
                            }
                            Err(error) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Read(error)),
                                );
                            }
                        };
                        let port = match this.port.take() {
                            Some(port) => port,
                            None => panic!("`FetchCall` polled after completion"),
                        };
                        return ::core::task::Poll::Ready(
                            Ok(FetchStream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                                offset: 0,
                                end: 0,
                            }),
                        );
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for FetchCall<'_, P> {
        fn drop(&mut self) {
            if let (Some(port), FetchPhase::Waiting(correlation)) = (
                &mut self.port,
                &self.phase,
            ) {
                port.forget(correlation.0);
            }
        }
    }
    /// A streamed reply with one inline staging buffer. Dropping it before
    /// the end forgets the call and interrupts the provider. A locally
    /// refused stream keeps its call until drop, so the next provider push
    /// can read the refusal. After end or a provider failure it is forgotten.
    #[must_use = "dropping a reply stream before its end interrupts it"]
    pub struct FetchStream<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        correlation: FetchCorrelation,
        buffer: [u8; 16128],
        state: StreamState,
        offset: usize,
        end: usize,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > FetchStream<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// The maximum number of bytes in one port chunk.
        pub const LIMIT: usize = 16128;
        fn poll_chunk(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<Option<usize>, ::ridl_rt::error::ClientError>> {
            match self.state {
                StreamState::Done | StreamState::Stopped => {
                    return ::core::task::Poll::Ready(Ok(None));
                }
                StreamState::Refused(error) => {
                    self.state = StreamState::Stopped;
                    return ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Call(error)),
                    );
                }
                StreamState::First(len) => {
                    self.state = StreamState::Open;
                    return ::core::task::Poll::Ready(Ok(Some(len)));
                }
                StreamState::Open => {}
            }
            self.port
                .wake_on(
                    ::ridl_rt::port::Interest::Stream(
                        self.correlation.0,
                        ::ridl_rt::port::REPLY_STREAM,
                    ),
                    cx.waker(),
                );
            match self
                .port
                .pull(
                    self.correlation.0,
                    ::ridl_rt::port::REPLY_STREAM,
                    &mut self.buffer,
                )
            {
                Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                    ::core::task::Poll::Ready(Ok(Some(len)))
                }
                Ok(::ridl_rt::port::Pull::Pending) => ::core::task::Poll::Pending,
                Ok(::ridl_rt::port::Pull::End) => {
                    self.finish();
                    ::core::task::Poll::Ready(Ok(None))
                }
                Ok(::ridl_rt::port::Pull::Failed(error)) => {
                    self.finish();
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Call(error)),
                    )
                }
                Err(::ridl_rt::port::ReadError::Short { .. }) => {
                    ::core::task::Poll::Ready(
                        Err(
                            self
                                .refuse(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                ),
                        ),
                    )
                }
                Err(error) => {
                    self.finish();
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Read(error)),
                    )
                }
            }
        }
        fn finish(&mut self) {
            self.state = StreamState::Done;
            self.port.forget(self.correlation.0);
        }
        fn refuse(
            &mut self,
            error: ::ridl_rt::error::CallError,
        ) -> ::ridl_rt::error::ClientError {
            let _ = self
                .port
                .abort(self.correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
            self.state = StreamState::Stopped;
            ::ridl_rt::error::ClientError::Call(error)
        }
        /// Reads bytes, retaining the rest of a chunk for a short buffer.
        /// An empty buffer returns zero without pulling. With a nonempty
        /// buffer, zero is the end. A failure is yielded once.
        pub fn poll_read(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
            buf: &mut [u8],
        ) -> ::core::task::Poll<Result<usize, ::ridl_rt::error::ClientError>> {
            if buf.is_empty() {
                return ::core::task::Poll::Ready(Ok(0));
            }
            loop {
                if self.offset < self.end {
                    let n = buf.len().min(self.end - self.offset);
                    buf[..n].copy_from_slice(&self.buffer[self.offset..self.offset + n]);
                    self.offset += n;
                    return ::core::task::Poll::Ready(Ok(n));
                }
                match self.poll_chunk(cx) {
                    ::core::task::Poll::Pending => return ::core::task::Poll::Pending,
                    ::core::task::Poll::Ready(Err(error)) => {
                        return ::core::task::Poll::Ready(Err(error));
                    }
                    ::core::task::Poll::Ready(Ok(None)) => {
                        return ::core::task::Poll::Ready(Ok(0));
                    }
                    ::core::task::Poll::Ready(Ok(Some(len))) => {
                        self.offset = 0;
                        self.end = len;
                    }
                }
            }
        }
        /// Awaits a read into the supplied buffer.
        pub async fn read(
            &mut self,
            buf: &mut [u8],
        ) -> Result<usize, ::ridl_rt::error::ClientError> {
            ::core::future::poll_fn(|cx| self.poll_read(cx, buf)).await
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for FetchStream<'_, P> {
        fn drop(&mut self) {
            if self.state != StreamState::Done {
                self.port.forget(self.correlation.0);
            }
        }
    }
    ///The future of query `text`, resolving to `TextStream` when its first chunk or end arrives. It yields a send error if admission fails, or Timeout if the bound passes before the stream starts. The first chunk is retained inline. Dropping a waiting future forgets the call and interrupts the stream. A chunk above LIMIT is refused as Corrupt, and the stream yields that error once; text chunks are checked for UTF-8 before the application sees them. Polling a completed future panics.
    #[must_use = "the query was sent, or waits for a slot; its stream is taken only when the future is polled"]
    pub struct TextCall<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: ::core::option::Option<&'a mut P>,
        phase: TextPhase,
        deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
    }
    enum TextPhase {
        Unsent(super::Id),
        Waiting(TextCorrelation),
        Failed(::ridl_rt::port::SendError),
        Done,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > TextCall<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        fn expired(&self) -> bool {
            match (&self.port, self.deadline) {
                (Some(port), Some(deadline)) => port.now() > deadline,
                _ => false,
            }
        }
        fn port(&mut self) -> &mut P {
            match self.port.as_deref_mut() {
                Some(port) => port,
                None => panic!("`TextCall` polled after completion"),
            }
        }
    }
    impl<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::future::Future for TextCall<'a, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        type Output = ::core::result::Result<
            TextStream<'a, P>,
            ::ridl_rt::error::ClientError,
        >;
        fn poll(
            self: ::core::pin::Pin<&mut Self>,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Self::Output> {
            let this = self.get_mut();
            loop {
                match ::core::mem::replace(&mut this.phase, TextPhase::Done) {
                    TextPhase::Done => panic!("`TextCall` polled after completion"),
                    TextPhase::Failed(error) => {
                        return ::core::task::Poll::Ready(
                            Err(::ridl_rt::error::ClientError::Send(error)),
                        );
                    }
                    TextPhase::Unsent(__arg) => {
                        if this.expired() {
                            return ::core::task::Poll::Ready(
                                Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ),
                            );
                        }
                        let port = this.port();
                        port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                        match send_text(port, &__arg) {
                            Ok(correlation) => {
                                this.phase = TextPhase::Waiting(correlation);
                            }
                            Err(::ridl_rt::port::SendError::Busy) => {
                                this.phase = TextPhase::Unsent(__arg);
                                return ::core::task::Poll::Pending;
                            }
                            Err(error) => {
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Send(error)),
                                );
                            }
                        }
                    }
                    TextPhase::Waiting(correlation) => {
                        let expired = this.expired();
                        let port = this.port();
                        port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        let mut __buf = [0u8; 16128];
                        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
                            &mut __buf,
                        );
                        let state = match port
                            .pull(correlation.0, ::ridl_rt::port::REPLY_STREAM, buf)
                        {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                                StreamState::First(len)
                            }
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                if !expired {
                                    this.phase = TextPhase::Waiting(correlation);
                                    return ::core::task::Poll::Pending;
                                }
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ),
                                );
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                port.forget(correlation.0);
                                StreamState::Done
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Call(error)),
                                );
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                let error = ::ridl_rt::error::CallError::Transport(
                                    ::ridl_rt::error::Transport::Corrupt,
                                );
                                let _ = port
                                    .abort(correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
                                StreamState::Refused(error)
                            }
                            Err(error) => {
                                port.forget(correlation.0);
                                return ::core::task::Poll::Ready(
                                    Err(::ridl_rt::error::ClientError::Read(error)),
                                );
                            }
                        };
                        let port = match this.port.take() {
                            Some(port) => port,
                            None => panic!("`TextCall` polled after completion"),
                        };
                        return ::core::task::Poll::Ready(
                            Ok(TextStream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                            }),
                        );
                    }
                }
            }
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for TextCall<'_, P> {
        fn drop(&mut self) {
            if let (Some(port), TextPhase::Waiting(correlation)) = (
                &mut self.port,
                &self.phase,
            ) {
                port.forget(correlation.0);
            }
        }
    }
    /// A streamed reply with one inline staging buffer. Dropping it before
    /// the end forgets the call and interrupts the provider. A locally
    /// refused stream keeps its call until drop, so the next provider push
    /// can read the refusal. After end or a provider failure it is forgotten.
    #[must_use = "dropping a reply stream before its end interrupts it"]
    pub struct TextStream<
        'a,
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > {
        port: &'a mut P,
        correlation: TextCorrelation,
        buffer: [u8; 16128],
        state: StreamState,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > TextStream<'_, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        /// The maximum number of bytes in one port chunk.
        pub const LIMIT: usize = 16128;
        fn poll_chunk(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Result<Option<usize>, ::ridl_rt::error::ClientError>> {
            match self.state {
                StreamState::Done | StreamState::Stopped => {
                    return ::core::task::Poll::Ready(Ok(None));
                }
                StreamState::Refused(error) => {
                    self.state = StreamState::Stopped;
                    return ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Call(error)),
                    );
                }
                StreamState::First(len) => {
                    self.state = StreamState::Open;
                    return ::core::task::Poll::Ready(Ok(Some(len)));
                }
                StreamState::Open => {}
            }
            self.port
                .wake_on(
                    ::ridl_rt::port::Interest::Stream(
                        self.correlation.0,
                        ::ridl_rt::port::REPLY_STREAM,
                    ),
                    cx.waker(),
                );
            match self
                .port
                .pull(
                    self.correlation.0,
                    ::ridl_rt::port::REPLY_STREAM,
                    &mut self.buffer,
                )
            {
                Ok(::ridl_rt::port::Pull::Chunk(len)) => {
                    ::core::task::Poll::Ready(Ok(Some(len)))
                }
                Ok(::ridl_rt::port::Pull::Pending) => ::core::task::Poll::Pending,
                Ok(::ridl_rt::port::Pull::End) => {
                    self.finish();
                    ::core::task::Poll::Ready(Ok(None))
                }
                Ok(::ridl_rt::port::Pull::Failed(error)) => {
                    self.finish();
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Call(error)),
                    )
                }
                Err(::ridl_rt::port::ReadError::Short { .. }) => {
                    ::core::task::Poll::Ready(
                        Err(
                            self
                                .refuse(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                ),
                        ),
                    )
                }
                Err(error) => {
                    self.finish();
                    ::core::task::Poll::Ready(
                        Err(::ridl_rt::error::ClientError::Read(error)),
                    )
                }
            }
        }
        fn finish(&mut self) {
            self.state = StreamState::Done;
            self.port.forget(self.correlation.0);
        }
        fn refuse(
            &mut self,
            error: ::ridl_rt::error::CallError,
        ) -> ::ridl_rt::error::ClientError {
            let _ = self
                .port
                .abort(self.correlation.0, ::ridl_rt::port::REPLY_STREAM, error);
            self.state = StreamState::Stopped;
            ::ridl_rt::error::ClientError::Call(error)
        }
        fn poll_text_len(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Option<Result<usize, ::ridl_rt::error::ClientError>>> {
            loop {
                match self.poll_chunk(cx) {
                    ::core::task::Poll::Pending => return ::core::task::Poll::Pending,
                    ::core::task::Poll::Ready(Ok(None)) => {
                        return ::core::task::Poll::Ready(None);
                    }
                    ::core::task::Poll::Ready(Err(error)) => {
                        return ::core::task::Poll::Ready(Some(Err(error)));
                    }
                    ::core::task::Poll::Ready(Ok(Some(0))) => continue,
                    ::core::task::Poll::Ready(Ok(Some(len))) => {
                        if ::core::str::from_utf8(&self.buffer[..len]).is_err() {
                            let error = ::ridl_rt::error::CallError::Contract(
                                ::ridl_rt::error::Contract::InvalidValue(::ridl_rt::payload::Violation {
                                    type_name: Some("string"),
                                    rule: ::ridl_rt::payload::Rule::Utf8,
                                }),
                            );
                            return ::core::task::Poll::Ready(
                                Some(Err(self.refuse(error))),
                            );
                        }
                        return ::core::task::Poll::Ready(Some(Ok(len)));
                    }
                }
            }
        }
        /// Pulls one complete UTF-8 chunk, borrowed until the next call.
        /// Empty chunks are skipped. Invalid UTF-8 aborts the stream
        /// and yields one error, then None.
        pub fn poll_next(
            &mut self,
            cx: &mut ::core::task::Context<'_>,
        ) -> ::core::task::Poll<Option<Result<&str, ::ridl_rt::error::ClientError>>> {
            self.poll_text_len(cx)
                .map(|item| {
                    item
                        .map(|result| {
                            result
                                .map(|len| {
                                    ::core::str::from_utf8(&self.buffer[..len])
                                        .expect("the text chunk was checked")
                                })
                        })
                })
        }
        /// Awaits the next complete text chunk.
        pub async fn next(
            &mut self,
        ) -> Option<Result<&str, ::ridl_rt::error::ClientError>> {
            let item = ::core::future::poll_fn(|cx| self.poll_text_len(cx)).await;
            item.map(|result| {
                result
                    .map(|len| {
                        ::core::str::from_utf8(&self.buffer[..len])
                            .expect("the text chunk was checked")
                    })
            })
        }
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > ::core::ops::Drop for TextStream<'_, P> {
        fn drop(&mut self) {
            if self.state != StreamState::Done {
                self.port.forget(self.correlation.0);
            }
        }
    }
    pub(crate) fn send_put<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        info: &super::Info,
    ) -> ::core::result::Result<PutCorrelation, ::ridl_rt::port::SendError>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = info;
        <super::DocsPut as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Info as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Info,
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
                    "encoding `Info` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Info` failed"),
        };
        __port
            .command(
                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(3u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(PutCorrelation)
    }
    pub(crate) fn poll_put_ack<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: PutCorrelation,
    ) -> ::core::option::Option<::core::result::Result<(), ::ridl_rt::error::CallError>>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port.ack(correlation.0)
    }
    pub(crate) fn send_note<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        args: &super::DocsNoteParams,
    ) -> ::core::result::Result<NoteCorrelation, ::ridl_rt::port::SendError>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = args;
        <super::DocsNote as ::ridl_rt::contract::Command>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::DocsNoteParams as ::ridl_rt::payload::Payload<
            P::Encoding,
        >>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::DocsNoteParams,
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
                    "encoding `DocsNoteParams` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `DocsNoteParams` failed"),
        };
        __port
            .command(
                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(4u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(NoteCorrelation)
    }
    pub(crate) fn poll_note_ack<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        port: &mut P,
        correlation: NoteCorrelation,
    ) -> ::core::option::Option<::core::result::Result<(), ::ridl_rt::error::CallError>>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        port.ack(correlation.0)
    }
    pub(crate) fn send_fetch<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        id: &super::Id,
    ) -> ::core::result::Result<FetchCorrelation, ::ridl_rt::port::SendError>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = id;
        <super::DocsFetch as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Id as ::ridl_rt::payload::Payload<P::Encoding>>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Id,
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
                    "encoding `Id` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Id` failed"),
        };
        __port
            .query(
                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(1u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(FetchCorrelation)
    }
    pub(crate) fn send_text<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>(
        __port: &mut P,
        id: &super::Id,
    ) -> ::core::result::Result<TextCorrelation, ::ridl_rt::port::SendError>
    where
        super::Docs: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<P::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<P::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<P::Encoding>,
    {
        let __arg = id;
        <super::DocsText as ::ridl_rt::contract::Query>::require(__arg)
            .map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
        let mut __buf = <super::Id as ::ridl_rt::payload::Payload<P::Encoding>>::BUFFER;
        let buf: &mut [::core::primitive::u8] = ::core::convert::AsMut::as_mut(
            &mut __buf,
        );
        let bytes = match ::ridl_rt::payload::Ref::<
            super::Id,
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
                    "encoding `Id` needs {} bytes and the argument buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                    needed, available
                )
            }
            Err(_) => unreachable!("encoding `Id` failed"),
        };
        __port
            .query(
                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                ::ridl_rt::contract::Ordinal(2u32),
                bytes,
                ::core::option::Option::None,
            )
            .map(TextCorrelation)
    }
    /// What an application implements to serve interface `Docs`'s calls.
    ///
    /// An argument is taken by reference because `serve` reads it again when it evaluates a query's `ensure` clauses.
    pub trait Provider {
        ///The consumer of this command's parameter stream.
        type Put: ::ridl_rt::face::ByteConsumer;
        /// Stores bytes.
        ///
        ///Returns the consumer driven by serve after argument validation and successful delivery acknowledgment. Stream failures are reported to the consumer and the producer without changing that acknowledgment.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `info`
        /// - `data`
        fn put(&mut self, info: &super::Info) -> Self::Put;
        ///The consumer of this command's parameter stream.
        type Note: ::ridl_rt::face::ByteConsumer;
        /// Stores text.
        ///
        ///Returns the consumer driven by serve after argument validation and successful delivery acknowledgment. Stream failures are reported to the consumer and the producer without changing that acknowledgment.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `data`
        fn note(&mut self, args: &super::DocsNoteParams) -> Self::Note;
        ///The byte source of query `fetch`'s reply stream.
        type Fetch: ::ridl_rt::face::ByteSource;
        /// Fetches the document bytes.
        ///
        ///Returns the byte source `serve` drives. It fills chunks up to the stream's LIMIT and sends a partial chunk when the source is pending or ends. Text is checked for UTF-8 and chunk boundaries preserve complete code points. A source interruption stops the stream with Transport::Interrupted.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `id`
        ///   - range 0 ..= 100
        fn fetch(&mut self, id: &super::Id) -> Self::Fetch;
        ///The byte source of query `text`'s reply stream.
        type Text: ::ridl_rt::face::ByteSource;
        /// Fetches the document text.
        ///
        ///Returns the byte source `serve` drives. It fills chunks up to the stream's LIMIT and sends a partial chunk when the source is pending or ends. Text is checked for UTF-8 and chunk boundaries preserve complete code points. A source interruption stops the stream with Transport::Interrupted.
        ///
        /// # Rules
        ///
        /// - response within 1000000 us
        ///
        /// # Parameters
        ///
        /// - `id`
        ///   - range 0 ..= 100
        fn text(&mut self, id: &super::Id) -> Self::Text;
    }
    pub(crate) fn dispatch<H, P>(
        h: &mut H,
        p: &mut P,
        buf: &mut [u8],
        budget: &mut usize,
        streams: &mut Streams<H, P>,
    ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::StreamHandler,
        P: Provider,
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<H::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        if buf.len()
            < <super::Docs as ::ridl_rt::contract::Sizes<H::Encoding>>::MAX_BUFFER_SIZE
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
                        != <super::Docs as ::ridl_rt::contract::Interface>::NUMBER
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
                                match match ::ridl_rt::payload::Ref::<
                                    super::Info,
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
                                } {
                                    Err(e) => h.settle(claim.id, Err(e)),
                                    Ok(__arg) => {
                                        match <super::DocsPut as ::ridl_rt::contract::Command>::require(
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
                                                if streams.put.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    let accepted = h.settle(claim.id, Ok(&[]));
                                                    if accepted.is_ok() {
                                                        streams.put = Some(ByteInputSlot {
                                                            claim: claim.id,
                                                            consumer: p.put(&__arg),
                                                            buffer: [0u8; 16128],
                                                            held: None,
                                                        });
                                                        streams.__started = true;
                                                    }
                                                    accepted
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(4u32) => {
                                match match ::ridl_rt::payload::Ref::<
                                    super::DocsNoteParams,
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
                                } {
                                    Err(e) => h.settle(claim.id, Err(e)),
                                    Ok(__arg) => {
                                        match <super::DocsNote as ::ridl_rt::contract::Command>::require(
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
                                                if streams.note.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    let accepted = h.settle(claim.id, Ok(&[]));
                                                    if accepted.is_ok() {
                                                        streams.note = Some(ByteInputSlot {
                                                            claim: claim.id,
                                                            consumer: p.note(&__arg),
                                                            buffer: [0u8; 16128],
                                                            held: None,
                                                        });
                                                        streams.__started = true;
                                                    }
                                                    accepted
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(1u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Id,
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
                                        match <super::DocsFetch as ::ridl_rt::contract::Query>::require(
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
                                                if streams.fetch.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    streams.fetch = Some(ByteStreamSlot {
                                                        claim: claim.id,
                                                        source: p.fetch(&__arg),
                                                        buffer: [0u8; 16128],
                                                        held: None,
                                                        len: 0,
                                                        terminal: None,
                                                    });
                                                    streams.__started = true;
                                                    Ok(())
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ::ridl_rt::contract::Ordinal(2u32) => {
                                let decoded = match ::ridl_rt::payload::Ref::<
                                    super::Id,
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
                                        match <super::DocsText as ::ridl_rt::contract::Query>::require(
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
                                                if streams.text.is_some() {
                                                    h.settle(
                                                        claim.id,
                                                        Err(
                                                            ::ridl_rt::error::CallError::Transport(
                                                                ::ridl_rt::error::Transport::Busy,
                                                            ),
                                                        ),
                                                    )
                                                } else {
                                                    streams.text = Some(ByteStreamSlot {
                                                        claim: claim.id,
                                                        source: p.text(&__arg),
                                                        buffer: [0u8; 16128],
                                                        held: None,
                                                        len: 0,
                                                        terminal: None,
                                                    });
                                                    streams.__started = true;
                                                    Ok(())
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
    struct ByteInputSlot<C, B> {
        claim: ::ridl_rt::port::ClaimId,
        consumer: C,
        buffer: B,
        held: Option<(usize, usize)>,
    }
    fn drive_input_put<H, P>(
        h: &mut H,
        slot: &mut Option<ByteInputSlot<P::Put, [u8; 16128]>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        if slot.is_none() {
            return false;
        }
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let mut stream = slot.take().expect("checked above");
        if let Some((start, end)) = stream.held {
            if start != end {
                match ::ridl_rt::face::ByteConsumer::poll_write(
                    &mut stream.consumer,
                    cx,
                    &stream.buffer[start..end],
                ) {
                    ::core::task::Poll::Pending => {
                        *slot = Some(stream);
                        return false;
                    }
                    ::core::task::Poll::Ready(n) => {
                        assert!(
                            n > 0 && n <= end - start,
                            "ByteConsumer returned invalid length"
                        );
                        stream.held = if start + n == end {
                            None
                        } else {
                            Some((start + n, end))
                        };
                        *slot = Some(stream);
                        return true;
                    }
                }
            }
            stream.held = None;
        }
        h.wake_on(
            ::ridl_rt::port::Interest::ClaimStream(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
            ),
            cx.waker(),
        );
        match h
            .pull_claim(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
                ::core::convert::AsMut::as_mut(&mut stream.buffer),
            )
        {
            Ok(::ridl_rt::port::Pull::Chunk(n)) => {
                stream.held = Some((0, n));
                *slot = Some(stream);
                true
            }
            Ok(::ridl_rt::port::Pull::End) => {
                ::ridl_rt::face::ByteConsumer::end(&mut stream.consumer);
                false
            }
            Ok(::ridl_rt::port::Pull::Failed(e)) => {
                ::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer, e);
                false
            }
            Ok(::ridl_rt::port::Pull::Pending) => {
                *slot = Some(stream);
                false
            }
            Err(_) => {
                let e = ::ridl_rt::error::CallError::Transport(
                    ::ridl_rt::error::Transport::Corrupt,
                );
                let _ = h.abort_claim(stream.claim, ::ridl_rt::contract::StreamNo(1), e);
                ::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer, e);
                false
            }
        }
    }
    fn drive_input_note<H, P>(
        h: &mut H,
        slot: &mut Option<ByteInputSlot<P::Note, [u8; 16128]>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        P: Provider,
    {
        if slot.is_none() {
            return false;
        }
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let mut stream = slot.take().expect("checked above");
        if let Some((start, end)) = stream.held {
            if start != end {
                match ::ridl_rt::face::ByteConsumer::poll_write(
                    &mut stream.consumer,
                    cx,
                    &stream.buffer[start..end],
                ) {
                    ::core::task::Poll::Pending => {
                        *slot = Some(stream);
                        return false;
                    }
                    ::core::task::Poll::Ready(n) => {
                        assert!(
                            n > 0 && n <= end - start,
                            "ByteConsumer returned invalid length"
                        );
                        stream.held = if start + n == end {
                            None
                        } else {
                            Some((start + n, end))
                        };
                        *slot = Some(stream);
                        return true;
                    }
                }
            }
            stream.held = None;
        }
        h.wake_on(
            ::ridl_rt::port::Interest::ClaimStream(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
            ),
            cx.waker(),
        );
        match h
            .pull_claim(
                stream.claim,
                ::ridl_rt::contract::StreamNo(1),
                ::core::convert::AsMut::as_mut(&mut stream.buffer),
            )
        {
            Ok(::ridl_rt::port::Pull::Chunk(n)) => {
                if ::core::str::from_utf8(&stream.buffer[..n]).is_err() {
                    let e = ::ridl_rt::error::CallError::Contract(
                        ::ridl_rt::error::Contract::InvalidValue(::ridl_rt::payload::Violation {
                            type_name: Some("string"),
                            rule: ::ridl_rt::payload::Rule::Utf8,
                        }),
                    );
                    let _ = h
                        .abort_claim(stream.claim, ::ridl_rt::contract::StreamNo(1), e);
                    ::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer, e);
                    return false;
                }
                stream.held = Some((0, n));
                *slot = Some(stream);
                true
            }
            Ok(::ridl_rt::port::Pull::End) => {
                ::ridl_rt::face::ByteConsumer::end(&mut stream.consumer);
                false
            }
            Ok(::ridl_rt::port::Pull::Failed(e)) => {
                ::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer, e);
                false
            }
            Ok(::ridl_rt::port::Pull::Pending) => {
                *slot = Some(stream);
                false
            }
            Err(_) => {
                let e = ::ridl_rt::error::CallError::Transport(
                    ::ridl_rt::error::Transport::Corrupt,
                );
                let _ = h.abort_claim(stream.claim, ::ridl_rt::contract::StreamNo(1), e);
                ::ridl_rt::face::ByteConsumer::failed(&mut stream.consumer, e);
                false
            }
        }
    }
    struct ByteStreamSlot<S, const N: usize> {
        claim: ::ridl_rt::port::ClaimId,
        source: S,
        buffer: [u8; N],
        len: usize,
        held: Option<usize>,
        terminal: Option<Result<(), ::ridl_rt::error::CallError>>,
    }
    fn drive_bytes<H, S, const N: usize, const TEXT: bool>(
        h: &mut H,
        slot: &mut Option<ByteStreamSlot<S, N>>,
        cx: &mut ::core::task::Context<'_>,
        budget: &mut usize,
    ) -> bool
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::StreamHandler
            + ::ridl_rt::port::Wakeable,
        S: ::ridl_rt::face::ByteSource,
    {
        let Some(stream) = slot.as_mut() else {
            return false;
        };
        let claim = stream.claim;
        let interest = ::ridl_rt::port::Interest::ClaimStream(
            claim,
            ::ridl_rt::port::REPLY_STREAM,
        );
        let mut pending = false;
        let mut polled = false;
        loop {
            if let Some(end) = stream.held {
                h.wake_on(interest, cx.waker());
                match h
                    .push_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        &stream.buffer[..end],
                    )
                {
                    Ok(taken) if taken == end => {
                        stream.buffer.copy_within(end..stream.len, 0);
                        stream.len -= end;
                        stream.held = None;
                    }
                    Ok(_) => return false,
                    Err(error) => {
                        stop_stream(h, claim, error);
                        *slot = None;
                        return false;
                    }
                }
                if pending {
                    return false;
                }
            }
            if let (0, Some(terminal)) = (stream.len, stream.terminal) {
                match terminal {
                    Ok(()) => {
                        let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
                    }
                    Err(error) => {
                        let _ = h
                            .abort_claim(claim, ::ridl_rt::port::REPLY_STREAM, error);
                    }
                }
                *slot = None;
                return false;
            }
            if stream.len == N || pending || stream.terminal.is_some() {
                let mut end = stream.len;
                let text_error = if TEXT {
                    ::core::str::from_utf8(&stream.buffer[..stream.len]).err()
                } else {
                    None
                };
                if let Some(error) = text_error {
                    end = error.valid_up_to();
                    if error.error_len().is_some() || stream.terminal.is_some() {
                        stream.terminal = Some(
                            Err(
                                ::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::InvalidValue(::ridl_rt::payload::Violation {
                                        type_name: Some("string"),
                                        rule: ::ridl_rt::payload::Rule::Utf8,
                                    }),
                                ),
                            ),
                        );
                        stream.len = end;
                    }
                }
                if end != 0 {
                    stream.held = Some(end);
                    continue;
                }
                if stream.terminal.is_some() {
                    continue;
                }
                if pending {
                    return false;
                }
            }
            if polled || *budget == 0 {
                return true;
            }
            *budget -= 1;
            polled = true;
            match stream.source.poll_read(cx, &mut stream.buffer[stream.len..]) {
                ::core::task::Poll::Pending => pending = true,
                ::core::task::Poll::Ready(Ok(0)) => stream.terminal = Some(Ok(())),
                ::core::task::Poll::Ready(Ok(n)) => stream.len += n,
                ::core::task::Poll::Ready(Err(::ridl_rt::face::Interrupted)) => {
                    let _ = h
                        .abort_claim(
                            claim,
                            ::ridl_rt::port::REPLY_STREAM,
                            ::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Interrupted,
                            ),
                        );
                    *slot = None;
                    return false;
                }
            }
        }
    }
    pub(crate) struct Streams<H, P>
    where
        H: ::ridl_rt::port::Encoded,
        P: Provider,
    {
        __started: bool,
        __port: ::core::marker::PhantomData<H>,
        fetch: Option<ByteStreamSlot<P::Fetch, 16128>>,
        text: Option<ByteStreamSlot<P::Text, 16128>>,
        put: Option<ByteInputSlot<P::Put, [u8; 16128]>>,
        note: Option<ByteInputSlot<P::Note, [u8; 16128]>>,
    }
    fn stop_stream<H: ::ridl_rt::port::StreamHandler>(
        h: &mut H,
        claim: ::ridl_rt::port::ClaimId,
        error: ::ridl_rt::port::PushError,
    ) {
        match error {
            ::ridl_rt::port::PushError::Failed(_) => {
                let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
            }
            ::ridl_rt::port::PushError::TooLarge { .. } => {
                let _ = h
                    .abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            _ => {}
        }
    }
    /// Serves interface `Docs`'s commands and queries with `p`, over the handler port `h`, and returns the future that does the serving. `Handler::serve` is called with the interface's command and query ordinals when this function runs; a refusal is a future that is ready with `ProviderError::Serve`. Each poll of the future registers its interest in the interface's claims, then takes and settles the claims the handler has, at most 32 in one poll, so that one poll does not hold a single-threaded executor while callers keep sending. A poll that took 32 claims wakes the future's waker and is `Pending`, so the executor polls it again after other tasks have run; a poll that found no claim left before 32 is `Pending` without waking it. Under `ridl_rt::task::noop_waker` that wake is discarded, so a frame loop that polls the future once per frame settles at most 32 claims per frame; a frame loop that polls with `ridl_rt::task::flag_waker` polls again while its flag was set, up to the loop's own limit of polls per frame. The future resolves only when the handler port fails, to `ProviderError::Claim`; every claim settled before the failure stays settled. `h` is held by value and `p` by `&mut` until the future is dropped.
    ///
    /// # Panics
    ///
    /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, before `Handler::serve` is called. A program that must not panic makes the same comparison first, `h.catalog() == <Docs as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Docs` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
    ///
    /// # Streamed replies
    ///
    /// Each query's Provider method returns its reply source after the arguments and require clauses pass. Each serve poll first drives running streams, then takes claims, sharing a budget of 32. A source is polled at most once for each running call and spends one budget unit. A chunk held while the port window is full is retried before another source poll.
    ///
    /// A named-element source uses Source::poll_next: each element is encoded and checked in an inline Payload buffer before transmission. Ready(None) ends it; Ready(Some(Err(Interrupted))) interrupts it. A byte or text source uses ByteSource::poll_read and fills an inline byte buffer without element encoding: Ready(Ok(0)) ends it and Ready(Err(Interrupted)) interrupts it. A partial buffer is pushed when the source is pending or ends. Text chunks preserve complete code points; invalid text or an incomplete code point at normal end is refused with Contract::InvalidValue and Rule::Utf8, after the valid prefix.
    ///
    /// An end settles the reply claim successfully; an interruption stops it with Transport::Interrupted. PushError::TooLarge stops it with Transport::Interrupted, and PushError::Failed ends the claim with the caller's outcome. The source is dropped when its stream ends or stops. One stream runs per member; a concurrent call is settled Transport::Busy. A caller's forget is detected by the next push, so a pending source can keep its member busy until it yields again. Ready progress wakes serve for another poll; a pending source registers its own waker. Dropping serve interrupts every running stream and drops its source.
    #[track_caller]
    pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
    where
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<H::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                &[
                    ::ridl_rt::contract::Ordinal(3u32),
                    ::ridl_rt::contract::Ordinal(4u32),
                    ::ridl_rt::contract::Ordinal(1u32),
                    ::ridl_rt::contract::Ordinal(2u32),
                ],
            )
        {
            Ok(()) => ServeState::Serving,
            Err(error) => ServeState::Refused(error),
        };
        Serve {
            handler: h,
            provider: p,
            buf: <super::Docs as ::ridl_rt::contract::Sizes<H::Encoding>>::CLAIM_BUFFER,
            state,
            streams: Streams {
                __started: false,
                __port: ::core::marker::PhantomData,
                fetch: None,
                text: None,
                put: None,
                note: None,
            },
        }
    }
    ///The future `serve` returns over interface `Docs`. It holds the handler, the provider, and the claim buffer of `<Docs as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the encoding the handler port carries. It never resolves to `Ok`, and polling it again after it resolved panics.
    #[must_use = "claims are served only while the future is polled"]
    pub struct Serve<
        'a,
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    >
    where
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
    {
        handler: H,
        provider: &'a mut P,
        buf: <super::Docs as ::ridl_rt::contract::Sizes<H::Encoding>>::ClaimBuffer,
        state: ServeState,
        streams: Streams<H, P>,
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
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::marker::Unpin for Serve<'_, H, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<H::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<H::Encoding>,
    {}
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::future::Future for Serve<'_, H, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Info: ::ridl_rt::payload::Payload<H::Encoding>,
        super::DocsNoteParams: ::ridl_rt::payload::Payload<H::Encoding>,
        super::Id: ::ridl_rt::payload::Payload<H::Encoding>,
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
                                <super::Docs as ::ridl_rt::contract::Interface>::NUMBER,
                            ),
                            cx.waker(),
                        );
                    let mut budget = SERVE_BUDGET;
                    let mut again = false;
                    again
                        |= drive_bytes::<
                            H,
                            P::Fetch,
                            16128,
                            false,
                        >(&mut this.handler, &mut this.streams.fetch, cx, &mut budget);
                    again
                        |= drive_bytes::<
                            H,
                            P::Text,
                            16128,
                            true,
                        >(&mut this.handler, &mut this.streams.text, cx, &mut budget);
                    again
                        |= drive_input_put::<
                            H,
                            P,
                        >(&mut this.handler, &mut this.streams.put, cx, &mut budget);
                    again
                        |= drive_input_note::<
                            H,
                            P,
                        >(&mut this.handler, &mut this.streams.note, cx, &mut budget);
                    match dispatch(
                        &mut this.handler,
                        &mut *this.provider,
                        ::core::convert::AsMut::as_mut(&mut this.buf),
                        &mut budget,
                        &mut this.streams,
                    ) {
                        Ok(_) => {
                            if ::core::mem::take(&mut this.streams.__started)
                                || budget == 0 || again
                            {
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
    impl<
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
            + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
        P: Provider,
    > ::core::ops::Drop for Serve<'_, H, P>
    where
        super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
    {
        fn drop(&mut self) {
            if let Some(stream) = self.streams.fetch.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            if let Some(stream) = self.streams.text.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
            }
            if let Some(mut stream) = self.streams.put.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::contract::StreamNo(1),
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                ::ridl_rt::face::ByteConsumer::failed(
                    &mut stream.consumer,
                    ::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Interrupted,
                    ),
                );
            }
            if let Some(mut stream) = self.streams.note.take() {
                let _ = self
                    .handler
                    .abort_claim(
                        stream.claim,
                        ::ridl_rt::contract::StreamNo(1),
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                ::ridl_rt::face::ByteConsumer::failed(
                    &mut stream.consumer,
                    ::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Interrupted,
                    ),
                );
            }
        }
    }
    ///The traits a consumer of interface `Docs`'s face needs in scope. Glob-import this module, `use <this interface's module>::prelude::*;`, and every method of those traits — `new` — is called as an inherent method would be. Only the `ridl-rt` traits are re-exported by name; this module's own traits are re-exported as `_`, so the preludes of two interfaces can share one scope.
    pub mod prelude {
        pub use ::ridl_rt::face::Bind;
    }
    ///The blocking provider face of interface `Docs`, under the crate's std feature. The serve function runs the async provider until its timeout or a port failure.
    #[cfg(feature = "std")]
    pub mod blocking {
        fn __deadline_after(
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::option::Option<::std::time::Instant> {
            timeout.and_then(|timeout| ::std::time::Instant::now().checked_add(timeout))
        }
        /// Serves interface `Docs`'s commands and queries with `p`, over the handler port `h`, on the calling thread, until the handler port fails or `timeout` passes: it is `block_on` over `serve`. A failure is returned as `serve`'s future resolves to it; the timeout is `Ok(())`, so a loop that also does other work can call this repeatedly. With `None` it returns only on a failure. `h` is dropped when this returns.
        ///
        /// # Panics
        ///
        /// Panics when `h` is attached to a catalog other than the one this face was generated from, that is, when the package name or the catalog hash differs (ADR-0023 decision 8); the comparison is made once, by `super::serve`. A program that must not panic makes the same comparison first, `h.catalog() == <Docs as ridl_rt::contract::Interface>::CATALOG` with `ridl_rt::port::Attached` in scope, where `Docs` is the interface's descriptor type, declared beside this interface's module, and handles a mismatch its own way.
        #[track_caller]
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler
                + ::ridl_rt::port::Wakeable + ::ridl_rt::port::StreamHandler,
            P: super::Provider,
            super::super::Docs: ::ridl_rt::contract::Sizes<H::Encoding>,
            super::super::Info: ::ridl_rt::payload::Payload<H::Encoding>,
            super::super::DocsNoteParams: ::ridl_rt::payload::Payload<H::Encoding>,
            super::super::Id: ::ridl_rt::payload::Payload<H::Encoding>,
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
