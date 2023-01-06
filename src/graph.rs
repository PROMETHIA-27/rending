use naga::{FastHashMap, FastHashSet};
use slotmap::SecondaryMap;
use thiserror::Error;
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ImageCopyTexture, ImageDataLayout, TextureDescriptor,
};

use crate::bitset::Bitset;
use crate::commands::{
    ComputePassCommand, RenderCommand, RenderCommands, ResourceAccesses, ResourceList,
    VirtualBuffers, VirtualTextures,
};
use crate::named_slotmap::NamedSlotMap;
use crate::node::{NodeKey, OrderingList, RenderNode, RenderNodeMeta};
use crate::resources::{
    BindGroupCache, BufferBinding, BufferBindings, BufferError, NodeResourceAccess,
    PipelineStorage, RenderResources, ResourceConstraints, ResourceHandle, TextureBinding,
    TextureBindings, TextureError,
};
use crate::RenderContext;

#[derive(Debug, Error)]
pub enum RenderGraphError {
    #[error("referenced node that does not exist")]
    MissingNode,
    #[error("a cycle was detected in the node order between nodes `{0}` and `{1}`")]
    CycleDetected(String, String),
    #[error("Write order ambiguities were detected between the following nodes: {0:#?}. 
    Please ensure each of these nodes are explicitly ordered using `after` and `before` constraints.")]
    WriteOrderAmbiguity(Vec<(String, String)>),
    #[error(transparent)]
    Buffer(#[from] BufferError),
    #[error(transparent)]
    Texture(#[from] TextureError),
}

#[derive(Debug)]
pub struct RenderGraph {
    // TODO: Store nodes in a NamedDenseSlotMap
    nodes: NamedSlotMap<NodeKey, RenderNodeMeta>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: NamedSlotMap::new(),
        }
    }

    pub fn add_node<T: RenderNode>(&mut self) {
        let meta = RenderNodeMeta {
            // Vec::into_iter is used over .into_iter so that this errors if I change the functions to not be Vec
            before: Vec::into_iter(T::before()).collect(),
            after: Vec::into_iter(T::after()).collect(),
            run_fn: T::run,
            type_name: Some(std::any::type_name::<T>()),
        };

        self.nodes.insert(T::name(), meta);
    }

    pub fn compile<'g>(
        &'g mut self,
        ctx: RenderContext,
        pipelines: &'g PipelineStorage,
    ) -> Result<RenderGraphCompilation, RenderGraphError> {
        // Map of { dependent: dependencies }
        // TODO: Pool this
        let mut dependencies: SecondaryMap<NodeKey, Vec<NodeKey>> =
            SecondaryMap::with_capacity(self.nodes.len());

        for (key, node) in self.nodes.iter_key_value() {
            // Gather explicit ordering constraints, converted to `after`
            node.before
                .iter()
                .filter_map(|name| self.nodes.get_key(&name[..]))
                .for_each(|dependent_key| {
                    dependencies
                        .entry(dependent_key)
                        .unwrap()
                        .or_default()
                        .push(key);
                });

            dependencies.entry(key).unwrap().or_default().extend(
                node.after
                    .iter()
                    .filter_map(|name| self.nodes.get_key(&name[..])),
            );
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
        // TODO: Pool these bits
        let mut queue = vec![];
        let mut bind_cache = BindGroupCache::new();
        let mut constraints = ResourceConstraints::default();

        let mut commands = RenderCommands {
            pipelines: &pipelines,
            queue: &mut queue,
            bind_cache: &mut bind_cache,
            constraints: &mut constraints,
            node_index: 0,
            resources: ResourceList::new(),
            resource_accesses: ResourceAccesses::from_iter(
                std::iter::repeat(NodeResourceAccess::new()).take(self.nodes.len()),
            ),
            virtual_buffers: VirtualBuffers::new(),
            virtual_textures: VirtualTextures::new(),
        };

        for (index, node) in nodes
            .iter()
            .map(|&key| self.nodes.get(key).unwrap())
            .enumerate()
        {
            commands.node_index = index;

            (node.run_fn)(&mut commands)
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
                    if do_nodes_conflict(&commands, index_a, index_b) {
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

        let RenderCommands {
            virtual_buffers,
            virtual_textures,
            ..
        } = commands;

        // Verify constraints
        for (name, texture) in virtual_textures.iter_names() {
            let constraints = constraints.textures.get(texture).unwrap();
            if let Some(err) = constraints.verify(name) {
                return Err(err.into());
            }
        }

        Ok(RenderGraphCompilation {
            graph: self,
            pipelines,
            queue,
            bind_cache,
            constraints,
            virtual_buffers,
            virtual_textures,
        })
    }
}

#[derive(Debug)]
pub struct RenderGraphCompilation<'g> {
    graph: &'g RenderGraph,
    pipelines: &'g PipelineStorage,
    queue: Vec<RenderCommand>,
    bind_cache: BindGroupCache,
    constraints: ResourceConstraints,
    virtual_buffers: VirtualBuffers,
    virtual_textures: VirtualTextures,
}

impl RenderGraphCompilation<'_> {
    pub fn run(
        &mut self,
        ctx: RenderContext,
        res: &RenderResources,
    ) -> Result<(), RenderGraphError> {
        let bound_buffers: BufferBindings = self
            .virtual_buffers
            .iter_names()
            .map(|(name, handle)| {
                let constraints = self.constraints.buffers.get(handle).unwrap();

                // Bind retained resources
                if let Some(buf) = res.buffers.get(name) {
                    if let Some(err) = constraints.verify_retained(buf, name) {
                        return Err(err);
                    }

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
                let constraints =
                    self.constraints.textures.get(handle).unwrap();

                // Bind retained resources
                if let Some(texture) = res.textures.get(name) {
                    if let Some(err) = constraints.verify_retained(texture, name) {
                        return Err(err)
                    }

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

        // Make bind groups
        let bind_groups =
            self.bind_cache
                .create_groups(ctx, &self.pipelines, &bound_buffers, &bound_textures);

        // Execute render command queue
        let mut encoder = ctx
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });
        for command in self.queue.iter() {
            match command {
                RenderCommand::WriteBuffer(handle, offset, data) => {
                    let buffer = bound_buffers.get(*handle).unwrap().as_ref();
                    ctx.queue.write_buffer(&buffer, *offset, &data[..]);
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
                    encoder.copy_buffer_to_buffer(&src, src_off, &dst, dst_off, size);
                }
            }
        }
        let commandbuffer = encoder.finish();
        ctx.queue.submit([commandbuffer]);

        Ok(())
    }
}

fn do_nodes_conflict(cmd: &RenderCommands, left: usize, right: usize) -> bool {
    let (left, right) = (&cmd.resource_accesses[left], &cmd.resource_accesses[right]);

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
