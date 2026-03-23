use boltffi_ffi_rules::naming;
use boltffi_ffi_rules::primitive::Primitive;
use quote::quote;
use syn::Type;

use crate::lowering::transport::{NamedTypeTransport, NamedTypeTransportClassifier, TypeShapeExt};

pub(super) fn ptr_ident(base: &syn::Ident) -> syn::Ident {
    syn::Ident::new(
        &format!("{}{}", base, naming::param_ptr_suffix()),
        base.span(),
    )
}

pub(super) fn len_ident(base: &syn::Ident) -> syn::Ident {
    syn::Ident::new(
        &format!("{}{}", base, naming::param_len_suffix()),
        base.span(),
    )
}

pub(super) enum ParamTransform {
    PassThrough,
    StrRef,
    OwnedString,
    Callback {
        params: Vec<syn::Type>,
        returns: Option<syn::Type>,
    },
    SliceRef(syn::Type),
    SliceMut(syn::Type),
    BoxedDynTrait(syn::Path),
    ArcDynTrait(syn::Path),
    OptionArcDynTrait(syn::Path),
    ImplTrait(syn::Path),
    VecPrimitive(syn::Type),
    VecPassable(syn::Type),
    WireEncoded(WireEncodedParam),
    Passable(syn::Type),
}

#[derive(Clone)]
pub(super) struct WireEncodedParam {
    pub(super) kind: WireEncodedParamKind,
    pub(super) rust_type: syn::Type,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WireEncodedParamKind {
    Required,
    Vec,
    Option,
}

#[derive(Clone, Copy)]
pub(super) struct ParamTransformClassifier<'a> {
    named_type_transport_classifier: NamedTypeTransportClassifier<'a>,
}

impl<'a> ParamTransformClassifier<'a> {
    pub(super) fn new(named_type_transport_classifier: NamedTypeTransportClassifier<'a>) -> Self {
        Self {
            named_type_transport_classifier,
        }
    }

