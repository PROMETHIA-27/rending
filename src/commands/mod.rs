use std::borrow::Cow;

use naga::FastHashMap;
use wgpu::BufferUsages;
use wgpu_core::hub::Resource;

use crate::resources::{
    BindGroupCache, BufferHandle, PipelineStorage, ResourceHandle, ResourceUse,
};

pub use self::inout::{ReadBuffer, ReadWriteBuffer, WriteBuffer};
pub(crate) use self::pass::{ComputePassCommand, ComputePassCommands};

mod inout;
mod pass;

#[derive(Debug)]
pub(crate) enum RenderCommand {
    WriteBuffer(BufferHandle, u64, Vec<u8>),
    CopyBufferToBuffer(BufferHandle, u64, BufferHandle, u64, u64),
    ComputePass(Option<Cow<'static, str>>, Vec<ComputePassCommand>),
}

pub struct RenderCommands<'q, 'r> {
    pub(crate) pipelines: &'r PipelineStorage,
    pub(crate) queue: &'q mut Vec<RenderCommand>,
    pub(crate) bind_cache: &'q mut BindGroupCache,
    pub(crate) resource_meta: &'q mut FastHashMap<ResourceHandle, ResourceUse>,
}

impl<'q, 'r> RenderCommands<'q, 'r> {
    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
    }

    fn buffer_meta(&mut self, handle: BufferHandle) -> (&mut u64, &mut BufferUsages, &mut bool) {
        let handle = ResourceHandle::Buffer(handle);
        match self
            .resource_meta
            .entry(handle)
            .or_insert(ResourceUse::default_from_handle(handle))
        {
            ResourceUse::Buffer {
                size,
                usage,
                mapped,
            } => (size, usage, mapped),
            _ => unreachable!(
                "this should not be hit; buffer_meta() should only be called on buffer metadata"
            ),
        }
    }

    pub fn write_buffer(&mut self, buffer: WriteBuffer, offset: u64, bytes: &[u8]) {
        let (size, usage, _) = self.buffer_meta(buffer.0);
        *size = (*size).max(offset + bytes.len() as u64);
        *usage |= BufferUsages::COPY_DST;
        self.enqueue(RenderCommand::WriteBuffer(
            buffer.0,
            offset,
            bytes.to_owned(),
        ))
    }

    pub fn compute_pass<'c>(
        &'c mut self,
        label: Option<impl Into<Cow<'static, str>>>,
    ) -> ComputePassCommands<'c, 'q, 'r> {
        ComputePassCommands {
            commands: self,
            label: label.map(Into::into),
            queue: vec![],
            pipeline: None,
            bindings: std::array::from_fn(|_| None),
        }
    }

    pub fn copy_buffer_to_buffer(
        &mut self,
        src: ReadBuffer,
        src_offset: u64,
        dst: WriteBuffer,
        dst_offset: u64,
        size: u64,
    ) {
        let (src, dst) = (src.0, dst.0);

        let (src_size, usage, _) = self.buffer_meta(src);
        *src_size = (*src_size).max(src_offset + size);
        *usage |= BufferUsages::COPY_SRC;
        let (dst_size, usage, _) = self.buffer_meta(dst);
        *dst_size = (*dst_size).max(dst_offset + size);
        *usage |= BufferUsages::COPY_DST;

        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }
}
