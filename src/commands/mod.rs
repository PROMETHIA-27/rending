use std::borrow::Cow;

use naga::{FastHashMap, FastHashSet};

use crate::node::{NodeInput, NodeOutput};
use crate::resources::{
    BindGroupCache, BindGroupLayouts, BufferHandle, ComputePipelines, DataResources,
    PipelineLayouts, RenderResources, ResourceHandle, ResourceUse, VirtualBuffers,
};

pub use self::inout::{ReadBuffer, ReadWriteBuffer, WriteBuffer};
pub(crate) use self::pass::{ComputePassCommand, ComputePassCommands};

mod inout;
mod pass;

pub(crate) enum RenderCommand {
    WriteBuffer(BufferHandle, u64, Vec<u8>),
    CopyBufferToBuffer(BufferHandle, u64, BufferHandle, u64, u64),
    ComputePass(Option<Cow<'static, str>>, Vec<ComputePassCommand>),
}

pub(crate) struct RenderCommandResources<'r> {
    pub data_resources: &'r DataResources,
    pub virtual_buffers: &'r mut VirtualBuffers,
    pub compute_pipelines: &'r ComputePipelines,
    pub bind_group_layouts: &'r BindGroupLayouts,
    pub pipeline_layouts: &'r PipelineLayouts,
}

pub struct RenderCommands<'q, 'r> {
    pub(crate) resources: RenderCommandResources<'r>,
    pub(crate) queue: &'q mut Vec<RenderCommand>,
    pub(crate) bind_cache: &'q mut BindGroupCache,
    pub(crate) resource_meta: &'q mut FastHashMap<ResourceHandle, ResourceUse>,
}

impl<'q, 'r> RenderCommands<'q, 'r> {
    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
    }

    pub fn write_buffer(&mut self, buffer: WriteBuffer, offset: u64, bytes: &[u8]) {
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

        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }
}
