use boltffi_ffi_rules::transport::{
    ReturnInvocationContext, ReturnPlatform, ValueReturnMethod, ValueReturnStrategy,
};
use syn::Type;

use crate::returns::{ReturnAbi, ReturnLoweringContext};

pub(super) struct LoweredCallbackReturn {
    abi: ReturnAbi,
}

impl LoweredCallbackReturn {
    pub(super) fn new(ty: &Type, return_lowering: &ReturnLoweringContext<'_>) -> Self {
        Self {
            abi: return_lowering.lower_type(ty),
        }
    }

    pub(super) fn value_return_method(
        &self,
        return_lowering: &ReturnLoweringContext<'_>,
        context: ReturnInvocationContext,
        platform: ReturnPlatform,
    ) -> ValueReturnMethod {
        self.abi
            .value_return_method(return_lowering, context, platform)
    }

    pub(super) fn uses_wire_payload(&self, return_lowering: &ReturnLoweringContext<'_>) -> bool {
        !matches!(
            self.abi.value_return_strategy(return_lowering),
            ValueReturnStrategy::Scalar(_)
        )
    }
}
