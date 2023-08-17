use std::borrow::{Borrow, Cow};

use wgpu::hal::TextureBinding;
use wgpu::{
    BufferDescriptor, CommandEncoderDescriptor, ComputePassDescriptor, Extent3d, ImageCopyTexture,
    ImageDataLayout,
};

use crate::named_slotmap::NamedSlotMap;
use crate::resources::{
    BindGroupCache, BufferBinding, BufferBindings, BufferHandle, ComputePipelineHandle, Pipelines,
    ResourceHandle, /*SamplerConstraints, SamplerHandle,*/
    TextureBindings, TextureCopyView, TextureError, TextureHandle,
};
use crate::{RenderContext, RenderResources};

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
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, usize>;
pub(crate) type VirtualTextures = NamedSlotMap<TextureHandle, usize>;
// pub(crate) type VirtualSamplers = NamedSlotMap<SamplerHandle, usize>;
// pub(crate) type SamplerRev<'c> = FastHashMap<&'c SamplerConstraints, SamplerHandle>;

pub struct RenderCommands<'r> {
    /// Access pipelines for getting handles and dispatch, etc.
    pub(crate) pipelines: &'r Pipelines,
    /// Queue of rendercommands being built up
    pub(crate) queue: Vec<RenderCommand>,
    /// Cache for bind groups being selected
    pub(crate) bind_cache: BindGroupCache,
    /// A linear list of all resources that have been accessed so far
    pub(crate) resources: ResourceList,
    /// Virtual handles for each accessed buffer
    pub(crate) virtual_buffers: VirtualBuffers,
    /// Virtual handles for each accessed texture
    pub(crate) virtual_textures: VirtualTextures,
    // /// Virtual handles for each accessed sampler
    // pub(crate) virtual_samplers: VirtualSamplers,
}

impl<'r> RenderCommands<'r> {
    pub fn new(pipelines: &'r Pipelines) -> Self {
        Self {
            pipelines,
            queue: vec![],
            bind_cache: BindGroupCache::new(),
            resources: ResourceList::new(),
            virtual_buffers: VirtualBuffers::new(),
            virtual_textures: VirtualTextures::new(),
        }
    }

