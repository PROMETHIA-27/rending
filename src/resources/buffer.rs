use std::num::NonZeroU64;
use std::ops::RangeBounds;

use slotmap::new_key_type;
use wgpu::BufferUsages;

use super::{RWMode, ResourceBinding};

new_key_type! { pub struct BufferHandle; }

impl BufferHandle {
    pub fn slice(self, range: impl RangeBounds<u64>) -> BufferSlice {
        let offset = match range.start_bound() {
            std::ops::Bound::Included(&i) => i,
            std::ops::Bound::Excluded(&i) => i + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let size = match range.end_bound() {
            std::ops::Bound::Included(&i) => {
                assert!(i >= offset);
                NonZeroU64::new(i - offset + 1)
            }
            std::ops::Bound::Excluded(&i) => {
                assert!(i > offset);
                NonZeroU64::new(i - offset)
            }
            std::ops::Bound::Unbounded => None,
        };
        BufferSlice {
            handle: self,
            offset,
            size,
        }
    }
}

/// Represents a slice of a buffer starting at offset, and which is either size long or the rest of the slice.
///
/// Consider using [`BufferHandle::slice()`] instead of manually constructing.
#[derive(Debug, Copy, Clone)]
pub struct BufferSlice {
    handle: BufferHandle,
    offset: u64,
    size: Option<NonZeroU64>,
}

impl BufferSlice {
    /// Turn a buffer slice into a usable resource binding to pass to functions like
    /// [`ComputePassCommands::bind_group()`](crate::commands::ComputePassCommands).
    /// This specifies that the buffer is a uniform, and so it must be bound to a uniform slot.
    /// This also means that the buffer must be marked as an input to a `RenderNode` that it is being
    /// used in.
    pub fn uniform(self) -> ResourceBinding {
        let Self {
            handle,
            offset,
            size,
        } = self;
        ResourceBinding::Buffer {
            handle,
            offset,
            size,
            usage: BufferUse::Uniform,
        }
    }

    /// Turn a buffer slice into a usable resource binding to pass to functions like
    /// [`ComputePassCommands::bind_group()`](crate::commands::ComputePassCommands).
    /// This specifies that the buffer is a storage, and so it must be bound to a storage
    /// slot with the same RWMode. It also means that the buffer must be marked as:
    /// - An input if RWMode is Read or ReadWrite
    /// - An output if RWMode is Write or ReadWrite
    ///
    /// in the `RenderNode` that it is being used in.
    pub fn storage(self, mode: RWMode) -> ResourceBinding {
        let Self {
            handle,
            offset,
            size,
        } = self;
        ResourceBinding::Buffer {
            handle,
            offset,
            size,
            usage: BufferUse::Storage(mode),
        }
    }

    /// Turn a buffer slice into a usable resource binding to pass to functions like
    /// [`ComputePassCommands::bind_group()`](crate::commands::ComputePassCommands).
    /// This will infer what kind of binding the buffer will be. This inference will *always*
    /// succeed, however this makes it less clear from a glance what kind of operations are being
    /// done to the buffer, and you must still get the input/output declaration correct.
    /// For this reason it is recommended to use `uniform()`, `storage()`, etc. instead.
    pub fn infer(self) -> ResourceBinding {
        let Self {
            handle,
            offset,
            size,
        } = self;
        ResourceBinding::Buffer {
            handle,
            offset,
            size,
            usage: BufferUse::Infer,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BufferUse {
    Uniform,
    Storage(RWMode),
    Infer,
}

impl BufferUse {
    pub fn matches_use(&self, usage: BufferUse) -> bool {
        match (self, usage) {
            (BufferUse::Uniform, BufferUse::Uniform) => true,
            (&BufferUse::Storage(left), BufferUse::Storage(right)) if left == right => true,
            (BufferUse::Storage(_), BufferUse::Storage(_)) => false,
            (BufferUse::Infer, _) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct VirtualBuffer {
    pub retained: bool,
}
