use std::borrow::Cow;
use std::collections::BTreeMap;

use naga::{FastHashMap, FastHashSet};
use slotmap::{SecondaryMap, SlotMap};
use thiserror::Error;
use wgpu::{Buffer, CommandEncoderDescriptor, ComputePassDescriptor};

use crate::bitset::Bitset;
use crate::commands::{ComputePassCommand, RenderCommand, RenderCommandResources, RenderCommands};
use crate::named_slotmap::NamedSlotMap;
use crate::node::{NodeInput, NodeKey, NodeOutput, OrderingList, RenderNode, RenderNodeMeta};
use crate::reflect::ReflectedComputePipeline;
use crate::resources::{
    BindGroupCache, BufferHandle, ComputePipelineHandle, RenderResources, ResourceHandle,
    ResourceProvider, VirtualBuffer,
};
use crate::util::IterCombinations;
use crate::RenderContext;

#[derive(Debug, Error)]
pub enum RenderGraphError {
    #[error("referenced node that does not exist")]
    MissingNode,
    #[error("could not find retained resource {0}")]
    MissingResource(String),
    #[error("a cycle was detected in the node order between nodes `{0}` and `{1}`")]
    CycleDetected(String, String),
    #[error("Write order ambiguities were detected between the following nodes: {0:#?}. 
    Please ensure each of these nodes are explicitly ordered using `after` and `before` constraints.")]
    WriteOrderAmbiguity(Vec<(String, String)>),
}

#[derive(Debug)]
pub struct RenderGraph {
    // TODO: Store nodes in a NamedDenseSlotMap
    nodes: NamedSlotMap<NodeKey, RenderNodeMeta>,
    resources: RenderResources,
}

// TODO: What is the lifetime of bind groups?
impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: NamedSlotMap::new(),
            resources: RenderResources::new(),
        }
    }

    pub fn add_node<T: RenderNode>(&mut self) {
        let (inputs, outputs, run_fn, type_name) = (
            T::reads(),
            T::writes(),
            T::run,
            Some(std::any::type_name::<T>()),
        );

        let meta = RenderNodeMeta {
            reads: Vec::into_iter(T::reads())
                .map(|input| input.resource)
                .collect(),
            writes: Vec::into_iter(T::writes())
                .map(|output| output.resource)
                .collect(),
            // Vec::into_iter is used over .into_iter so that this errors if I change the functions to not be Vec
            before: OrderingList::Names(Vec::into_iter(T::before()).collect()),
            after: OrderingList::Names(Vec::into_iter(T::after()).collect()),
            run_fn,
            type_name,
        };

        self.nodes.insert(T::name(), meta);
    }

    pub fn compile(
        &mut self,
        ctx: RenderContext,
    ) -> Result<RenderGraphCompilation, RenderGraphError> {
        // Map of { dependent: dependencies }
        // TODO: Pool this
        let mut dependencies: SecondaryMap<NodeKey, Vec<NodeKey>> =
            SecondaryMap::with_capacity(self.nodes.len());

        for (key, node) in self.nodes.iter_key_value() {
            // Gather explicit ordering constraints, converted to `after`
            match &node.before {
                OrderingList::Names(names) => {
                    names
                        .iter()
                        .filter_map(|name| self.nodes.get_key(&name[..]))
                        .for_each(|dependent_key| {
                            dependencies
                                .entry(dependent_key)
                                .unwrap()
                                .or_default()
                                .push(key);
                        });
                }
                _ => panic!(), // TODO: Remove OrderingList variants!
            }

            match &node.after {
                OrderingList::Names(names) => {
                    dependencies.entry(key).unwrap().or_default().extend(
                        names
                            .iter()
                            .filter_map(|name| self.nodes.get_key(&name[..])),
                    );
                }
                _ => panic!(),
            }
        }

        // Topological sort the nodes into a linear order for execution, taking into account
        // explicit ordering. At the same time, detect cycles, and detect write order ambiguities.
        // TODO: Pool these too
        let mut queue = vec![];
        let mut queue_indices = SecondaryMap::new();
        let mut stack = vec![];
        let mut visited = FastHashSet::default();

        for (key, _) in self.nodes.iter_key_value() {
            if visited.contains(&key) {
                continue;
            }

            let mut pointer = 0;
            stack.push(key);
            visited.insert(key);
            while pointer < stack.len() {
                let next = stack[pointer];
                for &dependency in dependencies.get(next).unwrap() {
                    if visited.contains(&dependency) {
                        if dependency == key {
                            return Err(RenderGraphError::CycleDetected(
                                self.nodes.get_name(key).unwrap().to_string(),
                                self.nodes.get_name(next).unwrap().to_string(),
                            ));
                        }

                        continue;
                    }

                    stack.push(dependency);
                    visited.insert(dependency);
                }
                pointer += 1;
            }

            while let Some(next) = stack.pop() {
                queue.push(next);
                queue_indices.insert(next, queue.len() - 1);
            }
        }

        // # Detect ambiguities
        // TODO: Make this optional since it's so expensive
        // Traverse the graph and build up bitsets of all dependencies
        let mut stack = vec![];
        let all_dependencies: Vec<Bitset> = (0..queue.len())
            .into_iter()
            .map(|index| {
                let mut bitset = Bitset::new(queue.len());
                stack.push(index);
                while let Some(next) = stack.pop() {
                    if bitset.contains(next).unwrap() {
                        continue;
                    }
                    bitset.insert(next).unwrap();
                    for &dep in &dependencies[queue[next]] {
                        stack.push(queue_indices[dep]);
                    }
                }
                bitset
            })
            .collect();

        let mut ambiguities = vec![];
        for index_a in 0..queue.len() {
            for index_b in all_dependencies[index_a].inverted().iter() {
                if !all_dependencies[index_b].contains(index_a).unwrap() {
                    let (a, b) = (
                        self.nodes.get(queue[index_a]).unwrap(),
                        self.nodes.get(queue[index_b]).unwrap(),
                    );

                    if a.conflicts_with(b) {
                        ambiguities.push((
                            self.nodes.get_name(queue[index_a]).unwrap().to_string(),
                            self.nodes.get_name(queue[index_b]).unwrap().to_string(),
                        ))
                    }
                }
            }
        }

        if ambiguities.len() > 0 {
            return Err(RenderGraphError::WriteOrderAmbiguity(ambiguities));
        }

        Ok(RenderGraphCompilation {
            nodes: queue
                .into_iter()
                .map(|key| self.nodes.get(key).unwrap().clone())
                .collect(),
            resources: &mut self.resources,
        })
    }

    pub fn insert_compute_pipeline(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        pipeline: ReflectedComputePipeline,
    ) -> ComputePipelineHandle {
        self.resources.insert_compute_pipeline(name, pipeline)
    }

    pub fn insert_buffer(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        buffer: Buffer,
    ) -> BufferHandle {
        self.resources.insert_buffer(name, buffer)
    }

    pub fn get_buffer(&self, handle: BufferHandle) -> Option<&Buffer> {
        self.resources.buffers.get(handle)
    }

    pub fn get_buffer_named(&mut self, name: &str) -> Option<&Buffer> {
        self.resources
            .virtual_buffers
            .get_key(name)
            .map(|handle| self.resources.buffers.get(handle).unwrap())
    }
}

