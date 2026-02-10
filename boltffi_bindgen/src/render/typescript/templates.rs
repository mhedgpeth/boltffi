use crate::render::typescript::plan::*;

pub struct TypeScriptEmitter;

impl TypeScriptEmitter {
    pub fn emit(module: &TsModule) -> String {
        let mut output = String::new();

        output.push_str(&Self::emit_preamble(module));

        for record in &module.records {
            output.push_str(&Self::emit_record(record));
        }

        for enumeration in &module.enums {
            output.push_str(&Self::emit_enum(enumeration));
        }

        for function in &module.functions {
            output.push_str(&Self::emit_function(function));
        }

        output.push_str(&Self::emit_wasm_interface(module));

        output
    }

    fn emit_preamble(_module: &TsModule) -> String {
        format!(
            "import {{ WireReader, WireWriter, BoltFFIModule }} from \"@boltffi/runtime\";\n\n"
        )
    }

    fn emit_record(record: &TsRecord) -> String {
        let mut output = String::new();

        if let Some(doc) = &record.doc {
            output.push_str(&format!("/** {} */\n", doc));
        }

        output.push_str(&format!("export interface {} {{\n", record.name));
        for field in &record.fields {
            if let Some(doc) = &field.doc {
                output.push_str(&format!("  /** {} */\n", doc));
            }
            output.push_str(&format!(
                "  readonly {}: {};\n",
                field.name, field.ts_type
            ));
        }
        output.push_str("}\n\n");

        output.push_str(&format!(
            "function decode{}(reader: WireReader): {} {{\n",
            record.name, record.name
        ));
        output.push_str("  return {\n");
        for field in &record.fields {
            output.push_str(&format!(
                "    {}: {},\n",
                field.name, field.wire_decode_expr
            ));
        }
        output.push_str("  };\n");
        output.push_str("}\n\n");

        output.push_str(&format!(
            "function encode{}(writer: WireWriter, value: {}): void {{\n",
            record.name, record.name
        ));
        for field in &record.fields {
            let encode = field
                .wire_encode_expr
                .replace("this.", "value.");
            output.push_str(&format!("  {};\n", encode));
        }
        output.push_str("}\n\n");

        output
    }

