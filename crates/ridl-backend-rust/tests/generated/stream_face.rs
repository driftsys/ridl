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
            42, 252, 70, 39, 222, 230, 137, 155, 108, 153, 163, 78, 148, 44, 47, 109,
            201, 40, 56, 193, 214, 92, 111, 196, 71, 12, 182, 209, 95, 130, 205, 100,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(1);
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
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::Proto3> for Log {
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
            42, 252, 70, 39, 222, 230, 137, 155, 108, 153, 163, 78, 148, 44, 47, 109,
            201, 40, 56, 193, 214, 92, 111, 196, 71, 12, 182, 209, 95, 130, 205, 100,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(2);
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
            42, 252, 70, 39, 222, 230, 137, 155, 108, 153, 163, 78, 148, 44, 47, 109,
            201, 40, 56, 193, 214, 92, 111, 196, 71, 12, 182, 209, 95, 130, 205, 100,
        ]),
    };
    const NUMBER: ::ridl_rt::contract::InterfaceNo = ::ridl_rt::contract::InterfaceNo(3);
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
/// A log of readings, read as a stream.
///
///The generated interaction face of interface `Log`.
#[allow(clippy::module_inception)]
pub mod log {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub(crate) struct ClearCorrelation(pub ::ridl_rt::port::Correlation);
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
        port: P,
    }
    impl<
        P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    > Client<P>
    where
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
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
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_clear(&mut self.port, &__arg) {
                Ok(correlation) => ClearPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => ClearPhase::Unsent(__arg),
                Err(error) => ClearPhase::Failed(error),
            };
            ClearCall {
                port: &mut self.port,
                phase,
                deadline,
            }
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
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match send_history(&mut self.port, &__arg) {
                Ok(correlation) => HistoryPhase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => HistoryPhase::Unsent(__arg),
                Err(error) => HistoryPhase::Failed(error),
            };
            HistoryCall {
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
        super::Log: ::ridl_rt::contract::Sizes<P::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
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
            Client { port }
        }
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
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler,
        P: Provider,
        super::Log: ::ridl_rt::contract::Sizes<H::Encoding>,
        super::Level: ::ridl_rt::payload::Payload<H::Encoding>,
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
    {
        __started: bool,
        history: ::core::option::Option<
            StreamSlot<P::History, ElementBuffer<H, super::Reading>>,
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
        super::Reading: ::ridl_rt::payload::Payload<H::Encoding>,
    {
        check_catalog(::ridl_rt::port::Attached::catalog(&h));
        let state = match h
            .serve(
                <super::Log as ::ridl_rt::contract::Interface>::NUMBER,
                &[::ridl_rt::contract::Ordinal(2u32), ::ridl_rt::contract::Ordinal(1u32)],
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
                history: None,
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
