use boltffi_ffi_rules::classification::{self, FieldPrimitive, PassableCategory};
use proc_macro2::Span;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use syn::{Attribute, Fields, Item, ItemEnum, ItemStruct, Type};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataTypeCategory {
    Scalar,
    Blittable,
    WireEncoded,
}

impl DataTypeCategory {
    pub fn supports_direct_vec(self) -> bool {
        matches!(self, Self::Scalar | Self::Blittable)
    }
}

#[derive(Default, Clone)]
pub struct DataTypeRegistry {
    categories_by_path: HashMap<Vec<String>, DataTypeCategory>,
    unique_name_categories: HashMap<String, DataTypeCategory>,
}

impl DataTypeRegistry {
    fn insert(&mut self, qualified_path: Vec<String>, category: DataTypeCategory) {
        self.categories_by_path.insert(qualified_path, category);
    }

    fn finalize_unique_names(&mut self) {
        let name_counts = self.categories_by_path.keys().fold(
            HashMap::<String, usize>::new(),
            |mut counts, qualified_path| {
                if let Some(name) = qualified_path.last() {
                    *counts.entry(name.clone()).or_insert(0) += 1;
                }
                counts
            },
        );

        self.unique_name_categories = self.categories_by_path.iter().fold(
            HashMap::<String, DataTypeCategory>::new(),
            |mut unique, (qualified_path, category)| {
                if let Some(name) = qualified_path.last()
                    && name_counts.get(name).copied() == Some(1)
                {
                    unique.insert(name.clone(), *category);
                }
                unique
            },
        );
    }

    pub fn category_for(&self, ty: &Type) -> Option<DataTypeCategory> {
        let names = type_path_names(ty)?;
        if names.len() == 1 {
            return names
                .first()
                .and_then(|name| self.unique_name_categories.get(name).copied());
        }

        if let Some(category) = self.categories_by_path.get(&names).copied() {
            return Some(category);
        }

        let mut matches = self
            .categories_by_path
            .iter()
            .filter(|(registered_path, _)| path_suffix_matches(&names, registered_path))
            .map(|(_, category)| *category);
        let first = matches.next()?;
        matches.all(|next| next == first).then_some(first)
    }
}

fn path_suffix_matches(path: &[String], suffix: &[String]) -> bool {
    path.len() >= suffix.len() && path[path.len() - suffix.len()..] == *suffix
}

fn type_path_names(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::Path(type_path) if type_path.qself.is_none() => Some(
            type_path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect(),
        ),
        Type::Group(group) => type_path_names(group.elem.as_ref()),
        Type::Paren(paren) => type_path_names(paren.elem.as_ref()),
        _ => None,
    }
}

static REGISTRY_CACHE: OnceLock<Mutex<HashMap<PathBuf, DataTypeRegistry>>> = OnceLock::new();

pub fn registry_for_current_crate() -> syn::Result<DataTypeRegistry> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(Span::call_site(), "CARGO_MANIFEST_DIR not set"))?;
    let manifest_dir = PathBuf::from(manifest_dir);

    let cache = REGISTRY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cached = cache
        .lock()
        .map_err(|_| syn::Error::new(Span::call_site(), "data type registry lock poisoned"))?
        .get(&manifest_dir)
        .cloned();

    if let Some(registry) = cached {
        return Ok(registry);
    }

    let registry = build_registry(&manifest_dir)?;
    let mut guard = cache
        .lock()
        .map_err(|_| syn::Error::new(Span::call_site(), "data type registry lock poisoned"))?;
    guard.insert(manifest_dir, registry.clone());
    Ok(registry)
}

fn build_registry(manifest_dir: &Path) -> syn::Result<DataTypeRegistry> {
    let mut registry = DataTypeRegistry::default();
    source_roots(manifest_dir)
        .iter()
        .try_for_each(|root| collect_root_types(root, &mut registry))?;

    registry.finalize_unique_names();
    Ok(registry)
}

fn source_roots(manifest_dir: &Path) -> Vec<PathBuf> {
    let src = manifest_dir.join("src");
    if src.is_dir() { vec![src] } else { Vec::new() }
}

fn collect_root_types(root: &Path, registry: &mut DataTypeRegistry) -> syn::Result<()> {
    let files = list_rs_files(root)?;
    files.iter().try_for_each(|file_path| {
        let module_path = module_path_for_rs_file(root, file_path)?;
        let content = fs::read_to_string(file_path).map_err(|error| {
            syn::Error::new(
                Span::call_site(),
                format!("read {}: {}", file_path.display(), error),
            )
        })?;
        let syntax = syn::parse_file(&content)?;

        let mut collector = DataTypeCollector {
            module_path,
            registry,
        };
        syntax
            .items
            .iter()
            .try_for_each(|item| collector.collect_item(item))
    })
}

struct DataTypeCollector<'a> {
    module_path: Vec<String>,
    registry: &'a mut DataTypeRegistry,
}

impl<'a> DataTypeCollector<'a> {
    fn collect_item(&mut self, item: &Item) -> syn::Result<()> {
        match item {
            Item::Struct(item_struct) => {
                self.collect_struct(item_struct);
                Ok(())
            }
            Item::Enum(item_enum) => {
                self.collect_enum(item_enum);
                Ok(())
            }
            Item::Mod(item_mod) => {
                let Some((_, items)) = &item_mod.content else {
                    return Ok(());
                };
                self.module_path.push(item_mod.ident.to_string());
                let collect_result = items
                    .iter()
                    .try_for_each(|nested| self.collect_item(nested));
                self.module_path.pop();
                collect_result
            }
            _ => Ok(()),
        }
    }