    fn emit_enum(enumeration: &TsEnum) -> String {
        let mut output = String::new();

        if let Some(doc) = &enumeration.doc {
            output.push_str(&format!("/** {} */\n", doc));
        }

        if enumeration.is_c_style() {
            output.push_str(&format!("export enum {} {{\n", enumeration.name));
            for variant in &enumeration.variants {
                if let Some(doc) = &variant.doc {
                    output.push_str(&format!("  /** {} */\n", doc));
                }
                output.push_str(&format!(
                    "  {} = {},\n",
                    variant.name, variant.discriminant
                ));
            }
            output.push_str("}\n\n");

            output.push_str(&format!(
                "function decode{}(value: number): {} {{\n",
                enumeration.name, enumeration.name
            ));
            output.push_str("  switch (value) {\n");
            for variant in &enumeration.variants {
                output.push_str(&format!(
                    "    case {}: return {}.{};\n",
                    variant.discriminant, enumeration.name, variant.name
                ));
            }
            output.push_str(&format!(
                "    default: throw new Error(`Unknown {} discriminant: ${{value}}`);\n",
                enumeration.name
            ));
            output.push_str("  }\n");
            output.push_str("}\n\n");
        } else {
            output.push_str(&format!("export type {} =\n", enumeration.name));
            for (idx, variant) in enumeration.variants.iter().enumerate() {
                let separator = if idx < enumeration.variants.len() - 1 {
                    " |"
                } else {
                    ";"
                };
                if variant.is_unit() {
                    output.push_str(&format!(
                        "  | {{ readonly tag: \"{}\"{} }}{}\n",
                        variant.name, "", separator
                    ));
                } else {
                    let field_types: Vec<String> = variant
                        .fields
                        .iter()
                        .map(|f| format!("readonly {}: {}", f.name, f.ts_type))
                        .collect();
                    output.push_str(&format!(
                        "  | {{ readonly tag: \"{}\"; {} }}{}\n",
                        variant.name,
                        field_types.join("; "),
                        separator
                    ));
                }
            }
            output.push('\n');

            output.push_str(&format!(
                "function decode{}(reader: WireReader): {} {{\n",
                enumeration.name, enumeration.name
            ));
            output.push_str("  const tag = reader.readI32();\n");
            output.push_str("  switch (tag) {\n");
            for variant in &enumeration.variants {
                output.push_str(&format!("    case {}:\n", variant.discriminant));
                if variant.is_unit() {
                    output.push_str(&format!(
                        "      return {{ tag: \"{}\" }};\n",
                        variant.name
                    ));
                } else {
                    let decode_fields: Vec<String> = variant
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, f.wire_decode_expr))
                        .collect();
                    output.push_str(&format!(
                        "      return {{ tag: \"{}\", {} }};\n",
                        variant.name,
                        decode_fields.join(", ")
                    ));
                }
            }
            output.push_str(&format!(
                "    default: throw new Error(`Unknown {} discriminant: ${{tag}}`);\n",
                enumeration.name
            ));
            output.push_str("  }\n");
            output.push_str("}\n\n");
        }

        output
    }

    fn emit_function(function: &TsFunction) -> String {
        let mut output = String::new();

        if let Some(doc) = &function.doc {
            output.push_str(&format!("/** {} */\n", doc));
        }

        let param_list: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ts_type))
            .collect();

        let return_type = function
            .return_type
            .as_deref()
            .unwrap_or("void");

        output.push_str(&format!(
            "export function {}(module: BoltFFIModule, {}): {} {{\n",
            function.name,
            param_list.join(", "),
            return_type
        ));

        match &function.return_abi {
            TsReturnAbi::Void => {
                output.push_str(&format!(
                    "  module.exports.{}({});\n",
                    function.ffi_name,
                    function
                        .params
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            TsReturnAbi::Direct { ts_cast } => {
                let call_args = function
                    .params
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!(
                    "  return module.exports.{}({}){};",
                    function.ffi_name, call_args, ts_cast
                ));
                output.push('\n');
            }
            TsReturnAbi::WireEncoded => {
                let call_args = function
                    .params
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!(
                    "  const bufPtr = module.exports.{}({});\n",
                    function.ffi_name, call_args
                ));
                output.push_str(
                    "  const reader = module.readerFromBuf(bufPtr);\n",
                );
                output.push_str(&format!(
                    "  const result = {};\n",
                    function.decode_expr
                ));
                output.push_str("  module.freeBuf(bufPtr);\n");
                output.push_str("  return result;\n");
            }
        }

        output.push_str("}\n\n");
        output
    }

    fn emit_wasm_interface(module: &TsModule) -> String {
        let mut output = String::new();

        output.push_str("export interface WasmExports {\n");
        output.push_str("  boltffi_wasm_abi_version(): number;\n");
        output.push_str("  boltffi_wasm_alloc(size: number): number;\n");
        output.push_str("  boltffi_wasm_free(ptr: number, size: number): void;\n");
        output.push_str("  boltffi_free_buf_u8(ptr: number): void;\n");
        output.push_str("  boltffi_last_error_message(outPtr: number): number;\n");
        output.push_str("  boltffi_clear_last_error(): void;\n");

        for import in &module.wasm_imports {
            let params: Vec<String> = import
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.wasm_type))
                .collect();
            let return_type = import
                .return_wasm_type
                .as_deref()
                .unwrap_or("void");
            output.push_str(&format!(
                "  {}({}): {};\n",
                import.ffi_name,
                params.join(", "),
                return_type
            ));
        }

        output.push_str("}\n");

        output
    }
}
