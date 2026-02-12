use proc_macro2::Span;
use quote::quote;
use syn::{FnArg, Pat};

use crate::callback_registry::CallbackTraitRegistry;
use crate::custom_types::{
    CustomTypeRegistry, contains_custom_types, from_wire_expr_owned, to_wire_expr_owned,
    wire_type_for,
};
use crate::util::{
    ParamTransform, classify_param_transform, foreign_trait_path, is_primitive_vec_inner,
    len_ident, ptr_ident,
};

pub struct FfiParams {
    pub ffi_params: Vec<proc_macro2::TokenStream>,
    pub conversions: Vec<proc_macro2::TokenStream>,
    pub call_args: Vec<proc_macro2::TokenStream>,
}

struct ImplTraitResolution {
    foreign_type: proc_macro2::TokenStream,
    error: Option<proc_macro2::TokenStream>,
}

fn impl_trait_resolution(
    trait_path: &syn::Path,
    callback_registry: &CallbackTraitRegistry,
) -> ImplTraitResolution {
    if let Some(resolution) = callback_registry.resolve(trait_path) {
        let foreign_path = resolution.foreign_path;
        if resolution.is_object_safe {
            return ImplTraitResolution {
                foreign_type: quote! {
                    <dyn #trait_path as ::boltffi::__private::CallbackForeignType>::Foreign
                },
                error: None,
            };
        }
        return ImplTraitResolution {
            foreign_type: quote! { #foreign_path },
            error: None,
        };
    }

    let foreign_path = foreign_trait_path(trait_path);
    let trait_name = quote!(#trait_path).to_string();
    let message = format!(
        "boltffi: cannot resolve callback trait `impl {}`. If this is a cross-crate async callback, use the full module path or make the trait object-safe with #[async_trait], e.g. `impl crate::path::to::{}` or `Box<dyn {}>`.",
        trait_name, trait_name, trait_name
    );
    let message_lit = syn::LitStr::new(&message, Span::call_site());
    ImplTraitResolution {
        foreign_type: quote! { #foreign_path },
        error: Some(quote! { compile_error!(#message_lit); }),
    }
}

pub fn transform_params(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
    custom_types: &CustomTypeRegistry,
    callback_registry: &CallbackTraitRegistry,
) -> FfiParams {
    inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(pat_type),
            FnArg::Receiver(_) => None,
        })
        .fold(
            FfiParams {
                ffi_params: Vec::new(),
                conversions: Vec::new(),
                call_args: Vec::new(),
            },
            |mut acc, pat_type| {
                let Some(name) = (match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => Some(ident.ident.clone()),
                    _ => None,
                }) else {
                    return acc;
                };

                match classify_param_transform(&pat_type.ty) {
                ParamTransform::StrRef => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &str = if #ptr_name.is_null() {
                            ""
                        } else {
                            ::core::str::from_utf8(::core::slice::from_raw_parts(#ptr_name, #len_name))
                                .expect(concat!(stringify!(#name), ": invalid UTF-8"))
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OwnedString => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            ::core::str::from_utf8(::core::slice::from_raw_parts(#ptr_name, #len_name))
                                .expect(concat!(stringify!(#name), ": invalid UTF-8"))
                                .to_string()
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::Callback(arg_types) => {
                    let cb_name = syn::Ident::new(&format!("{}_cb", name), name.span());
                    let ud_name = syn::Ident::new(&format!("{}_ud", name), name.span());

                    let (ffi_cb_args, arg_names, cb_call_args, wire_vars) = arg_types.iter().enumerate().fold(
                        (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                        |(mut ffi_cb_args, mut arg_names, mut cb_call_args, mut wire_vars),
                         (index, arg_ty)| {
                            let arg_name =
                                syn::Ident::new(&format!("__arg{}", index), name.span());
                            let arg_ty_str = quote!(#arg_ty).to_string().replace(' ', "");

                            if is_primitive_vec_inner(&arg_ty_str) {
                                ffi_cb_args.push(quote! { #arg_ty });
                                cb_call_args.push(quote! { #arg_name });
                            } else {
                                let wire_name =
                                    syn::Ident::new(&format!("__wire{}", index), name.span());
                                ffi_cb_args.push(quote! { *const u8 });
                                ffi_cb_args.push(quote! { usize });
                                let wire_vars_expr = if contains_custom_types(arg_ty, custom_types) {
                                    let wire_ty = wire_type_for(arg_ty, custom_types);
                                    let wire_value_ident = syn::Ident::new(&format!("__wire_value{}", index), name.span());
                                    let to_wire = to_wire_expr_owned(arg_ty, custom_types, &arg_name);
                                    quote! {
                                        let #wire_value_ident: #wire_ty = { #to_wire };
                                        let #wire_name = ::boltffi::__private::wire::encode(&#wire_value_ident);
                                    }
                                } else {
                                    quote! {
                                        let #wire_name = ::boltffi::__private::wire::encode(&#arg_name);
                                    }
                                };
                                wire_vars.push(wire_vars_expr);
                                cb_call_args.push(quote! { #wire_name.as_ptr() });
                                cb_call_args.push(quote! { #wire_name.len() });
                            }

                            arg_names.push(arg_name);

                            (ffi_cb_args, arg_names, cb_call_args, wire_vars)
                        },
                    );

                    acc.ffi_params.push(
                        quote! { #cb_name: extern "C" fn(*mut ::core::ffi::c_void, #(#ffi_cb_args),*) },
                    );
                    acc.ffi_params
                        .push(quote! { #ud_name: *mut ::core::ffi::c_void });

                    acc.conversions.push(quote! {
                        let #name = |#(#arg_names: #arg_types),*| {
                            #(#wire_vars)*
                            #cb_name(#ud_name, #(#cb_call_args),*)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::SliceRef(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &[#inner_ty] = if #ptr_name.is_null() {
                            &[]
                        } else {
                            ::core::slice::from_raw_parts(#ptr_name, #len_name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::SliceMut(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *mut #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &mut [#inner_ty] = if #ptr_name.is_null() {
                            &mut []
                        } else {
                            ::core::slice::from_raw_parts_mut(#ptr_name, #len_name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::BoxedDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: Box<dyn #trait_path> = unsafe {
                            <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ArcDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: ::std::sync::Arc<dyn #trait_path> = unsafe {
                            <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::arc_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionArcDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: Option<::std::sync::Arc<dyn #trait_path>> = if #name.is_null() {
                            None
                        } else {
                            Some(unsafe {
                                <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::arc_from_callback_handle(#name)
                            })
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecPrimitive(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: Vec<#inner_ty> = if #ptr_name.is_null() {
                            Vec::new()
                        } else {
                            ::core::slice::from_raw_parts(#ptr_name, #len_name).to_vec()
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: Vec<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: Option<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::RecordWireEncoded(record_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: #record_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ImplTrait(trait_path) => {
                    let resolution = impl_trait_resolution(&trait_path, callback_registry);
                    if let Some(error) = resolution.error {
                        acc.conversions.push(error);
                    }
                    let foreign_type = resolution.foreign_type;

                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name = unsafe {
                            <#foreign_type as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { *#name });
                }
                ParamTransform::PassThrough => {
                    let ty = &pat_type.ty;
                    acc.ffi_params.push(quote! { #name: #ty });
                    acc.call_args.push(quote! { #name });
                }
            }
                acc
            },
        )
}

pub struct AsyncFfiParams {
    pub ffi_params: Vec<proc_macro2::TokenStream>,
    pub pre_spawn: Vec<proc_macro2::TokenStream>,
    pub thread_setup: Vec<proc_macro2::TokenStream>,
    pub call_args: Vec<proc_macro2::TokenStream>,
    pub move_vars: Vec<syn::Ident>,
}

pub fn transform_params_async(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
    custom_types: &CustomTypeRegistry,
    callback_registry: &CallbackTraitRegistry,
) -> AsyncFfiParams {
    inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(pat_type),
            FnArg::Receiver(_) => None,
        })
        .fold(
            AsyncFfiParams {
                ffi_params: Vec::new(),
                pre_spawn: Vec::new(),
                thread_setup: Vec::new(),
                call_args: Vec::new(),
                move_vars: Vec::new(),
            },
            |mut acc, pat_type| {
                let Some(name) = (match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => Some(ident.ident.clone()),
                    _ => None,
                }) else {
                    return acc;
                };

                match classify_param_transform(&pat_type.ty) {
                ParamTransform::StrRef => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);
                    let owned_name = syn::Ident::new(&format!("{}_owned", name), name.span());

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #owned_name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            match ::core::str::from_utf8(unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }) {
                                Ok(s) => s.to_string(),
                                Err(_) => {
                                    panic!(concat!(stringify!(#name), " is not valid UTF-8"));
                                }
                            }
                        };
                    });

                    acc.thread_setup.push(quote! {
                        let #name: &str = &#owned_name;
                    });

                    acc.move_vars.push(owned_name);
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OwnedString => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            match ::core::str::from_utf8(unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }) {
                                Ok(s) => s.to_string(),
                                Err(_) => {
                                    panic!(concat!(stringify!(#name), " is not valid UTF-8"));
                                }
                            }
                        };
                    });

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::Callback(_) => {
                    panic!("Callbacks are not supported in async functions");
                }
                ParamTransform::SliceRef(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);
                    let owned_name = syn::Ident::new(&format!("{}_vec", name), name.span());

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #owned_name: Vec<#inner_ty> = if #ptr_name.is_null() {
                            Vec::new()
                        } else {
                            unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }.to_vec()
                        };
                    });

                    acc.thread_setup.push(quote! {
                        let #name: &[#inner_ty] = &#owned_name;
                    });

                    acc.move_vars.push(owned_name);
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::SliceMut(_) => {
                    panic!("Mutable slices are not supported in async functions");
                }
                ParamTransform::BoxedDynTrait(_)
                | ParamTransform::ArcDynTrait(_)
                | ParamTransform::OptionArcDynTrait(_) => {
                    panic!("Trait object parameters are not yet supported in async functions");
                }
                ParamTransform::VecPrimitive(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #name: Vec<#inner_ty> = if #ptr_name.is_null() {
                            Vec::new()
                        } else {
                            unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }.to_vec()
                        };
                    });

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: Vec<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: Option<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::RecordWireEncoded(record_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: #record_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ImplTrait(trait_path) => {
                    let resolution = impl_trait_resolution(&trait_path, callback_registry);
                    if let Some(error) = resolution.error {
                        acc.pre_spawn.push(error);
                    }
                    let foreign_type = resolution.foreign_type;
                    let boxed_name = syn::Ident::new(&format!("{}_boxed", name), name.span());

                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.pre_spawn.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #boxed_name = unsafe {
                            <#foreign_type as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.move_vars.push(boxed_name.clone());
                    acc.call_args.push(quote! { *#boxed_name });
                }
                ParamTransform::PassThrough => {
                    let ty = &pat_type.ty;
                    acc.ffi_params.push(quote! { #name: #ty });
                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
            }
                acc
            },
        )
}

pub fn transform_method_params(
    inputs: impl Iterator<Item = syn::FnArg>,
    custom_types: &CustomTypeRegistry,
    callback_registry: &CallbackTraitRegistry,
) -> FfiParams {
    inputs
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(pat_type),
            FnArg::Receiver(_) => None,
        })
        .fold(
            FfiParams {
                ffi_params: Vec::new(),
                conversions: Vec::new(),
                call_args: Vec::new(),
            },
            |mut acc, pat_type| {
                let Some(name) = (match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => Some(ident.ident.clone()),
                    _ => None,
                }) else {
                    return acc;
                };

                match classify_param_transform(&pat_type.ty) {
                ParamTransform::StrRef => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &str = if #ptr_name.is_null() {
                            ""
                        } else {
                            ::core::str::from_utf8(::core::slice::from_raw_parts(#ptr_name, #len_name))
                                .expect(concat!(stringify!(#name), ": invalid UTF-8"))
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OwnedString => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            ::core::str::from_utf8(::core::slice::from_raw_parts(#ptr_name, #len_name))
                                .expect(concat!(stringify!(#name), ": invalid UTF-8"))
                                .to_string()
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::Callback(arg_types) => {
                    let cb_name = syn::Ident::new(&format!("{}_cb", name), name.span());
                    let ud_name = syn::Ident::new(&format!("{}_ud", name), name.span());

                    let (ffi_cb_args, arg_names, cb_call_args, wire_vars) = arg_types.iter().enumerate().fold(
                        (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                        |(mut ffi_cb_args, mut arg_names, mut cb_call_args, mut wire_vars),
                         (index, arg_ty)| {
                            let arg_name =
                                syn::Ident::new(&format!("__arg{}", index), name.span());
                            let arg_ty_str = quote!(#arg_ty).to_string().replace(' ', "");

                            if is_primitive_vec_inner(&arg_ty_str) {
                                ffi_cb_args.push(quote! { #arg_ty });
                                cb_call_args.push(quote! { #arg_name });
                            } else {
                                let wire_name =
                                    syn::Ident::new(&format!("__wire{}", index), name.span());
                                ffi_cb_args.push(quote! { *const u8 });
                                ffi_cb_args.push(quote! { usize });
                                let wire_vars_expr = if contains_custom_types(arg_ty, custom_types) {
                                    let wire_ty = wire_type_for(arg_ty, custom_types);
                                    let wire_value_ident = syn::Ident::new(&format!("__wire_value{}", index), name.span());
                                    let to_wire = to_wire_expr_owned(arg_ty, custom_types, &arg_name);
                                    quote! {
                                        let #wire_value_ident: #wire_ty = { #to_wire };
                                        let #wire_name = ::boltffi::__private::wire::encode(&#wire_value_ident);
                                    }
                                } else {
                                    quote! {
                                        let #wire_name = ::boltffi::__private::wire::encode(&#arg_name);
                                    }
                                };
                                wire_vars.push(wire_vars_expr);
                                cb_call_args.push(quote! { #wire_name.as_ptr() });
                                cb_call_args.push(quote! { #wire_name.len() });
                            }

                            arg_names.push(arg_name);

                            (ffi_cb_args, arg_names, cb_call_args, wire_vars)
                        },
                    );

                    acc.ffi_params.push(
                        quote! { #cb_name: extern "C" fn(*mut ::core::ffi::c_void, #(#ffi_cb_args),*) },
                    );
                    acc.ffi_params
                        .push(quote! { #ud_name: *mut ::core::ffi::c_void });

                    acc.conversions.push(quote! {
                        let #name = |#(#arg_names: #arg_types),*| {
                            #(#wire_vars)*
                            #cb_name(#ud_name, #(#cb_call_args),*)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::SliceRef(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &[#inner_ty] = if #ptr_name.is_null() {
                            &[]
                        } else {
                            ::core::slice::from_raw_parts(#ptr_name, #len_name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::SliceMut(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *mut #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: &mut [#inner_ty] = if #ptr_name.is_null() {
                            &mut []
                        } else {
                            ::core::slice::from_raw_parts_mut(#ptr_name, #len_name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::BoxedDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: Box<dyn #trait_path> = unsafe {
                            <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ArcDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: ::std::sync::Arc<dyn #trait_path> = unsafe {
                            <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::arc_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionArcDynTrait(trait_path) => {
                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name: Option<::std::sync::Arc<dyn #trait_path>> = if #name.is_null() {
                            None
                        } else {
                            Some(unsafe {
                                <dyn #trait_path as ::boltffi::__private::FromCallbackHandle>::arc_from_callback_handle(#name)
                            })
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecPrimitive(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.conversions.push(quote! {
                        let #name: Vec<#inner_ty> = if #ptr_name.is_null() {
                            Vec::new()
                        } else {
                            ::core::slice::from_raw_parts(#ptr_name, #len_name).to_vec()
                        };
                    });

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: Vec<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: Option<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::RecordWireEncoded(record_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.conversions.push(quote! {
                            let #name: #original_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.conversions.push(quote! {
                            let #name: #record_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = ::core::slice::from_raw_parts(#ptr_name, #len_name);
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ImplTrait(trait_path) => {
                    let resolution = impl_trait_resolution(&trait_path, callback_registry);
                    if let Some(error) = resolution.error {
                        acc.conversions.push(error);
                    }
                    let foreign_type = resolution.foreign_type;

                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.conversions.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #name = unsafe {
                            <#foreign_type as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.call_args.push(quote! { *#name });
                }
                ParamTransform::PassThrough => {
                    let ty = &pat_type.ty;
                    acc.ffi_params.push(quote! { #name: #ty });
                    acc.call_args.push(quote! { #name });
                }
            }
                acc
            },
        )
}

pub fn transform_method_params_async(
    inputs: impl Iterator<Item = syn::FnArg>,
    custom_types: &CustomTypeRegistry,
    callback_registry: &CallbackTraitRegistry,
) -> AsyncFfiParams {
    inputs
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(pat_type),
            FnArg::Receiver(_) => None,
        })
        .fold(
            AsyncFfiParams {
                ffi_params: Vec::new(),
                pre_spawn: Vec::new(),
                thread_setup: Vec::new(),
                call_args: Vec::new(),
                move_vars: Vec::new(),
            },
            |mut acc, pat_type| {
                let Some(name) = (match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => Some(ident.ident.clone()),
                    _ => None,
                }) else {
                    return acc;
                };

                match classify_param_transform(&pat_type.ty) {
                ParamTransform::StrRef => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);
                    let owned_name = syn::Ident::new(&format!("{}_owned", name), name.span());

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #owned_name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            match ::core::str::from_utf8(unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }) {
                                Ok(s) => s.to_string(),
                                Err(_) => panic!(concat!(stringify!(#name), " is not valid UTF-8")),
                            }
                        };
                    });

                    acc.thread_setup.push(quote! {
                        let #name: &str = &#owned_name;
                    });

                    acc.move_vars.push(owned_name);
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OwnedString => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #name: String = if #ptr_name.is_null() {
                            String::new()
                        } else {
                            match ::core::str::from_utf8(unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }) {
                                Ok(s) => s.to_string(),
                                Err(_) => panic!(concat!(stringify!(#name), " is not valid UTF-8")),
                            }
                        };
                    });

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::Callback(_) => {
                    panic!("Callbacks are not supported in async methods");
                }
                ParamTransform::SliceRef(_) | ParamTransform::SliceMut(_) => {
                    panic!("Slices are not supported in async methods");
                }
                ParamTransform::BoxedDynTrait(_)
                | ParamTransform::ArcDynTrait(_)
                | ParamTransform::OptionArcDynTrait(_) => {
                    panic!("Trait object parameters are not supported in async methods");
                }
                ParamTransform::VecPrimitive(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const #inner_ty });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    acc.pre_spawn.push(quote! {
                        let #name: Vec<#inner_ty> = if #ptr_name.is_null() {
                            Vec::new()
                        } else {
                            unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) }.to_vec()
                        };
                    });

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::VecWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: Vec<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                Vec::new()
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::OptionWireEncoded(inner_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: Option<#inner_ty> = if #ptr_name.is_null() || #len_name == 0 {
                                None
                            } else {
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::RecordWireEncoded(record_ty) => {
                    let ptr_name = ptr_ident(&name);
                    let len_name = len_ident(&name);

                    acc.ffi_params.push(quote! { #ptr_name: *const u8 });
                    acc.ffi_params.push(quote! { #len_name: usize });

                    if contains_custom_types(&pat_type.ty, custom_types) {
                        let original_ty = &pat_type.ty;
                        let wire_ty = wire_type_for(original_ty, custom_types);
                        let wire_value_ident = syn::Ident::new("__boltffi_wire_value", name.span());
                        let from_wire = from_wire_expr_owned(original_ty, custom_types, &wire_value_ident);
                        acc.pre_spawn.push(quote! {
                            let #name: #original_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                let #wire_value_ident: #wire_ty = ::boltffi::__private::wire::decode(__bytes)
                                    .unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name));
                                #from_wire
                            };
                        });
                    } else {
                        acc.pre_spawn.push(quote! {
                            let #name: #record_ty = {
                                assert!(!#ptr_name.is_null(), concat!(stringify!(#name), ": null pointer"));
                                let __bytes = unsafe { ::core::slice::from_raw_parts(#ptr_name, #len_name) };
                                ::boltffi::__private::wire::decode(__bytes).unwrap_or_else(|e| panic!("{}: wire decode failed: {} (buf_len={})", stringify!(#name), e, #len_name))
                            };
                        });
                    }

                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
                ParamTransform::ImplTrait(trait_path) => {
                    let resolution = impl_trait_resolution(&trait_path, callback_registry);
                    if let Some(error) = resolution.error {
                        acc.pre_spawn.push(error);
                    }
                    let foreign_type = resolution.foreign_type;
                    let boxed_name = syn::Ident::new(&format!("{}_boxed", name), name.span());

                    acc.ffi_params.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        #name: ::boltffi::__private::CallbackHandle,
                        #[cfg(target_arch = "wasm32")]
                        #name: u32
                    });

                    acc.pre_spawn.push(quote! {
                        #[cfg(not(target_arch = "wasm32"))]
                        assert!(!#name.is_null(), concat!(stringify!(#name), ": null callback handle"));
                        #[cfg(target_arch = "wasm32")]
                        let #name = ::boltffi::__private::CallbackHandle::from_wasm_handle(#name);
                        let #boxed_name = unsafe {
                            <#foreign_type as ::boltffi::__private::FromCallbackHandle>::box_from_callback_handle(#name)
                        };
                    });

                    acc.move_vars.push(boxed_name.clone());
                    acc.call_args.push(quote! { *#boxed_name });
                }
                ParamTransform::PassThrough => {
                    let ty = &pat_type.ty;
                    acc.ffi_params.push(quote! { #name: #ty });
                    acc.move_vars.push(name.clone());
                    acc.call_args.push(quote! { #name });
                }
            }
                acc
            },
        )
}
