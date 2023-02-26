//! TODO: This entire module should later be separated and made into a standalone crate

use std::alloc::Layout;

use naga::FastHashMap;

pub struct VecPool {
    layouts: FastHashMap<Layout, Vec<ErasedVec>>,
}

struct ErasedVec {
    ptr: *mut (),
    len: usize,
    cap: usize,
}