use boltffi_ffi_rules::naming;
use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, ReturnType, Type};

use crate::callback_registry;
use crate::custom_types;
use crate::data_types;
use crate::method_common::{
    exported_methods, impl_type_name, is_factory_constructor, is_result_of_self_type_path,
};
use crate::params::{FfiParams, transform_method_params};
use crate::returns::{ReturnAbi, ReturnLoweringContext, encoded_return_body};

enum RecordMethodKind {
    Constructor,
    InstanceRef,
    InstanceMut,
    Static,
}

fn classify_record_method(method: &syn::ImplItemFn, type_name: &syn::Ident) -> RecordMethodKind {
    match method.sig.inputs.first() {
        Some(FnArg::Receiver(receiver)) => {
            if receiver.mutability.is_some() {
                RecordMethodKind::InstanceMut
            } else {
                RecordMethodKind::InstanceRef
            }
        }
        _ => {
            if is_factory_constructor(method, type_name) {
                RecordMethodKind::Constructor
            } else {
                RecordMethodKind::Static
            }
        }
    }
}

fn resolve_self_in_return_type(output: &ReturnType, type_name: &syn::Ident) -> ReturnType {
    match output {
        ReturnType::Default => ReturnType::Default,
        ReturnType::Type(arrow, ty) => {
            let resolved = resolve_self_type(ty, type_name);
            ReturnType::Type(*arrow, Box::new(resolved))
        }
    }
}

fn resolve_self_type(ty: &Type, type_name: &syn::Ident) -> Type {
    match ty {
        Type::Path(type_path) => {
            let mut path = type_path.clone();
            path.path.segments.iter_mut().for_each(|segment| {
                if segment.ident == "Self" {
                    segment.ident = type_name.clone();
                }
                if let syn::PathArguments::AngleBracketed(args) = &mut segment.arguments {
                    args.args.iter_mut().for_each(|arg| {
                        if let syn::GenericArgument::Type(inner) = arg {
                            *inner = resolve_self_type(inner, type_name);
                        }
                    });
                }
            });
            Type::Path(path)
        }
        Type::Reference(reference) => {
            let mut resolved = reference.clone();
            resolved.elem = Box::new(resolve_self_type(&reference.elem, type_name));
            Type::Reference(resolved)
        }
        Type::Tuple(tuple) => {
            let mut resolved = tuple.clone();
            resolved
                .elems
                .iter_mut()
                .for_each(|elem| *elem = resolve_self_type(elem, type_name));
            Type::Tuple(resolved)
        }
        _ => ty.clone(),
    }
}

