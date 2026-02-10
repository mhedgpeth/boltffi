#[derive(Debug, Clone)]
pub struct TsModule {
    pub module_name: String,
    pub records: Vec<TsRecord>,
    pub enums: Vec<TsEnum>,
    pub functions: Vec<TsFunction>,
    pub wasm_imports: Vec<TsWasmImport>,
}

#[derive(Debug, Clone)]
pub struct TsRecord {
    pub name: String,
    pub fields: Vec<TsField>,
    pub is_blittable: bool,
    pub wire_size: Option<usize>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TsField {
    pub name: String,
    pub ts_type: String,
    pub wire_decode_expr: String,
    pub wire_encode_expr: String,
    pub wire_size_expr: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TsEnum {
    pub name: String,
    pub variants: Vec<TsVariant>,
    pub kind: TsEnumKind,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum TsEnumKind {
    CStyle,
    Data,
}

impl TsEnum {
    pub fn is_c_style(&self) -> bool {
        matches!(self.kind, TsEnumKind::CStyle)
    }
}

#[derive(Debug, Clone)]
pub struct TsVariant {
    pub name: String,
    pub discriminant: i64,
    pub fields: Vec<TsVariantField>,
    pub doc: Option<String>,
}

impl TsVariant {
    pub fn is_unit(&self) -> bool {
        self.fields.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct TsVariantField {
    pub name: String,
    pub ts_type: String,
    pub wire_decode_expr: String,
    pub wire_encode_expr: String,
    pub wire_size_expr: String,
}

#[derive(Debug, Clone)]
pub struct TsFunction {
    pub name: String,
    pub ffi_name: String,
    pub params: Vec<TsParam>,
    pub return_type: Option<String>,
    pub return_abi: TsReturnAbi,
    pub decode_expr: String,
    pub throws: bool,
    pub err_type: String,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TsParam {
    pub name: String,
    pub ts_type: String,
    pub conversion: TsParamConversion,
}

#[derive(Debug, Clone)]
pub enum TsParamConversion {
    Direct,
    String,
    WireEncoded {
        encode_expr: String,
        size_expr: String,
    },
}

#[derive(Debug, Clone)]
pub enum TsReturnAbi {
    Void,
    Direct { ts_cast: String },
    WireEncoded,
}

impl TsReturnAbi {
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }

    pub fn is_wire_encoded(&self) -> bool {
        matches!(self, Self::WireEncoded)
    }
}

#[derive(Debug, Clone)]
pub struct TsWasmImport {
    pub ffi_name: String,
    pub params: Vec<TsWasmParam>,
    pub return_wasm_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TsWasmParam {
    pub name: String,
    pub wasm_type: String,
}