    fn enqueue(&mut self, c: RenderCommand) {
        self.queue.push(c)
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

    pub fn compute_pipeline(&self, name: &str) -> ComputePipelineHandle {
        self.pipelines
            .compute_pipelines
            .get_key(name)
            .unwrap_or_else(|| panic!("no compute pipeline named `{name}` available"))
    }

    pub fn write_buffer(&mut self, buffer: BufferHandle, offset: u64, bytes: &[u8]) {
        self.enqueue(RenderCommand::WriteBuffer(buffer, offset, bytes.to_owned()))
    }

    pub fn write_texture(
        &mut self,
        texture_view: TextureCopyView,
        data: &[u8],
        layout: ImageDataLayout,
        size: Extent3d,
    ) {
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
    ) -> ComputePassCommands<'c, 'r> {
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
        self.enqueue(RenderCommand::CopyBufferToBuffer(
            src, src_offset, dst, dst_offset, size,
        ))
    }

    pub fn run(
        &mut self,
        ctx: RenderContext,
        res: &RenderResources,
    ) -> Result<(), RenderGraphError> {
        let bound_buffers: BufferBindings = self
            .virtual_buffers
            .iter_names()
            .map(|(name, handle)| {
                // Bind retained resources
                if let Some(buf) = res.buffers.get(name) {
                    Ok((handle, BufferBinding::Retained(buf)))
                }
                // Create transients
                else {
                    let buffer = ctx.device.create_buffer(&BufferDescriptor {
                        label: None,
                        size: constraints.min_size,
                        usage: constraints.min_usages,
                        mapped_at_creation: false,
                    });
                    Ok((handle, BufferBinding::Transient(buffer)))
                }
            })
            .collect::<Result<BufferBindings, BufferError>>()?;

        let bound_textures: TextureBindings = self
            .virtual_textures
            .iter_names()
            .map(|(name, handle)| {
                // Bind retained resources
                if let Some(texture) = res.textures.get(name) {
                    Ok((handle, TextureBinding::Retained(texture)))
                }
                // Create transients
                else {
                    let Some(size) = constraints.size else { return Err(TextureError::UnconstrainedTextureSize(name.to_string())) };
                    let Some(format) = constraints.format else { return Err(TextureError::UnconstrainedTextureFormat(name.to_string())) };
                    let texture = ctx.texture(
                        None,
                        size,
                        format,
                        constraints.min_usages,
                        constraints.min_mip_level_count,
                        constraints.min_sample_count,
                    );
                    Ok((handle, TextureBinding::Transient(texture)))
                }
            })
            .collect::<Result<TextureBindings, TextureError>>()?;

        // Verify retained sampler constraints
        // for (handle, constraints) in self.constraints.samplers.iter() {}

        // let bound_samplers: SamplerBindings = self
        //     .virtual_samplers
        //     .iter_keys()
        //     .map(|handle| {
        //         let constraints = self.constraints.samplers.get(handle).unwrap();

        //         // // Bind retained
        //         // if let Some(sampler) = res.samplers.get(name) {
        //         //     // TODO: Erase retained samplers' names and get them based off of constraints
        //         //     (handle, SamplerBinding::Retained(sampler))
        //         // } else {
        //         //     let sampler = ctx.sampler();
        //         //     (handle, SamplerBinding::Transient(sampler))
        //         // }
        //         let sampler = ctx.sampler();
        //         (handle, SamplerBinding::Transient(sampler))
        //     })
        //     .collect();

        // Make bind groups
        let bind_groups = self.bind_cache.create_groups(
            ctx,
            self.pipelines,
            &bound_buffers,
            &bound_textures,
            // &bound_samplers,
        );

        // Execute render command queue
        let mut encoder = ctx
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        for command in self.queue.iter() {
            match command {
                RenderCommand::WriteBuffer(handle, offset, data) => {
                    let buffer = bound_buffers.get(*handle).unwrap().as_ref();
                    ctx.queue.write_buffer(buffer, *offset, &data[..]);
                }
                RenderCommand::WriteTexture(view, data, layout, size) => {
                    let texture = bound_textures.get(view.handle).unwrap().as_ref();
                    let view = ImageCopyTexture {
                        texture: &texture.inner,
                        mip_level: view.mip_level,
                        origin: view.origin,
                        aspect: view.aspect.into_wgpu(),
                    };
                    ctx.queue.write_texture(view, &data[..], *layout, *size);
                }
                RenderCommand::ComputePass(label, commands) => {
                    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: label.as_ref().map(|cow| &cow[..]),
                    });
                    for command in commands.iter() {
                        match command {
                            ComputePassCommand::SetPipeline(handle) => {
                                let pipeline =
                                    self.pipelines.compute_pipelines.get(*handle).unwrap();
                                pass.set_pipeline(&pipeline.wgpu);
                            }
                            ComputePassCommand::BindGroup(index, handle) => {
                                let group = bind_groups.get(*handle).unwrap();
                                // TODO: Still haven't looked at dynamic offsets
                                pass.set_bind_group(*index, group, &[]);
                            }
                            &ComputePassCommand::Dispatch(x, y, z) => {
                                pass.dispatch_workgroups(x, y, z);
                            } // TODO: Compute pass indirect workgroups
                        }
                    }
                }
                &RenderCommand::CopyBufferToBuffer(src, src_off, dst, dst_off, size) => {
                    let src = bound_buffers.get(src).unwrap().as_ref();
                    let dst = bound_buffers.get(dst).unwrap().as_ref();
                    encoder.copy_buffer_to_buffer(src, src_off, dst, dst_off, size);
                }
            }
        }
        let commandbuffer = encoder.finish();
        ctx.queue.submit([commandbuffer]);

        Ok(())
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
