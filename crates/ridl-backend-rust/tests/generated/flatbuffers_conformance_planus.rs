pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `fb_demo.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `fb`
    ///
    /// Generated from these locations:
    /// * File `fb_demo.fbs`
    pub mod fb {
        /// The namespace `fb.demo`
        ///
        /// Generated from these locations:
        /// * File `fb_demo.fbs`
        pub mod demo {
            /// The enum `Health` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Enum `Health` in the file `fb_demo.fbs:3`
            #[derive(
                Copy,
                Clone,
                Debug,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            #[repr(i64)]
            pub enum Health {
                /// The variant `OK` in the enum `Health`
                Ok = 0,

                /// The variant `WARN` in the enum `Health`
                Warn = 1,

                /// The variant `FAIL` in the enum `Health`
                Fail = 2,
            }

            impl Health {
                /// Array containing all valid variants of Health
                pub const ENUM_VALUES: [Self; 3] = [Self::Ok, Self::Warn, Self::Fail];
            }

            impl ::core::convert::TryFrom<i64> for Health {
                type Error = ::planus::errors::UnknownEnumTagKind;
                #[inline]
                fn try_from(
                    value: i64,
                ) -> ::core::result::Result<Self, ::planus::errors::UnknownEnumTagKind>
                {
                    #[allow(clippy::match_single_binding)]
                    match value {
                        0 => ::core::result::Result::Ok(Health::Ok),
                        1 => ::core::result::Result::Ok(Health::Warn),
                        2 => ::core::result::Result::Ok(Health::Fail),

                        _ => ::core::result::Result::Err(::planus::errors::UnknownEnumTagKind {
                            tag: value as i128,
                        }),
                    }
                }
            }

            impl ::core::convert::From<Health> for i64 {
                #[inline]
                fn from(value: Health) -> Self {
                    value as i64
                }
            }

            /// # Safety
            /// The Planus compiler correctly calculates `ALIGNMENT` and `SIZE`.
            unsafe impl ::planus::Primitive for Health {
                const ALIGNMENT: usize = 8;
                const SIZE: usize = 8;
            }

            impl ::planus::WriteAsPrimitive<Health> for Health {
                #[inline]
                fn write<const N: usize>(
                    &self,
                    cursor: ::planus::Cursor<'_, N>,
                    buffer_position: u32,
                ) {
                    (*self as i64).write(cursor, buffer_position);
                }
            }

            impl ::planus::WriteAs<Health> for Health {
                type Prepared = Self;

                #[inline]
                fn prepare(&self, _builder: &mut ::planus::Builder) -> Health {
                    *self
                }
            }

            impl ::planus::WriteAsDefault<Health, Health> for Health {
                type Prepared = Self;

                #[inline]
                fn prepare(
                    &self,
                    _builder: &mut ::planus::Builder,
                    default: &Health,
                ) -> ::core::option::Option<Health> {
                    if self == default {
                        ::core::option::Option::None
                    } else {
                        ::core::option::Option::Some(*self)
                    }
                }
            }

            impl ::planus::WriteAsOptional<Health> for Health {
                type Prepared = Self;

                #[inline]
                fn prepare(
                    &self,
                    _builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<Health> {
                    ::core::option::Option::Some(*self)
                }
            }

            impl<'buf> ::planus::TableRead<'buf> for Health {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'buf>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    let n: i64 = ::planus::TableRead::from_buffer(buffer, offset)?;
                    ::core::result::Result::Ok(::core::convert::TryInto::try_into(n)?)
                }
            }

            impl<'buf> ::planus::VectorReadInner<'buf> for Health {
                type Error = ::planus::errors::UnknownEnumTag;
                const STRIDE: usize = 8;
                #[inline]
                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'buf>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::UnknownEnumTag>
                {
                    let value =
                        unsafe { <i64 as ::planus::VectorRead>::from_buffer(buffer, offset) };
                    let value: ::core::result::Result<Self, _> =
                        ::core::convert::TryInto::try_into(value);
                    value.map_err(|error_kind| {
                        error_kind.with_error_location(
                            "Health",
                            "VectorRead::from_buffer",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<Health> for Health {
                const STRIDE: usize = 8;

                type Value = Self;

                #[inline]
                fn prepare(&self, _builder: &mut ::planus::Builder) -> Self {
                    *self
                }

                #[inline]
                unsafe fn write_values(
                    values: &[Self],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 8];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (8 * i) as u32,
                        );
                    }
                }
            }

            /// The table `Inner` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `Inner` in the file `fb_demo.fbs:9`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct Inner {
                /// The field `speed` in the table `Inner`
                pub speed: u16,
                /// The field `label` in the table `Inner`
                pub label: ::core::option::Option<::planus::alloc::string::String>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for Inner {
                fn default() -> Self {
                    Self {
                        speed: 0,
                        label: ::core::default::Default::default(),
                    }
                }
            }

            impl Inner {
                /// Creates a [InnerBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> InnerBuilder<()> {
                    InnerBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_speed: impl ::planus::WriteAsDefault<u16, u16>,
                    field_label: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_speed = field_speed.prepare(builder, &0);
                    let prepared_label = field_label.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_label.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(1);
                    }
                    if prepared_speed.is_some() {
                        table_writer.write_entry::<u16>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_label) = prepared_label {
                                object_writer.write::<_, _, 4>(&prepared_label);
                            }
                            if let ::core::option::Option::Some(prepared_speed) = prepared_speed {
                                object_writer.write::<_, _, 2>(&prepared_speed);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<Inner>> for Inner {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Inner> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<Inner>> for Inner {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Inner>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<Inner> for Inner {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Inner> {
                    Inner::create(builder, self.speed, &self.label)
                }
            }

            /// Builder for serializing an instance of the [Inner] type.
            ///
            /// Can be created using the [Inner::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct InnerBuilder<State>(State);

            impl InnerBuilder<()> {
                /// Setter for the [`speed` field](Inner#structfield.speed).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn speed<T0>(self, value: T0) -> InnerBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u16, u16>,
                {
                    InnerBuilder((value,))
                }

                /// Sets the [`speed` field](Inner#structfield.speed) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn speed_as_default(self) -> InnerBuilder<(::planus::DefaultValue,)> {
                    self.speed(::planus::DefaultValue)
                }
            }

            impl<T0> InnerBuilder<(T0,)> {
                /// Setter for the [`label` field](Inner#structfield.label).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn label<T1>(self, value: T1) -> InnerBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0,) = self.0;
                    InnerBuilder((v0, value))
                }

                /// Sets the [`label` field](Inner#structfield.label) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn label_as_null(self) -> InnerBuilder<(T0, ())> {
                    self.label(())
                }
            }

            impl<T0, T1> InnerBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Inner].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Inner>
                where
                    Self: ::planus::WriteAsOffset<Inner>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                > ::planus::WriteAs<::planus::Offset<Inner>> for InnerBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<Inner>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Inner> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                > ::planus::WriteAsOptional<::planus::Offset<Inner>> for InnerBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<Inner>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Inner>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                > ::planus::WriteAsOffset<Inner> for InnerBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Inner> {
                    let (v0, v1) = &self.0;
                    Inner::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [Inner].
            #[derive(Copy, Clone)]
            pub struct InnerRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> InnerRef<'a> {
                /// Getter for the [`speed` field](Inner#structfield.speed).
                #[inline]
                pub fn speed(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(self.0.access(0, "Inner", "speed")?.unwrap_or(0))
                }

                /// Getter for the [`label` field](Inner#structfield.label).
                #[inline]
                pub fn label(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(1, "Inner", "label")
                }
            }

            impl<'a> ::core::fmt::Debug for InnerRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("InnerRef");
                    f.field("speed", &self.speed());
                    if let ::core::option::Option::Some(field_label) = self.label().transpose() {
                        f.field("label", &field_label);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<InnerRef<'a>> for Inner {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: InnerRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        speed: ::core::convert::TryInto::try_into(value.speed()?)?,
                        label: value.label()?.map(::core::convert::Into::into),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for InnerRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for InnerRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[InnerRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<Inner>> for Inner {
                type Value = ::planus::Offset<Inner>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<Inner>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for InnerRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[InnerRef]", "read_as_root", 0)
                    })
                }
            }

            /// The union `OutcomeUnion` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Union `OutcomeUnion` in the file `fb_demo.fbs:16`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub enum OutcomeUnion {
                /// The variant `ok` in the union `OutcomeUnion`
                Ok(::planus::alloc::boxed::Box<self::Inner>),

                /// The variant `bad` in the union `OutcomeUnion`
                Bad(::planus::alloc::boxed::Box<self::OutcomeBadBox>),
            }

            impl OutcomeUnion {
                /// Creates a [OutcomeUnionBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> OutcomeUnionBuilder<::planus::Uninitialized> {
                    OutcomeUnionBuilder(::planus::Uninitialized)
                }

                #[inline]
                pub fn create_ok(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::Inner>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(1, value.prepare(builder).downcast())
                }

                #[inline]
                pub fn create_bad(
                    builder: &mut ::planus::Builder,
                    value: impl ::planus::WriteAsOffset<self::OutcomeBadBox>,
                ) -> ::planus::UnionOffset<Self> {
                    ::planus::UnionOffset::new(2, value.prepare(builder).downcast())
                }
            }

            impl ::planus::WriteAsUnion<OutcomeUnion> for OutcomeUnion {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::UnionOffset<Self> {
                    match self {
                        Self::Ok(value) => Self::create_ok(builder, value),
                        Self::Bad(value) => Self::create_bad(builder, value),
                    }
                }
            }

            impl ::planus::WriteAsOptionalUnion<OutcomeUnion> for OutcomeUnion {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<Self>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Builder for serializing an instance of the [OutcomeUnion] type.
            ///
            /// Can be created using the [OutcomeUnion::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct OutcomeUnionBuilder<T>(T);

            impl OutcomeUnionBuilder<::planus::Uninitialized> {
                /// Creates an instance of the [`ok` variant](OutcomeUnion#variant.Ok).
                #[inline]
                pub fn ok<T>(self, value: T) -> OutcomeUnionBuilder<::planus::Initialized<1, T>>
                where
                    T: ::planus::WriteAsOffset<self::Inner>,
                {
                    OutcomeUnionBuilder(::planus::Initialized(value))
                }

                /// Creates an instance of the [`bad` variant](OutcomeUnion#variant.Bad).
                #[inline]
                pub fn bad<T>(self, value: T) -> OutcomeUnionBuilder<::planus::Initialized<2, T>>
                where
                    T: ::planus::WriteAsOffset<self::OutcomeBadBox>,
                {
                    OutcomeUnionBuilder(::planus::Initialized(value))
                }
            }

            impl<const N: u8, T> OutcomeUnionBuilder<::planus::Initialized<N, T>> {
                /// Finish writing the builder to get an [UnionOffset](::planus::UnionOffset) to a serialized [OutcomeUnion].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<OutcomeUnion>
                where
                    Self: ::planus::WriteAsUnion<OutcomeUnion>,
                {
                    ::planus::WriteAsUnion::prepare(&self, builder)
                }
            }

            impl<T> ::planus::WriteAsUnion<OutcomeUnion> for OutcomeUnionBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::Inner>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<OutcomeUnion> {
                    ::planus::UnionOffset::new(1, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<OutcomeUnion>
                for OutcomeUnionBuilder<::planus::Initialized<1, T>>
            where
                T: ::planus::WriteAsOffset<self::Inner>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<OutcomeUnion>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }
            impl<T> ::planus::WriteAsUnion<OutcomeUnion> for OutcomeUnionBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::OutcomeBadBox>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::UnionOffset<OutcomeUnion> {
                    ::planus::UnionOffset::new(2, (self.0).0.prepare(builder).downcast())
                }
            }

            impl<T> ::planus::WriteAsOptionalUnion<OutcomeUnion>
                for OutcomeUnionBuilder<::planus::Initialized<2, T>>
            where
                T: ::planus::WriteAsOffset<self::OutcomeBadBox>,
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::UnionOffset<OutcomeUnion>> {
                    ::core::option::Option::Some(::planus::WriteAsUnion::prepare(self, builder))
                }
            }

            /// Reference to a deserialized [OutcomeUnion].
            #[derive(Copy, Clone, Debug)]
            pub enum OutcomeUnionRef<'a> {
                Ok(self::InnerRef<'a>),
                Bad(self::OutcomeBadBoxRef<'a>),
            }

            impl<'a> ::core::convert::TryFrom<OutcomeUnionRef<'a>> for OutcomeUnion {
                type Error = ::planus::Error;

                fn try_from(value: OutcomeUnionRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(match value {
                        OutcomeUnionRef::Ok(value) => Self::Ok(::planus::alloc::boxed::Box::new(
                            ::core::convert::TryFrom::try_from(value)?,
                        )),

                        OutcomeUnionRef::Bad(value) => Self::Bad(::planus::alloc::boxed::Box::new(
                            ::core::convert::TryFrom::try_from(value)?,
                        )),
                    })
                }
            }

            impl<'a> ::planus::TableReadUnion<'a> for OutcomeUnionRef<'a> {
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    tag: u8,
                    field_offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    match tag {
                        1 => ::core::result::Result::Ok(Self::Ok(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        2 => ::core::result::Result::Ok(Self::Bad(
                            ::planus::TableRead::from_buffer(buffer, field_offset)?,
                        )),
                        _ => ::core::result::Result::Err(
                            ::planus::errors::ErrorKind::UnknownUnionTag { tag },
                        ),
                    }
                }
            }

            impl<'a> ::planus::VectorReadUnion<'a> for OutcomeUnionRef<'a> {
                const VECTOR_NAME: &'static str = "[OutcomeUnionRef]";
            }

            /// The table `Outcome` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `Outcome` in the file `fb_demo.fbs:18`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct Outcome {
                /// The field `value` in the table `Outcome`
                pub value: ::core::option::Option<self::OutcomeUnion>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for Outcome {
                fn default() -> Self {
                    Self {
                        value: ::core::default::Default::default(),
                    }
                }
            }

            impl Outcome {
                /// Creates a [OutcomeBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> OutcomeBuilder<()> {
                    OutcomeBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsOptionalUnion<self::OutcomeUnion>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::OutcomeUnion>>(1);
                    }
                    if prepared_value.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 4>(&prepared_value.offset());
                            }
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 1>(&prepared_value.tag());
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<Outcome>> for Outcome {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Outcome> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<Outcome>> for Outcome {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Outcome>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<Outcome> for Outcome {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Outcome> {
                    Outcome::create(builder, &self.value)
                }
            }

            /// Builder for serializing an instance of the [Outcome] type.
            ///
            /// Can be created using the [Outcome::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct OutcomeBuilder<State>(State);

            impl OutcomeBuilder<()> {
                /// Setter for the [`value` field](Outcome#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> OutcomeBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptionalUnion<self::OutcomeUnion>,
                {
                    OutcomeBuilder((value,))
                }

                /// Sets the [`value` field](Outcome#structfield.value) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_null(self) -> OutcomeBuilder<((),)> {
                    self.value(())
                }
            }

            impl<T0> OutcomeBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Outcome].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Outcome>
                where
                    Self: ::planus::WriteAsOffset<Outcome>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::OutcomeUnion>>
                ::planus::WriteAs<::planus::Offset<Outcome>> for OutcomeBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<Outcome>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Outcome> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::OutcomeUnion>>
                ::planus::WriteAsOptional<::planus::Offset<Outcome>> for OutcomeBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<Outcome>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Outcome>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsOptionalUnion<self::OutcomeUnion>>
                ::planus::WriteAsOffset<Outcome> for OutcomeBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Outcome> {
                    let (v0,) = &self.0;
                    Outcome::create(builder, v0)
                }
            }

            /// Reference to a deserialized [Outcome].
            #[derive(Copy, Clone)]
            pub struct OutcomeRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> OutcomeRef<'a> {
                /// Getter for the [`value` field](Outcome#structfield.value).
                #[inline]
                pub fn value(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::OutcomeUnionRef<'a>>>
                {
                    self.0.access_union(0, "Outcome", "value")
                }
            }

            impl<'a> ::core::fmt::Debug for OutcomeRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("OutcomeRef");
                    if let ::core::option::Option::Some(field_value) = self.value().transpose() {
                        f.field("value", &field_value);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<OutcomeRef<'a>> for Outcome {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: OutcomeRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: if let ::core::option::Option::Some(value) = value.value()? {
                            ::core::option::Option::Some(::core::convert::TryInto::try_into(value)?)
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for OutcomeRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for OutcomeRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[OutcomeRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<Outcome>> for Outcome {
                type Value = ::planus::Offset<Outcome>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<Outcome>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for OutcomeRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[OutcomeRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `Report` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `Report` in the file `fb_demo.fbs:22`
            #[derive(
                Clone, Debug, PartialEq, PartialOrd, ::serde::Serialize, ::serde::Deserialize,
            )]
            pub struct Report {
                /// The field `id` in the table `Report`
                pub id: u8,
                /// The field `name` in the table `Report`
                pub name: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `blob` in the table `Report`
                pub blob: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
                /// The field `ratio` in the table `Report`
                pub ratio: f32,
                /// The field `engaged` in the table `Report`
                pub engaged: bool,
                /// The field `health` in the table `Report`
                pub health: self::Health,
                /// The field `flags` in the table `Report`
                pub flags: u8,
                /// The field `inner` in the table `Report`
                pub inner: ::core::option::Option<::planus::alloc::boxed::Box<self::Inner>>,
                /// The field `outcome` in the table `Report`
                pub outcome: ::core::option::Option<::planus::alloc::boxed::Box<self::Outcome>>,
                /// The field `range` in the table `Report`
                pub range: ::core::option::Option<::planus::alloc::boxed::Box<self::ReportRange>>,
                /// The field `readings` in the table `Report`
                pub readings: ::core::option::Option<::planus::alloc::vec::Vec<u16>>,
                /// The field `faults` in the table `Report`
                pub faults: ::core::option::Option<::planus::alloc::vec::Vec<self::Health>>,
                /// The field `meta` in the table `Report`
                pub meta: ::core::option::Option<::planus::alloc::vec::Vec<self::ReportMetaEntry>>,
                /// The field `names` in the table `Report`
                pub names: ::core::option::Option<
                    ::planus::alloc::vec::Vec<::planus::alloc::string::String>,
                >,
                /// The field `inners` in the table `Report`
                pub inners: ::core::option::Option<::planus::alloc::vec::Vec<self::Inner>>,
                /// The field `outcomes` in the table `Report`
                pub outcomes: ::core::option::Option<::planus::alloc::vec::Vec<self::Outcome>>,
                /// The field `points` in the table `Report`
                pub points:
                    ::core::option::Option<::planus::alloc::vec::Vec<self::ReportPointsElement>>,
                /// The field `pair` in the table `Report`
                pub pair: ::core::option::Option<::planus::alloc::boxed::Box<self::ReportPair>>,
                /// The field `note` in the table `Report`
                pub note: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `spare` in the table `Report`
                pub spare: u16,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for Report {
                fn default() -> Self {
                    Self {
                        id: 0,
                        name: ::core::default::Default::default(),
                        blob: ::core::default::Default::default(),
                        ratio: 0.0,
                        engaged: false,
                        health: self::Health::Ok,
                        flags: 0,
                        inner: ::core::default::Default::default(),
                        outcome: ::core::default::Default::default(),
                        range: ::core::default::Default::default(),
                        readings: ::core::default::Default::default(),
                        faults: ::core::default::Default::default(),
                        meta: ::core::default::Default::default(),
                        names: ::core::default::Default::default(),
                        inners: ::core::default::Default::default(),
                        outcomes: ::core::default::Default::default(),
                        points: ::core::default::Default::default(),
                        pair: ::core::default::Default::default(),
                        note: ::core::default::Default::default(),
                        spare: 0,
                    }
                }
            }

            impl Report {
                /// Creates a [ReportBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ReportBuilder<()> {
                    ReportBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_id: impl ::planus::WriteAsDefault<u8, u8>,
                    field_name: impl ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    field_blob: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                    field_ratio: impl ::planus::WriteAsDefault<f32, f32>,
                    field_engaged: impl ::planus::WriteAsDefault<bool, bool>,
                    field_health: impl ::planus::WriteAsDefault<self::Health, self::Health>,
                    field_flags: impl ::planus::WriteAsDefault<u8, u8>,
                    field_inner: impl ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                    field_outcome: impl ::planus::WriteAsOptional<::planus::Offset<self::Outcome>>,
                    field_range: impl ::planus::WriteAsOptional<::planus::Offset<self::ReportRange>>,
                    field_readings: impl ::planus::WriteAsOptional<::planus::Offset<[u16]>>,
                    field_faults: impl ::planus::WriteAsOptional<::planus::Offset<[self::Health]>>,
                    field_meta: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>,
                    >,
                    field_names: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<str>]>,
                    >,
                    field_inners: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::Inner>]>,
                    >,
                    field_outcomes: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::Outcome>]>,
                    >,
                    field_points: impl ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>,
                    >,
                    field_pair: impl ::planus::WriteAsOptional<::planus::Offset<self::ReportPair>>,
                    field_note: impl ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    field_spare: impl ::planus::WriteAsDefault<u16, u16>,
                ) -> ::planus::Offset<Self> {
                    let prepared_id = field_id.prepare(builder, &0);
                    let prepared_name = field_name.prepare(builder);
                    let prepared_blob = field_blob.prepare(builder);
                    let prepared_ratio = field_ratio.prepare(builder, &0.0);
                    let prepared_engaged = field_engaged.prepare(builder, &false);
                    let prepared_health = field_health.prepare(builder, &self::Health::Ok);
                    let prepared_flags = field_flags.prepare(builder, &0);
                    let prepared_inner = field_inner.prepare(builder);
                    let prepared_outcome = field_outcome.prepare(builder);
                    let prepared_range = field_range.prepare(builder);
                    let prepared_readings = field_readings.prepare(builder);
                    let prepared_faults = field_faults.prepare(builder);
                    let prepared_meta = field_meta.prepare(builder);
                    let prepared_names = field_names.prepare(builder);
                    let prepared_inners = field_inners.prepare(builder);
                    let prepared_outcomes = field_outcomes.prepare(builder);
                    let prepared_points = field_points.prepare(builder);
                    let prepared_pair = field_pair.prepare(builder);
                    let prepared_note = field_note.prepare(builder);
                    let prepared_spare = field_spare.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<46> =
                        ::core::default::Default::default();
                    if prepared_health.is_some() {
                        table_writer.write_entry::<self::Health>(5);
                    }
                    if prepared_name.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(1);
                    }
                    if prepared_blob.is_some() {
                        table_writer.write_entry::<::planus::Offset<[u8]>>(2);
                    }
                    if prepared_ratio.is_some() {
                        table_writer.write_entry::<f32>(3);
                    }
                    if prepared_inner.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::Inner>>(7);
                    }
                    if prepared_outcome.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::Outcome>>(8);
                    }
                    if prepared_range.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::ReportRange>>(9);
                    }
                    if prepared_readings.is_some() {
                        table_writer.write_entry::<::planus::Offset<[u16]>>(10);
                    }
                    if prepared_faults.is_some() {
                        table_writer.write_entry::<::planus::Offset<[self::Health]>>(11);
                    }
                    if prepared_meta.is_some() {
                        table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>>(12);
                    }
                    if prepared_names.is_some() {
                        table_writer.write_entry::<::planus::Offset<[::planus::Offset<str>]>>(13);
                    }
                    if prepared_inners.is_some() {
                        table_writer
                            .write_entry::<::planus::Offset<[::planus::Offset<self::Inner>]>>(14);
                    }
                    if prepared_outcomes.is_some() {
                        table_writer
                            .write_entry::<::planus::Offset<[::planus::Offset<self::Outcome>]>>(15);
                    }
                    if prepared_points.is_some() {
                        table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>>(16);
                    }
                    if prepared_pair.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::ReportPair>>(18);
                    }
                    if prepared_note.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(19);
                    }
                    if prepared_spare.is_some() {
                        table_writer.write_entry::<u16>(20);
                    }
                    if prepared_id.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }
                    if prepared_engaged.is_some() {
                        table_writer.write_entry::<bool>(4);
                    }
                    if prepared_flags.is_some() {
                        table_writer.write_entry::<u8>(6);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_health) = prepared_health {
                                object_writer.write::<_, _, 8>(&prepared_health);
                            }
                            if let ::core::option::Option::Some(prepared_name) = prepared_name {
                                object_writer.write::<_, _, 4>(&prepared_name);
                            }
                            if let ::core::option::Option::Some(prepared_blob) = prepared_blob {
                                object_writer.write::<_, _, 4>(&prepared_blob);
                            }
                            if let ::core::option::Option::Some(prepared_ratio) = prepared_ratio {
                                object_writer.write::<_, _, 4>(&prepared_ratio);
                            }
                            if let ::core::option::Option::Some(prepared_inner) = prepared_inner {
                                object_writer.write::<_, _, 4>(&prepared_inner);
                            }
                            if let ::core::option::Option::Some(prepared_outcome) = prepared_outcome
                            {
                                object_writer.write::<_, _, 4>(&prepared_outcome);
                            }
                            if let ::core::option::Option::Some(prepared_range) = prepared_range {
                                object_writer.write::<_, _, 4>(&prepared_range);
                            }
                            if let ::core::option::Option::Some(prepared_readings) =
                                prepared_readings
                            {
                                object_writer.write::<_, _, 4>(&prepared_readings);
                            }
                            if let ::core::option::Option::Some(prepared_faults) = prepared_faults {
                                object_writer.write::<_, _, 4>(&prepared_faults);
                            }
                            if let ::core::option::Option::Some(prepared_meta) = prepared_meta {
                                object_writer.write::<_, _, 4>(&prepared_meta);
                            }
                            if let ::core::option::Option::Some(prepared_names) = prepared_names {
                                object_writer.write::<_, _, 4>(&prepared_names);
                            }
                            if let ::core::option::Option::Some(prepared_inners) = prepared_inners {
                                object_writer.write::<_, _, 4>(&prepared_inners);
                            }
                            if let ::core::option::Option::Some(prepared_outcomes) =
                                prepared_outcomes
                            {
                                object_writer.write::<_, _, 4>(&prepared_outcomes);
                            }
                            if let ::core::option::Option::Some(prepared_points) = prepared_points {
                                object_writer.write::<_, _, 4>(&prepared_points);
                            }
                            if let ::core::option::Option::Some(prepared_pair) = prepared_pair {
                                object_writer.write::<_, _, 4>(&prepared_pair);
                            }
                            if let ::core::option::Option::Some(prepared_note) = prepared_note {
                                object_writer.write::<_, _, 4>(&prepared_note);
                            }
                            if let ::core::option::Option::Some(prepared_spare) = prepared_spare {
                                object_writer.write::<_, _, 2>(&prepared_spare);
                            }
                            if let ::core::option::Option::Some(prepared_id) = prepared_id {
                                object_writer.write::<_, _, 1>(&prepared_id);
                            }
                            if let ::core::option::Option::Some(prepared_engaged) = prepared_engaged
                            {
                                object_writer.write::<_, _, 1>(&prepared_engaged);
                            }
                            if let ::core::option::Option::Some(prepared_flags) = prepared_flags {
                                object_writer.write::<_, _, 1>(&prepared_flags);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<Report>> for Report {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Report> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<Report>> for Report {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Report>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<Report> for Report {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Report> {
                    Report::create(
                        builder,
                        self.id,
                        &self.name,
                        &self.blob,
                        self.ratio,
                        self.engaged,
                        self.health,
                        self.flags,
                        &self.inner,
                        &self.outcome,
                        &self.range,
                        &self.readings,
                        &self.faults,
                        &self.meta,
                        &self.names,
                        &self.inners,
                        &self.outcomes,
                        &self.points,
                        &self.pair,
                        &self.note,
                        self.spare,
                    )
                }
            }

            /// Builder for serializing an instance of the [Report] type.
            ///
            /// Can be created using the [Report::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ReportBuilder<State>(State);

            impl ReportBuilder<()> {
                /// Setter for the [`id` field](Report#structfield.id).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn id<T0>(self, value: T0) -> ReportBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    ReportBuilder((value,))
                }

                /// Sets the [`id` field](Report#structfield.id) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn id_as_default(self) -> ReportBuilder<(::planus::DefaultValue,)> {
                    self.id(::planus::DefaultValue)
                }
            }

            impl<T0> ReportBuilder<(T0,)> {
                /// Setter for the [`name` field](Report#structfield.name).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn name<T1>(self, value: T1) -> ReportBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (v0,) = self.0;
                    ReportBuilder((v0, value))
                }

                /// Sets the [`name` field](Report#structfield.name) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn name_as_null(self) -> ReportBuilder<(T0, ())> {
                    self.name(())
                }
            }

            impl<T0, T1> ReportBuilder<(T0, T1)> {
                /// Setter for the [`blob` field](Report#structfield.blob).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn blob<T2>(self, value: T2) -> ReportBuilder<(T0, T1, T2)>
                where
                    T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                {
                    let (v0, v1) = self.0;
                    ReportBuilder((v0, v1, value))
                }

                /// Sets the [`blob` field](Report#structfield.blob) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn blob_as_null(self) -> ReportBuilder<(T0, T1, ())> {
                    self.blob(())
                }
            }

            impl<T0, T1, T2> ReportBuilder<(T0, T1, T2)> {
                /// Setter for the [`ratio` field](Report#structfield.ratio).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn ratio<T3>(self, value: T3) -> ReportBuilder<(T0, T1, T2, T3)>
                where
                    T3: ::planus::WriteAsDefault<f32, f32>,
                {
                    let (v0, v1, v2) = self.0;
                    ReportBuilder((v0, v1, v2, value))
                }

                /// Sets the [`ratio` field](Report#structfield.ratio) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn ratio_as_default(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                    self.ratio(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3> ReportBuilder<(T0, T1, T2, T3)> {
                /// Setter for the [`engaged` field](Report#structfield.engaged).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn engaged<T4>(self, value: T4) -> ReportBuilder<(T0, T1, T2, T3, T4)>
                where
                    T4: ::planus::WriteAsDefault<bool, bool>,
                {
                    let (v0, v1, v2, v3) = self.0;
                    ReportBuilder((v0, v1, v2, v3, value))
                }

                /// Sets the [`engaged` field](Report#structfield.engaged) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn engaged_as_default(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                    self.engaged(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4> ReportBuilder<(T0, T1, T2, T3, T4)> {
                /// Setter for the [`health` field](Report#structfield.health).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn health<T5>(self, value: T5) -> ReportBuilder<(T0, T1, T2, T3, T4, T5)>
                where
                    T5: ::planus::WriteAsDefault<self::Health, self::Health>,
                {
                    let (v0, v1, v2, v3, v4) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, value))
                }

                /// Sets the [`health` field](Report#structfield.health) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn health_as_default(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, ::planus::DefaultValue)> {
                    self.health(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5> ReportBuilder<(T0, T1, T2, T3, T4, T5)> {
                /// Setter for the [`flags` field](Report#structfield.flags).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn flags<T6>(self, value: T6) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6)>
                where
                    T6: ::planus::WriteAsDefault<u8, u8>,
                {
                    let (v0, v1, v2, v3, v4, v5) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, value))
                }

                /// Sets the [`flags` field](Report#structfield.flags) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn flags_as_default(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, ::planus::DefaultValue)>
                {
                    self.flags(::planus::DefaultValue)
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6)> {
                /// Setter for the [`inner` field](Report#structfield.inner).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn inner<T7>(self, value: T7) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)>
                where
                    T7: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, value))
                }

                /// Sets the [`inner` field](Report#structfield.inner) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn inner_as_null(self) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, ())> {
                    self.inner(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7)> {
                /// Setter for the [`outcome` field](Report#structfield.outcome).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcome<T8>(
                    self,
                    value: T8,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)>
                where
                    T8: ::planus::WriteAsOptional<::planus::Offset<self::Outcome>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, value))
                }

                /// Sets the [`outcome` field](Report#structfield.outcome) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcome_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, ())> {
                    self.outcome(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8)> {
                /// Setter for the [`range` field](Report#structfield.range).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn range<T9>(
                    self,
                    value: T9,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
                where
                    T9: ::planus::WriteAsOptional<::planus::Offset<self::ReportRange>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, value))
                }

                /// Sets the [`range` field](Report#structfield.range) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn range_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, ())> {
                    self.range(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9>
                ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9)>
            {
                /// Setter for the [`readings` field](Report#structfield.readings).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn readings<T10>(
                    self,
                    value: T10,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
                where
                    T10: ::planus::WriteAsOptional<::planus::Offset<[u16]>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, value))
                }

                /// Sets the [`readings` field](Report#structfield.readings) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn readings_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, ())> {
                    self.readings(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>
                ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)>
            {
                /// Setter for the [`faults` field](Report#structfield.faults).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn faults<T11>(
                    self,
                    value: T11,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
                where
                    T11: ::planus::WriteAsOptional<::planus::Offset<[self::Health]>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, value))
                }

                /// Sets the [`faults` field](Report#structfield.faults) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn faults_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, ())>
                {
                    self.faults(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>
                ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)>
            {
                /// Setter for the [`meta` field](Report#structfield.meta).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn meta<T12>(
                    self,
                    value: T12,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
                where
                    T12: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>,
                    >,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, value))
                }

                /// Sets the [`meta` field](Report#structfield.meta) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn meta_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, ())>
                {
                    self.meta(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12>
                ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)>
            {
                /// Setter for the [`names` field](Report#structfield.names).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn names<T13>(
                    self,
                    value: T13,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)>
                where
                    T13: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<str>]>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12) = self.0;
                    ReportBuilder((v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, value))
                }

                /// Sets the [`names` field](Report#structfield.names) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn names_as_null(
                    self,
                ) -> ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, ())>
                {
                    self.names(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>
                ReportBuilder<(T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13)>
            {
                /// Setter for the [`inners` field](Report#structfield.inners).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn inners<T14>(
                    self,
                    value: T14,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                )>
                where
                    T14: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::Inner>]>,
                    >,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13) = self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, value,
                    ))
                }

                /// Sets the [`inners` field](Report#structfield.inners) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn inners_as_null(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    (),
                )> {
                    self.inners(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14>
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                )>
            {
                /// Setter for the [`outcomes` field](Report#structfield.outcomes).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcomes<T15>(
                    self,
                    value: T15,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                )>
                where
                    T15: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::Outcome>]>,
                    >,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14) = self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, value,
                    ))
                }

                /// Sets the [`outcomes` field](Report#structfield.outcomes) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn outcomes_as_null(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    (),
                )> {
                    self.outcomes(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15>
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                )>
            {
                /// Setter for the [`points` field](Report#structfield.points).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn points<T16>(
                    self,
                    value: T16,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                )>
                where
                    T16: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>,
                    >,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15) =
                        self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, value,
                    ))
                }

                /// Sets the [`points` field](Report#structfield.points) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn points_as_null(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    (),
                )> {
                    self.points(())
                }
            }

            impl<T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16>
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                )>
            {
                /// Setter for the [`pair` field](Report#structfield.pair).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn pair<T17>(
                    self,
                    value: T17,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                )>
                where
                    T17: ::planus::WriteAsOptional<::planus::Offset<self::ReportPair>>,
                {
                    let (v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16) =
                        self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        value,
                    ))
                }

                /// Sets the [`pair` field](Report#structfield.pair) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn pair_as_null(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    (),
                )> {
                    self.pair(())
                }
            }

            impl<
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                >
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                )>
            {
                /// Setter for the [`note` field](Report#structfield.note).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn note<T18>(
                    self,
                    value: T18,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                )>
                where
                    T18: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                    ) = self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, value,
                    ))
                }

                /// Sets the [`note` field](Report#structfield.note) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn note_as_null(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    (),
                )> {
                    self.note(())
                }
            }

            impl<
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                >
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                )>
            {
                /// Setter for the [`spare` field](Report#structfield.spare).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn spare<T19>(
                    self,
                    value: T19,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
                where
                    T19: ::planus::WriteAsDefault<u16, u16>,
                {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                    ) = self.0;
                    ReportBuilder((
                        v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15, v16,
                        v17, v18, value,
                    ))
                }

                /// Sets the [`spare` field](Report#structfield.spare) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn spare_as_default(
                    self,
                ) -> ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    ::planus::DefaultValue,
                )> {
                    self.spare(::planus::DefaultValue)
                }
            }

            impl<
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                >
                ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
            {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Report].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Report>
                where
                    Self: ::planus::WriteAsOffset<Report>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                    T3: ::planus::WriteAsDefault<f32, f32>,
                    T4: ::planus::WriteAsDefault<bool, bool>,
                    T5: ::planus::WriteAsDefault<self::Health, self::Health>,
                    T6: ::planus::WriteAsDefault<u8, u8>,
                    T7: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                    T8: ::planus::WriteAsOptional<::planus::Offset<self::Outcome>>,
                    T9: ::planus::WriteAsOptional<::planus::Offset<self::ReportRange>>,
                    T10: ::planus::WriteAsOptional<::planus::Offset<[u16]>>,
                    T11: ::planus::WriteAsOptional<::planus::Offset<[self::Health]>>,
                    T12: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>,
                    >,
                    T13: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<str>]>>,
                    T14: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Inner>]>>,
                    T15: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Outcome>]>>,
                    T16: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>,
                    >,
                    T17: ::planus::WriteAsOptional<::planus::Offset<self::ReportPair>>,
                    T18: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T19: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAs<::planus::Offset<Report>>
                for ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
            {
                type Prepared = ::planus::Offset<Report>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Report> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                    T3: ::planus::WriteAsDefault<f32, f32>,
                    T4: ::planus::WriteAsDefault<bool, bool>,
                    T5: ::planus::WriteAsDefault<self::Health, self::Health>,
                    T6: ::planus::WriteAsDefault<u8, u8>,
                    T7: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                    T8: ::planus::WriteAsOptional<::planus::Offset<self::Outcome>>,
                    T9: ::planus::WriteAsOptional<::planus::Offset<self::ReportRange>>,
                    T10: ::planus::WriteAsOptional<::planus::Offset<[u16]>>,
                    T11: ::planus::WriteAsOptional<::planus::Offset<[self::Health]>>,
                    T12: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>,
                    >,
                    T13: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<str>]>>,
                    T14: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Inner>]>>,
                    T15: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Outcome>]>>,
                    T16: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>,
                    >,
                    T17: ::planus::WriteAsOptional<::planus::Offset<self::ReportPair>>,
                    T18: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T19: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOptional<::planus::Offset<Report>>
                for ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
            {
                type Prepared = ::planus::Offset<Report>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<Report>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u8, u8>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T2: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                    T3: ::planus::WriteAsDefault<f32, f32>,
                    T4: ::planus::WriteAsDefault<bool, bool>,
                    T5: ::planus::WriteAsDefault<self::Health, self::Health>,
                    T6: ::planus::WriteAsDefault<u8, u8>,
                    T7: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                    T8: ::planus::WriteAsOptional<::planus::Offset<self::Outcome>>,
                    T9: ::planus::WriteAsOptional<::planus::Offset<self::ReportRange>>,
                    T10: ::planus::WriteAsOptional<::planus::Offset<[u16]>>,
                    T11: ::planus::WriteAsOptional<::planus::Offset<[self::Health]>>,
                    T12: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportMetaEntry>]>,
                    >,
                    T13: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<str>]>>,
                    T14: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Inner>]>>,
                    T15: ::planus::WriteAsOptional<::planus::Offset<[::planus::Offset<self::Outcome>]>>,
                    T16: ::planus::WriteAsOptional<
                        ::planus::Offset<[::planus::Offset<self::ReportPointsElement>]>,
                    >,
                    T17: ::planus::WriteAsOptional<::planus::Offset<self::ReportPair>>,
                    T18: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T19: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOffset<Report>
                for ReportBuilder<(
                    T0,
                    T1,
                    T2,
                    T3,
                    T4,
                    T5,
                    T6,
                    T7,
                    T8,
                    T9,
                    T10,
                    T11,
                    T12,
                    T13,
                    T14,
                    T15,
                    T16,
                    T17,
                    T18,
                    T19,
                )>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Report> {
                    let (
                        v0,
                        v1,
                        v2,
                        v3,
                        v4,
                        v5,
                        v6,
                        v7,
                        v8,
                        v9,
                        v10,
                        v11,
                        v12,
                        v13,
                        v14,
                        v15,
                        v16,
                        v17,
                        v18,
                        v19,
                    ) = &self.0;
                    Report::create(
                        builder, v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14,
                        v15, v16, v17, v18, v19,
                    )
                }
            }

            /// Reference to a deserialized [Report].
            #[derive(Copy, Clone)]
            pub struct ReportRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> ReportRef<'a> {
                /// Getter for the [`id` field](Report#structfield.id).
                #[inline]
                pub fn id(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(self.0.access(0, "Report", "id")?.unwrap_or(0))
                }

                /// Getter for the [`name` field](Report#structfield.name).
                #[inline]
                pub fn name(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(1, "Report", "name")
                }

                /// Getter for the [`blob` field](Report#structfield.blob).
                #[inline]
                pub fn blob(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                    self.0.access(2, "Report", "blob")
                }

                /// Getter for the [`ratio` field](Report#structfield.ratio).
                #[inline]
                pub fn ratio(&self) -> ::planus::Result<f32> {
                    ::core::result::Result::Ok(self.0.access(3, "Report", "ratio")?.unwrap_or(0.0))
                }

                /// Getter for the [`engaged` field](Report#structfield.engaged).
                #[inline]
                pub fn engaged(&self) -> ::planus::Result<bool> {
                    ::core::result::Result::Ok(
                        self.0.access(4, "Report", "engaged")?.unwrap_or(false),
                    )
                }

                /// Getter for the [`health` field](Report#structfield.health).
                #[inline]
                pub fn health(&self) -> ::planus::Result<self::Health> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(5, "Report", "health")?
                            .unwrap_or(self::Health::Ok),
                    )
                }

                /// Getter for the [`flags` field](Report#structfield.flags).
                #[inline]
                pub fn flags(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(self.0.access(6, "Report", "flags")?.unwrap_or(0))
                }

                /// Getter for the [`inner` field](Report#structfield.inner).
                #[inline]
                pub fn inner(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::InnerRef<'a>>> {
                    self.0.access(7, "Report", "inner")
                }

                /// Getter for the [`outcome` field](Report#structfield.outcome).
                #[inline]
                pub fn outcome(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::OutcomeRef<'a>>>
                {
                    self.0.access(8, "Report", "outcome")
                }

                /// Getter for the [`range` field](Report#structfield.range).
                #[inline]
                pub fn range(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::ReportRangeRef<'a>>>
                {
                    self.0.access(9, "Report", "range")
                }

                /// Getter for the [`readings` field](Report#structfield.readings).
                #[inline]
                pub fn readings(
                    &self,
                ) -> ::planus::Result<::core::option::Option<::planus::Vector<'a, u16>>>
                {
                    self.0.access(10, "Report", "readings")
                }

                /// Getter for the [`faults` field](Report#structfield.faults).
                #[inline]
                pub fn faults(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<
                            'a,
                            ::core::result::Result<self::Health, ::planus::errors::UnknownEnumTag>,
                        >,
                    >,
                > {
                    self.0.access(11, "Report", "faults")
                }

                /// Getter for the [`meta` field](Report#structfield.meta).
                #[inline]
                pub fn meta(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::ReportMetaEntryRef<'a>>>,
                    >,
                > {
                    self.0.access(12, "Report", "meta")
                }

                /// Getter for the [`names` field](Report#structfield.names).
                #[inline]
                pub fn names(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<&'a ::core::primitive::str>>,
                    >,
                > {
                    self.0.access(13, "Report", "names")
                }

                /// Getter for the [`inners` field](Report#structfield.inners).
                #[inline]
                pub fn inners(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::InnerRef<'a>>>,
                    >,
                > {
                    self.0.access(14, "Report", "inners")
                }

                /// Getter for the [`outcomes` field](Report#structfield.outcomes).
                #[inline]
                pub fn outcomes(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::OutcomeRef<'a>>>,
                    >,
                > {
                    self.0.access(15, "Report", "outcomes")
                }

                /// Getter for the [`points` field](Report#structfield.points).
                #[inline]
                pub fn points(
                    &self,
                ) -> ::planus::Result<
                    ::core::option::Option<
                        ::planus::Vector<'a, ::planus::Result<self::ReportPointsElementRef<'a>>>,
                    >,
                > {
                    self.0.access(16, "Report", "points")
                }

                /// Getter for the [`pair` field](Report#structfield.pair).
                #[inline]
                pub fn pair(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::ReportPairRef<'a>>>
                {
                    self.0.access(18, "Report", "pair")
                }

                /// Getter for the [`note` field](Report#structfield.note).
                #[inline]
                pub fn note(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(19, "Report", "note")
                }

                /// Getter for the [`spare` field](Report#structfield.spare).
                #[inline]
                pub fn spare(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(self.0.access(20, "Report", "spare")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for ReportRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ReportRef");
                    f.field("id", &self.id());
                    if let ::core::option::Option::Some(field_name) = self.name().transpose() {
                        f.field("name", &field_name);
                    }
                    if let ::core::option::Option::Some(field_blob) = self.blob().transpose() {
                        f.field("blob", &field_blob);
                    }
                    f.field("ratio", &self.ratio());
                    f.field("engaged", &self.engaged());
                    f.field("health", &self.health());
                    f.field("flags", &self.flags());
                    if let ::core::option::Option::Some(field_inner) = self.inner().transpose() {
                        f.field("inner", &field_inner);
                    }
                    if let ::core::option::Option::Some(field_outcome) = self.outcome().transpose()
                    {
                        f.field("outcome", &field_outcome);
                    }
                    if let ::core::option::Option::Some(field_range) = self.range().transpose() {
                        f.field("range", &field_range);
                    }
                    if let ::core::option::Option::Some(field_readings) =
                        self.readings().transpose()
                    {
                        f.field("readings", &field_readings);
                    }
                    if let ::core::option::Option::Some(field_faults) = self.faults().transpose() {
                        f.field("faults", &field_faults);
                    }
                    if let ::core::option::Option::Some(field_meta) = self.meta().transpose() {
                        f.field("meta", &field_meta);
                    }
                    if let ::core::option::Option::Some(field_names) = self.names().transpose() {
                        f.field("names", &field_names);
                    }
                    if let ::core::option::Option::Some(field_inners) = self.inners().transpose() {
                        f.field("inners", &field_inners);
                    }
                    if let ::core::option::Option::Some(field_outcomes) =
                        self.outcomes().transpose()
                    {
                        f.field("outcomes", &field_outcomes);
                    }
                    if let ::core::option::Option::Some(field_points) = self.points().transpose() {
                        f.field("points", &field_points);
                    }
                    if let ::core::option::Option::Some(field_pair) = self.pair().transpose() {
                        f.field("pair", &field_pair);
                    }
                    if let ::core::option::Option::Some(field_note) = self.note().transpose() {
                        f.field("note", &field_note);
                    }
                    f.field("spare", &self.spare());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ReportRef<'a>> for Report {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ReportRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        id: ::core::convert::TryInto::try_into(value.id()?)?,
                        name: value.name()?.map(::core::convert::Into::into),
                        blob: value.blob()?.map(|v| v.to_vec()),
                        ratio: ::core::convert::TryInto::try_into(value.ratio()?)?,
                        engaged: ::core::convert::TryInto::try_into(value.engaged()?)?,
                        health: ::core::convert::TryInto::try_into(value.health()?)?,
                        flags: ::core::convert::TryInto::try_into(value.flags()?)?,
                        inner: if let ::core::option::Option::Some(inner) = value.inner()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(inner)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                        outcome: if let ::core::option::Option::Some(outcome) = value.outcome()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(outcome)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                        range: if let ::core::option::Option::Some(range) = value.range()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(range)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                        readings: if let ::core::option::Option::Some(readings) =
                            value.readings()?
                        {
                            ::core::option::Option::Some(readings.to_vec()?)
                        } else {
                            ::core::option::Option::None
                        },
                        faults: if let ::core::option::Option::Some(faults) = value.faults()? {
                            ::core::option::Option::Some(faults.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        meta: if let ::core::option::Option::Some(meta) = value.meta()? {
                            ::core::option::Option::Some(meta.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        names: if let ::core::option::Option::Some(names) = value.names()? {
                            ::core::option::Option::Some(names.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        inners: if let ::core::option::Option::Some(inners) = value.inners()? {
                            ::core::option::Option::Some(inners.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        outcomes: if let ::core::option::Option::Some(outcomes) =
                            value.outcomes()?
                        {
                            ::core::option::Option::Some(outcomes.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        points: if let ::core::option::Option::Some(points) = value.points()? {
                            ::core::option::Option::Some(points.to_vec_result()?)
                        } else {
                            ::core::option::Option::None
                        },
                        pair: if let ::core::option::Option::Some(pair) = value.pair()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(pair)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                        note: value.note()?.map(::core::convert::Into::into),
                        spare: ::core::convert::TryInto::try_into(value.spare()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ReportRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ReportRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<Report>> for Report {
                type Value = ::planus::Offset<Report>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<Report>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for ReportRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ReportRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `SpeedBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `SpeedBox` in the file `fb_demo.fbs:57`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct SpeedBox {
                /// The field `value` in the table `SpeedBox`
                pub value: u16,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for SpeedBox {
                fn default() -> Self {
                    Self { value: 0 }
                }
            }

            impl SpeedBox {
                /// Creates a [SpeedBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> SpeedBoxBuilder<()> {
                    SpeedBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<u16, u16>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<u16>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 2>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<SpeedBox>> for SpeedBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpeedBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<SpeedBox>> for SpeedBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SpeedBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<SpeedBox> for SpeedBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpeedBox> {
                    SpeedBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [SpeedBox] type.
            ///
            /// Can be created using the [SpeedBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct SpeedBoxBuilder<State>(State);

            impl SpeedBoxBuilder<()> {
                /// Setter for the [`value` field](SpeedBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> SpeedBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u16, u16>,
                {
                    SpeedBoxBuilder((value,))
                }

                /// Sets the [`value` field](SpeedBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> SpeedBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> SpeedBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [SpeedBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpeedBox>
                where
                    Self: ::planus::WriteAsOffset<SpeedBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u16, u16>>
                ::planus::WriteAs<::planus::Offset<SpeedBox>> for SpeedBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<SpeedBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpeedBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u16, u16>>
                ::planus::WriteAsOptional<::planus::Offset<SpeedBox>> for SpeedBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<SpeedBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<SpeedBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u16, u16>> ::planus::WriteAsOffset<SpeedBox>
                for SpeedBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<SpeedBox> {
                    let (v0,) = &self.0;
                    SpeedBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [SpeedBox].
            #[derive(Copy, Clone)]
            pub struct SpeedBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> SpeedBoxRef<'a> {
                /// Getter for the [`value` field](SpeedBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(self.0.access(0, "SpeedBox", "value")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for SpeedBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("SpeedBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<SpeedBoxRef<'a>> for SpeedBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: SpeedBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for SpeedBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for SpeedBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[SpeedBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<SpeedBox>> for SpeedBox {
                type Value = ::planus::Offset<SpeedBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<SpeedBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for SpeedBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[SpeedBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `CountBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `CountBox` in the file `fb_demo.fbs:62`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct CountBox {
                /// The field `value` in the table `CountBox`
                pub value: u8,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for CountBox {
                fn default() -> Self {
                    Self { value: 0 }
                }
            }

            impl CountBox {
                /// Creates a [CountBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> CountBoxBuilder<()> {
                    CountBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<u8, u8>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 1>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<CountBox>> for CountBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CountBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<CountBox>> for CountBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CountBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<CountBox> for CountBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CountBox> {
                    CountBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [CountBox] type.
            ///
            /// Can be created using the [CountBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct CountBoxBuilder<State>(State);

            impl CountBoxBuilder<()> {
                /// Setter for the [`value` field](CountBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> CountBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    CountBoxBuilder((value,))
                }

                /// Sets the [`value` field](CountBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> CountBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> CountBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [CountBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<CountBox>
                where
                    Self: ::planus::WriteAsOffset<CountBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>> ::planus::WriteAs<::planus::Offset<CountBox>>
                for CountBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CountBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CountBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>>
                ::planus::WriteAsOptional<::planus::Offset<CountBox>> for CountBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<CountBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<CountBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>> ::planus::WriteAsOffset<CountBox>
                for CountBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<CountBox> {
                    let (v0,) = &self.0;
                    CountBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [CountBox].
            #[derive(Copy, Clone)]
            pub struct CountBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> CountBoxRef<'a> {
                /// Getter for the [`value` field](CountBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(self.0.access(0, "CountBox", "value")?.unwrap_or(0))
                }
            }

            impl<'a> ::core::fmt::Debug for CountBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("CountBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<CountBoxRef<'a>> for CountBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: CountBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for CountBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for CountBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[CountBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<CountBox>> for CountBox {
                type Value = ::planus::Offset<CountBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<CountBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for CountBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[CountBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `RatioBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `RatioBox` in the file `fb_demo.fbs:67`
            #[derive(
                Clone, Debug, PartialEq, PartialOrd, ::serde::Serialize, ::serde::Deserialize,
            )]
            pub struct RatioBox {
                /// The field `value` in the table `RatioBox`
                pub value: f32,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for RatioBox {
                fn default() -> Self {
                    Self { value: 0.0 }
                }
            }

            impl RatioBox {
                /// Creates a [RatioBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> RatioBoxBuilder<()> {
                    RatioBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<f32, f32>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &0.0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<f32>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 4>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<RatioBox>> for RatioBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<RatioBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<RatioBox>> for RatioBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RatioBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<RatioBox> for RatioBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<RatioBox> {
                    RatioBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [RatioBox] type.
            ///
            /// Can be created using the [RatioBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct RatioBoxBuilder<State>(State);

            impl RatioBoxBuilder<()> {
                /// Setter for the [`value` field](RatioBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> RatioBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<f32, f32>,
                {
                    RatioBoxBuilder((value,))
                }

                /// Sets the [`value` field](RatioBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> RatioBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> RatioBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [RatioBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<RatioBox>
                where
                    Self: ::planus::WriteAsOffset<RatioBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<f32, f32>>
                ::planus::WriteAs<::planus::Offset<RatioBox>> for RatioBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<RatioBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<RatioBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<f32, f32>>
                ::planus::WriteAsOptional<::planus::Offset<RatioBox>> for RatioBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<RatioBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<RatioBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<f32, f32>> ::planus::WriteAsOffset<RatioBox>
                for RatioBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<RatioBox> {
                    let (v0,) = &self.0;
                    RatioBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [RatioBox].
            #[derive(Copy, Clone)]
            pub struct RatioBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> RatioBoxRef<'a> {
                /// Getter for the [`value` field](RatioBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<f32> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "RatioBox", "value")?.unwrap_or(0.0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for RatioBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("RatioBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<RatioBoxRef<'a>> for RatioBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: RatioBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for RatioBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for RatioBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[RatioBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<RatioBox>> for RatioBox {
                type Value = ::planus::Offset<RatioBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<RatioBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for RatioBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[RatioBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `LabelBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `LabelBox` in the file `fb_demo.fbs:72`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct LabelBox {
                /// The field `value` in the table `LabelBox`
                pub value: ::core::option::Option<::planus::alloc::string::String>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for LabelBox {
                fn default() -> Self {
                    Self {
                        value: ::core::default::Default::default(),
                    }
                }
            }

            impl LabelBox {
                /// Creates a [LabelBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> LabelBoxBuilder<()> {
                    LabelBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsOptional<
                        ::planus::Offset<::core::primitive::str>,
                    >,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 4>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<LabelBox>> for LabelBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<LabelBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<LabelBox>> for LabelBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<LabelBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<LabelBox> for LabelBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<LabelBox> {
                    LabelBox::create(builder, &self.value)
                }
            }

            /// Builder for serializing an instance of the [LabelBox] type.
            ///
            /// Can be created using the [LabelBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct LabelBoxBuilder<State>(State);

            impl LabelBoxBuilder<()> {
                /// Setter for the [`value` field](LabelBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> LabelBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    LabelBoxBuilder((value,))
                }

                /// Sets the [`value` field](LabelBox#structfield.value) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_null(self) -> LabelBoxBuilder<((),)> {
                    self.value(())
                }
            }

            impl<T0> LabelBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [LabelBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<LabelBox>
                where
                    Self: ::planus::WriteAsOffset<LabelBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>>
                ::planus::WriteAs<::planus::Offset<LabelBox>> for LabelBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<LabelBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<LabelBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>>
                ::planus::WriteAsOptional<::planus::Offset<LabelBox>> for LabelBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<LabelBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<LabelBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>>
                ::planus::WriteAsOffset<LabelBox> for LabelBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<LabelBox> {
                    let (v0,) = &self.0;
                    LabelBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [LabelBox].
            #[derive(Copy, Clone)]
            pub struct LabelBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> LabelBoxRef<'a> {
                /// Getter for the [`value` field](LabelBox#structfield.value).
                #[inline]
                pub fn value(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(0, "LabelBox", "value")
                }
            }

            impl<'a> ::core::fmt::Debug for LabelBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("LabelBoxRef");
                    if let ::core::option::Option::Some(field_value) = self.value().transpose() {
                        f.field("value", &field_value);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<LabelBoxRef<'a>> for LabelBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: LabelBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: value.value()?.map(::core::convert::Into::into),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for LabelBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for LabelBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[LabelBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<LabelBox>> for LabelBox {
                type Value = ::planus::Offset<LabelBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<LabelBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for LabelBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[LabelBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `BlobBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `BlobBox` in the file `fb_demo.fbs:77`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct BlobBox {
                /// The field `value` in the table `BlobBox`
                pub value: ::core::option::Option<::planus::alloc::vec::Vec<u8>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for BlobBox {
                fn default() -> Self {
                    Self {
                        value: ::core::default::Default::default(),
                    }
                }
            }

            impl BlobBox {
                /// Creates a [BlobBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> BlobBoxBuilder<()> {
                    BlobBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<::planus::Offset<[u8]>>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 4>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<BlobBox>> for BlobBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<BlobBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<BlobBox>> for BlobBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<BlobBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<BlobBox> for BlobBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<BlobBox> {
                    BlobBox::create(builder, &self.value)
                }
            }

            /// Builder for serializing an instance of the [BlobBox] type.
            ///
            /// Can be created using the [BlobBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct BlobBoxBuilder<State>(State);

            impl BlobBoxBuilder<()> {
                /// Setter for the [`value` field](BlobBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> BlobBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptional<::planus::Offset<[u8]>>,
                {
                    BlobBoxBuilder((value,))
                }

                /// Sets the [`value` field](BlobBox#structfield.value) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_null(self) -> BlobBoxBuilder<((),)> {
                    self.value(())
                }
            }

            impl<T0> BlobBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [BlobBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<BlobBox>
                where
                    Self: ::planus::WriteAsOffset<BlobBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<[u8]>>>
                ::planus::WriteAs<::planus::Offset<BlobBox>> for BlobBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<BlobBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<BlobBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<[u8]>>>
                ::planus::WriteAsOptional<::planus::Offset<BlobBox>> for BlobBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<BlobBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<BlobBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsOptional<::planus::Offset<[u8]>>>
                ::planus::WriteAsOffset<BlobBox> for BlobBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<BlobBox> {
                    let (v0,) = &self.0;
                    BlobBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [BlobBox].
            #[derive(Copy, Clone)]
            pub struct BlobBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> BlobBoxRef<'a> {
                /// Getter for the [`value` field](BlobBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<::core::option::Option<&'a [u8]>> {
                    self.0.access(0, "BlobBox", "value")
                }
            }

            impl<'a> ::core::fmt::Debug for BlobBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("BlobBoxRef");
                    if let ::core::option::Option::Some(field_value) = self.value().transpose() {
                        f.field("value", &field_value);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<BlobBoxRef<'a>> for BlobBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: BlobBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: value.value()?.map(|v| v.to_vec()),
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for BlobBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for BlobBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[BlobBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<BlobBox>> for BlobBox {
                type Value = ::planus::Offset<BlobBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<BlobBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for BlobBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[BlobBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `EngagedBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `EngagedBox` in the file `fb_demo.fbs:82`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct EngagedBox {
                /// The field `value` in the table `EngagedBox`
                pub value: bool,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for EngagedBox {
                fn default() -> Self {
                    Self { value: false }
                }
            }

            impl EngagedBox {
                /// Creates a [EngagedBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> EngagedBoxBuilder<()> {
                    EngagedBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<bool, bool>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &false);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<bool>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 1>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<EngagedBox>> for EngagedBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EngagedBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<EngagedBox>> for EngagedBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<EngagedBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<EngagedBox> for EngagedBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EngagedBox> {
                    EngagedBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [EngagedBox] type.
            ///
            /// Can be created using the [EngagedBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct EngagedBoxBuilder<State>(State);

            impl EngagedBoxBuilder<()> {
                /// Setter for the [`value` field](EngagedBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> EngagedBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<bool, bool>,
                {
                    EngagedBoxBuilder((value,))
                }

                /// Sets the [`value` field](EngagedBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> EngagedBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> EngagedBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [EngagedBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<EngagedBox>
                where
                    Self: ::planus::WriteAsOffset<EngagedBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<bool, bool>>
                ::planus::WriteAs<::planus::Offset<EngagedBox>> for EngagedBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<EngagedBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EngagedBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<bool, bool>>
                ::planus::WriteAsOptional<::planus::Offset<EngagedBox>>
                for EngagedBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<EngagedBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<EngagedBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<bool, bool>> ::planus::WriteAsOffset<EngagedBox>
                for EngagedBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<EngagedBox> {
                    let (v0,) = &self.0;
                    EngagedBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [EngagedBox].
            #[derive(Copy, Clone)]
            pub struct EngagedBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> EngagedBoxRef<'a> {
                /// Getter for the [`value` field](EngagedBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<bool> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "EngagedBox", "value")?.unwrap_or(false),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for EngagedBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("EngagedBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<EngagedBoxRef<'a>> for EngagedBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: EngagedBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for EngagedBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for EngagedBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[EngagedBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<EngagedBox>> for EngagedBox {
                type Value = ::planus::Offset<EngagedBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<EngagedBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for EngagedBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[EngagedBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `HealthBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `HealthBox` in the file `fb_demo.fbs:87`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct HealthBox {
                /// The field `value` in the table `HealthBox`
                pub value: self::Health,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for HealthBox {
                fn default() -> Self {
                    Self {
                        value: self::Health::Ok,
                    }
                }
            }

            impl HealthBox {
                /// Creates a [HealthBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> HealthBoxBuilder<()> {
                    HealthBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<self::Health, self::Health>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &self::Health::Ok);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<self::Health>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 8>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<HealthBox>> for HealthBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<HealthBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<HealthBox>> for HealthBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<HealthBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<HealthBox> for HealthBox {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<HealthBox> {
                    HealthBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [HealthBox] type.
            ///
            /// Can be created using the [HealthBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct HealthBoxBuilder<State>(State);

            impl HealthBoxBuilder<()> {
                /// Setter for the [`value` field](HealthBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> HealthBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<self::Health, self::Health>,
                {
                    HealthBoxBuilder((value,))
                }

                /// Sets the [`value` field](HealthBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> HealthBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> HealthBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [HealthBox].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<HealthBox>
                where
                    Self: ::planus::WriteAsOffset<HealthBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAs<::planus::Offset<HealthBox>> for HealthBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<HealthBox>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<HealthBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAsOptional<::planus::Offset<HealthBox>> for HealthBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<HealthBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<HealthBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAsOffset<HealthBox> for HealthBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<HealthBox> {
                    let (v0,) = &self.0;
                    HealthBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [HealthBox].
            #[derive(Copy, Clone)]
            pub struct HealthBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> HealthBoxRef<'a> {
                /// Getter for the [`value` field](HealthBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<self::Health> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "HealthBox", "value")?
                            .unwrap_or(self::Health::Ok),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for HealthBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("HealthBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<HealthBoxRef<'a>> for HealthBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: HealthBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for HealthBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for HealthBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[HealthBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<HealthBox>> for HealthBox {
                type Value = ::planus::Offset<HealthBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<HealthBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for HealthBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[HealthBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `WarningFlagsBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `WarningFlagsBox` in the file `fb_demo.fbs:91`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct WarningFlagsBox {
                /// The field `value` in the table `WarningFlagsBox`
                pub value: u8,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for WarningFlagsBox {
                fn default() -> Self {
                    Self { value: 0 }
                }
            }

            impl WarningFlagsBox {
                /// Creates a [WarningFlagsBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> WarningFlagsBoxBuilder<()> {
                    WarningFlagsBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<u8, u8>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<u8>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 1>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<WarningFlagsBox>> for WarningFlagsBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<WarningFlagsBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<WarningFlagsBox>> for WarningFlagsBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<WarningFlagsBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<WarningFlagsBox> for WarningFlagsBox {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<WarningFlagsBox> {
                    WarningFlagsBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [WarningFlagsBox] type.
            ///
            /// Can be created using the [WarningFlagsBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct WarningFlagsBoxBuilder<State>(State);

            impl WarningFlagsBoxBuilder<()> {
                /// Setter for the [`value` field](WarningFlagsBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> WarningFlagsBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u8, u8>,
                {
                    WarningFlagsBoxBuilder((value,))
                }

                /// Sets the [`value` field](WarningFlagsBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> WarningFlagsBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> WarningFlagsBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [WarningFlagsBox].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<WarningFlagsBox>
                where
                    Self: ::planus::WriteAsOffset<WarningFlagsBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>>
                ::planus::WriteAs<::planus::Offset<WarningFlagsBox>>
                for WarningFlagsBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<WarningFlagsBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<WarningFlagsBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>>
                ::planus::WriteAsOptional<::planus::Offset<WarningFlagsBox>>
                for WarningFlagsBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<WarningFlagsBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<WarningFlagsBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<u8, u8>> ::planus::WriteAsOffset<WarningFlagsBox>
                for WarningFlagsBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<WarningFlagsBox> {
                    let (v0,) = &self.0;
                    WarningFlagsBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [WarningFlagsBox].
            #[derive(Copy, Clone)]
            pub struct WarningFlagsBoxRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> WarningFlagsBoxRef<'a> {
                /// Getter for the [`value` field](WarningFlagsBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "WarningFlagsBox", "value")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for WarningFlagsBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("WarningFlagsBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<WarningFlagsBoxRef<'a>> for WarningFlagsBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: WarningFlagsBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for WarningFlagsBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for WarningFlagsBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[WarningFlagsBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<WarningFlagsBox>> for WarningFlagsBox {
                type Value = ::planus::Offset<WarningFlagsBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<WarningFlagsBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for WarningFlagsBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[WarningFlagsBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `OutcomeBadBox` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `OutcomeBadBox` in the file `fb_demo.fbs:97`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct OutcomeBadBox {
                /// The field `value` in the table `OutcomeBadBox`
                pub value: self::Health,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for OutcomeBadBox {
                fn default() -> Self {
                    Self {
                        value: self::Health::Ok,
                    }
                }
            }

            impl OutcomeBadBox {
                /// Creates a [OutcomeBadBoxBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> OutcomeBadBoxBuilder<()> {
                    OutcomeBadBoxBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_value: impl ::planus::WriteAsDefault<self::Health, self::Health>,
                ) -> ::planus::Offset<Self> {
                    let prepared_value = field_value.prepare(builder, &self::Health::Ok);

                    let mut table_writer: ::planus::table_writer::TableWriter<6> =
                        ::core::default::Default::default();
                    if prepared_value.is_some() {
                        table_writer.write_entry::<self::Health>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 8>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<OutcomeBadBox>> for OutcomeBadBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<OutcomeBadBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<OutcomeBadBox>> for OutcomeBadBox {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<OutcomeBadBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<OutcomeBadBox> for OutcomeBadBox {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<OutcomeBadBox> {
                    OutcomeBadBox::create(builder, self.value)
                }
            }

            /// Builder for serializing an instance of the [OutcomeBadBox] type.
            ///
            /// Can be created using the [OutcomeBadBox::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct OutcomeBadBoxBuilder<State>(State);

            impl OutcomeBadBoxBuilder<()> {
                /// Setter for the [`value` field](OutcomeBadBox#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T0>(self, value: T0) -> OutcomeBadBoxBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<self::Health, self::Health>,
                {
                    OutcomeBadBoxBuilder((value,))
                }

                /// Sets the [`value` field](OutcomeBadBox#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(self) -> OutcomeBadBoxBuilder<(::planus::DefaultValue,)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0> OutcomeBadBoxBuilder<(T0,)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [OutcomeBadBox].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<OutcomeBadBox>
                where
                    Self: ::planus::WriteAsOffset<OutcomeBadBox>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAs<::planus::Offset<OutcomeBadBox>> for OutcomeBadBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<OutcomeBadBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<OutcomeBadBox> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAsOptional<::planus::Offset<OutcomeBadBox>>
                for OutcomeBadBoxBuilder<(T0,)>
            {
                type Prepared = ::planus::Offset<OutcomeBadBox>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<OutcomeBadBox>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<T0: ::planus::WriteAsDefault<self::Health, self::Health>>
                ::planus::WriteAsOffset<OutcomeBadBox> for OutcomeBadBoxBuilder<(T0,)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<OutcomeBadBox> {
                    let (v0,) = &self.0;
                    OutcomeBadBox::create(builder, v0)
                }
            }

            /// Reference to a deserialized [OutcomeBadBox].
            #[derive(Copy, Clone)]
            pub struct OutcomeBadBoxRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> OutcomeBadBoxRef<'a> {
                /// Getter for the [`value` field](OutcomeBadBox#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<self::Health> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "OutcomeBadBox", "value")?
                            .unwrap_or(self::Health::Ok),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for OutcomeBadBoxRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("OutcomeBadBoxRef");
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<OutcomeBadBoxRef<'a>> for OutcomeBadBox {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: OutcomeBadBoxRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for OutcomeBadBoxRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for OutcomeBadBoxRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[OutcomeBadBoxRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<OutcomeBadBox>> for OutcomeBadBox {
                type Value = ::planus::Offset<OutcomeBadBox>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<OutcomeBadBox>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for OutcomeBadBoxRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[OutcomeBadBoxRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `ReportRange` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `ReportRange` in the file `fb_demo.fbs:101`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct ReportRange {
                /// The field `field_1` in the table `ReportRange`
                pub field_1: u16,
                /// The field `field_2` in the table `ReportRange`
                pub field_2: u16,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ReportRange {
                fn default() -> Self {
                    Self {
                        field_1: 0,
                        field_2: 0,
                    }
                }
            }

            impl ReportRange {
                /// Creates a [ReportRangeBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ReportRangeBuilder<()> {
                    ReportRangeBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_field_1: impl ::planus::WriteAsDefault<u16, u16>,
                    field_field_2: impl ::planus::WriteAsDefault<u16, u16>,
                ) -> ::planus::Offset<Self> {
                    let prepared_field_1 = field_field_1.prepare(builder, &0);
                    let prepared_field_2 = field_field_2.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_field_1.is_some() {
                        table_writer.write_entry::<u16>(0);
                    }
                    if prepared_field_2.is_some() {
                        table_writer.write_entry::<u16>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_field_1) = prepared_field_1
                            {
                                object_writer.write::<_, _, 2>(&prepared_field_1);
                            }
                            if let ::core::option::Option::Some(prepared_field_2) = prepared_field_2
                            {
                                object_writer.write::<_, _, 2>(&prepared_field_2);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ReportRange>> for ReportRange {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportRange> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ReportRange>> for ReportRange {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportRange>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ReportRange> for ReportRange {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportRange> {
                    ReportRange::create(builder, self.field_1, self.field_2)
                }
            }

            /// Builder for serializing an instance of the [ReportRange] type.
            ///
            /// Can be created using the [ReportRange::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ReportRangeBuilder<State>(State);

            impl ReportRangeBuilder<()> {
                /// Setter for the [`field_1` field](ReportRange#structfield.field_1).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1<T0>(self, value: T0) -> ReportRangeBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u16, u16>,
                {
                    ReportRangeBuilder((value,))
                }

                /// Sets the [`field_1` field](ReportRange#structfield.field_1) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1_as_default(self) -> ReportRangeBuilder<(::planus::DefaultValue,)> {
                    self.field_1(::planus::DefaultValue)
                }
            }

            impl<T0> ReportRangeBuilder<(T0,)> {
                /// Setter for the [`field_2` field](ReportRange#structfield.field_2).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2<T1>(self, value: T1) -> ReportRangeBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u16, u16>,
                {
                    let (v0,) = self.0;
                    ReportRangeBuilder((v0, value))
                }

                /// Sets the [`field_2` field](ReportRange#structfield.field_2) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2_as_default(
                    self,
                ) -> ReportRangeBuilder<(T0, ::planus::DefaultValue)> {
                    self.field_2(::planus::DefaultValue)
                }
            }

            impl<T0, T1> ReportRangeBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ReportRange].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportRange>
                where
                    Self: ::planus::WriteAsOffset<ReportRange>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAs<::planus::Offset<ReportRange>>
                for ReportRangeBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportRange>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportRange> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOptional<::planus::Offset<ReportRange>>
                for ReportRangeBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportRange>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportRange>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOffset<ReportRange> for ReportRangeBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportRange> {
                    let (v0, v1) = &self.0;
                    ReportRange::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [ReportRange].
            #[derive(Copy, Clone)]
            pub struct ReportRangeRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> ReportRangeRef<'a> {
                /// Getter for the [`field_1` field](ReportRange#structfield.field_1).
                #[inline]
                pub fn field_1(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "ReportRange", "field_1")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`field_2` field](ReportRange#structfield.field_2).
                #[inline]
                pub fn field_2(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(
                        self.0.access(1, "ReportRange", "field_2")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for ReportRangeRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ReportRangeRef");
                    f.field("field_1", &self.field_1());
                    f.field("field_2", &self.field_2());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ReportRangeRef<'a>> for ReportRange {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ReportRangeRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        field_1: ::core::convert::TryInto::try_into(value.field_1()?)?,
                        field_2: ::core::convert::TryInto::try_into(value.field_2()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ReportRangeRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ReportRangeRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportRangeRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ReportRange>> for ReportRange {
                type Value = ::planus::Offset<ReportRange>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ReportRange>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for ReportRangeRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ReportRangeRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `ReportMetaEntry` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `ReportMetaEntry` in the file `fb_demo.fbs:108`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct ReportMetaEntry {
                /// The field `key` in the table `ReportMetaEntry`
                pub key: ::core::option::Option<::planus::alloc::string::String>,
                /// The field `value` in the table `ReportMetaEntry`
                pub value: u8,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ReportMetaEntry {
                fn default() -> Self {
                    Self {
                        key: ::core::default::Default::default(),
                        value: 0,
                    }
                }
            }

            impl ReportMetaEntry {
                /// Creates a [ReportMetaEntryBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ReportMetaEntryBuilder<()> {
                    ReportMetaEntryBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_key: impl ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    field_value: impl ::planus::WriteAsDefault<u8, u8>,
                ) -> ::planus::Offset<Self> {
                    let prepared_key = field_key.prepare(builder);
                    let prepared_value = field_value.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_key.is_some() {
                        table_writer.write_entry::<::planus::Offset<str>>(0);
                    }
                    if prepared_value.is_some() {
                        table_writer.write_entry::<u8>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_key) = prepared_key {
                                object_writer.write::<_, _, 4>(&prepared_key);
                            }
                            if let ::core::option::Option::Some(prepared_value) = prepared_value {
                                object_writer.write::<_, _, 1>(&prepared_value);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ReportMetaEntry>> for ReportMetaEntry {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportMetaEntry> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ReportMetaEntry>> for ReportMetaEntry {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportMetaEntry>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ReportMetaEntry> for ReportMetaEntry {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportMetaEntry> {
                    ReportMetaEntry::create(builder, &self.key, self.value)
                }
            }

            /// Builder for serializing an instance of the [ReportMetaEntry] type.
            ///
            /// Can be created using the [ReportMetaEntry::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ReportMetaEntryBuilder<State>(State);

            impl ReportMetaEntryBuilder<()> {
                /// Setter for the [`key` field](ReportMetaEntry#structfield.key).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn key<T0>(self, value: T0) -> ReportMetaEntryBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                {
                    ReportMetaEntryBuilder((value,))
                }

                /// Sets the [`key` field](ReportMetaEntry#structfield.key) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn key_as_null(self) -> ReportMetaEntryBuilder<((),)> {
                    self.key(())
                }
            }

            impl<T0> ReportMetaEntryBuilder<(T0,)> {
                /// Setter for the [`value` field](ReportMetaEntry#structfield.value).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value<T1>(self, value: T1) -> ReportMetaEntryBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u8, u8>,
                {
                    let (v0,) = self.0;
                    ReportMetaEntryBuilder((v0, value))
                }

                /// Sets the [`value` field](ReportMetaEntry#structfield.value) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn value_as_default(
                    self,
                ) -> ReportMetaEntryBuilder<(T0, ::planus::DefaultValue)> {
                    self.value(::planus::DefaultValue)
                }
            }

            impl<T0, T1> ReportMetaEntryBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ReportMetaEntry].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportMetaEntry>
                where
                    Self: ::planus::WriteAsOffset<ReportMetaEntry>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T1: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAs<::planus::Offset<ReportMetaEntry>>
                for ReportMetaEntryBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportMetaEntry>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportMetaEntry> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T1: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAsOptional<::planus::Offset<ReportMetaEntry>>
                for ReportMetaEntryBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportMetaEntry>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportMetaEntry>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsOptional<::planus::Offset<::core::primitive::str>>,
                    T1: ::planus::WriteAsDefault<u8, u8>,
                > ::planus::WriteAsOffset<ReportMetaEntry> for ReportMetaEntryBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportMetaEntry> {
                    let (v0, v1) = &self.0;
                    ReportMetaEntry::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [ReportMetaEntry].
            #[derive(Copy, Clone)]
            pub struct ReportMetaEntryRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> ReportMetaEntryRef<'a> {
                /// Getter for the [`key` field](ReportMetaEntry#structfield.key).
                #[inline]
                pub fn key(
                    &self,
                ) -> ::planus::Result<::core::option::Option<&'a ::core::primitive::str>>
                {
                    self.0.access(0, "ReportMetaEntry", "key")
                }

                /// Getter for the [`value` field](ReportMetaEntry#structfield.value).
                #[inline]
                pub fn value(&self) -> ::planus::Result<u8> {
                    ::core::result::Result::Ok(
                        self.0.access(1, "ReportMetaEntry", "value")?.unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for ReportMetaEntryRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ReportMetaEntryRef");
                    if let ::core::option::Option::Some(field_key) = self.key().transpose() {
                        f.field("key", &field_key);
                    }
                    f.field("value", &self.value());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ReportMetaEntryRef<'a>> for ReportMetaEntry {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ReportMetaEntryRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        key: value.key()?.map(::core::convert::Into::into),
                        value: ::core::convert::TryInto::try_into(value.value()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ReportMetaEntryRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ReportMetaEntryRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportMetaEntryRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ReportMetaEntry>> for ReportMetaEntry {
                type Value = ::planus::Offset<ReportMetaEntry>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ReportMetaEntry>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for ReportMetaEntryRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ReportMetaEntryRef]", "read_as_root", 0)
                    })
                }
            }

            /// The table `ReportPointsElement` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `ReportPointsElement` in the file `fb_demo.fbs:115`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct ReportPointsElement {
                /// The field `field_1` in the table `ReportPointsElement`
                pub field_1: u16,
                /// The field `field_2` in the table `ReportPointsElement`
                pub field_2: u16,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ReportPointsElement {
                fn default() -> Self {
                    Self {
                        field_1: 0,
                        field_2: 0,
                    }
                }
            }

            impl ReportPointsElement {
                /// Creates a [ReportPointsElementBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ReportPointsElementBuilder<()> {
                    ReportPointsElementBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_field_1: impl ::planus::WriteAsDefault<u16, u16>,
                    field_field_2: impl ::planus::WriteAsDefault<u16, u16>,
                ) -> ::planus::Offset<Self> {
                    let prepared_field_1 = field_field_1.prepare(builder, &0);
                    let prepared_field_2 = field_field_2.prepare(builder, &0);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_field_1.is_some() {
                        table_writer.write_entry::<u16>(0);
                    }
                    if prepared_field_2.is_some() {
                        table_writer.write_entry::<u16>(1);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_field_1) = prepared_field_1
                            {
                                object_writer.write::<_, _, 2>(&prepared_field_1);
                            }
                            if let ::core::option::Option::Some(prepared_field_2) = prepared_field_2
                            {
                                object_writer.write::<_, _, 2>(&prepared_field_2);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ReportPointsElement>> for ReportPointsElement {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportPointsElement> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ReportPointsElement>> for ReportPointsElement {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportPointsElement>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ReportPointsElement> for ReportPointsElement {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportPointsElement> {
                    ReportPointsElement::create(builder, self.field_1, self.field_2)
                }
            }

            /// Builder for serializing an instance of the [ReportPointsElement] type.
            ///
            /// Can be created using the [ReportPointsElement::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ReportPointsElementBuilder<State>(State);

            impl ReportPointsElementBuilder<()> {
                /// Setter for the [`field_1` field](ReportPointsElement#structfield.field_1).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1<T0>(self, value: T0) -> ReportPointsElementBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u16, u16>,
                {
                    ReportPointsElementBuilder((value,))
                }

                /// Sets the [`field_1` field](ReportPointsElement#structfield.field_1) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1_as_default(
                    self,
                ) -> ReportPointsElementBuilder<(::planus::DefaultValue,)> {
                    self.field_1(::planus::DefaultValue)
                }
            }

            impl<T0> ReportPointsElementBuilder<(T0,)> {
                /// Setter for the [`field_2` field](ReportPointsElement#structfield.field_2).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2<T1>(self, value: T1) -> ReportPointsElementBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsDefault<u16, u16>,
                {
                    let (v0,) = self.0;
                    ReportPointsElementBuilder((v0, value))
                }

                /// Sets the [`field_2` field](ReportPointsElement#structfield.field_2) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2_as_default(
                    self,
                ) -> ReportPointsElementBuilder<(T0, ::planus::DefaultValue)> {
                    self.field_2(::planus::DefaultValue)
                }
            }

            impl<T0, T1> ReportPointsElementBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ReportPointsElement].
                #[inline]
                pub fn finish(
                    self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportPointsElement>
                where
                    Self: ::planus::WriteAsOffset<ReportPointsElement>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAs<::planus::Offset<ReportPointsElement>>
                for ReportPointsElementBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportPointsElement>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportPointsElement> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOptional<::planus::Offset<ReportPointsElement>>
                for ReportPointsElementBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportPointsElement>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportPointsElement>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsDefault<u16, u16>,
                > ::planus::WriteAsOffset<ReportPointsElement>
                for ReportPointsElementBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::planus::Offset<ReportPointsElement> {
                    let (v0, v1) = &self.0;
                    ReportPointsElement::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [ReportPointsElement].
            #[derive(Copy, Clone)]
            pub struct ReportPointsElementRef<'a>(
                #[allow(dead_code)] ::planus::table_reader::Table<'a>,
            );

            impl<'a> ReportPointsElementRef<'a> {
                /// Getter for the [`field_1` field](ReportPointsElement#structfield.field_1).
                #[inline]
                pub fn field_1(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(0, "ReportPointsElement", "field_1")?
                            .unwrap_or(0),
                    )
                }

                /// Getter for the [`field_2` field](ReportPointsElement#structfield.field_2).
                #[inline]
                pub fn field_2(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(
                        self.0
                            .access(1, "ReportPointsElement", "field_2")?
                            .unwrap_or(0),
                    )
                }
            }

            impl<'a> ::core::fmt::Debug for ReportPointsElementRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ReportPointsElementRef");
                    f.field("field_1", &self.field_1());
                    f.field("field_2", &self.field_2());
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ReportPointsElementRef<'a>> for ReportPointsElement {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ReportPointsElementRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        field_1: ::core::convert::TryInto::try_into(value.field_1()?)?,
                        field_2: ::core::convert::TryInto::try_into(value.field_2()?)?,
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ReportPointsElementRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ReportPointsElementRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportPointsElementRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ReportPointsElement>> for ReportPointsElement {
                type Value = ::planus::Offset<ReportPointsElement>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ReportPointsElement>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for ReportPointsElementRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportPointsElementRef]",
                            "read_as_root",
                            0,
                        )
                    })
                }
            }

            /// The table `ReportPair` in the namespace `fb.demo`
            ///
            /// Generated from these locations:
            /// * Table `ReportPair` in the file `fb_demo.fbs:122`
            #[derive(
                Clone,
                Debug,
                PartialEq,
                PartialOrd,
                Eq,
                Ord,
                Hash,
                ::serde::Serialize,
                ::serde::Deserialize,
            )]
            pub struct ReportPair {
                /// The field `field_1` in the table `ReportPair`
                pub field_1: u16,
                /// The field `field_2` in the table `ReportPair`
                pub field_2: ::core::option::Option<::planus::alloc::boxed::Box<self::Inner>>,
            }

            #[allow(clippy::derivable_impls)]
            impl ::core::default::Default for ReportPair {
                fn default() -> Self {
                    Self {
                        field_1: 0,
                        field_2: ::core::default::Default::default(),
                    }
                }
            }

            impl ReportPair {
                /// Creates a [ReportPairBuilder] for serializing an instance of this table.
                #[inline]
                pub fn builder() -> ReportPairBuilder<()> {
                    ReportPairBuilder(())
                }

                #[allow(clippy::too_many_arguments)]
                pub fn create(
                    builder: &mut ::planus::Builder,
                    field_field_1: impl ::planus::WriteAsDefault<u16, u16>,
                    field_field_2: impl ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                ) -> ::planus::Offset<Self> {
                    let prepared_field_1 = field_field_1.prepare(builder, &0);
                    let prepared_field_2 = field_field_2.prepare(builder);

                    let mut table_writer: ::planus::table_writer::TableWriter<8> =
                        ::core::default::Default::default();
                    if prepared_field_2.is_some() {
                        table_writer.write_entry::<::planus::Offset<self::Inner>>(1);
                    }
                    if prepared_field_1.is_some() {
                        table_writer.write_entry::<u16>(0);
                    }

                    unsafe {
                        table_writer.finish(builder, |object_writer| {
                            if let ::core::option::Option::Some(prepared_field_2) = prepared_field_2
                            {
                                object_writer.write::<_, _, 4>(&prepared_field_2);
                            }
                            if let ::core::option::Option::Some(prepared_field_1) = prepared_field_1
                            {
                                object_writer.write::<_, _, 2>(&prepared_field_1);
                            }
                        });
                    }
                    builder.current_offset()
                }
            }

            impl ::planus::WriteAs<::planus::Offset<ReportPair>> for ReportPair {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ReportPair> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl ::planus::WriteAsOptional<::planus::Offset<ReportPair>> for ReportPair {
                type Prepared = ::planus::Offset<Self>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportPair>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl ::planus::WriteAsOffset<ReportPair> for ReportPair {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ReportPair> {
                    ReportPair::create(builder, self.field_1, &self.field_2)
                }
            }

            /// Builder for serializing an instance of the [ReportPair] type.
            ///
            /// Can be created using the [ReportPair::builder] method.
            #[derive(Debug)]
            #[must_use]
            pub struct ReportPairBuilder<State>(State);

            impl ReportPairBuilder<()> {
                /// Setter for the [`field_1` field](ReportPair#structfield.field_1).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1<T0>(self, value: T0) -> ReportPairBuilder<(T0,)>
                where
                    T0: ::planus::WriteAsDefault<u16, u16>,
                {
                    ReportPairBuilder((value,))
                }

                /// Sets the [`field_1` field](ReportPair#structfield.field_1) to the default value.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_1_as_default(self) -> ReportPairBuilder<(::planus::DefaultValue,)> {
                    self.field_1(::planus::DefaultValue)
                }
            }

            impl<T0> ReportPairBuilder<(T0,)> {
                /// Setter for the [`field_2` field](ReportPair#structfield.field_2).
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2<T1>(self, value: T1) -> ReportPairBuilder<(T0, T1)>
                where
                    T1: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                {
                    let (v0,) = self.0;
                    ReportPairBuilder((v0, value))
                }

                /// Sets the [`field_2` field](ReportPair#structfield.field_2) to null.
                #[inline]
                #[allow(clippy::type_complexity)]
                pub fn field_2_as_null(self) -> ReportPairBuilder<(T0, ())> {
                    self.field_2(())
                }
            }

            impl<T0, T1> ReportPairBuilder<(T0, T1)> {
                /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [ReportPair].
                #[inline]
                pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<ReportPair>
                where
                    Self: ::planus::WriteAsOffset<ReportPair>,
                {
                    ::planus::WriteAsOffset::prepare(&self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                > ::planus::WriteAs<::planus::Offset<ReportPair>> for ReportPairBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportPair>;

                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ReportPair> {
                    ::planus::WriteAsOffset::prepare(self, builder)
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                > ::planus::WriteAsOptional<::planus::Offset<ReportPair>>
                for ReportPairBuilder<(T0, T1)>
            {
                type Prepared = ::planus::Offset<ReportPair>;

                #[inline]
                fn prepare(
                    &self,
                    builder: &mut ::planus::Builder,
                ) -> ::core::option::Option<::planus::Offset<ReportPair>> {
                    ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
                }
            }

            impl<
                    T0: ::planus::WriteAsDefault<u16, u16>,
                    T1: ::planus::WriteAsOptional<::planus::Offset<self::Inner>>,
                > ::planus::WriteAsOffset<ReportPair> for ReportPairBuilder<(T0, T1)>
            {
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<ReportPair> {
                    let (v0, v1) = &self.0;
                    ReportPair::create(builder, v0, v1)
                }
            }

            /// Reference to a deserialized [ReportPair].
            #[derive(Copy, Clone)]
            pub struct ReportPairRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

            impl<'a> ReportPairRef<'a> {
                /// Getter for the [`field_1` field](ReportPair#structfield.field_1).
                #[inline]
                pub fn field_1(&self) -> ::planus::Result<u16> {
                    ::core::result::Result::Ok(
                        self.0.access(0, "ReportPair", "field_1")?.unwrap_or(0),
                    )
                }

                /// Getter for the [`field_2` field](ReportPair#structfield.field_2).
                #[inline]
                pub fn field_2(
                    &self,
                ) -> ::planus::Result<::core::option::Option<self::InnerRef<'a>>> {
                    self.0.access(1, "ReportPair", "field_2")
                }
            }

            impl<'a> ::core::fmt::Debug for ReportPairRef<'a> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    let mut f = f.debug_struct("ReportPairRef");
                    f.field("field_1", &self.field_1());
                    if let ::core::option::Option::Some(field_field_2) = self.field_2().transpose()
                    {
                        f.field("field_2", &field_field_2);
                    }
                    f.finish()
                }
            }

            impl<'a> ::core::convert::TryFrom<ReportPairRef<'a>> for ReportPair {
                type Error = ::planus::Error;

                #[allow(unreachable_code)]
                fn try_from(value: ReportPairRef<'a>) -> ::planus::Result<Self> {
                    ::core::result::Result::Ok(Self {
                        field_1: ::core::convert::TryInto::try_into(value.field_1()?)?,
                        field_2: if let ::core::option::Option::Some(field_2) = value.field_2()? {
                            ::core::option::Option::Some(::planus::alloc::boxed::Box::new(
                                ::core::convert::TryInto::try_into(field_2)?,
                            ))
                        } else {
                            ::core::option::Option::None
                        },
                    })
                }
            }

            impl<'a> ::planus::TableRead<'a> for ReportPairRef<'a> {
                #[inline]
                fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                    ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                        buffer, offset,
                    )?))
                }
            }

            impl<'a> ::planus::VectorReadInner<'a> for ReportPairRef<'a> {
                type Error = ::planus::Error;
                const STRIDE: usize = 4;

                unsafe fn from_buffer(
                    buffer: ::planus::SliceWithStartOffset<'a>,
                    offset: usize,
                ) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                        error_kind.with_error_location(
                            "[ReportPairRef]",
                            "get",
                            buffer.offset_from_start,
                        )
                    })
                }
            }

            /// # Safety
            /// The planus compiler generates implementations that initialize
            /// the bytes in `write_values`.
            unsafe impl ::planus::VectorWrite<::planus::Offset<ReportPair>> for ReportPair {
                type Value = ::planus::Offset<ReportPair>;
                const STRIDE: usize = 4;
                #[inline]
                fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                    ::planus::WriteAs::prepare(self, builder)
                }

                #[inline]
                unsafe fn write_values(
                    values: &[::planus::Offset<ReportPair>],
                    bytes: *mut ::core::mem::MaybeUninit<u8>,
                    buffer_position: u32,
                ) {
                    let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                    for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                        ::planus::WriteAsPrimitive::write(
                            v,
                            ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                            buffer_position - (Self::STRIDE * i) as u32,
                        );
                    }
                }
            }

            impl<'a> ::planus::ReadAsRoot<'a> for ReportPairRef<'a> {
                fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                    ::planus::TableRead::from_buffer(
                        ::planus::SliceWithStartOffset {
                            buffer: slice,
                            offset_from_start: 0,
                        },
                        0,
                    )
                    .map_err(|error_kind| {
                        error_kind.with_error_location("[ReportPairRef]", "read_as_root", 0)
                    })
                }
            }
        }
    }
}
