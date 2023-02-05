use std::borrow::{Borrow, Cow};

use wgpu::{BufferUsages, Extent3d, ImageDataLayout, TextureFormat};

use crate::named_slotmap::NamedSlotMap;
use crate::resources::{
    BindGroupCache, BufferConstraints, BufferHandle, ComputePipelineHandle, NodeResourceAccess,
    PipelineStorage, ResourceConstraints,
    ResourceHandle, /*SamplerConstraints, SamplerHandle,*/
    TextureAspect, TextureCopyView, TextureHandle, TextureSize,
};

pub(crate) use self::compute_pass::{ComputePassCommand, ComputePassCommands};

mod compute_pass;

// TODO: Pool vecs in commands
#[derive(Debug)]
pub(crate) enum RenderCommand {
    WriteBuffer(BufferHandle, u64, Vec<u8>),
    WriteTexture(TextureCopyView, Vec<u8>, ImageDataLayout, Extent3d),
    CopyBufferToBuffer(BufferHandle, u64, BufferHandle, u64, u64),
    ComputePass(Option<Cow<'static, str>>, Vec<ComputePassCommand>),
}

pub(crate) type ResourceList = Vec<(Cow<'static, str>, ResourceHandle)>;
pub(crate) type ResourceAccesses = Vec<NodeResourceAccess>;
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, usize>;
pub(crate) type VirtualTextures = NamedSlotMap<TextureHandle, usize>;
// pub(crate) type VirtualSamplers = NamedSlotMap<SamplerHandle, usize>;
// pub(crate) type SamplerRev<'c> = FastHashMap<&'c SamplerConstraints, SamplerHandle>;

pub struct RenderCommands<'q, 'r> {
    /// Access pipelines for getting handles and dispatch, etc.
    pub(crate) pipelines: &'r PipelineStorage,
    /// Queue of rendercommands being built up
    pub(crate) queue: &'q mut Vec<RenderCommand>,
    /// Cache for bind groups being selected
    pub(crate) bind_cache: &'q mut BindGroupCache,
    /// Resource usage information for transients/retained verification
    pub(crate) constraints: &'q mut ResourceConstraints,
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
    // /// Virtual handles for each accessed sampler
    // pub(crate) virtual_samplers: VirtualSamplers,
}

impl<'q, 'r> RenderCommands<'q, 'r> {
    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
    }

    fn get_buffer_constraints(&mut self, handle: BufferHandle) -> &mut BufferConstraints {
        self.constraints.buffers.entry(handle).unwrap().or_default()
    }

    fn get_texture_constraints(
        &mut self,
        handle: TextureHandle,
    ) -> &mut crate::resources::TextureConstraints {
        self.constraints
            .textures
            .entry(handle)
            .unwrap()
            .or_default()
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
            } // ResourceHandle::Sampler(handle) => {
              //     let &index = self.virtual_samplers.get(handle).unwrap();
              //     self.resource_accesses[self.node_index].reads.insert(index);
              // }
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
            } // ResourceHandle::Sampler(handle) => {
              //     let &index = self.virtual_samplers.get(handle).unwrap();
              //     self.resource_accesses[self.node_index].writes.insert(index);
              // }
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

    // pub fn sampler(&mut self, name: impl Into<Cow<'static, str>> + Borrow<str>) -> SamplerHandle {
    //     match self.virtual_samplers.get_key(name.borrow()) {
    //         Some(handle) => handle,
    //         None => {
    //             let name = name.into();
    //             let index = self.resources.len();
    //             let handle = self.virtual_samplers.insert(name.clone(), index);
    //             self.resources.push((name, handle.into()));
    //             handle
    //         }
    //     }
    // }

    pub fn texture_constraints(&mut self, texture: TextureHandle) -> TextureConstraints {
        let constraints = self
            .constraints
            .textures
            .entry(texture)
            .unwrap()
            .or_default();
        TextureConstraints { constraints }
    }

    pub fn compute_pipeline(&self, name: &str) -> ComputePipelineHandle {
        self.pipelines
            .compute_pipelines
            .get_key(name)
            .unwrap_or_else(|| panic!("no compute pipeline named `{name}` available"))
    }

    pub fn write_buffer(&mut self, buffer: BufferHandle, offset: u64, bytes: &[u8]) {
        let constraints = self.get_buffer_constraints(buffer);
        constraints.set_size(offset + bytes.len() as u64);
        constraints.set_usages(BufferUsages::COPY_DST);

        self.mark_resource_write(buffer.into());

        self.enqueue(RenderCommand::WriteBuffer(buffer, offset, bytes.to_owned()))
    }

