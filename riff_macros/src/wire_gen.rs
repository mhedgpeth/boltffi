use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemEnum, ItemStruct, Type};

fn is_primitive_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            if let Some(ident) = type_path.path.get_ident() {
                let name = ident.to_string();
                matches!(
                    name.as_str(),
                    "bool"
                        | "i8"
                        | "u8"
                        | "i16"
                        | "u16"
                        | "i32"
                        | "u32"
                        | "i64"
                        | "u64"
                        | "f32"
                        | "f64"
                        | "isize"
                        | "usize"
                )
            } else {
                false
            }
        }
        _ => false,
    }
}

fn is_struct_blittable(field_types: &[&Type]) -> bool {
    field_types.iter().all(|ty| is_primitive_type(ty))
}

pub fn generate_wire_impls(item_struct: &ItemStruct) -> TokenStream {
    let struct_name = &item_struct.ident;
    let (impl_generics, ty_generics, where_clause) = item_struct.generics.split_for_impl();

    let fields = match &item_struct.fields {
        Fields::Named(named) => &named.named,
        _ => return quote! {},
    };

    if fields.is_empty() {
        return generate_empty_struct_impls(struct_name);
    }

    let field_names: Vec<_> = fields.iter().filter_map(|f| f.ident.as_ref()).collect();

    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let is_blittable = is_struct_blittable(&field_types);

    let wire_size_impl = generate_wire_size_impl(
        struct_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &field_names,
        &field_types,
        is_blittable,
    );

    let wire_encode_impl = generate_wire_encode_impl(
        struct_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &field_names,
        is_blittable,
    );

    let wire_decode_impl = generate_wire_decode_impl(
        struct_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &field_names,
        &field_types,
        is_blittable,
    );

    quote! {
        #wire_size_impl
        #wire_encode_impl
        #wire_decode_impl
    }
}

fn generate_empty_struct_impls(struct_name: &syn::Ident) -> TokenStream {
    quote! {
        impl riff_core::wire::WireSize for #struct_name {
            fn is_fixed_size() -> bool { true }
            fn fixed_size() -> Option<usize> { Some(2) }
            fn wire_size(&self) -> usize { 2 }
        }

        impl riff_core::wire::WireEncode for #struct_name {
            fn encode_to(&self, buf: &mut [u8]) -> usize {
                buf[0..2].copy_from_slice(&0u16.to_le_bytes());
                2
            }
        }

        impl riff_core::wire::WireDecode for #struct_name {
            fn decode_from(buf: &[u8]) -> riff_core::wire::DecodeResult<Self> {
                if buf.len() < 2 {
                    return Err(riff_core::wire::DecodeError::BufferTooSmall);
                }
                Ok((Self {}, 2))
            }
        }
    }
}

fn generate_wire_size_impl(
    struct_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    field_names: &[&syn::Ident],
    field_types: &[&Type],
    is_blittable: bool,
) -> TokenStream {
    if is_blittable {
        return quote! {
            impl #impl_generics riff_core::wire::WireSize for #struct_name #ty_generics #where_clause {
                fn is_fixed_size() -> bool { true }
                fn fixed_size() -> Option<usize> { Some(core::mem::size_of::<Self>()) }
                fn wire_size(&self) -> usize { core::mem::size_of::<Self>() }
            }
        };
    }

    let all_fixed_check = field_types.iter().map(|ty| {
        quote! { <#ty as riff_core::wire::WireSize>::is_fixed_size() }
    });

    let fixed_size_sum = field_types.iter().map(|ty| {
        quote! { <#ty as riff_core::wire::WireSize>::fixed_size().unwrap_or(0) }
    });

    let field_wire_sizes = field_names.iter().map(|name| {
        quote! { riff_core::wire::WireSize::wire_size(&self.#name) }
    });

    quote! {
        impl #impl_generics riff_core::wire::WireSize for #struct_name #ty_generics #where_clause {
            fn is_fixed_size() -> bool {
                #(#all_fixed_check)&&*
            }

            fn fixed_size() -> Option<usize> {
                if <Self as riff_core::wire::WireSize>::is_fixed_size() {
                    Some(#(#fixed_size_sum)+*)
                } else {
                    None
                }
            }

            fn wire_size(&self) -> usize {
                <Self as riff_core::wire::WireSize>::fixed_size().unwrap_or_else(|| {
                    #(#field_wire_sizes)+*
                })
            }
        }
    }
}

fn generate_wire_encode_impl(
    struct_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    field_names: &[&syn::Ident],
    is_blittable: bool,
) -> TokenStream {
    if is_blittable {
        return quote! {
            impl #impl_generics riff_core::wire::WireEncode for #struct_name #ty_generics #where_clause {
                fn encode_to(&self, buf: &mut [u8]) -> usize {
                    let size = core::mem::size_of::<Self>();
                    let src = self as *const Self as *const u8;
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), size);
                    }
                    size
                }
            }
        };
    }

    let encode_fields = field_names.iter().map(|name| {
        quote! {
            written += riff_core::wire::WireEncode::encode_to(&self.#name, &mut buf[written..]);
        }
    });

    quote! {
        impl #impl_generics riff_core::wire::WireEncode for #struct_name #ty_generics #where_clause {
            fn encode_to(&self, buf: &mut [u8]) -> usize {
                let mut written = 0usize;
                #(#encode_fields)*
                written
            }
        }
    }
}

