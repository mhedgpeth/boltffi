#[derive(Clone)]
pub struct FfiPatterns {
    pub allocate: Vec<&'static str>,
    pub deallocate: Vec<&'static str>,
    pub retain: Vec<&'static str>,
    pub release: Vec<&'static str>,
    pub take_retained: Vec<&'static str>,
    pub status_check: Vec<&'static str>,
    pub defer_keyword: Vec<&'static str>,
    pub ffi_prefix: Vec<&'static str>,
    pub callback_bridge: Vec<&'static str>,
}

impl FfiPatterns {
    pub fn swift() -> Self {
        Self {
            allocate: vec![".allocate(capacity:"],
            deallocate: vec![".deallocate()"],
            retain: vec!["passRetained("],
            release: vec![".release()"],
            take_retained: vec!["takeRetainedValue()"],
            status_check: vec!["checkStatus(", "ensureOk("],
            defer_keyword: vec!["defer "],
            ffi_prefix: vec!["riff_", "ffi_"],
            callback_bridge: vec!["Bridge", "ContinuationBox", "Box)"],
        }
    }

    pub fn kotlin() -> Self {
        Self {
            allocate: vec!["nativeHeap.allocArray", "allocArray<", "memScoped {"],
            deallocate: vec!["nativeHeap.free(", ".free()"],
            retain: vec!["StableRef.create("],
            release: vec![".dispose()"],
            take_retained: vec![".get()"],
            status_check: vec!["checkStatus(", "ensureOk("],
            defer_keyword: vec!["finally {"],
            ffi_prefix: vec!["riff_", "ffi_"],
            callback_bridge: vec!["Bridge", "CallbackRef"],
        }
    }

    pub fn is_allocate(&self, text: &str) -> bool {
        self.allocate.iter().any(|p| text.contains(p))
    }

    pub fn is_deallocate(&self, text: &str) -> bool {
        self.deallocate.iter().any(|p| text.contains(p))
    }

    pub fn is_retain(&self, text: &str) -> bool {
        self.retain.iter().any(|p| text.contains(p))
    }

    pub fn is_release(&self, text: &str) -> bool {
        self.release.iter().any(|p| text.contains(p))
    }

    pub fn is_take_retained(&self, text: &str) -> bool {
        self.take_retained.iter().any(|p| text.contains(p))
    }

    pub fn is_status_check(&self, text: &str) -> bool {
        self.status_check.iter().any(|p| text.contains(p))
    }

    pub fn is_defer(&self, text: &str) -> bool {
        self.defer_keyword.iter().any(|p| text.starts_with(p))
    }

    pub fn is_ffi_call(&self, text: &str) -> bool {
        self.ffi_prefix.iter().any(|p| text.starts_with(p))
    }

    pub fn is_callback_bridge(&self, text: &str) -> bool {
        self.callback_bridge.iter().any(|p| text.contains(p))
    }

    pub fn extract_from_opaque_var<'a>(&self, text: &'a str) -> Option<&'a str> {
        text.split("fromOpaque(")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .map(|s| s.trim())
    }

    pub fn extract_capacity<'a>(&self, text: &'a str) -> Option<&'a str> {
        text.split("capacity:")
            .nth(1)
            .or_else(|| text.split("allocArray<").nth(1).and_then(|s| s.split('>').nth(1)))
            .and_then(|s| {
                let trimmed = s.trim();
                let end = trimmed.find(|c: char| c == ')' || c == ',' || c == '}')?;
                Some(&trimmed[..end])
            })
            .map(|s| s.trim())
    }

    pub fn extract_element_type<'a>(&self, text: &'a str) -> Option<&'a str> {
        text.split('<')
            .nth(1)
            .and_then(|s| s.split('>').next())
            .map(|s| s.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swift_patterns() {
        let patterns = FfiPatterns::swift();
        
        assert!(patterns.is_allocate("UnsafeMutablePointer<Int32>.allocate(capacity: 10)"));
        assert!(patterns.is_deallocate("ptr.deallocate()"));
        assert!(patterns.is_retain("Unmanaged.passRetained(obj)"));
        assert!(patterns.is_release("handle.release()"));
        assert!(patterns.is_status_check("ensureOk(status)"));
        assert!(patterns.is_ffi_call("riff_echo_string"));
    }

    #[test]
    fn test_kotlin_patterns() {
        let patterns = FfiPatterns::kotlin();
        
        assert!(patterns.is_allocate("nativeHeap.allocArray<Int>(10)"));
        assert!(patterns.is_deallocate("nativeHeap.free(ptr)"));
        assert!(patterns.is_retain("StableRef.create(obj)"));
        assert!(patterns.is_release("ref.dispose()"));
    }

    #[test]
    fn test_extract_element_type() {
        let patterns = FfiPatterns::swift();
        
        assert_eq!(
            patterns.extract_element_type("UnsafeMutablePointer<Int32>.allocate"),
            Some("Int32")
        );
    }
}
