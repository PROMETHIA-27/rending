use smallvec::SmallVec;

use crate::resources::{
    BindGroupHandle, BufferUse, ComputePipelineHandle, RWMode, ResourceBinding,
    TextureViewDimension,
};

use super::{RenderCommand, RenderCommands};

#[derive(Debug)]
pub(crate) enum ComputePassCommand {
    SetPipeline(ComputePipelineHandle),
    BindGroup(u32, BindGroupHandle),
    Dispatch(u32, u32, u32),
}

type TempBindings = SmallVec<[(u32, ResourceBinding); 16]>;

pub struct ComputePassCommands<'c, 'r> {
    pub(crate) commands: &'c mut RenderCommands<'r>,
    pub(crate) command_index: usize,
    pub(crate) pipeline: Option<ComputePipelineHandle>,
    // TODO: This is a **heavy** array being passed by value
    pub(crate) bindings: [Option<TempBindings>; wgpu_core::MAX_BIND_GROUPS],
}

impl ComputePassCommands<'_, '_> {
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

    pub fn dispatch(mut self, x: u32, y: u32, z: u32) -> Self {
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
                .get(layout.groups[group_index])
                .unwrap();

            for &mut (binding, ref mut resource) in binding.iter_mut() {
                let Some(entry) = group_layout.entries.get(&binding) else { continue };

                match (resource, entry.ty) {
                    (
                        &mut ResourceBinding::Buffer { size, usage, .. },
                        wgpu::BindingType::Buffer {
                            ty,
                            min_binding_size,
                            ..
                        },
                    ) => {
                        let binding_size = size.map(u64::from);
                        let min_binding_size = min_binding_size.map(u64::from);
                        if let (Some(binding), Some(min)) = (binding_size, min_binding_size) {
                            assert!(
                                binding >= min,
                                "attempted to bind {binding} buffer bytes 
                                    when the minimum binding size was {min} at 
                                    binding slot {{ {group_index}, {binding} }}"
                            );
                        };

                        match ty {
                            wgpu::BufferBindingType::Uniform => {
                                assert!(
                                    usage.matches_use(BufferUse::Uniform),
                                    "buffer bound to uniform slot must be passed as a uniform; try using `.uniform()` on a `BufferSlice`"
                                );
                            }
                            wgpu::BufferBindingType::Storage { read_only } => {
                                assert!(
                                    usage.matches_use(BufferUse::Storage(match read_only {
                                        true => RWMode::READ,
                                        false => RWMode::READWRITE,
                                    })),
                                    "buffer bound to storage slot must be passed as a storage with the same ReadWrite access mode; try using `.storage()` on a `BufferSlice`, and ensure both have the same access mode"
                                );
                            }
                        }
                    }
                    (
                        &mut ResourceBinding::Texture {
                            ref mut dimension, ..
                        },
                        wgpu::BindingType::Texture { view_dimension, .. },
                    ) => {
                        *dimension = Some(TextureViewDimension::from_wgpu(view_dimension));
                    }
                    (
                        &mut ResourceBinding::Texture {
                            ref mut dimension, ..
                        },
                        wgpu::BindingType::StorageTexture { view_dimension, .. },
                    ) => {
                        *dimension = Some(TextureViewDimension::from_wgpu(view_dimension));
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

        self = Self {
            commands,
            command_index,
            pipeline,
            bindings,
        };

        self.enqueue(ComputePassCommand::Dispatch(x, y, z));
        self
    }
}