fn generate_wire_decode_impl(
    struct_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    field_names: &[&syn::Ident],
    field_types: &[&Type],
    is_blittable: bool,
) -> TokenStream {
    let field_names_for_struct: Vec<_> = field_names.iter().map(|n| quote! { #n }).collect();

    if is_blittable {
        return quote! {
            impl #impl_generics riff_core::wire::WireDecode for #struct_name #ty_generics #where_clause {
                const IS_BLITTABLE: bool = true;

                fn decode_from(buf: &[u8]) -> riff_core::wire::DecodeResult<Self> {
                    let size = core::mem::size_of::<Self>();
                    if buf.len() < size {
                        return Err(riff_core::wire::DecodeError::BufferTooSmall);
                    }
                    let value = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const Self) };
                    Ok((value, size))
                }
            }
        };
    }

    let decode_fields = field_names.iter().zip(field_types.iter()).map(|(name, ty)| {
        quote! {
            let (#name, size) = <#ty as riff_core::wire::WireDecode>::decode_from(&buf[position..])?;
            position += size;
        }
    });

    quote! {
        impl #impl_generics riff_core::wire::WireDecode for #struct_name #ty_generics #where_clause {
            const IS_BLITTABLE: bool = false;

            fn decode_from(buf: &[u8]) -> riff_core::wire::DecodeResult<Self> {
                let mut position = 0usize;
                #(#decode_fields)*
                Ok((Self { #(#field_names_for_struct),* }, position))
            }
        }
    }
}

pub fn generate_enum_wire_impls(item_enum: &ItemEnum) -> TokenStream {
    let enum_name = &item_enum.ident;
    let (impl_generics, ty_generics, where_clause) = item_enum.generics.split_for_impl();

    let variants: Vec<_> = item_enum.variants.iter().collect();

    if variants.is_empty() {
        return quote! {};
    }

    let wire_size_impl = generate_enum_wire_size_impl(
        enum_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &variants,
    );

    let wire_encode_impl = generate_enum_wire_encode_impl(
        enum_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &variants,
    );

    let wire_decode_impl = generate_enum_wire_decode_impl(
        enum_name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &variants,
    );

    quote! {
        #wire_size_impl
        #wire_encode_impl
        #wire_decode_impl
    }
}

fn generate_enum_wire_size_impl(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    variants: &[&syn::Variant],
) -> TokenStream {
    let all_unit = variants.iter().all(|v| v.fields.is_empty());

    let wire_size_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        match &variant.fields {
            Fields::Unit => {
                quote! { Self::#variant_name => 4 }
            }
            Fields::Unnamed(fields) => {
                let field_bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("f{}", i))
                    .collect();
                quote! {
                    Self::#variant_name(#(#field_bindings),*) => {
                        4 + #(riff_core::wire::WireSize::wire_size(#field_bindings))+*
                    }
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields
                    .named
                    .iter()
                    .filter_map(|f| f.ident.as_ref())
                    .collect();
                quote! {
                    Self::#variant_name { #(#field_names),* } => {
                        4 + #(riff_core::wire::WireSize::wire_size(#field_names))+*
                    }
                }
            }
        }
    });

    if all_unit {
        quote! {
            impl #impl_generics riff_core::wire::WireSize for #enum_name #ty_generics #where_clause {
                fn is_fixed_size() -> bool { true }
                fn fixed_size() -> Option<usize> { Some(4) }
                fn wire_size(&self) -> usize { 4 }
            }
        }
    } else {
        quote! {
            impl #impl_generics riff_core::wire::WireSize for #enum_name #ty_generics #where_clause {
                fn is_fixed_size() -> bool { false }
                fn fixed_size() -> Option<usize> { None }
                fn wire_size(&self) -> usize {
                    match self {
                        #(#wire_size_arms),*
                    }
                }
            }
        }
    }
}