    pub(super) fn classify(&self, ty: &Type) -> ParamTransform {
        let type_str = quote!(#ty).to_string().replace(' ', "");

        if let Some((params, returns)) = extract_closure_signature(ty) {
            return ParamTransform::Callback { params, returns };
        }

        if let Some(trait_path) = extract_impl_callback_trait(ty) {
            return ParamTransform::ImplTrait(trait_path);
        }

        if let Some((inner_ty, is_mut)) = extract_slice_inner(ty) {
            return if is_mut {
                ParamTransform::SliceMut(inner_ty)
            } else {
                ParamTransform::SliceRef(inner_ty)
            };
        }

        if let Some(trait_path) = extract_dyn_trait_in_container(ty, "Box") {
            return ParamTransform::BoxedDynTrait(trait_path);
        }

        if let Some(trait_path) = extract_dyn_trait_in_container(ty, "Arc") {
            return ParamTransform::ArcDynTrait(trait_path);
        }

        if type_str.starts_with("*const") || type_str.starts_with("*mut") {
            return ParamTransform::PassThrough;
        }

        if type_str.contains("extern") && type_str.contains("fn(") {
            return ParamTransform::PassThrough;
        }

        if let Some(inner_ty) = extract_vec_param_inner(ty) {
            let inner_str = quote!(#inner_ty).to_string().replace(' ', "");
            if is_primitive_vec_inner(&inner_str) {
                return ParamTransform::VecPrimitive(inner_ty);
            }
            if self
                .named_type_transport_classifier
                .supports_direct_vec_transport(&inner_ty)
            {
                return ParamTransform::VecPassable(inner_ty);
            }
            return ParamTransform::WireEncoded(WireEncodedParam {
                kind: WireEncodedParamKind::Vec,
                rust_type: ty.clone(),
            });
        }

        if let Some(inner_ty) = extract_option_param_inner(ty) {
            if let Some(trait_path) = extract_dyn_trait_in_container(&inner_ty, "Arc") {
                return ParamTransform::OptionArcDynTrait(trait_path);
            }
            return ParamTransform::WireEncoded(WireEncodedParam {
                kind: WireEncodedParamKind::Option,
                rust_type: ty.clone(),
            });
        }

        if ty.is_generic_nominal_type() {
            return ParamTransform::WireEncoded(WireEncodedParam {
                kind: WireEncodedParamKind::Required,
                rust_type: ty.clone(),
            });
        }

        if type_str == "&str" || (type_str.starts_with("&'") && type_str.ends_with("str")) {
            return ParamTransform::StrRef;
        }

        if type_str == "String" || type_str == "std::string::String" {
            return ParamTransform::OwnedString;
        }

        if ty.is_named_nominal_type() {
            return match self
                .named_type_transport_classifier
                .classify_named_type_transport(ty)
            {
                NamedTypeTransport::Passable => ParamTransform::Passable(ty.clone()),
                NamedTypeTransport::WireEncoded => ParamTransform::WireEncoded(WireEncodedParam {
                    kind: WireEncodedParamKind::Required,
                    rust_type: ty.clone(),
                }),
            };
        }

        ParamTransform::PassThrough
    }
}

fn extract_closure_signature(ty: &Type) -> Option<(Vec<syn::Type>, Option<syn::Type>)> {
    if let Type::BareFn(bare_fn) = ty {
        let params: Vec<syn::Type> = bare_fn.inputs.iter().map(|arg| arg.ty.clone()).collect();
        let returns = match &bare_fn.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some((**ty).clone()),
        };
        return Some((params, returns));
    }

    if let Type::ImplTrait(impl_trait) = ty {
        return impl_trait
            .bounds
            .iter()
            .filter_map(|bound| match bound {
                syn::TypeParamBound::Trait(trait_bound) => Some(&trait_bound.path),
                _ => None,
            })
            .filter_map(|path| path.segments.last())
            .filter_map(|segment| {
                let ident = segment.ident.to_string();
                (ident == "Fn" || ident == "FnMut" || ident == "FnOnce")
                    .then_some(&segment.arguments)
            })
            .filter_map(|arguments| match arguments {
                syn::PathArguments::Parenthesized(args) => Some(args),
                _ => None,
            })
            .map(|args| {
                let params: Vec<syn::Type> = args.inputs.iter().cloned().collect();
                let returns = match &args.output {
                    syn::ReturnType::Default => None,
                    syn::ReturnType::Type(_, ty) => Some((**ty).clone()),
                };
                (params, returns)
            })
            .next();
    }

    None
}

fn extract_slice_inner(ty: &Type) -> Option<(syn::Type, bool)> {
    if let Type::Reference(ref_ty) = ty {
        if let Type::Slice(slice_ty) = ref_ty.elem.as_ref() {
            let is_mut = ref_ty.mutability.is_some();
            return Some((*slice_ty.elem.clone(), is_mut));
        }
    }
    None
}

fn extract_impl_callback_trait(ty: &Type) -> Option<syn::Path> {
    if let Type::ImplTrait(impl_trait) = ty {
        return impl_trait
            .bounds
            .iter()
            .filter_map(|bound| match bound {
                syn::TypeParamBound::Trait(trait_bound) => {
                    Some((trait_bound.modifier, &trait_bound.path))
                }
                _ => None,
            })
            .filter(|(modifier, path)| {
                let trait_name = path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .unwrap_or_default();
                !is_non_callback_bound(*modifier, &trait_name)
            })
            .map(|(_, path)| path.clone())
            .next();
    }
    None
}

fn is_non_callback_bound(modifier: syn::TraitBoundModifier, name: &str) -> bool {
    if matches!(modifier, syn::TraitBoundModifier::Maybe(_)) && name == "Sized" {
        return true;
    }
    matches!(
        name,
        "Fn" | "FnMut"
            | "FnOnce"
            | "Send"
            | "Sync"
            | "Unpin"
            | "UnwindSafe"
            | "RefUnwindSafe"
            | "Sized"
            | "Copy"
            | "Clone"
            | "Default"
            | "Debug"
            | "Eq"
            | "PartialEq"
            | "Ord"
            | "PartialOrd"
            | "Hash"
    )
}

fn extract_dyn_trait_in_container(ty: &Type, container: &str) -> Option<syn::Path> {
    if let Type::Path(type_path) = ty {
        if type_path.qself.is_none() {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == container {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(Type::TraitObject(trait_obj))) =
                            args.args.first()
                        {
                            if let Some(syn::TypeParamBound::Trait(trait_bound)) =
                                trait_obj.bounds.first()
                            {
                                return Some(trait_bound.path.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_vec_param_inner(ty: &Type) -> Option<syn::Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty.clone());
                    }
                }
            }
        }
    }
    None
}

fn extract_option_param_inner(ty: &Type) -> Option<syn::Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty.clone());
                    }
                }
            }
        }
    }
    None
}

pub(super) fn is_primitive_vec_inner(type_string: &str) -> bool {
    type_string.parse::<Primitive>().is_ok()
}
