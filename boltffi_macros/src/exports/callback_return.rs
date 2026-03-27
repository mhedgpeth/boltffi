use proc_macro2::TokenStream;
use quote::quote;
use syn::{ReturnType, Type};

use crate::callbacks::registry::CallbackTraitRegistry;

#[derive(Clone)]
pub(crate) struct SyncCallbackReturn {
    ownership: CallbackReturnOwnership,
    is_optional: bool,
    local_handle_path: syn::Path,
}

#[derive(Clone, Copy)]
enum CallbackReturnOwnership {
    Boxed,
    Shared,
}

enum CallbackReturnCandidate {
    Direct {
        trait_path: syn::Path,
        ownership: CallbackReturnOwnership,
    },
    Optional {
        trait_path: syn::Path,
        ownership: CallbackReturnOwnership,
    },
}

impl SyncCallbackReturn {
    pub(crate) fn native_ffi_return_type(&self) -> TokenStream {
        quote! { ::boltffi::__private::CallbackHandle }
    }

    pub(crate) fn wasm_ffi_return_type(&self) -> TokenStream {
        quote! { u32 }
    }

    pub(crate) fn native_invalid_arg_early_return_statement(&self) -> TokenStream {
        quote! {
            return ::boltffi::__private::CallbackHandle::NULL;
        }
    }

    pub(crate) fn wasm_invalid_arg_early_return_statement(&self) -> TokenStream {
        quote! {
            return 0u32;
        }
    }

    pub(crate) fn lower_native_result_expression(
        &self,
        callback_expression: TokenStream,
    ) -> TokenStream {
        let local_handle_path = &self.local_handle_path;

        match (self.ownership, self.is_optional) {
            (CallbackReturnOwnership::Boxed, false) => quote! {
                #local_handle_path(::std::sync::Arc::from(#callback_expression))
            },
            (CallbackReturnOwnership::Shared, false) => quote! {
                #local_handle_path(#callback_expression)
            },
            (CallbackReturnOwnership::Boxed, true) => quote! {
                #callback_expression
                    .map(|callback_impl| #local_handle_path(::std::sync::Arc::from(callback_impl)))
                    .unwrap_or(::boltffi::__private::CallbackHandle::NULL)
            },
            (CallbackReturnOwnership::Shared, true) => quote! {
                #callback_expression
                    .map(#local_handle_path)
                    .unwrap_or(::boltffi::__private::CallbackHandle::NULL)
            },
        }
    }

    pub(crate) fn lower_wasm_result_expression(
        &self,
        callback_expression: TokenStream,
    ) -> TokenStream {
        let local_handle_expression = self.lower_native_result_expression(callback_expression);
        quote! {
            (#local_handle_expression).handle() as u32
        }
    }
}

pub(crate) fn resolve_sync_callback_return(
    output: &ReturnType,
    callback_registry: &CallbackTraitRegistry,
) -> syn::Result<Option<SyncCallbackReturn>> {
    let ReturnType::Type(_, rust_type) = output else {
        return Ok(None);
    };

    let Some(candidate) = extract_callback_return_candidate(rust_type) else {
        return Ok(None);
    };

    let (trait_path, ownership, is_optional) = match candidate {
        CallbackReturnCandidate::Direct {
            trait_path,
            ownership,
        } => (trait_path, ownership, false),
        CallbackReturnCandidate::Optional {
            trait_path,
            ownership,
        } => (trait_path, ownership, true),
    };

    let Some(resolution) = callback_registry.resolve(&trait_path) else {
        return Ok(None);
    };

    if !resolution.supports_local_handle {
        return Err(syn::Error::new_spanned(
            rust_type,
            "boltffi: sync callback returns require an object-safe exported callback trait without async methods",
        ));
    }

    Ok(Some(SyncCallbackReturn {
        ownership,
        is_optional,
        local_handle_path: resolution.local_handle_path,
    }))
}

fn extract_callback_return_candidate(rust_type: &Type) -> Option<CallbackReturnCandidate> {
    if let Some((trait_path, ownership)) = extract_trait_object_container(rust_type) {
        return Some(CallbackReturnCandidate::Direct {
            trait_path,
            ownership,
        });
    }

    let Type::Path(type_path) = rust_type else {
        return None;
    };
    let option_segment = type_path.path.segments.last()?;
    if option_segment.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &option_segment.arguments else {
        return None;
    };
    let inner_type = arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(inner_type) => Some(inner_type),
        _ => None,
    })?;
    let (trait_path, ownership) = extract_trait_object_container(inner_type)?;
    Some(CallbackReturnCandidate::Optional {
        trait_path,
        ownership,
    })
}

fn extract_trait_object_container(
    rust_type: &Type,
) -> Option<(syn::Path, CallbackReturnOwnership)> {
    if let Some(trait_path) = extract_dyn_trait_in_container(rust_type, "Box") {
        return Some((trait_path, CallbackReturnOwnership::Boxed));
    }

    extract_dyn_trait_in_container(rust_type, "Arc")
        .map(|trait_path| (trait_path, CallbackReturnOwnership::Shared))
}

fn extract_dyn_trait_in_container(rust_type: &Type, container_name: &str) -> Option<syn::Path> {
    let Type::Path(type_path) = rust_type else {
        return None;
    };
    let container_segment = type_path.path.segments.last()?;
    if container_segment.ident != container_name {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &container_segment.arguments else {
        return None;
    };
    let inner_type = arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(inner_type) => Some(inner_type),
        _ => None,
    })?;
    extract_dyn_trait_path(inner_type)
}

fn extract_dyn_trait_path(rust_type: &Type) -> Option<syn::Path> {
    let Type::TraitObject(trait_object) = rust_type else {
        return None;
    };
    trait_object.bounds.iter().find_map(|bound| match bound {
        syn::TypeParamBound::Trait(trait_bound) => Some(trait_bound.path.clone()),
        _ => None,
    })
}
