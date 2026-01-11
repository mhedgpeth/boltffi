use super::names::NamingConvention;
use super::types::TypeMapper;
use crate::model::{ReturnType, Type};

#[derive(Debug, Clone, Default)]
pub struct ReturnInfo {
    pub swift_type: Option<String>,
    pub is_void: bool,
    pub is_result: bool,
    pub result_ok_type: Option<String>,
}

impl ReturnInfo {
    pub fn from_return_type(returns: &ReturnType) -> Self {
        match returns {
            ReturnType::Void => Self {
                is_void: true,
                ..Default::default()
            },
            ReturnType::Value(ty) => match ty {
                Type::Void => Self {
                    is_void: true,
                    ..Default::default()
                },
                _ => Self {
                    swift_type: Some(TypeMapper::map_type(ty)),
                    ..Default::default()
                },
            },
            ReturnType::Fallible { ok, .. } => {
                let ok_type = match ok {
                    Type::Void => None,
                    _ => Some(TypeMapper::map_type(ok)),
                };
                Self {
                    swift_type: ok_type.clone(),
                    is_result: true,
                    result_ok_type: ok_type,
                    is_void: matches!(ok, Type::Void),
                }
            }
        }
    }

    pub fn has_return(&self) -> bool {
        !self.is_void && self.swift_type.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParamInfo {
    pub swift_name: String,
    pub swift_type: String,
    pub ffi_conversion: String,
    pub is_string: bool,
    pub is_slice: bool,
    pub is_mut_slice: bool,
    pub is_vec: bool,
    pub is_callback: bool,
    pub is_enum: bool,
    pub is_boxed_trait: bool,
    pub is_escaping: bool,
    pub is_first_pointer_param: bool,
    pub slice_inner_type: Option<String>,
    pub vec_inner_type: Option<String>,
}

impl ParamInfo {
    pub fn from_param(name: &str, ty: &Type, is_first_pointer: bool) -> Self {
        let swift_name = NamingConvention::param_name(name);
        let swift_type = TypeMapper::map_type(ty);

        let is_string = matches!(ty, Type::String);
        let is_slice = matches!(ty, Type::Slice(_));
        let is_mut_slice = matches!(ty, Type::MutSlice(_));
        let is_vec = matches!(ty, Type::Vec(_));
        let is_callback = matches!(ty, Type::Closure(_));
        let is_enum = matches!(ty, Type::Enum(_));
        let is_boxed_trait = matches!(ty, Type::BoxedTrait(_));
        let is_escaping = is_callback;

        let is_pointer_param = is_string || is_slice || is_mut_slice || is_vec;
        let is_first_pointer_param = is_pointer_param && is_first_pointer;

        let ffi_conversion = match ty {
            Type::Enum(_) => format!("{}.cValue", swift_name),
            Type::BoxedTrait(trait_name) => {
                let class_name = NamingConvention::class_name(trait_name);
                format!("{}Bridge.create({})", class_name, swift_name)
            }
            _ => swift_name.clone(),
        };

        let slice_inner_type = match ty {
            Type::Slice(inner) | Type::MutSlice(inner) => Some(TypeMapper::map_type(inner)),
            _ => None,
        };

        let vec_inner_type = match ty {
            Type::Vec(inner) => Some(TypeMapper::map_type(inner)),
            _ => None,
        };

        Self {
            swift_name,
            swift_type,
            ffi_conversion,
            is_string,
            is_slice,
            is_mut_slice,
            is_vec,
            is_callback,
            is_enum,
            is_boxed_trait,
            is_escaping,
            is_first_pointer_param,
            slice_inner_type,
            vec_inner_type,
        }
    }

    pub fn needs_wrapper(&self) -> bool {
        self.is_string || self.is_slice || self.is_mut_slice || self.is_vec
    }
}

#[derive(Debug, Clone)]
pub struct CallbackInfo {
    pub param_name: String,
    pub swift_type: String,
    pub ffi_arg_type: String,
    pub context_type: String,
    pub box_type: String,
    pub box_name: String,
    pub ptr_name: String,
    pub trampoline_name: String,
}

impl CallbackInfo {
    pub fn from_param(name: &str, ty: &Type, func_name_pascal: &str, index: usize) -> Option<Self> {
        let Type::Closure(sig) = ty else {
            return None;
        };

        let param_name = NamingConvention::param_name(name);
        let suffix = if index > 0 {
            format!("{}", index + 1)
        } else {
            String::new()
        };

        let params_swift = sig
            .params
            .iter()
            .map(|p| TypeMapper::map_type(p))
            .collect::<Vec<_>>()
            .join(", ");

        let params_ffi = sig
            .params
            .iter()
            .map(|p| TypeMapper::ffi_type(p))
            .collect::<Vec<_>>()
            .join(", ");

        Some(Self {
            param_name: param_name.clone(),
            swift_type: params_swift,
            ffi_arg_type: params_ffi,
            context_type: format!("{}CallbackFn{}", func_name_pascal, suffix),
            box_type: format!("{}CallbackBox{}", func_name_pascal, suffix),
            box_name: format!("{}Box{}", param_name, suffix),
            ptr_name: format!("{}Ptr{}", param_name, suffix),
            trampoline_name: format!("{}Trampoline{}", param_name, suffix),
        })
    }
}

pub struct ParamsInfo {
    pub params: Vec<ParamInfo>,
    pub callbacks: Vec<CallbackInfo>,
    pub has_string_params: bool,
    pub has_slice_params: bool,
    pub has_vec_params: bool,
    pub has_callbacks: bool,
    pub has_pointer_params: bool,
}

impl ParamsInfo {
    pub fn from_inputs<'a>(
        inputs: impl Iterator<Item = (&'a str, &'a Type)>,
        func_name_pascal: &str,
    ) -> Self {
        let mut params = Vec::new();
        let mut callbacks = Vec::new();
        let mut seen_pointer = false;
        let mut callback_index = 0;

        for (name, ty) in inputs {
            let is_first = !seen_pointer;
            let info = ParamInfo::from_param(name, ty, is_first);
            if info.needs_wrapper() {
                seen_pointer = true;
            }
            params.push(info);

            if matches!(ty, Type::Closure(_)) {
                if let Some(cb) =
                    CallbackInfo::from_param(name, ty, func_name_pascal, callback_index)
                {
                    callbacks.push(cb);
                    callback_index += 1;
                }
            }
        }

        let has_string_params = params.iter().any(|p| p.is_string);
        let has_slice_params = params.iter().any(|p| p.is_slice || p.is_mut_slice);
        let has_vec_params = params.iter().any(|p| p.is_vec);
        let has_callbacks = !callbacks.is_empty();
        let has_pointer_params = params.iter().any(|p| p.needs_wrapper());

        Self {
            params,
            callbacks,
            has_string_params,
            has_slice_params,
            has_vec_params,
            has_callbacks,
            has_pointer_params,
        }
    }
}
