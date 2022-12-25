use std::borrow::Cow;

use naga::{FastHashMap, FastHashSet};

use crate::node::{NodeInput, NodeOutput};
use crate::resources::{
    BindGroupCache, BufferHandle, RenderResources, ResourceHandle, ResourceUse,
};

pub use self::inout::{InOutBuffer, InputBuffer, OutputBuffer};
pub(crate) use self::pass::{ComputePassCommand, ComputePassCommands};

mod inout;
mod pass;

pub(crate) enum RenderCommand {
    WriteBuffer(BufferHandle, u64, Vec<u8>),
    CopyBufferToBuffer(BufferHandle, u64, BufferHandle, u64, u64),
    ComputePass(Option<Cow<'static, str>>, Vec<ComputePassCommand>),
}

pub struct RenderCommands<'r> {
    pub(crate) queue: Vec<RenderCommand>,
    pub(crate) resources: &'r RenderResources,
    pub(crate) bind_cache: BindGroupCache,
    pub(crate) resource_meta: FastHashMap<ResourceHandle, ResourceUse>,
}

impl<'r> RenderCommands<'r> {
    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
    }

    pub fn write_buffer(&mut self, buffer: OutputBuffer, offset: u64, bytes: &[u8]) {
        self.enqueue(RenderCommand::WriteBuffer(
            buffer.0,
            offset,
            bytes.to_owned(),
        ))
    }

    pub fn compute_pass<'c>(
        &'c mut self,
        label: Option<impl Into<Cow<'static, str>>>,
    ) -> ComputePassCommands<'c, 'r> {
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
        src: InputBuffer,
        src_offset: u64,
        dst: OutputBuffer,
        dst_offset: u64,
        size: u64,
    ) {
        let (src, dst) = (src.0, dst.0);

        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }
}