fn generate_record_constructor_export(
    type_name: &syn::Ident,
    record_name: &str,
    method: &syn::ImplItemFn,
    return_lowering: &ReturnLoweringContext<'_>,
    callback_registry: &callback_registry::CallbackTraitRegistry,
) -> Option<proc_macro2::TokenStream> {
    let custom_types = return_lowering.custom_types();
    let method_name = &method.sig.ident;
    let export_name = if method_name == "new" {
        naming::class_ffi_new(record_name)
    } else {
        naming::method_ffi_name(record_name, &method_name.to_string())
    };
    let export_name = syn::Ident::new(export_name.as_str(), method_name.span());

    let resolved_output = resolve_self_in_return_type(&method.sig.output, type_name);
    let return_abi = return_lowering.lower_output(&resolved_output);
    let on_error = return_abi.invalid_arg_early_return_statement();

    let inputs = method.sig.inputs.iter().cloned();
    let FfiParams {
        ffi_params,
        conversions,
        call_args,
    } = transform_method_params(inputs, return_lowering, callback_registry, &on_error);

    let call_expr = quote! { #type_name::#method_name(#(#call_args),*) };

    let is_fallible = matches!(
        &resolved_output,
        ReturnType::Type(_, ty)
            if matches!(ty.as_ref(), Type::Path(tp) if is_result_of_self_type_path(&tp.path, type_name))
    );

    generate_value_return_export(
        &export_name,
        &ffi_params,
        &conversions,
        call_expr,
        is_fallible,
        &return_abi,
        custom_types,
    )
}

fn generate_record_instance_export(
    type_name: &syn::Ident,
    record_name: &str,
    method: &syn::ImplItemFn,
    is_mut: bool,
    return_lowering: &ReturnLoweringContext<'_>,
    callback_registry: &callback_registry::CallbackTraitRegistry,
) -> Option<proc_macro2::TokenStream> {
    let custom_types = return_lowering.custom_types();
    let method_name = &method.sig.ident;
    let export_name = naming::method_ffi_name(record_name, &method_name.to_string());
    let export_name = syn::Ident::new(export_name.as_str(), method_name.span());

    let resolved_output = resolve_self_in_return_type(&method.sig.output, type_name);
    let return_abi = return_lowering.lower_output(&resolved_output);
    let on_error = return_abi.invalid_arg_early_return_statement();

    let other_inputs = method.sig.inputs.iter().skip(1).cloned();
    let FfiParams {
        ffi_params: param_ffi,
        conversions: param_conversions,
        call_args,
    } = transform_method_params(other_inputs, return_lowering, callback_registry, &on_error);

    let self_param = quote! { self_value: <#type_name as ::boltffi::__private::Passable>::In };
    let self_unpack = if is_mut {
        quote! { let mut self_value: #type_name = unsafe { <#type_name as ::boltffi::__private::Passable>::unpack(self_value) }; }
    } else {
        quote! { let self_value: #type_name = unsafe { <#type_name as ::boltffi::__private::Passable>::unpack(self_value) }; }
    };

    let mut all_ffi_params = vec![self_param];
    all_ffi_params.extend(param_ffi);

    let mut all_conversions = vec![self_unpack];
    all_conversions.extend(param_conversions);

    let call_expr = quote! { self_value.#method_name(#(#call_args),*) };

    if is_mut {
        return generate_mut_instance_export(
            &export_name,
            type_name,
            &all_ffi_params,
            &all_conversions,
            call_expr,
            &return_abi,
            method_name,
        );
    }

    let (body, return_type, is_wire_encoded) = build_return_arms(
        &return_abi,
        call_expr,
        &all_conversions,
        custom_types,
        method_name,
    )?;

    Some(emit_ffi_function(
        &export_name,
        &all_ffi_params,
        body,
        return_type,
        is_wire_encoded,
    ))
}

fn generate_mut_instance_export(
    export_name: &syn::Ident,
    type_name: &syn::Ident,
    ffi_params: &[proc_macro2::TokenStream],
    conversions: &[proc_macro2::TokenStream],
    call_expr: proc_macro2::TokenStream,
    return_abi: &ReturnAbi,
    method_name: &syn::Ident,
) -> Option<proc_macro2::TokenStream> {
    match return_abi {
        ReturnAbi::Unit => {
            let body = quote! {
                #(#conversions)*
                #call_expr;
                ::boltffi::__private::Passable::pack(self_value)
            };
            let return_type = quote! { -> <#type_name as ::boltffi::__private::Passable>::Out };

            Some(emit_ffi_function(
                export_name,
                ffi_params,
                body,
                return_type,
                false,
            ))
        }
        _ => Some(
            syn::Error::new_spanned(
                method_name,
                "&mut self methods on records that return values are not yet supported",
            )
            .to_compile_error(),
        ),
    }
}

fn generate_record_static_export(
    type_name: &syn::Ident,
    record_name: &str,
    method: &syn::ImplItemFn,
    return_lowering: &ReturnLoweringContext<'_>,
    callback_registry: &callback_registry::CallbackTraitRegistry,
) -> Option<proc_macro2::TokenStream> {
    let custom_types = return_lowering.custom_types();
    let method_name = &method.sig.ident;
    let export_name = naming::method_ffi_name(record_name, &method_name.to_string());
    let export_name = syn::Ident::new(export_name.as_str(), method_name.span());

    let resolved_output = resolve_self_in_return_type(&method.sig.output, type_name);
    let return_abi = return_lowering.lower_output(&resolved_output);
    let on_error = return_abi.invalid_arg_early_return_statement();

    let all_inputs = method.sig.inputs.iter().cloned();
    let FfiParams {
        ffi_params,
        conversions,
        call_args,
    } = transform_method_params(all_inputs, return_lowering, callback_registry, &on_error);

    let call_expr = quote! { #type_name::#method_name(#(#call_args),*) };

    let (body, return_type, is_wire_encoded) = build_return_arms(
        &return_abi,
        call_expr,
        &conversions,
        custom_types,
        method_name,
    )?;

    Some(emit_ffi_function(
        &export_name,
        &ffi_params,
        body,
        return_type,
        is_wire_encoded,
    ))
}

fn generate_value_return_export(
    export_name: &syn::Ident,
    ffi_params: &[proc_macro2::TokenStream],
    conversions: &[proc_macro2::TokenStream],
    call_expr: proc_macro2::TokenStream,
    is_fallible: bool,
    return_abi: &ReturnAbi,
    custom_types: &custom_types::CustomTypeRegistry,
) -> Option<proc_macro2::TokenStream> {
    let on_error = return_abi.invalid_arg_early_return_statement();

    let unwrapped_call = if is_fallible {
        quote! {
            match #call_expr {
                Ok(value) => value,
                Err(error) => {
                    ::boltffi::__private::set_last_error(format!("{error:?}"));
                    return #on_error;
                }
            }
        }
    } else {
        call_expr.clone()
    };

    let passable_call = if conversions.is_empty() {
        unwrapped_call
    } else {
        quote! {
            #(#conversions)*
            #unwrapped_call
        }
    };

    match return_abi {
        ReturnAbi::Passable { rust_type } => {
            let body = quote! {
                ::boltffi::__private::Passable::pack({ #passable_call })
            };
            let return_type = quote! { -> <#rust_type as ::boltffi::__private::Passable>::Out };

            Some(emit_ffi_function(
                export_name,
                ffi_params,
                body,
                return_type,
                false,
            ))
        }
        ReturnAbi::Encoded {
            rust_type: inner_ty,
            strategy,
        } => {
            let encoded_call = if conversions.is_empty() {
                call_expr
            } else {
                quote! {
                    #(#conversions)*
                    #call_expr
                }
            };
            let result_ident = syn::Ident::new("result", export_name.span());
            let body = encoded_return_body(
                inner_ty,
                *strategy,
                &result_ident,
                encoded_call,
                &[],
                custom_types,
            );
            Some(emit_ffi_function(
                export_name,
                ffi_params,
                body,
                quote! { -> ::boltffi::__private::FfiBuf },
                true,
            ))
        }
        _ => Some(
            syn::Error::new_spanned(
                export_name,
                "record constructors must return Self or Result<Self, E>",
            )
            .to_compile_error(),
        ),
    }
}

fn build_return_arms(
    return_abi: &ReturnAbi,
    call_expr: proc_macro2::TokenStream,
    conversions: &[proc_macro2::TokenStream],
    custom_types: &custom_types::CustomTypeRegistry,
    method_name: &syn::Ident,
) -> Option<(proc_macro2::TokenStream, proc_macro2::TokenStream, bool)> {
    let has_conversions = !conversions.is_empty();

    match return_abi {
        ReturnAbi::Unit => {
            let body = if has_conversions {
                quote! {
                    #(#conversions)*
                    #call_expr;
                    ::boltffi::__private::FfiStatus::OK
                }
            } else {
                quote! {
                    #call_expr;
                    ::boltffi::__private::FfiStatus::OK
                }
            };
            Some((body, quote! { -> ::boltffi::__private::FfiStatus }, false))
        }
        ReturnAbi::Scalar { rust_type } => {
            let fn_output = quote! { -> #rust_type };
            let body = if has_conversions {
                quote! {
                    #(#conversions)*
                    #call_expr
                }
            } else {
                call_expr
            };
            Some((body, fn_output, false))
        }
        ReturnAbi::Encoded {
            rust_type: inner_ty,
            strategy,
        } => {
            let result_ident = syn::Ident::new("result", method_name.span());
            let body = encoded_return_body(
                inner_ty,
                *strategy,
                &result_ident,
                call_expr,
                conversions,
                custom_types,
            );
            Some((body, quote! { -> ::boltffi::__private::FfiBuf }, true))
        }
        ReturnAbi::Passable { rust_type } => {
            let body = if has_conversions {
                quote! {
                    #(#conversions)*
                    ::boltffi::__private::Passable::pack(#call_expr)
                }
            } else {
                quote! {
                    ::boltffi::__private::Passable::pack(#call_expr)
                }
            };
            let return_type = quote! { -> <#rust_type as ::boltffi::__private::Passable>::Out };
            Some((body, return_type, false))
        }
    }
}

fn emit_ffi_function(
    export_name: &syn::Ident,
    ffi_params: &[proc_macro2::TokenStream],
    body: proc_macro2::TokenStream,
    return_type: proc_macro2::TokenStream,
    is_wire_encoded: bool,
) -> proc_macro2::TokenStream {
    if is_wire_encoded {
        return emit_encoded_ffi_function(export_name, ffi_params, body);
    }

    if ffi_params.is_empty() {
        quote! {
            #[unsafe(no_mangle)]
            pub extern "C" fn #export_name() #return_type {
                #body
            }
        }
    } else {
        quote! {
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn #export_name(#(#ffi_params),*) #return_type {
                #body
            }
        }
    }
}

fn emit_encoded_ffi_function(
    export_name: &syn::Ident,
    ffi_params: &[proc_macro2::TokenStream],
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if ffi_params.is_empty() {
        quote! {
            #[cfg(target_arch = "wasm32")]
            #[unsafe(no_mangle)]
            pub extern "C" fn #export_name() -> u64 {
                let __boltffi_buf: ::boltffi::__private::FfiBuf = { #body };
                __boltffi_buf.into_packed()
            }

            #[cfg(not(target_arch = "wasm32"))]
            #[unsafe(no_mangle)]
            pub extern "C" fn #export_name() -> ::boltffi::__private::FfiBuf {
                #body
            }
        }
    } else {
        quote! {
            #[cfg(target_arch = "wasm32")]
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn #export_name(
                #(#ffi_params),*
            ) -> u64 {
                let __boltffi_buf: ::boltffi::__private::FfiBuf = { #body };
                __boltffi_buf.into_packed()
            }

            #[cfg(not(target_arch = "wasm32"))]
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn #export_name(
                #(#ffi_params),*
            ) -> ::boltffi::__private::FfiBuf {
                #body
            }
        }
    }
}

pub fn data_impl_block(item: TokenStream) -> TokenStream {
    let input = match syn::parse::<syn::ItemImpl>(item.clone()) {
        Ok(parsed) => parsed,
        Err(error) => return error.to_compile_error().into(),
    };

    let type_name = match impl_type_name(&input) {
        Some(name) => name,
        None => {
            return syn::Error::new_spanned(&input, "#[data(impl)] requires a named type")
                .to_compile_error()
                .into();
        }
    };

    let custom_types = match custom_types::registry_for_current_crate() {
        Ok(registry) => registry,
        Err(error) => return error.to_compile_error().into(),
    };
    let callback_registry = match callback_registry::registry_for_current_crate() {
        Ok(registry) => registry,
        Err(error) => return error.to_compile_error().into(),
    };
    let data_types = match data_types::registry_for_current_crate() {
        Ok(registry) => registry,
        Err(error) => return error.to_compile_error().into(),
    };
    let return_lowering = ReturnLoweringContext::new(&custom_types, &data_types);

    let record_name = type_name.to_string();
    let original_impl: proc_macro2::TokenStream = item.into();

    let method_exports: Vec<_> = exported_methods(&input)
        .filter_map(|method| match classify_record_method(method, &type_name) {
            RecordMethodKind::Constructor => generate_record_constructor_export(
                &type_name,
                &record_name,
                method,
                &return_lowering,
                &callback_registry,
            ),
            RecordMethodKind::InstanceRef => generate_record_instance_export(
                &type_name,
                &record_name,
                method,
                false,
                &return_lowering,
                &callback_registry,
            ),
            RecordMethodKind::InstanceMut => generate_record_instance_export(
                &type_name,
                &record_name,
                method,
                true,
                &return_lowering,
                &callback_registry,
            ),
            RecordMethodKind::Static => generate_record_static_export(
                &type_name,
                &record_name,
                method,
                &return_lowering,
                &callback_registry,
            ),
        })
        .collect();

    TokenStream::from(quote! {
        #original_impl
        #(#method_exports)*
    })
}