    fn collect_struct(&mut self, item_struct: &ItemStruct) {
        if !has_data_marker(&item_struct.attrs) {
            return;
        }
        let category = classify_struct(item_struct);
        let mut qualified_path = self.module_path.clone();
        qualified_path.push(item_struct.ident.to_string());
        self.registry.insert(qualified_path, category);
    }

    fn collect_enum(&mut self, item_enum: &ItemEnum) {
        if !has_data_marker(&item_enum.attrs) {
            return;
        }
        let category = classify_enum(item_enum);
        let mut qualified_path = self.module_path.clone();
        qualified_path.push(item_enum.ident.to_string());
        self.registry.insert(qualified_path, category);
    }
}

fn classify_struct(item_struct: &ItemStruct) -> DataTypeCategory {
    let struct_has_repr_c = has_effective_repr_c(&item_struct.attrs);
    let field_primitives: Vec<FieldPrimitive> = match &item_struct.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| field_primitive(&field.ty))
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .filter_map(|field| field_primitive(&field.ty))
            .collect(),
        Fields::Unit => Vec::new(),
    };
    let total_fields = match &item_struct.fields {
        Fields::Named(named) => named.named.len(),
        Fields::Unnamed(unnamed) => unnamed.unnamed.len(),
        Fields::Unit => 0,
    };
    let classify_fields = if field_primitives.len() == total_fields {
        field_primitives
    } else {
        Vec::new()
    };

    match classification::classify_struct(struct_has_repr_c, &classify_fields) {
        PassableCategory::Blittable => DataTypeCategory::Blittable,
        PassableCategory::Scalar | PassableCategory::WireEncoded => DataTypeCategory::WireEncoded,
    }
}

fn has_effective_repr_c(attrs: &[Attribute]) -> bool {
    has_repr_c(attrs) || !has_any_repr(attrs)
}

fn has_any_repr(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("repr"))
}

fn classify_enum(item_enum: &ItemEnum) -> DataTypeCategory {
    let is_c_style = item_enum
        .variants
        .iter()
        .all(|variant| variant.fields.is_empty());
    let has_explicit_integer_repr = extract_integer_repr(&item_enum.attrs).is_some();
    let has_effective_integer_repr =
        has_explicit_integer_repr || (is_c_style && !has_any_repr(&item_enum.attrs));
    match classification::classify_enum(is_c_style, has_effective_integer_repr) {
        PassableCategory::Scalar => DataTypeCategory::Scalar,
        PassableCategory::Blittable | PassableCategory::WireEncoded => {
            DataTypeCategory::WireEncoded
        }
    }
}

fn has_data_marker(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        attribute.path().is_ident("data")
            || attribute.path().is_ident("error")
            || attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "data" || segment.ident == "error")
    })
}

pub fn has_repr_c(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("repr"))
        .any(|attribute| {
            attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )
                .ok()
                .is_some_and(|items| {
                    items.into_iter().any(|item| match item {
                        syn::Meta::Path(path) => path.is_ident("C"),
                        _ => false,
                    })
                })
        })
}

pub fn extract_integer_repr(attrs: &[Attribute]) -> Option<syn::Ident> {
    attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("repr"))
        .find_map(|attribute| {
            attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
                )
                .ok()
                .and_then(|idents| {
                    idents.into_iter().find(|ident| {
                        matches!(
                            ident.to_string().as_str(),
                            "i8" | "i16"
                                | "i32"
                                | "i64"
                                | "u8"
                                | "u16"
                                | "u32"
                                | "u64"
                                | "isize"
                                | "usize"
                        )
                    })
                })
        })
}

fn field_primitive(ty: &Type) -> Option<FieldPrimitive> {
    match ty {
        Type::Path(path) => path
            .path
            .get_ident()
            .and_then(|ident| FieldPrimitive::from_type_name(&ident.to_string())),
        _ => None,
    }
}

fn list_rs_files(src_root: &Path) -> syn::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_rs_files(src_root, &mut files)?;
    Ok(files)
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> syn::Result<()> {
    let entries = fs::read_dir(dir).map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("read_dir {}: {}", dir.display(), error),
        )
    })?;

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .try_for_each(|path| {
            if path.is_dir() {
                collect_rs_files(&path, files)
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
                Ok(())
            } else {
                Ok(())
            }
        })
}

fn module_path_for_rs_file(src_root: &Path, file_path: &Path) -> syn::Result<Vec<String>> {
    let relative = file_path
        .strip_prefix(src_root)
        .map_err(|_| syn::Error::new(Span::call_site(), "path not under src"))?;
    let mut parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let file_name = parts.pop().unwrap_or_default();
    let mut module_path = parts;
    if file_name == "mod.rs" {
        return Ok(module_path);
    }

    if let Some(stem) = file_name.strip_suffix(".rs")
        && stem != "lib"
        && stem != "main"
    {
        module_path.push(stem.to_string());
    }

    Ok(module_path)
}
