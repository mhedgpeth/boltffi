pub(super) struct BoltffiFieldAttributes;

#[derive(Clone)]
enum BoltffiFieldAttribute {
    Default,
    Extension(syn::Ident),
}

impl BoltffiFieldAttributes {
    pub(super) fn strip_from_fields(fields: &mut syn::Fields) {
        fields
            .iter_mut()
            .for_each(|field| Self::strip_from_field(field));
    }

    fn strip_from_field(field: &mut syn::Field) {
        field
            .attrs
            .retain(|attribute| match BoltffiFieldAttribute::parse(attribute) {
                Some(field_attribute) => {
                    std::mem::drop(field_attribute.attribute_name());
                    false
                }
                None => true,
            });
    }
}

impl BoltffiFieldAttribute {
    fn parse(attribute: &syn::Attribute) -> Option<Self> {
        let mut segments = attribute.path().segments.iter();
        let boltffi_segment = segments.next()?;
        let field_attribute_segment = segments.next()?;
        if segments.next().is_some() || boltffi_segment.ident != "boltffi" {
            return None;
        }

        Some(match field_attribute_segment.ident.to_string().as_str() {
            "default" => Self::Default,
            _ => Self::Extension(field_attribute_segment.ident.clone()),
        })
    }

    fn attribute_name(&self) -> String {
        match self {
            Self::Default => "default".to_string(),
            Self::Extension(attribute_name) => attribute_name.to_string(),
        }
    }
}
