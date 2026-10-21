//! The `wasm` module of a generated crate: the raw exports a WebAssembly host
//! calls to check FlatBuffers payload bytes in the generated Rust codec.
//!
//! The module is one per crate, not one per package, because its exports are
//! unmangled symbols: two packages that each emitted `ridl_alloc` would not
//! link. `ridlc` calls [`generate_wasm_module`] once with the lowered model of
//! every package of the crate, and writes the result beside the crate root
//! when `[backend.rust] wasm = true`.
//!
//! The exports are `ridl_alloc`, `ridl_free`, `ridl_catalog_hash` and
//! `ridl_verify`, and no other. They are compiled only for
//! `target_arch = "wasm32"` with the generated crate's feature `wasm-exports`
//! on, so a Rust consumer that links the crate for wasm32 gets no second copy
//! of the symbols. The two lists a host needs to read the results,
//! [`PAYLOAD_TYPES_CONST`] and [`RULES_CONST`], are compiled for every target,
//! so a native build and a test can read them.

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};
use ridl_ir::codegen::v1;

use crate::{Ctx, GenerateError, Generated, declared, ident, package_prefix, render, tuple_name};

/// The name of the constant that lists the payload types, sorted, so that a
/// `type_id` is an index into it.
pub const PAYLOAD_TYPES_CONST: &str = "WASM_PAYLOAD_TYPES";

/// The name of the constant that lists the typl rules, sorted, so that a
/// rule code of 2 or more is 2 plus an index into it.
pub const RULES_CONST: &str = "WASM_RULES";

/// The typl rules `ridl_rt::payload::Rule` names, sorted by their text, with
/// the variant each text stands for.
const RULES: [(&str, &str); 6] = [
    ("length", "Length"),
    ("pattern", "Pattern"),
    ("range", "Range"),
    ("step", "Step"),
    ("unique", "Unique"),
    ("variant", "Variant"),
];

/// The alignment of a buffer `ridl_alloc` returns. It is the alignment the
/// FlatBuffers codec finishes a buffer at, so a buffer the host copies in is
/// aligned as the encoder wrote it.
const BUFFER_ALIGN: usize = 8;

/// One payload type of the crate: its qualified ridl name (`veh.cabin.Level`),
/// which orders the list, and the Rust path of its type.
struct PayloadType {
    name: String,
    path: TokenStream,
}

