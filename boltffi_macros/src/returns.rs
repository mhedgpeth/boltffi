use boltffi_ffi_rules::primitive::Primitive;
pub use boltffi_ffi_rules::transport::{
    EncodedReturnStrategy, ErrorReturnStrategy, ReturnInvocationContext, ReturnPlatform,
    ScalarReturnStrategy, ValueReturnMethod, ValueReturnStrategy,
};
use proc_macro2::Span;
use quote::quote;
use syn::{ReturnType, Type};

use crate::custom_types::{self, CustomTypeRegistry};
use crate::data_types::{DataTypeCategory, DataTypeRegistry};
use crate::type_classification::{
    NamedTypeTransport, classify_named_type_transport, supports_direct_vec_transport,
};

pub enum OptionReturnAbi {
    OutValue { inner: syn::Type },
    OutFfiString,
    Vec { inner: syn::Type },
}

#[allow(clippy::large_enum_variant)]
pub enum ReturnKind {
    Unit,
    Primitive(syn::Type),
    String,
    ResultPrimitive { ok: syn::Type, err: syn::Type },
    ResultString { err: syn::Type },
    ResultUnit { err: syn::Type },
    Vec(syn::Type),
    Option(OptionReturnAbi),
    WireEncoded(syn::Type),
}

pub enum ReturnAbi {
    Unit,
    Scalar {
        rust_type: syn::Type,
    },
    Encoded {
        rust_type: syn::Type,
        strategy: EncodedReturnStrategy,
    },
    Passable {
        rust_type: syn::Type,
    },
}

#[derive(Clone, Copy)]
pub struct ReturnLoweringContext<'a> {
    custom_types: &'a CustomTypeRegistry,
    data_types: &'a DataTypeRegistry,
}

impl<'a> ReturnLoweringContext<'a> {
    pub fn new(custom_types: &'a CustomTypeRegistry, data_types: &'a DataTypeRegistry) -> Self {
        Self {
            custom_types,
            data_types,
        }
    }

    pub fn custom_types(&self) -> &'a CustomTypeRegistry {
        self.custom_types
    }

    pub fn lower_output(&self, output: &ReturnType) -> ReturnAbi {
        ReturnAbi::lower(classify_return(output), self)
    }

    pub fn lower_type(&self, ty: &Type) -> ReturnAbi {
        ReturnAbi::lower(classify_return_type(ty), self)
    }
}

#[derive(Clone, Copy)]
pub struct WasmOptionScalarEncoding {
    primitive: Primitive,
}

pub fn extract_vec_inner(ty: &Type) -> Option<syn::Type> {
    if let Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
        && segment.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return Some(inner_ty.clone());
    }
    None
}

pub fn extract_option_inner(ty: &Type) -> Option<syn::Type> {
    if let Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
        && segment.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return Some(inner_ty.clone());
    }
    None
}

fn is_string_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "String"),
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Path(path) => path.path.segments.last().is_some_and(|s| s.ident == "str"),
            _ => false,
        },
        _ => false,
    }
}

pub fn is_primitive_type(s: &str) -> bool {
    s == "()" || s.parse::<Primitive>().is_ok()
}

