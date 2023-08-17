use std::num::NonZeroU64;
use std::ops::RangeBounds;

use slotmap::{new_key_type, SecondaryMap};
use thiserror::Error;
use wgpu::{Buffer, BufferUsages};

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
    /// slot with the same RWMode. Only RWModes READ and READWRITE are permitted.
    pub fn storage(self, mode: RWMode) -> ResourceBinding {
        assert_ne!(
            mode,
            RWMode::WRITE,
            "Only RWModes READ and READWRITE are permitted in storage buffers"
        );
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

pub(crate) enum BufferBinding<'b> {
    Retained(&'b Buffer),
    Transient(Buffer),
}

impl<'b> AsRef<Buffer> for BufferBinding<'b> {
    fn as_ref(&self) -> &Buffer {
        match self {
            BufferBinding::Retained(buffer) => buffer,
            BufferBinding::Transient(buffer) => buffer,
        }
    }
}

pub(crate) type BufferBindings<'b> = SecondaryMap<BufferHandle, BufferBinding<'b>>;

#[derive(Debug, Error)]
pub enum BufferError {
    #[error("the retained buffer `{0}` has size {1} when its minimum size based on usage is {2}")]
    TooSmall(String, u64, u64),
    #[error(
        "the retained buffer `{0}` is used with usages `{1:?}` but was not created with those flags"
    )]
    MissingUsages(String, BufferUsages),
}