/// The source of the `wasm` module for a crate whose packages lowered to
/// `models`, with `catalog_hash` as the value `ridl_catalog_hash` writes.
///
/// The payload types are every type of `models` that has a
/// `Payload<FlatBuffers>` implementation: a declaration or an induced tuple
/// that the FlatBuffers projection gives a root with a finite bound, which is
/// the selection the codec makes. A model of the same package given twice
/// adds its types once.
///
/// Refuses a `catalog_hash` that is not 32 bytes.
pub fn generate_wasm_module(
    catalog_hash: &[u8],
    models: &[&v1::Model],
) -> Result<Generated, GenerateError> {
    if catalog_hash.len() != 32 {
        return Err(GenerateError {
            message: format!(
                "the catalog hash for the `wasm` module is {} bytes, not 32",
                catalog_hash.len()
            ),
        });
    }
    let mut types: Vec<PayloadType> = Vec::new();
    for model in models {
        types.extend(payload_types(model));
    }
    types.sort_by(|a, b| a.name.cmp(&b.name));
    types.dedup_by(|a, b| a.name == b.name);

    let names = types.iter().map(|ty| ty.name.as_str());
    let arms = types.iter().enumerate().map(|(index, ty)| {
        let index = Literal::u32_unsuffixed(index as u32);
        let path = &ty.path;
        quote! { #index => verify_as::<#path>(bytes), }
    });
    let rule_texts = RULES.iter().map(|(text, _)| *text);
    let rule_arms = RULES.iter().enumerate().map(|(index, (_, variant))| {
        let variant = format_ident!("{variant}");
        let code = Literal::i32_unsuffixed(2 + index as i32);
        quote! { ::ridl_rt::payload::Rule::#variant => #code, }
    });
    let hash = catalog_hash
        .iter()
        .map(|byte| Literal::u8_unsuffixed(*byte));
    let types_const = format_ident!("{PAYLOAD_TYPES_CONST}");
    let rules_const = format_ident!("{RULES_CONST}");
    let align = Literal::usize_unsuffixed(BUFFER_ALIGN);

    render(vec![quote! {
        /// The payload types `ridl_verify` checks, sorted by qualified name.
        /// A `type_id` is an index into this list.
        pub const #types_const: &[&str] = &[#(#names),*];

        /// The typl rules a `ridl_verify` result names, sorted. A result of 2
        /// or more is 2 plus an index into this list; a result of 2 plus its
        /// length is a rule this list does not name.
        pub const #rules_const: &[&str] = &[#(#rule_texts),*];

        /// The result code `ridl_verify` returns for `bytes` read as the
        /// payload type at `type_id`. Compiled for every target, so that a
        /// native test checks the dispatch the export runs; not a stable API.
        #[doc(hidden)]
        #[allow(deprecated)]
        pub fn verify_by_id(type_id: ::core::primitive::u32, bytes: &[::core::primitive::u8]) -> ::core::primitive::i32 {
            match type_id {
                #(#arms)*
                _ => {
                    let _ = bytes;
                    -1
                }
            }
        }

        /// The result code of verifying `bytes` as a `T`.
        #[allow(dead_code)]
        fn verify_as<T: ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>(
            bytes: &[::core::primitive::u8],
        ) -> ::core::primitive::i32 {
            verify_result_code(<T as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>::verify(bytes).map(|_| ()))
        }

        /// The result code of one verification: 0, 1 for a malformed buffer,
        /// or the code of the broken rule. Not a stable API.
        #[doc(hidden)]
        pub fn verify_result_code(
            result: ::core::result::Result<(), ::ridl_rt::payload::VerifyError>,
        ) -> ::core::primitive::i32 {
            match result {
                ::core::result::Result::Ok(()) => 0,
                ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Structure(_)) => 1,
                ::core::result::Result::Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                    rule_code(violation.rule)
                }
                // A failure `VerifyError` adds later is not a typl rule,
                // so it is reported as a buffer that does not verify.
                ::core::result::Result::Err(_) => 1,
            }
        }

        /// The result code of a broken rule: 2 plus its index in
        /// `WASM_RULES`, or 2 plus its length for a rule the list does not
        /// name. Not a stable API.
        #[doc(hidden)]
        pub fn rule_code(rule: ::ridl_rt::payload::Rule) -> ::core::primitive::i32 {
            match rule {
                #(#rule_arms)*
                _ => 2 + #rules_const.len() as ::core::primitive::i32,
            }
        }

        #[cfg(all(target_arch = "wasm32", feature = "wasm-exports"))]
        mod exports {
            /// The catalog hash of the unit, the value of every
            /// `Interface::CATALOG` the unit generates.
            const CATALOG_HASH: [::core::primitive::u8; 32] = [#(#hash),*];

            /// The layout of a buffer of `len` bytes, or `None` when `len` is
            /// too large for one.
            fn layout(len: ::core::primitive::usize) -> ::core::option::Option<::std::alloc::Layout> {
                ::std::alloc::Layout::from_size_align(len, #align).ok()
            }

            /// Allocates `len` bytes in the module's memory, aligned to 8.
            /// Returns null when `len` is 0 or the allocation fails.
            #[unsafe(no_mangle)]
            pub extern "C" fn ridl_alloc(len: ::core::primitive::usize) -> *mut ::core::primitive::u8 {
                match layout(len) {
                    ::core::option::Option::Some(layout) if len > 0 => {
                        // SAFETY: the layout has a size that is not zero.
                        unsafe { ::std::alloc::alloc(layout) }
                    }
                    _ => ::core::ptr::null_mut(),
                }
            }

            /// Frees a buffer that `ridl_alloc(len)` returned.
            ///
            /// # Safety
            ///
            /// `ptr` is null, or was returned by `ridl_alloc` with the same
            /// `len` and is not freed yet.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn ridl_free(ptr: *mut ::core::primitive::u8, len: ::core::primitive::usize) {
                if ptr.is_null() || len == 0 {
                    return;
                }
                if let ::core::option::Option::Some(layout) = layout(len) {
                    // SAFETY: the caller passes a buffer `ridl_alloc` returned
                    // for `len`, which used this layout.
                    unsafe { ::std::alloc::dealloc(ptr, layout) }
                }
            }

            /// Writes the 32-byte catalog hash of the unit to `out_ptr`.
            ///
            /// # Safety
            ///
            /// `out_ptr` is null, which writes nothing, or is valid for a
            /// write of 32 bytes.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn ridl_catalog_hash(out_ptr: *mut ::core::primitive::u8) {
                if out_ptr.is_null() {
                    return;
                }
                // SAFETY: the caller passes a buffer of 32 writable bytes.
                unsafe { ::core::ptr::copy_nonoverlapping(CATALOG_HASH.as_ptr(), out_ptr, 32) }
            }

            /// Checks the `len` bytes at `ptr` as the payload type `type_id`
            /// names in `WASM_PAYLOAD_TYPES`. Returns 0 when the bytes verify,
            /// 1 for a malformed buffer, 2 or more for a broken typl rule
            /// (`WASM_RULES`), and -1 for an unknown `type_id`. A null `ptr`
            /// with a `len` that is not 0, a `len` above `isize::MAX`, and a
            /// range that passes the end of the address space return 1
            /// without reading.
            ///
            /// # Safety
            ///
            /// `ptr` is valid for a read of `len` bytes, or `len` is 0.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn ridl_verify(
                type_id: ::core::primitive::u32,
                ptr: *const ::core::primitive::u8,
                len: ::core::primitive::usize,
            ) -> ::core::primitive::i32 {
                if len > 0
                    && (ptr.is_null()
                        || len > ::core::primitive::isize::MAX as ::core::primitive::usize
                        || (ptr as ::core::primitive::usize).checked_add(len).is_none())
                {
                    return 1;
                }
                let bytes: &[::core::primitive::u8] = if len == 0 {
                    &[]
                } else {
                    // SAFETY: the caller passes `len` readable bytes at `ptr`.
                    unsafe { ::core::slice::from_raw_parts(ptr, len) }
                };
                super::verify_by_id(type_id, bytes)
            }

        }
    }])
}

/// The payload types of one model, unsorted: each root of the FlatBuffers
/// projection that has a finite bound, which is the set the codec gives a
/// `Payload<FlatBuffers>` implementation.
fn payload_types(model: &v1::Model) -> Vec<PayloadType> {
    // The context installs the build's package list, which the path of a type
    // whose module a child package hides reads.
    let ctx = Ctx::over(model);
    let package = ctx.package_name();
    let Some(projection) = model.flatbuffers.as_ref() else {
        return Vec::new();
    };
    projection
        .roots
        .iter()
        .filter(|root| matches!(root.bound, Some(v1::fb_root::Bound::MaxSize(_))))
        .filter_map(|root| {
            let owner = match root.tuple {
                Some(index) => tuple_name(ctx.tuple(index)?),
                None => declared(ctx.declaration(root.declaration)?.name.as_ref()),
            };
            if owner.is_empty() {
                return None;
            }
            let name = ident(owner);
            let prefix = package_prefix(package, &name);
            Some(PayloadType {
                name: format!("{package}.{owner}"),
                path: quote! { #prefix #name },
            })
        })
        .collect()
}