#[derive(Debug)]
pub struct RenderGraphCompilation<'g> {
    nodes: Vec<RenderNodeMeta>,
    resources: &'g mut RenderResources,
}

impl RenderGraphCompilation<'_> {
    pub fn run(&mut self, ctx: RenderContext) -> Result<(), RenderGraphError> {
        // TODO: Pool these bits
        let queue = &mut vec![];
        let bind_cache = &mut BindGroupCache::new();
        let resource_meta = &mut FastHashMap::default();

        let mut provider = ResourceProvider::new();

        let RenderCommandResources {
            data_resources,
            virtual_buffers,
            compute_pipelines,
            bind_group_layouts,
            pipeline_layouts,
        } = self.resources.split_for_render();

        for (name, pipeline) in compute_pipelines.iter_names() {
            provider.compute_pipelines.insert(name, pipeline);
        }

        for node in &self.nodes {
            provider.buffer_reads.clear();
            provider.buffer_writes.clear();

            for read in &node.reads {
                if let Some(&resource) = data_resources.get(&read[..]) {
                    match resource {
                        ResourceHandle::Buffer(handle) => {
                            provider.buffer_reads.insert(&read[..], handle);
                        }
                    }
                } else {
                    // There's no retained/previously instantiated transient resource, so create a transient
                    provider.transient_reads.insert(&read[..]);
                }
            }

            for write in &node.writes {
                if let Some(&resource) = data_resources.get(&write[..]) {
                    match resource {
                        ResourceHandle::Buffer(handle) => {
                            provider.buffer_writes.insert(&write[..], handle);
                        }
                    }
                } else {
                    provider.transient_writes.insert(&write[..]);
                }
            }

            let mut commands = RenderCommands {
                resources: RenderCommandResources {
                    data_resources,
                    virtual_buffers,
                    compute_pipelines,
                    bind_group_layouts,
                    pipeline_layouts,
                },
                queue,
                bind_cache,
                resource_meta,
            };

            (node.run_fn)(&mut commands, &provider)
        }

        // Make bind groups
        bind_cache.create_groups(ctx, &mut self.resources, &resource_meta);

        // Execute render command queue
        let mut encoder = ctx
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        for command in queue.iter() {
            match command {
                RenderCommand::WriteBuffer(handle, offset, data) => {
                    let buffer = self.resources.buffers.get(*handle).unwrap();
                    ctx.queue.write_buffer(&buffer, *offset, &data[..]);
                }
                RenderCommand::ComputePass(label, commands) => {
                    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: label.as_ref().map(|cow| &cow[..]),
                    });
                    for command in commands.iter() {
                        match command {
                            ComputePassCommand::SetPipeline(handle) => {
                                let pipeline =
                                    self.resources.compute_pipelines.get(*handle).unwrap();
                                pass.set_pipeline(&pipeline.wgpu);
                            }
                            ComputePassCommand::BindGroup(index, handle) => {
                                let group = self.resources.bind_groups.get(*handle).unwrap();
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
                    let src = self.resources.buffers.get(src).unwrap();
                    let dst = self.resources.buffers.get(dst).unwrap();
                    encoder.copy_buffer_to_buffer(&src, src_off, &dst, dst_off, size);
                }
            }
        }
        let commandbuffer = encoder.finish();
        ctx.queue.submit([commandbuffer]);

        Ok(())
    }
}
