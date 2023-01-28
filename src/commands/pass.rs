use std::num::NonZeroU32;

use smallvec::SmallVec;
use wgpu::Extent3d;

use crate::resources::{
    BindGroupHandle, BufferUse, ComputePipelineHandle, RWMode, ResourceBinding, TextureAspect,
    TextureSampleType, TextureViewDimension,
};

use super::{RenderCommand, RenderCommands};

#[derive(Debug)]
pub(crate) enum ComputePassCommand {
    SetPipeline(ComputePipelineHandle),
    BindGroup(u32, BindGroupHandle),
    Dispatch(u32, u32, u32),
}

type TempBindings = SmallVec<[(u32, ResourceBinding); 16]>;

pub struct ComputePassCommands<'c, 'q, 'r> {
    pub(crate) commands: &'c mut RenderCommands<'q, 'r>,
    pub(crate) command_index: usize,
    pub(crate) pipeline: Option<ComputePipelineHandle>,
    pub(crate) bindings: [Option<TempBindings>; wgpu_core::MAX_BIND_GROUPS],
}

impl ComputePassCommands<'_, '_, '_> {
    fn enqueue(&mut self, c: ComputePassCommand) {
        match &mut self.commands.queue[self.command_index] {
            RenderCommand::ComputePass(_, queue) => queue.push(c),
            _ => unreachable!(),
        }
    }

    pub fn pipeline(mut self, pipeline: ComputePipelineHandle) -> Self {
        self.pipeline = Some(pipeline);
        self.enqueue(ComputePassCommand::SetPipeline(pipeline));
        self
    }

    pub fn bind_group<I: IntoIterator<Item = (u32, ResourceBinding)>>(
        mut self,
        index: u32,
        bind_group: I,
    ) -> Self {
        self.bindings[index as usize] = Some(SmallVec::from_iter(bind_group));
        self
    }

    pub fn dispatch(self, x: u32, y: u32, z: u32) -> Self {
        // Have to temporarily destruct to get around aliasing borrows
        let Self {
            commands,
            command_index,
            pipeline,
            mut bindings,
        } = self;

        let compute_pipeline = pipeline
            .map(|handle| commands.pipelines.compute_pipelines.get(handle))
            .expect("attempted to dispatch without a pipeline set")
            .unwrap();
        let layout = commands
            .pipelines
            .pipeline_layouts
            .get(compute_pipeline.layout)
            .unwrap();

        for (group_index, (binding, &group_layout)) in bindings
            .iter_mut()
            .take(layout.groups.len())
            .zip(layout.groups.iter())
            .enumerate()
        {
            let Some(binding) = binding.as_mut() else { panic!("not enough groups bound for pipeline") };

            let handle = commands.bind_cache.get_handle(group_layout, &binding[..]);
            let group_layout = commands
                .pipelines
                .bind_group_layouts
                .get(layout.groups[group_index as usize])
                .unwrap();

            for &mut (binding, ref mut resource) in binding.iter_mut() {
                let entry = group_layout.entries[binding as usize];

                match (resource, entry.ty) {
                    (
                        &mut ResourceBinding::Buffer {
                            handle,
                            offset,
                            size,
                            usage,
                        },
                        wgpu::BindingType::Buffer {
                            ty,
                            min_binding_size,
                            ..
                        },
                    ) => {
                        let constraints = commands
                            .constraints
                            .buffers
                            .entry(handle)
                            .unwrap()
                            .or_default();
                        let binding_size = size.map(u64::from);
                        let min_binding_size = min_binding_size.map(u64::from);
                        let min_size = match (binding_size, min_binding_size) {
                            (Some(binding), Some(min)) => {
                                assert!(
                                    binding >= min,
                                    "attempted to bind {binding} buffer bytes 
                                    when the minimum binding size was {min} at 
                                    binding slot {{ {group_index}, {binding} }}"
                                );
                                binding + offset
                            }
                            (Some(binding), None) => binding + offset,
                            (None, Some(min)) => min + offset,
                            (None, None) => 0, // TODO: Might be a better way to handle this case,
                                               // since right now it'll probably break if no other usage makes the buffer large enough.
                                               // That should be really silly and rare though
                        };
                        constraints.set_size(min_size);

                        match ty {
                            wgpu::BufferBindingType::Uniform => {
                                assert!(
                                    usage.matches_use(BufferUse::Uniform),
                                    "buffer bound to uniform slot must be passed as a uniform; try using `.uniform()` on a `BufferSlice`"
                                );
                                constraints.set_uniform();
                                commands.mark_resource_read(handle.into());
                            }
                            wgpu::BufferBindingType::Storage { read_only } => {
                                assert!(
                                    usage.matches_use(BufferUse::Storage(match read_only {
                                        true => RWMode::READ,
                                        false => RWMode::READWRITE,
                                    })),
                                    "buffer bound to storage slot must be passed as a storage with the same ReadWrite access mode; try using `.storage()` on a `BufferSlice`, and ensure both have the same access mode"
                                );
                                constraints.set_storage();
                                commands.mark_resource_read(handle.into());
                                if !read_only {
                                    commands.mark_resource_write(handle.into())
                                }
                            }
                        }
                    }
                    (
                        &mut ResourceBinding::Texture {
                            handle,
                            ref mut dimension,
                            base_mip,
                            mip_count,
                            base_layer,
                            layer_count,
                            aspect,
                        },
                        wgpu::BindingType::Texture {
                            sample_type,
                            view_dimension,
                            multisampled,
                        },
                    ) => {
                        let constraints = commands
                            .constraints
                            .textures
                            .entry(handle)
                            .unwrap()
                            .or_default();
                        let min_mips = match mip_count {
                            Some(count) => base_mip + count.get(),
                            None => base_mip,
                        };
                        constraints.set_mip_count(min_mips);
                        constraints.set_min_size(Extent3d {
                            width: 0,
                            height: 0,
                            depth_or_array_layers: base_layer
                                + layer_count.map(NonZeroU32::get).unwrap_or(0),
                        });
                        match aspect {
                            TextureAspect::StencilOnly => constraints.has_stencil = true,
                            TextureAspect::DepthOnly => constraints.has_depth = true,
                            _ => (),
                        }
                        constraints.set_sample_type(TextureSampleType::from_wgpu(sample_type));

                        *dimension = Some(TextureViewDimension::from_wgpu(view_dimension));

                        if multisampled {
                            constraints.set_multisampled();
                        }

                        constraints.set_texture_binding();
                        commands.mark_resource_read(handle.into());
                    }
                    (
                        &mut ResourceBinding::Texture {
                            handle,
                            ref mut dimension,
                            base_mip,
                            mip_count,
                            base_layer,
                            layer_count,
                            aspect,
                        },
                        wgpu::BindingType::StorageTexture {
                            access,
                            format,
                            view_dimension,
                        },
                    ) => {
                        let constraints = commands
                            .constraints
                            .textures
                            .entry(handle)
                            .unwrap()
                            .or_default();
                        let min_mips = match mip_count {
                            Some(count) => base_mip + count.get(),
                            None => base_mip,
                        };
                        constraints.set_mip_count(min_mips);
                        constraints.set_min_size(Extent3d {
                            width: 0,
                            height: 0,
                            depth_or_array_layers: base_layer
                                + layer_count.map(NonZeroU32::get).unwrap_or(0),
                        });
                        match aspect {
                            TextureAspect::StencilOnly => constraints.has_stencil = true,
                            TextureAspect::DepthOnly => constraints.has_depth = true,
                            _ => (),
                        }

                        *dimension = Some(TextureViewDimension::from_wgpu(view_dimension));

                        constraints.set_format(format);
                        constraints.set_storage_binding();
                        match access {
                            wgpu::StorageTextureAccess::WriteOnly => {
                                commands.mark_resource_write(handle.into())
                            }
                            wgpu::StorageTextureAccess::ReadOnly => {
                                commands.mark_resource_read(handle.into())
                            }
                            wgpu::StorageTextureAccess::ReadWrite => {
                                commands.mark_resource_read(handle.into());
                                commands.mark_resource_write(handle.into());
                            }
                        }
                    }
                    // (
                    //     &mut ResourceBinding::Sampler { handle },
                    //     wgpu::BindingType::Sampler(binding_ty),
                    // ) => {
                    //     let constraints = commands
                    //         .constraints
                    //         .samplers
                    //         .entry(handle)
                    //         .unwrap()
                    //         .or_default();
                    //     constraints.set_type(binding_ty);
                    // }
                    // TODO: Make good error messages for when binding does not match slot type
                    (binding, bind_ty) => panic!("Uh oh! {binding:?} ||| {bind_ty:?}"),
                }
            }

            match &mut commands.queue[command_index] {
                RenderCommand::ComputePass(_, queue) => {
                    queue.push(ComputePassCommand::BindGroup(group_index as u32, handle))
                }
                _ => unreachable!(),
            }
        }

        // this == self but `self` can't be used here
        let mut this = Self {
            commands,
            command_index,
            pipeline,
            bindings,
        };

        this.enqueue(ComputePassCommand::Dispatch(x, y, z));
        this
    }
}
