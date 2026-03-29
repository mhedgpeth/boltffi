use quote::quote;

pub(crate) struct AsyncExportNames {
    entry: syn::Ident,
    poll: syn::Ident,
    poll_sync: syn::Ident,
    complete: syn::Ident,
    panic_message: syn::Ident,
    cancel: syn::Ident,
    free: syn::Ident,
}

pub(crate) struct AsyncRuntimeExports<'a> {
    pub(crate) visibility: &'a syn::Visibility,
    pub(crate) names: &'a AsyncExportNames,
    pub(crate) rust_return_type: proc_macro2::TokenStream,
    pub(crate) ffi_return_type: proc_macro2::TokenStream,
    pub(crate) complete_conversion: proc_macro2::TokenStream,
    pub(crate) default_value: proc_macro2::TokenStream,
}

pub(crate) struct AsyncWasmCompleteExport {
    pub(crate) params: proc_macro2::TokenStream,
    pub(crate) return_type: proc_macro2::TokenStream,
    pub(crate) body: proc_macro2::TokenStream,
}

impl AsyncExportNames {
    pub(crate) fn new(base_name: &str, span: proc_macro2::Span) -> Self {
        Self {
            entry: syn::Ident::new(base_name, span),
            poll: syn::Ident::new(&format!("{}_poll", base_name), span),
            poll_sync: syn::Ident::new(&format!("{}_poll_sync", base_name), span),
            complete: syn::Ident::new(&format!("{}_complete", base_name), span),
            panic_message: syn::Ident::new(&format!("{}_panic_message", base_name), span),
            cancel: syn::Ident::new(&format!("{}_cancel", base_name), span),
            free: syn::Ident::new(&format!("{}_free", base_name), span),
        }
    }

    pub(crate) fn entry(&self) -> &syn::Ident {
        &self.entry
    }
}

impl<'a> AsyncRuntimeExports<'a> {
    pub(crate) fn render(
        &self,
        wasm_complete: AsyncWasmCompleteExport,
    ) -> proc_macro2::TokenStream {
        let native_complete = self.render_native_complete();
        let wasm_complete = self.render_wasm_complete(wasm_complete);
        let native_poll = self.render_native_poll();
        let wasm_poll = self.render_wasm_poll();
        let wasm_panic_message = self.render_wasm_panic_message();
        let cancel = self.render_cancel();
        let free = self.render_free();

        quote! {
            #native_poll
            #wasm_poll
            #wasm_panic_message
            #native_complete
            #wasm_complete
            #cancel
            #free
        }
    }

    fn render_native_complete(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let complete_ident = &self.names.complete;
        let rust_return_type = &self.rust_return_type;
        let ffi_return_type = &self.ffi_return_type;
        let complete_conversion = &self.complete_conversion;
        let default_value = &self.default_value;

        quote! {
            #[cfg(not(target_arch = "wasm32"))]
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #complete_ident(
                handle: ::boltffi::__private::RustFutureHandle,
                out_status: *mut ::boltffi::__private::FfiStatus,
            ) -> #ffi_return_type {
                match ::boltffi::__private::rustfuture::rust_future_complete::<#rust_return_type>(handle) {
                    Some(result) => { #complete_conversion }
                    None => {
                        if !out_status.is_null() { *out_status = ::boltffi::__private::FfiStatus::CANCELLED; }
                        #default_value
                    }
                }
            }
        }
    }

    fn render_wasm_complete(&self, export: AsyncWasmCompleteExport) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let complete_ident = &self.names.complete;
        let params = export.params;
        let return_type = export.return_type;
        let body = export.body;

        quote! {
            #[cfg(target_arch = "wasm32")]
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #complete_ident(
                #params
            ) #return_type {
                #body
            }
        }
    }

    fn render_native_poll(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let poll_ident = &self.names.poll;
        let rust_return_type = &self.rust_return_type;

        quote! {
            #[cfg(not(target_arch = "wasm32"))]
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #poll_ident(
                handle: ::boltffi::__private::RustFutureHandle,
                callback_data: u64,
                callback: ::boltffi::__private::RustFutureContinuationCallback,
            ) {
                ::boltffi::__private::rustfuture::rust_future_poll::<#rust_return_type>(handle, callback, callback_data)
            }
        }
    }

    fn render_wasm_poll(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let poll_sync_ident = &self.names.poll_sync;
        let rust_return_type = &self.rust_return_type;

        quote! {
            #[cfg(target_arch = "wasm32")]
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #poll_sync_ident(
                handle: ::boltffi::__private::RustFutureHandle,
            ) -> i32 {
                ::boltffi::__private::rust_future_poll_sync::<#rust_return_type>(handle)
            }
        }
    }

    fn render_wasm_panic_message(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let panic_message_ident = &self.names.panic_message;
        let rust_return_type = &self.rust_return_type;

        quote! {
            #[cfg(target_arch = "wasm32")]
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #panic_message_ident(
                handle: ::boltffi::__private::RustFutureHandle,
            ) -> ::boltffi::__private::FfiBuf {
                match ::boltffi::__private::rust_future_panic_message::<#rust_return_type>(handle) {
                    Some(message) => ::boltffi::__private::FfiBuf::wire_encode(&message),
                    None => ::boltffi::__private::FfiBuf::empty(),
                }
            }
        }
    }

    fn render_cancel(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let cancel_ident = &self.names.cancel;
        let rust_return_type = &self.rust_return_type;

        quote! {
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #cancel_ident(handle: ::boltffi::__private::RustFutureHandle) {
                ::boltffi::__private::rustfuture::rust_future_cancel::<#rust_return_type>(handle)
            }
        }
    }

    fn render_free(&self) -> proc_macro2::TokenStream {
        let visibility = self.visibility;
        let free_ident = &self.names.free;
        let rust_return_type = &self.rust_return_type;

        quote! {
            #[unsafe(no_mangle)]
            #visibility unsafe extern "C" fn #free_ident(handle: ::boltffi::__private::RustFutureHandle) {
                ::boltffi::__private::rustfuture::rust_future_free::<#rust_return_type>(handle)
            }
        }
    }
}