    pub fn write_texture(
        &mut self,
        texture_view: TextureCopyView,
        data: &[u8],
        layout: ImageDataLayout,
        size: Extent3d,
    ) {
        let constraints = self.get_texture_constraints(texture_view.handle);
        constraints.set_copy_dst();
        let min_size = Extent3d {
            width: texture_view.origin.x + size.width,
            height: texture_view.origin.y + size.height,
            depth_or_array_layers: texture_view.origin.z + size.depth_or_array_layers,
        };
        constraints.set_min_size(min_size);
        constraints.set_mip_count(texture_view.mip_level);
        match texture_view.aspect {
            TextureAspect::StencilOnly => constraints.has_stencil = true,
            TextureAspect::DepthOnly => constraints.has_depth = true,
            _ => (),
        }

        self.enqueue(RenderCommand::WriteTexture(
            texture_view,
            data.to_owned(),
            layout,
            size,
        ));
    }

    pub fn compute_pass<'c>(
        &'c mut self,
        label: Option<impl Into<Cow<'static, str>>>,
    ) -> ComputePassCommands<'c, 'q, 'r> {
        let command_index = self.queue.len();
        self.enqueue(RenderCommand::ComputePass(label.map(Into::into), vec![]));
        ComputePassCommands {
            commands: self,
            command_index,
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
        let constraints = self.get_buffer_constraints(src);
        constraints.set_size(src_offset + size);
        constraints.set_usages(BufferUsages::COPY_SRC);

        let constraints = self.get_buffer_constraints(dst);
        constraints.set_size(dst_offset + size);
        constraints.set_usages(BufferUsages::COPY_DST);

        self.mark_resource_read(src.into());
        self.mark_resource_write(dst.into());

        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }
}

pub struct TextureConstraints<'c> {
    constraints: &'c mut crate::resources::TextureConstraints,
}

impl TextureConstraints<'_> {
    pub fn has_size(&mut self, size: TextureSize) -> &mut Self {
        let new_size = size;
        match self.constraints.size {
            Some(size) => assert_eq!(size, new_size, "texture constrained to size {new_size:?} when it is already constrained to size {size:?}. Perhaps there is a typo or extra constraint set?"),
            None => self.constraints.size = Some(new_size)
        }

        self
    }

    pub fn has_format(&mut self, format: TextureFormat) -> &mut Self {
        let new_format = format;
        match self.constraints.format {
                Some(format) => assert_eq!(format, new_format, "texture constrained to format {new_format:?} when it is already constrained to format {format:?}. Perhaps there is a type or extra constraint set?"),
                None => self.constraints.format = Some(new_format),
            }

        self
    }
}

// impl<'l> SamplerParams<'_, '_, '_, 'l> {
//     pub fn get_handle(&mut self) -> SamplerHandle {
//         todo!()
//         // use the sampler cache
//     }

//     pub fn label(&mut self, label: Label<'l>) -> &mut Self {
//         self.label = label;
//         self
//     }

//     pub fn address_mode_u(&mut self, mode: AddressMode) -> &mut Self {
//         self.address_mode_u = mode;
//         self
//     }

//     pub fn address_mode_v(&mut self, mode: AddressMode) -> &mut Self {
//         self.address_mode_v = mode;
//         self
//     }

//     pub fn address_mode_w(&mut self, mode: AddressMode) -> &mut Self {
//         self.address_mode_w = mode;
//         self
//     }

//     pub fn mag_filter(&mut self, mode: FilterMode) -> &mut Self {
//         self.mag_filter = mode;
//         self
//     }

//     pub fn min_filter(&mut self, mode: FilterMode) -> &mut Self {
//         self.min_filter = mode;
//         self
//     }

//     pub fn mipmap_filter(&mut self, mode: FilterMode) -> &mut Self {
//         self.mipmap_filter = mode;
//         self
//     }

//     pub fn lod_min_clamp(&mut self, clamp: f32) -> &mut Self {
//         self.lod_min_clamp = clamp;
//         self
//     }

//     pub fn lod_max_clamp(&mut self, clamp: f32) -> &mut Self {
//         self.lod_max_clamp = clamp;
//         self
//     }

//     pub fn compare(&mut self, compare: CompareFunction) -> &mut Self {
//         self.compare = Some(compare);
//         self
//     }

//     pub fn aniso_clamp(&mut self, clamp: NonZeroU8) -> &mut Self {
//         self.anisotropy_clamp = Some(clamp);
//         self
//     }

//     pub fn border_color(&mut self, color: SamplerBorderColor) -> &mut Self {
//         self.border_color = Some(color);
//         self
//     }
// }