fn primitive_for_type(ty: &Type) -> Option<Primitive> {
    quote!(#ty).to_string().replace(' ', "").parse().ok()
}

pub fn classify_return(output: &ReturnType) -> ReturnKind {
    match output {
        ReturnType::Default => ReturnKind::Unit,
        ReturnType::Type(_, ty) => classify_return_type(ty),
    }
}

pub fn classify_return_type(ty: &Type) -> ReturnKind {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    if type_str == "String" || type_str == "std::string::String" {
        return ReturnKind::String;
    }

    if let Some(inner) = extract_vec_inner(ty) {
        return ReturnKind::Vec(inner);
    }

    if let Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
    {
        if segment.ident == "Result"
            && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
            && args.args.len() >= 2
            && let Some(syn::GenericArgument::Type(ok_ty)) = args.args.first()
            && let Some(syn::GenericArgument::Type(err_ty)) = args.args.iter().nth(1)
        {
            let ok_str = quote!(#ok_ty).to_string().replace(' ', "");
            if ok_str == "String" || ok_str == "std::string::String" {
                return ReturnKind::ResultString {
                    err: err_ty.clone(),
                };
            } else if ok_str == "()" {
                return ReturnKind::ResultUnit {
                    err: err_ty.clone(),
                };
            } else {
                return ReturnKind::ResultPrimitive {
                    ok: ok_ty.clone(),
                    err: err_ty.clone(),
                };
            }
        }
        if segment.ident == "Option"
            && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
        {
            if let Some(vec_inner) = extract_vec_inner(inner_ty) {
                return ReturnKind::Option(OptionReturnAbi::Vec { inner: vec_inner });
            }

            if is_string_like_type(inner_ty) {
                return ReturnKind::Option(OptionReturnAbi::OutFfiString);
            }

            return ReturnKind::Option(OptionReturnAbi::OutValue {
                inner: inner_ty.clone(),
            });
        }
    }

    if is_primitive_type(&type_str) {
        ReturnKind::Primitive(ty.clone())
    } else {
        ReturnKind::WireEncoded(ty.clone())
    }
}

fn option_rust_type(abi: OptionReturnAbi) -> syn::Type {
    match abi {
        OptionReturnAbi::OutValue { inner } => syn::parse_quote!(Option<#inner>),
        OptionReturnAbi::OutFfiString => syn::parse_quote!(Option<String>),
        OptionReturnAbi::Vec { inner } => syn::parse_quote!(Option<Vec<#inner>>),
    }
}

fn result_rust_type(ok: syn::Type, err: syn::Type) -> syn::Type {
    syn::parse_quote!(Result<#ok, #err>)
}

pub fn type_is_primitive(ty: &Type) -> bool {
    let type_str = quote!(#ty).to_string().replace(' ', "");
    is_primitive_type(&type_str)
}

fn passable_data_type_category(
    ty: &Type,
    data_types: &DataTypeRegistry,
) -> Option<DataTypeCategory> {
    data_types.category_for(ty)
}

impl ReturnAbi {
    pub fn lower(kind: ReturnKind, lowering: &ReturnLoweringContext<'_>) -> Self {
        match kind {
            ReturnKind::Unit => Self::Unit,
            ReturnKind::Primitive(rust_type) => Self::Scalar { rust_type },
            ReturnKind::String => Self::Encoded {
                rust_type: syn::parse_quote!(String),
                strategy: EncodedReturnStrategy::Utf8String,
            },
            ReturnKind::Vec(inner) => {
                let strategy = if supports_direct_vec_transport(
                    &inner,
                    lowering.custom_types,
                    lowering.data_types,
                ) {
                    EncodedReturnStrategy::DirectVec
                } else {
                    EncodedReturnStrategy::WireEncoded
                };
                Self::Encoded {
                    rust_type: syn::parse_quote!(Vec<#inner>),
                    strategy,
                }
            }
            ReturnKind::Option(abi) => match abi {
                OptionReturnAbi::OutValue { inner } if type_is_primitive(&inner) => {
                    Self::Encoded {
                        rust_type: syn::parse_quote!(Option<#inner>),
                        strategy: EncodedReturnStrategy::OptionScalar,
                    }
                }
                other => Self::Encoded {
                    rust_type: option_rust_type(other),
                    strategy: EncodedReturnStrategy::WireEncoded,
                },
            },
            ReturnKind::ResultString { err } => Self::Encoded {
                rust_type: result_rust_type(syn::parse_quote!(String), err),
                strategy: EncodedReturnStrategy::WireEncoded,
            },
            ReturnKind::ResultPrimitive { ok, err } => {
                if type_is_primitive(&ok) && type_is_primitive(&err) {
                    Self::Encoded {
                        rust_type: result_rust_type(ok.clone(), err.clone()),
                        strategy: EncodedReturnStrategy::ResultScalar,
                    }
                } else {
                    Self::Encoded {
                        rust_type: result_rust_type(ok, err),
                        strategy: EncodedReturnStrategy::WireEncoded,
                    }
                }
            }
            ReturnKind::ResultUnit { err } => Self::Encoded {
                rust_type: result_rust_type(syn::parse_quote!(()), err),
                strategy: EncodedReturnStrategy::WireEncoded,
            },
            ReturnKind::WireEncoded(rust_type) => {
                match classify_named_type_transport(
                    &rust_type,
                    lowering.custom_types,
                    lowering.data_types,
                ) {
                    NamedTypeTransport::Passable => Self::Passable { rust_type },
                    NamedTypeTransport::WireEncoded => Self::Encoded {
                        rust_type,
                        strategy: EncodedReturnStrategy::WireEncoded,
                    },
                }
            }
        }
    }

    pub fn value_return_strategy(&self, lowering: &ReturnLoweringContext<'_>) -> ValueReturnStrategy {
        match self {
            Self::Unit => ValueReturnStrategy::Void,
            Self::Scalar { .. } => {
                ValueReturnStrategy::Scalar(ScalarReturnStrategy::PrimitiveValue)
            }
            Self::Encoded { strategy, .. } => match strategy {
                EncodedReturnStrategy::DirectVec => ValueReturnStrategy::DirectBuffer,
                EncodedReturnStrategy::Utf8String
                | EncodedReturnStrategy::OptionScalar
                | EncodedReturnStrategy::ResultScalar
                | EncodedReturnStrategy::WireEncoded => ValueReturnStrategy::EncodedBuffer,
            },
            Self::Passable { rust_type } => match passable_data_type_category(rust_type, lowering.data_types) {
                Some(DataTypeCategory::Scalar) => {
                    ValueReturnStrategy::Scalar(ScalarReturnStrategy::CStyleEnumTag)
                }
                Some(DataTypeCategory::Blittable) => {
                    ValueReturnStrategy::CompositeValue
                }
                Some(DataTypeCategory::WireEncoded) | None => {
                    unreachable!("passable return abi requires a scalar or blittable data type")
                }
            },
        }
    }

    pub fn value_return_method(
        &self,
        lowering: &ReturnLoweringContext<'_>,
        context: ReturnInvocationContext,
        platform: ReturnPlatform,
    ) -> ValueReturnMethod {
        self.value_return_strategy(lowering)
            .return_method(ErrorReturnStrategy::None, context, platform)
    }

    pub fn invalid_arg_early_return_statement(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Unit => quote! {
                return ::boltffi::__private::FfiStatus::INVALID_ARG;
            },
            Self::Scalar { .. } => quote! {
                return ::core::default::Default::default();
            },
            Self::Encoded {
                rust_type,
                strategy: EncodedReturnStrategy::OptionScalar,
            } => {
                let _ = WasmOptionScalarEncoding::from_option_rust_type(rust_type)
                    .expect("OptionScalar return must have a primitive Option inner type");
                quote! {
                    return f64::NAN;
                }
            }
            Self::Encoded {
                strategy: EncodedReturnStrategy::DirectVec,
                ..
            } => {
                quote! {
                    return;
                }
            }
            Self::Encoded { .. } => quote! {
                #[cfg(target_arch = "wasm32")]
                {
                    return ::boltffi::__private::FfiBuf::default().into_packed();
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    return ::boltffi::__private::FfiBuf::default();
                }
            },
            Self::Passable { rust_type } => quote! {
                return unsafe {
                    ::core::mem::MaybeUninit::<<#rust_type as ::boltffi::__private::Passable>::Out>::zeroed().assume_init()
                };
            },
        }
    }

    pub fn async_ffi_return_type(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Unit => quote! { () },
            Self::Scalar { rust_type } => quote! { #rust_type },
            Self::Encoded { .. } => quote! { ::boltffi::__private::FfiBuf },
            Self::Passable { rust_type } => {
                quote! { <#rust_type as ::boltffi::__private::Passable>::Out }
            }
        }
    }

    pub fn async_rust_return_type(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Unit => quote! { () },
            Self::Scalar { rust_type }
            | Self::Encoded { rust_type, .. }
            | Self::Passable { rust_type } => {
                quote! { #rust_type }
            }
        }
    }

    pub fn async_complete_conversion(
        &self,
        lowering: &ReturnLoweringContext<'_>,
    ) -> proc_macro2::TokenStream {
        match self {
            Self::Unit => quote! {
                if !out_status.is_null() { *out_status = ::boltffi::__private::FfiStatus::OK; }
                ()
            },
            Self::Scalar { .. } => quote! {
                if !out_status.is_null() { *out_status = ::boltffi::__private::FfiStatus::OK; }
                result
            },
            Self::Encoded {
                rust_type,
                strategy,
            } => {
                let result_ident = syn::Ident::new("result", Span::call_site());
                let encode_expression = encoded_return_buffer_expression(
                    rust_type,
                    *strategy,
                    &result_ident,
                    Some(lowering.custom_types()),
                );
                quote! {
                    if !out_status.is_null() { *out_status = ::boltffi::__private::FfiStatus::OK; }
                    #encode_expression
                }
            }
            Self::Passable { .. } => quote! {
                if !out_status.is_null() { *out_status = ::boltffi::__private::FfiStatus::OK; }
                ::boltffi::__private::Passable::pack(result)
            },
        }
    }

    pub fn async_default_ffi_value(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Unit => quote! { () },
            Self::Scalar { .. } => quote! { Default::default() },
            Self::Encoded { .. } => quote! { ::boltffi::__private::FfiBuf::default() },
            Self::Passable { .. } => quote! { Default::default() },
        }
    }
}

impl WasmOptionScalarEncoding {
    pub fn from_option_rust_type(rust_type: &Type) -> Option<Self> {
        extract_option_inner(rust_type)
            .and_then(|inner| primitive_for_type(&inner))
            .map(|primitive| Self { primitive })
    }

    pub fn some_expression(self, value_ident: &syn::Ident) -> proc_macro2::TokenStream {
        match self.primitive {
            Primitive::Bool => quote! {
                if #value_ident { 1.0 } else { 0.0 }
            },
            Primitive::F64 => quote! { #value_ident },
            Primitive::I8
            | Primitive::U8
            | Primitive::I16
            | Primitive::U16
            | Primitive::I32
            | Primitive::U32
            | Primitive::I64
            | Primitive::U64
            | Primitive::ISize
            | Primitive::USize
            | Primitive::F32 => quote! {
                #value_ident as f64
            },
        }
    }
}

pub fn encoded_return_body(
    rust_type: &syn::Type,
    strategy: EncodedReturnStrategy,
    result_ident: &syn::Ident,
    evaluate_result_expression: proc_macro2::TokenStream,
    conversions: &[proc_macro2::TokenStream],
    custom_type_registry: &custom_types::CustomTypeRegistry,
) -> proc_macro2::TokenStream {
    let encode_expression = encoded_return_buffer_expression(
        rust_type,
        strategy,
        result_ident,
        Some(custom_type_registry),
    );

    quote! {
        #(#conversions)*
        let #result_ident: #rust_type = #evaluate_result_expression;
        #encode_expression
    }
}

pub fn encoded_return_buffer_expression(
    rust_type: &syn::Type,
    strategy: EncodedReturnStrategy,
    result_ident: &syn::Ident,
    custom_type_registry: Option<&custom_types::CustomTypeRegistry>,
) -> proc_macro2::TokenStream {
    match strategy {
        EncodedReturnStrategy::DirectVec => quote! {
            <::boltffi::__private::Seal as ::boltffi::__private::VecTransport<_>>::pack(#result_ident)
        },
        EncodedReturnStrategy::Utf8String => quote! {
            #[cfg(target_arch = "wasm32")]
            {
                ::boltffi::__private::FfiBuf::from_vec(#result_ident.into_bytes())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                ::boltffi::__private::FfiBuf::wire_encode(&#result_ident)
            }
        },
        EncodedReturnStrategy::OptionScalar | EncodedReturnStrategy::ResultScalar => quote! {
            ::boltffi::__private::FfiBuf::wire_encode(&#result_ident)
        },
        EncodedReturnStrategy::WireEncoded => {
            wire_encode_expression(rust_type, result_ident, custom_type_registry)
        }
    }
}

fn wire_encode_expression(
    rust_type: &syn::Type,
    result_ident: &syn::Ident,
    custom_type_registry: Option<&custom_types::CustomTypeRegistry>,
) -> proc_macro2::TokenStream {
    match custom_type_registry {
        Some(registry) if custom_types::contains_custom_types(rust_type, registry) => {
            let wire_ty = custom_types::wire_type_for(rust_type, registry);
            let wire_value_ident = syn::Ident::new("__boltffi_wire_value", result_ident.span());
            let to_wire = custom_types::to_wire_expr_owned(rust_type, registry, result_ident);
            quote! {
                let #wire_value_ident: #wire_ty = { #to_wire };
                ::boltffi::__private::FfiBuf::wire_encode(&#wire_value_ident)
            }
        }
        _ => quote! {
            ::boltffi::__private::FfiBuf::wire_encode(&#result_ident)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{ReturnAbi, ReturnLoweringContext, WasmOptionScalarEncoding};
    use boltffi_ffi_rules::transport::{
        EncodedReturnStrategy, ReturnInvocationContext, ReturnPlatform, ValueReturnMethod,
        ValueReturnStrategy,
    };
    use crate::custom_types::CustomTypeRegistry;
    use crate::data_types::DataTypeRegistry;
    use syn::parse_quote;

    fn lowering() -> ReturnLoweringContext<'static> {
        let custom_types = Box::leak(Box::new(CustomTypeRegistry::default()));
        let data_types = Box::leak(Box::new(DataTypeRegistry::default()));
        ReturnLoweringContext::new(custom_types, data_types)
    }

    #[test]
    fn wasm_option_bool_uses_numeric_bool_encoding() {
        let value_ident = syn::Ident::new("value", proc_macro2::Span::call_site());
        let expression =
            WasmOptionScalarEncoding::from_option_rust_type(&parse_quote!(Option<bool>))
                .expect("expected bool option encoding")
                .some_expression(&value_ident)
                .to_string();

        assert_eq!(expression, "if value { 1.0 } else { 0.0 }");
    }

    #[test]
    fn packed_encoded_return_uses_packed_default_on_wasm_failure() {
        let return_abi = ReturnAbi::Encoded {
            rust_type: parse_quote!(std::time::Duration),
            strategy: EncodedReturnStrategy::WireEncoded,
        };
        let lowering = lowering();

        let statement = return_abi.invalid_arg_early_return_statement().to_string();

        assert!(matches!(
            return_abi.value_return_strategy(&lowering),
            ValueReturnStrategy::EncodedBuffer
        ));
        assert!(matches!(
            return_abi.value_return_method(
                &lowering,
                ReturnInvocationContext::SyncExport,
                ReturnPlatform::Wasm,
            ),
            ValueReturnMethod::DirectReturn
        ));
        assert!(statement.contains("FfiBuf :: default () . into_packed ()"));
        assert!(statement.contains("return :: boltffi :: __private :: FfiBuf :: default ()"));
    }

    #[test]
    fn direct_vec_return_uses_void_wasm_failure() {
        let return_abi = ReturnAbi::Encoded {
            rust_type: parse_quote!(Vec<i32>),
            strategy: EncodedReturnStrategy::DirectVec,
        };
        let lowering = lowering();

        assert!(matches!(
            return_abi.value_return_method(
                &lowering,
                ReturnInvocationContext::SyncExport,
                ReturnPlatform::Wasm,
            ),
            ValueReturnMethod::WriteToReturnSlot
        ));
        assert_eq!(
            return_abi.invalid_arg_early_return_statement().to_string(),
            "return ;"
        );
    }
}
