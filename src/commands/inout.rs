use std::ops::RangeBounds;

use crate::resources::{BufferHandle, BufferSlice, ResourceBinding};

#[derive(Copy, Clone)]
pub struct ReadBuffer(pub(crate) BufferHandle);

impl ReadBuffer {
    pub fn slice(&self, range: impl RangeBounds<u64>) -> BufferSlice {
        self.0.slice(range)
    }
}

#[derive(Copy, Clone)]
pub struct WriteBuffer(pub(crate) BufferHandle);

impl WriteBuffer {
    pub fn slice(&self, range: impl RangeBounds<u64>) -> BufferSlice {
        self.0.slice(range)
    }
}

#[derive(Copy, Clone)]
pub struct ReadWriteBuffer(pub(crate) BufferHandle);

impl ReadWriteBuffer {
    pub fn slice(&self, range: impl RangeBounds<u64>) -> BufferSlice {
        self.0.slice(range)
    }
}
