pub mod swift;

use crate::ir::{FfiContract, LoweredContract};

pub trait Renderer {
    type Output;

    fn render(contract: &FfiContract, lowered: &LoweredContract) -> Self::Output;
}
