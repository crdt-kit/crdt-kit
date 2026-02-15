//! Proc macros for `crdt-migrate`.
//!
//! Provides two macros:
//!
//! - **`#[crdt_schema]`** — Attribute macro that generates `CrdtVersioned` and
//!   `Schema` implementations for a struct.
//!
//! - **`#[migration]`** — Attribute macro that wraps a migration function into
//!   a `MigrationStep` implementation.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, ItemFn, ItemStruct, Meta};

/// Attribute macro that generates schema + versioning impls for a struct.
///
/// # Attributes
///
/// - `version = N` — **Required.** The schema version number.
/// - `table = "name"` — **Required.** The storage namespace/table name.
/// - `min_version = N` — Optional. Minimum supported version (defaults to 1).
///
/// # Generated Implementations
///
/// - `crdt_store::CrdtVersioned` with `SCHEMA_VERSION = version`
/// - `crdt_migrate::Schema` with `VERSION`, `MIN_SUPPORTED_VERSION`, `NAMESPACE`
///
/// # Example
///
/// ```ignore
/// use crdt_migrate_macros::crdt_schema;
/// use serde::{Serialize, Deserialize};
///
/// #[crdt_schema(version = 1, table = "sensors")]
/// #[derive(Debug, Serialize, Deserialize)]
/// struct SensorData {
///     device_id: String,
///     temperature: f64,
/// }
/// ```
#[proc_macro_attribute]
pub fn crdt_schema(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let args = parse_macro_input!(attr with Punctuated::<Meta, Comma>::parse_terminated);

    let mut version: Option<u32> = None;
    let mut table: Option<String> = None;
    let mut min_version: Option<u32> = None;

    for meta in &args {
        if let Meta::NameValue(nv) = meta {
            let key = nv
                .path
                .get_ident()
                .map(|i| i.to_string())
                .unwrap_or_default();
            match key.as_str() {
                "version" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit),
                        ..
                    }) = &nv.value
                    {
                        version = lit.base10_parse().ok();
                    }
                }
                "table" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit),
                        ..
                    }) = &nv.value
                    {
                        table = Some(lit.value());
                    }
                }
                "min_version" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit),
                        ..
                    }) = &nv.value
                    {
                        min_version = lit.base10_parse().ok();
                    }
                }
                _ => {
                    return syn::Error::new_spanned(&nv.path, format!("unknown attribute `{key}`"))
                        .to_compile_error()
                        .into();
                }
            }
        }
    }

    let version = match version {
        Some(v) => v,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required attribute `version`",
            )
            .to_compile_error()
            .into();
        }
    };

    let table = match table {
        Some(t) => t,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required attribute `table`",
            )
            .to_compile_error()
            .into();
        }
    };

    let min_ver = min_version.unwrap_or(1);
    let version_u8 = version as u8;
    let struct_name = &input.ident;

    let expanded = quote! {
        #input

        impl crdt_store::CrdtVersioned for #struct_name {
            const SCHEMA_VERSION: u8 = #version_u8;
        }

        impl crdt_migrate::Schema for #struct_name {
            const VERSION: u32 = #version;
            const MIN_SUPPORTED_VERSION: u32 = #min_ver;
            const NAMESPACE: &'static str = #table;
        }
    };

    expanded.into()
}