fn generate_enum_wire_encode_impl(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    variants: &[&syn::Variant],
) -> TokenStream {
    let encode_arms = variants.iter().enumerate().map(|(discriminant, variant)| {
        let variant_name = &variant.ident;
        let discriminant_i32 = discriminant as i32;
        
        match &variant.fields {
            Fields::Unit => {
                quote! {
                    Self::#variant_name => {
                        buf[0..4].copy_from_slice(&(#discriminant_i32 as i32).to_le_bytes());
                        4
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let field_bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("f{}", i))
                    .collect();
                let encode_fields = field_bindings.iter().map(|f| {
                    quote! { written += riff_core::wire::WireEncode::encode_to(#f, &mut buf[written..]); }
                });
                quote! {
                    Self::#variant_name(#(#field_bindings),*) => {
                        buf[0..4].copy_from_slice(&(#discriminant_i32 as i32).to_le_bytes());
                        let mut written = 4usize;
                        #(#encode_fields)*
                        written
                    }
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields.named.iter()
                    .filter_map(|f| f.ident.as_ref())
                    .collect();
                let encode_fields = field_names.iter().map(|f| {
                    quote! { written += riff_core::wire::WireEncode::encode_to(#f, &mut buf[written..]); }
                });
                quote! {
                    Self::#variant_name { #(#field_names),* } => {
                        buf[0..4].copy_from_slice(&(#discriminant_i32 as i32).to_le_bytes());
                        let mut written = 4usize;
                        #(#encode_fields)*
                        written
                    }
                }
            }
        }
    });

    quote! {
        impl #impl_generics riff_core::wire::WireEncode for #enum_name #ty_generics #where_clause {
            fn encode_to(&self, buf: &mut [u8]) -> usize {
                match self {
                    #(#encode_arms),*
                }
            }
        }
    }
}

fn generate_enum_wire_decode_impl(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    variants: &[&syn::Variant],
) -> TokenStream {
    let decode_arms = variants.iter().enumerate().map(|(discriminant, variant)| {
        let variant_name = &variant.ident;
        let discriminant_i32 = discriminant as i32;
        
        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #discriminant_i32 => Ok((Self::#variant_name, 4))
                }
            }
            Fields::Unnamed(fields) => {
                let field_types: Vec<_> = fields.unnamed.iter()
                    .map(|f| &f.ty)
                    .collect();
                let field_bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| quote::format_ident!("f{}", i))
                    .collect();
                let decode_fields = field_bindings.iter().zip(field_types.iter()).map(|(binding, ty)| {
                    quote! {
                        let (#binding, size) = <#ty as riff_core::wire::WireDecode>::decode_from(&buf[position..])?;
                        position += size;
                    }
                });
                quote! {
                    #discriminant_i32 => {
                        let mut position = 4usize;
                        #(#decode_fields)*
                        Ok((Self::#variant_name(#(#field_bindings),*), position))
                    }
                }
            }
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields.named.iter()
                    .filter_map(|f| f.ident.as_ref())
                    .collect();
                let field_types: Vec<_> = fields.named.iter()
                    .map(|f| &f.ty)
                    .collect();
                let decode_fields = field_names.iter().zip(field_types.iter()).map(|(name, ty)| {
                    quote! {
                        let (#name, size) = <#ty as riff_core::wire::WireDecode>::decode_from(&buf[position..])?;
                        position += size;
                    }
                });
                quote! {
                    #discriminant_i32 => {
                        let mut position = 4usize;
                        #(#decode_fields)*
                        Ok((Self::#variant_name { #(#field_names),* }, position))
                    }
                }
            }
        }
    });

    quote! {
        impl #impl_generics riff_core::wire::WireDecode for #enum_name #ty_generics #where_clause {
            fn decode_from(buf: &[u8]) -> riff_core::wire::DecodeResult<Self> {
                let disc_bytes: [u8; 4] = buf.get(0..4)
                    .ok_or(riff_core::wire::DecodeError::BufferTooSmall)?
                    .try_into()
                    .map_err(|_| riff_core::wire::DecodeError::BufferTooSmall)?;
                let discriminant = i32::from_le_bytes(disc_bytes);
                match discriminant {
                    #(#decode_arms),*,
                    _ => Err(riff_core::wire::DecodeError::BufferTooSmall)
                }
            }
        }
    }
}
