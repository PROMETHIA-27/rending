use std::borrow::{Borrow, Cow};

use naga::FastHashMap;
use wgpu::{BufferUsages, TextureFormat};

use crate::named_slotmap::NamedSlotMap;
use crate::resources::{
    BindGroupCache, BufferHandle, ComputePipelineHandle, NodeResourceAccess, PipelineStorage,
    ResourceHandle, ResourceMeta, TextureHandle, TextureSize,
};

pub(crate) use self::pass::{ComputePassCommand, ComputePassCommands};

mod pass;

#[derive(Debug)]
pub(crate) enum RenderCommand {
    WriteBuffer(BufferHandle, u64, Vec<u8>),
    CopyBufferToBuffer(BufferHandle, u64, BufferHandle, u64, u64),
    ComputePass(Option<Cow<'static, str>>, Vec<ComputePassCommand>),
}

pub(crate) type ResourceList = Vec<(Cow<'static, str>, ResourceHandle)>;
pub(crate) type ResourceAccesses = Vec<NodeResourceAccess>;
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, usize>;
pub(crate) type VirtualTextures = NamedSlotMap<TextureHandle, usize>;

pub struct RenderCommands<'q, 'r> {
    /// Access pipelines for getting handles and dispatch, etc.
    pub(crate) pipelines: &'r PipelineStorage,
    /// Queue of rendercommands being built up
    pub(crate) queue: &'q mut Vec<RenderCommand>,
    /// Cache for bind groups being selected
    pub(crate) bind_cache: &'q mut BindGroupCache,
    // TODO: Split resource metas into individual resource types?
    /// Resource usage information for transients/retained verification
    pub(crate) resource_meta: &'q mut FastHashMap<ResourceHandle, ResourceMeta>,
    /// The index of the current node this is being passed to
    pub(crate) node_index: usize,
    /// A linear list of all resources that have been accessed so far
    pub(crate) resources: ResourceList,
    /// Bitsets for each node of which resources they access and how
    pub(crate) resource_accesses: ResourceAccesses,
    /// Virtual handles for each accessed buffer
    pub(crate) virtual_buffers: VirtualBuffers,
    /// Virtual handles for each accessed texture
    pub(crate) virtual_textures: VirtualTextures,
}

impl<'q, 'r> RenderCommands<'q, 'r> {
    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
    }

    fn set_buffer_meta(&mut self, handle: BufferHandle, size: u64, usage: BufferUsages) {
        let handle = handle.into();
        match self
            .resource_meta
            .entry(handle)
            .or_insert(ResourceMeta::default_from_handle(handle))
        {
            ResourceMeta::Buffer {
                size: buf_size,
                usage: buf_usage,
                ..
            } => {
                *buf_size = (*buf_size).max(size);
                *buf_usage |= usage;
            }
            _ => unreachable!(
                "this should not be hit; buffer_meta() should only be called on buffer metadata"
            ),
        }
    }

    fn mark_resource_read(&mut self, handle: ResourceHandle) {
        match handle {
            ResourceHandle::Buffer(handle) => {
                let &index = self.virtual_buffers.get(handle).unwrap();
                self.resource_accesses[self.node_index].reads.insert(index);
            }
            ResourceHandle::Texture(handle) => {
                let &index = self.virtual_textures.get(handle).unwrap();
                self.resource_accesses[self.node_index].reads.insert(index);
            }
        }
    }

    fn mark_resource_write(&mut self, handle: ResourceHandle) {
        match handle {
            ResourceHandle::Buffer(handle) => {
                let &index = self.virtual_buffers.get(handle).unwrap();
                self.resource_accesses[self.node_index].writes.insert(index);
            }
            ResourceHandle::Texture(handle) => {
                let &index = self.virtual_textures.get(handle).unwrap();
                self.resource_accesses[self.node_index].writes.insert(index);
            }
        }
    }

    pub fn buffer(&mut self, name: impl Into<Cow<'static, str>> + Borrow<str>) -> BufferHandle {
        match self.virtual_buffers.get_key(name.borrow()) {
            Some(handle) => handle,
            None => {
                let name = name.into();
                let index = self.resources.len();
                let handle = self.virtual_buffers.insert(name.clone(), index);
                self.resources.push((name, handle.into()));
                handle
            }
        }
    }

    pub fn texture(&mut self, name: impl Into<Cow<'static, str>> + Borrow<str>) -> TextureHandle {
        match self.virtual_textures.get_key(name.borrow()) {
            Some(handle) => handle,
            None => {
                let name = name.into();
                let index = self.resources.len();
                let handle = self.virtual_textures.insert(name.clone(), index);
                self.resources.push((name, handle.into()));
                handle
            }
        }
    }

    pub fn texture_constraints(&mut self, texture: TextureHandle) -> TextureConstraints {
        let meta = self.resource_meta.get_mut(&texture.into()).unwrap();
        TextureConstraints { texture_meta: meta }
    }

    pub fn compute_pipeline(&self, name: &str) -> ComputePipelineHandle {
        self.pipelines
            .compute_pipelines
            .get_key(name)
            .unwrap_or_else(|| panic!("no compute pipeline named `{name}` available"))
    }

    pub fn write_buffer(&mut self, buffer: BufferHandle, offset: u64, bytes: &[u8]) {
        self.set_buffer_meta(buffer, offset + bytes.len() as u64, BufferUsages::COPY_DST);
        self.mark_resource_write(buffer.into());

        self.enqueue(RenderCommand::WriteBuffer(buffer, offset, bytes.to_owned()))
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
        src: BufferHandle,
        src_offset: u64,
        dst: BufferHandle,
        dst_offset: u64,
        size: u64,
    ) {
        self.set_buffer_meta(src, src_offset + size, BufferUsages::COPY_SRC);
        self.set_buffer_meta(dst, dst_offset + size, BufferUsages::COPY_DST);

        self.mark_resource_read(src.into());
        self.mark_resource_write(dst.into());

        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }
}

pub struct TextureConstraints<'c> {
    texture_meta: &'c mut ResourceMeta,
}

impl TextureConstraints<'_> {
    pub fn has_size(&mut self, size: TextureSize) -> &mut Self {
        let new_size = size;
        match self.texture_meta {
            ResourceMeta::Texture {
                size,
                ..
            } => {
                match size {
                    &mut Some(size) => assert_eq!(size, new_size, "texture constrained to size {new_size:?} when it is already constrained to size {size:?}. Perhaps there is a typo or extra constraint set?"),
                    None => 
                        *size = Some(new_size)
                    ,
                }
            },
            _ => unreachable!(),
        }

        self
    }

    pub fn has_format(&mut self, format: TextureFormat) -> &mut Self {
        let new_format = format;
        match self.texture_meta {
            ResourceMeta::Texture {
                format,
                ..
            } => match format {
                &mut Some(format) => assert_eq!(format, new_format, "texture constrained to format {new_format:?} when it is already constrained to format {format:?}. Perhaps there is a type or extra constraint set?"),
                None => *format = Some(new_format),
            },
            _ => unreachable!(),
        }

        self
    }
}