/// Attribute macro that wraps a migration function into a `MigrationStep`.
///
/// The function must take a single argument (the old version's data) and return
/// the new version's data. Both types must implement `Serialize` and `DeserializeOwned`.
///
/// # Attributes
///
/// - `from = N` — **Required.** Source schema version.
/// - `to = M` — **Required.** Target schema version.
///
/// # Generated Code
///
/// Creates a struct `{FnName}Migration` that implements `MigrationStep`.
/// The struct handles deserialization of the old format, calls your function,
/// and serializes the result.
///
/// Also generates a `register_{fn_name}` function that returns a boxed
/// `MigrationStep` for convenient registration.
///
/// # Example
///
/// ```ignore
/// use crdt_migrate_macros::migration;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct SensorV1 { temperature: f32 }
///
/// #[derive(Serialize, Deserialize)]
/// struct SensorV2 { temperature: f32, humidity: Option<f32> }
///
/// #[migration(from = 1, to = 2)]
/// fn add_humidity(old: SensorV1) -> SensorV2 {
///     SensorV2 {
///         temperature: old.temperature,
///         humidity: None,
///     }
/// }
/// // Generates: AddHumidityMigration struct + impl MigrationStep
/// // Generates: fn register_add_humidity() -> Box<dyn MigrationStep>
/// ```
#[proc_macro_attribute]
pub fn migration(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(attr with Punctuated::<Meta, Comma>::parse_terminated);

    let mut from_version: Option<u32> = None;
    let mut to_version: Option<u32> = None;

    for meta in &args {
        if let Meta::NameValue(nv) = meta {
            let key = nv
                .path
                .get_ident()
                .map(|i| i.to_string())
                .unwrap_or_default();
            match key.as_str() {
                "from" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit),
                        ..
                    }) = &nv.value
                    {
                        from_version = lit.base10_parse().ok();
                    }
                }
                "to" => {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit),
                        ..
                    }) = &nv.value
                    {
                        to_version = lit.base10_parse().ok();
                    }
                }
                _ => {
                    return syn::Error::new_spanned(&nv.path, format!("unknown attribute `{key}`"))
                        .to_compile_error()
                        .into();
                }
            }
        }
    }

    let from_ver = match from_version {
        Some(v) => v,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required attribute `from`",
            )
            .to_compile_error()
            .into();
        }
    };

    let to_ver = match to_version {
        Some(v) => v,
        None => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing required attribute `to`",
            )
            .to_compile_error()
            .into();
        }
    };

    let fn_name = &input.sig.ident;

    // Extract the input type from the function signature
    let input_type = match input.sig.inputs.first() {
        Some(syn::FnArg::Typed(pat_type)) => &pat_type.ty,
        _ => {
            return syn::Error::new_spanned(
                &input.sig,
                "migration function must take exactly one argument",
            )
            .to_compile_error()
            .into();
        }
    };

    // Extract the output type
    let output_type = match &input.sig.output {
        syn::ReturnType::Type(_, ty) => ty,
        syn::ReturnType::Default => {
            return syn::Error::new_spanned(
                &input.sig,
                "migration function must have a return type",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate struct name: snake_case -> PascalCase + "Migration"
    let struct_name = {
        let name = fn_name.to_string();
        let pascal: String = name
            .split('_')
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect();
        syn::Ident::new(&format!("{pascal}Migration"), fn_name.span())
    };

    // Generate register function name
    let register_fn = syn::Ident::new(&format!("register_{fn_name}"), fn_name.span());

    let expanded = quote! {
        #input

        /// Auto-generated migration step struct.
        pub struct #struct_name;

        impl crdt_migrate::MigrationStep for #struct_name {
            fn source_version(&self) -> u32 {
                #from_ver
            }

            fn target_version(&self) -> u32 {
                #to_ver
            }

            fn migrate(&self, data: &[u8]) -> Result<Vec<u8>, crdt_migrate::MigrationError> {
                let old: #input_type = postcard::from_bytes(data)
                    .map_err(|e| crdt_migrate::MigrationError::Deserialization(
                        e.to_string()
                    ))?;
                let new: #output_type = #fn_name(old);
                postcard::to_allocvec(&new)
                    .map_err(|e| crdt_migrate::MigrationError::Serialization(
                        e.to_string()
                    ))
            }
        }

        /// Register this migration step for use with `MigrationEngine`.
        pub fn #register_fn() -> Box<dyn crdt_migrate::MigrationStep> {
            Box::new(#struct_name)
        }
    };

    expanded.into()
}
