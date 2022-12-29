use std::borrow::Cow;
use std::collections::BTreeMap;

use naga::{FastHashMap, FastHashSet};
use slotmap::{SecondaryMap, SlotMap};
use thiserror::Error;
use wgpu::{Buffer, CommandEncoderDescriptor, ComputePassDescriptor};

use crate::bitset::Bitset;
use crate::commands::{ComputePassCommand, RenderCommand, RenderCommands};
use crate::named_slotmap::NamedSlotMap;
use crate::node::{NodeInput, NodeKey, NodeOutput, OrderingList, RenderNode, RenderNodeMeta};
use crate::reflect::ReflectedComputePipeline;
use crate::resources::{
    BindGroupCache, BufferHandle, ComputePipelineHandle, NodeResourceAccess, PipelineStorage,
    RenderResources, ResourceHandle, ResourceUse, Resources, VirtualBuffer, VirtualBuffers,
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
    pipelines: PipelineStorage,
}

// TODO: What is the lifetime of bind groups?
impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: NamedSlotMap::new(),
            pipelines: PipelineStorage::new(),
        }
    }

    pub fn add_node<T: RenderNode>(&mut self) {
        let (reads, writes, run_fn, type_name) = (
            T::reads(),
            T::writes(),
            T::run,
            Some(std::any::type_name::<T>()),
        );

        let meta = RenderNodeMeta {
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
        let mut nodes = vec![];
        let mut nodes_indices = SecondaryMap::new();
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
                nodes.push(next);
                nodes_indices.insert(next, nodes.len() - 1);
            }
        }

        // Run nodes to determine resource usage/build command queue
        let mut resources = Resources {
            node_index: 0,
            resources: vec![],
            resource_rev: BTreeMap::new(),
            resource_accesses: Vec::from_iter(
                std::iter::repeat(NodeResourceAccess {
                    reads: Bitset::new(0),
                    writes: Bitset::new(0),
                })
                .take(self.nodes.len()),
            ),
            virtual_buffers: VirtualBuffers::new(),
            compute_pipelines: &self.pipelines.compute_pipelines,
        };

        // TODO: Pool these bits
        let mut queue = vec![];
        let mut bind_cache = BindGroupCache::new();
        let mut resource_meta = FastHashMap::default();

        for (index, node) in nodes
            .iter()
            .map(|&key| self.nodes.get(key).unwrap())
            .enumerate()
        {
            resources.node_index = index;

            let mut commands = RenderCommands {
                pipelines: &self.pipelines,
                queue: &mut queue,
                bind_cache: &mut bind_cache,
                resource_meta: &mut resource_meta,
            };

            (node.run_fn)(&mut commands, &mut resources)
        }

        // # Detect ambiguities
        // TODO: Make this optional since it's so expensive
        // Traverse the graph and build up bitsets of all dependencies
        let mut stack = vec![];
        let all_dependencies: Vec<Bitset> = (0..nodes.len())
            .into_iter()
            .map(|index| {
                let mut bitset = Bitset::new(nodes.len());
                stack.push(index);
                while let Some(next) = stack.pop() {
                    if bitset.contains(next).unwrap() {
                        continue;
                    }
                    bitset.insert(next);
                    for &dep in &dependencies[nodes[next]] {
                        stack.push(nodes_indices[dep]);
                    }
                }
                bitset
            })
            .collect();

        let mut ambiguities = vec![];
        for index_a in 0..nodes.len() {
            for index_b in all_dependencies[index_a].inverted().iter() {
                if !all_dependencies[index_b].contains(index_a).unwrap() {
                    if do_nodes_conflict(&resources, index_a, index_b) {
                        ambiguities.push((
                            self.nodes.get_name(nodes[index_a]).unwrap().to_string(),
                            self.nodes.get_name(nodes[index_b]).unwrap().to_string(),
                        ))
                    }
                }
            }
        }

        if ambiguities.len() > 0 {
            return Err(RenderGraphError::WriteOrderAmbiguity(ambiguities));
        }

        Ok(RenderGraphCompilation {
            graph: self,
            queue,
            bind_cache,
            resource_meta,
        })
    }
}

#[derive(Debug)]
pub struct RenderGraphCompilation<'g> {
    graph: &'g mut RenderGraph,
    queue: Vec<RenderCommand>,
    bind_cache: BindGroupCache,
    resource_meta: FastHashMap<ResourceHandle, ResourceUse>,
}

impl RenderGraphCompilation<'_> {
    pub fn run(
        &mut self,
        ctx: RenderContext,
        res: &mut RenderResources,
    ) -> Result<(), RenderGraphError> {
        // Make bind groups
        self.bind_cache
            .create_groups(ctx, &mut self.graph.pipelines, res, &self.resource_meta);

        // Execute render command queue
        let mut encoder = ctx
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        for command in self.queue.iter() {
            match command {
                RenderCommand::WriteBuffer(handle, offset, data) => {
                    let buffer = res.buffers.get(*handle).unwrap();
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
                                    self.graph.pipelines.compute_pipelines.get(*handle).unwrap();
                                pass.set_pipeline(&pipeline.wgpu);
                            }
                            ComputePassCommand::BindGroup(index, handle) => {
                                let group = self.graph.pipelines.bind_groups.get(*handle).unwrap();
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
                    let src = res.buffers.get(src).unwrap();
                    let dst = res.buffers.get(dst).unwrap();
                    encoder.copy_buffer_to_buffer(&src, src_off, &dst, dst_off, size);
                }
            }
        }
        let commandbuffer = encoder.finish();
        ctx.queue.submit([commandbuffer]);

        Ok(())
    }
}

fn do_nodes_conflict(res: &Resources, left: usize, right: usize) -> bool {
    let (left, right) = (&res.resource_accesses[left], &res.resource_accesses[right]);

    if left.reads.intersects_with(&right.writes) {
        true
    } else if right.reads.intersects_with(&left.writes) {
        true
    } else if left.writes.intersects_with(&right.writes) {
        true
    } else {
        false
    }
}
