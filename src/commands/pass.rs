use std::borrow::Cow;

use smallvec::SmallVec;

use crate::resources::{
    BindGroupHandle, BufferUse, ComputePipelineHandle, ResourceBinding,
    ResourceUse, RWMode,
};

use super::RenderCommands;

#[derive(Debug)]
pub(crate) enum ComputePassCommand {
    SetPipeline(ComputePipelineHandle),
    BindGroup(u32, BindGroupHandle),
    Dispatch(u32, u32, u32),
}

type TempBindings = SmallVec<[(u32, ResourceBinding); 16]>;

pub struct ComputePassCommands<'c, 'q, 'r> {
    pub(crate) commands: &'c mut RenderCommands<'q, 'r>,
    pub(crate) label: Option<Cow<'static, str>>,
    pub(crate) queue: Vec<ComputePassCommand>,
    pub(crate) pipeline: Option<ComputePipelineHandle>,
    pub(crate) bindings: [Option<TempBindings>; wgpu_core::MAX_BIND_GROUPS],
}

impl ComputePassCommands<'_, '_, '_> {
    fn enqueue(&mut self, c: ComputePassCommand) {
        self.queue.push(c)
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
            label,
            mut queue,
            pipeline,
            bindings,
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
            .iter()
            .take(layout.groups.len())
            .zip(layout.groups.iter())
            .enumerate()
        {
            let Some(binding) = binding.as_ref() else { panic!("not enough groups bound for pipeline") };

            let handle = commands.bind_cache.get_handle(group_layout, &binding[..]);
            let group_layout = commands
                .pipelines
                .bind_group_layouts
                .get(layout.groups[group_index as usize])
                .unwrap();

            // TODO: Test that input buffers can only be read from and output can only be written to now
            for &(binding, resource) in binding.iter() {
                let uses = commands
                    .resource_meta
                    .entry(resource.handle())
                    .or_insert_with(|| ResourceUse::default_from_handle(resource.handle()));
                let entry = group_layout.entries[binding as usize];

                match entry.ty {
                    wgpu::BindingType::Buffer { ty, .. } => match ty {
                        wgpu::BufferBindingType::Uniform => {
                            assert!(
                                resource.buffer_use().matches_use(BufferUse::Uniform), 
                                "buffer bound to uniform slot must be passed as a uniform; try using `.uniform()` on a `BufferSlice`"
                            );
                            uses.set_uniform_buffer();
                        }
                        wgpu::BufferBindingType::Storage { read_only } => {
                            assert!(
                                resource.buffer_use().matches_use(BufferUse::Storage(match read_only {
                                    true => RWMode::Read,
                                    false => RWMode::ReadWrite,
                                })), 
                                "buffer bound to storage slot must be passed as a storage with the same ReadWrite access mode; try using `.storage()` on a `BufferSlice`, and ensure both have the same access mode"
                            );
                            uses.set_storage_buffer();
                        }
                    },
                    wgpu::BindingType::Sampler(_) => todo!(),
                    wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension,
                        multisampled,
                    } => todo!(),
                    wgpu::BindingType::StorageTexture {
                        access,
                        format,
                        view_dimension,
                    } => todo!(),
                }
            }
            queue.push(ComputePassCommand::BindGroup(group_index as u32, handle));
        }

        // this == self but self can't be used here
        let mut this = Self {
            commands,
            label,
            queue,
            pipeline,
            bindings,
        };

        this.enqueue(ComputePassCommand::Dispatch(x, y, z));
        this
    }
}
