use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use slotmap::{SecondaryMap, SlotMap};
use wgpu::{BindGroup, Buffer, BufferUsages};

use crate::commands::{ReadBuffer, ReadWriteBuffer, RenderCommandResources, WriteBuffer};
use crate::named_slotmap::NamedSlotMap;
use crate::reflect::ReflectedComputePipeline;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub use self::buffer::BufferHandle;
pub(crate) use self::buffer::{BufferUse, VirtualBuffer};
pub use self::layout::BindGroupLayout;
pub use self::layout::{BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle};
pub use self::module::ShaderModule;
pub use self::pipeline::{ComputePipeline, ComputePipelineHandle};

mod bindgroup;
mod buffer;
mod layout;
mod module;
mod pipeline;

// TODO: Finish aliasing all fields
pub(crate) type DataResources = BTreeMap<Cow<'static, str>, ResourceHandle>;
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, VirtualBuffer>;
pub(crate) type ComputePipelines = NamedSlotMap<ComputePipelineHandle, ComputePipeline>;
pub(crate) type BindGroupLayouts = SlotMap<BindGroupLayoutHandle, BindGroupLayout>;
pub(crate) type PipelineLayouts = SlotMap<PipelineLayoutHandle, PipelineLayout>;

#[derive(Debug)]
pub(crate) struct RenderResources {
    // TODO: Consider whether resources should be stored in a different slotmap type. Probably not.
    pub data_resources: DataResources,
    pub virtual_buffers: VirtualBuffers,
    pub buffers: SecondaryMap<BufferHandle, Buffer>,
    pub compute_pipelines: ComputePipelines,
    pub bind_groups: SecondaryMap<BindGroupHandle, BindGroup>,
    pub bind_group_layouts: BindGroupLayouts,
    pub pipeline_layouts: PipelineLayouts,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            data_resources: BTreeMap::new(),
            virtual_buffers: NamedSlotMap::new(),
            buffers: SecondaryMap::new(),
            compute_pipelines: NamedSlotMap::new(),
            bind_groups: SecondaryMap::new(),
            bind_group_layouts: SlotMap::with_key(),
            pipeline_layouts: SlotMap::with_key(),
        }
    }

    pub fn insert_buffer(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        buffer: Buffer,
    ) -> BufferHandle {
        let name: Cow<str> = name.into();
        let handle = self
            .virtual_buffers
            .insert(name.clone(), VirtualBuffer { retained: true });
        self.buffers.insert(handle, buffer);
        self.data_resources
            .insert(name, ResourceHandle::Buffer(handle));
        handle
    }

    pub fn insert_compute_pipeline(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        ReflectedComputePipeline {
            pipeline,
            layout,
            group_layouts,
        }: ReflectedComputePipeline,
    ) -> ComputePipelineHandle {
        let groups = group_layouts
            .into_iter()
            .map(|(layout, entries)| {
                self.bind_group_layouts.insert(BindGroupLayout {
                    wgpu: layout,
                    entries,
                })
            })
            .collect();

        let layout = self.pipeline_layouts.insert(PipelineLayout {
            wgpu: layout,
            groups,
        });

        self.compute_pipelines.insert(
            name,
            ComputePipeline {
                wgpu: pipeline,
                layout,
            },
        )
    }

    pub fn split_for_render(&mut self) -> RenderCommandResources {
        RenderCommandResources {
            data_resources: &self.data_resources,
            virtual_buffers: &mut self.virtual_buffers,
            compute_pipelines: &self.compute_pipelines,
            bind_group_layouts: &self.bind_group_layouts,
            pipeline_layouts: &self.pipeline_layouts,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Buffer,
    Texture,
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceHandle {
    Buffer(BufferHandle),
}

impl From<BufferHandle> for ResourceHandle {
    fn from(handle: BufferHandle) -> Self {
        Self::Buffer(handle)
    }
}

#[derive(Debug)]
pub struct ResourceProvider<'s> {
    pub(crate) transient_reads: BTreeSet<&'s str>,
    pub(crate) transient_writes: BTreeSet<&'s str>,
    pub(crate) buffer_reads: BTreeMap<&'s str, BufferHandle>,
    pub(crate) buffer_writes: BTreeMap<&'s str, BufferHandle>,
    pub(crate) compute_pipelines: BTreeMap<&'s str, ComputePipelineHandle>,
    pub(crate) virtual_buffers: &'s mut VirtualBuffers,
}

impl ResourceProvider<'_> {
    pub(crate) fn new(virtual_buffers: &mut VirtualBuffers) -> Self {
        Self {
            transient_reads: BTreeSet::new(),
            transient_writes: BTreeSet::new(),
            buffer_reads: BTreeMap::new(),
            buffer_writes: BTreeMap::new(),
            compute_pipelines: BTreeMap::new(),
            virtual_buffers,
        }
    }

    pub fn read_buffer(&mut self, name: &str) -> ReadBuffer {
        ReadBuffer(
            self.buffer_reads
                .get(name)
                .copied()
                .or_else(|| {
                    self.transient_reads.contains(name).then(|| {
                        let handle = self
                            .virtual_buffers
                            .insert(name, VirtualBuffer { retained: false });
                        self.buffer_reads.insert(name, handle);
                        handle
                    })
                })
                .unwrap_or_else(|| panic!("no buffer named `{name}` available")),
        )
    }

    pub fn write_buffer(&mut self, name: &str) -> WriteBuffer {
        WriteBuffer(
            self.buffer_writes
                .get(name)
                .copied()
                .or_else(|| {
                    self.transient_writes.contains(name).then(|| {
                        let handle = self
                            .virtual_buffers
                            .insert(name, VirtualBuffer { retained: false });
                        self.buffer_writes.insert(name, handle);
                        handle
                    })
                })
                .unwrap_or_else(|| panic!("no buffer named `{name}` available")),
        )
    }

    pub fn readwrite_buffer(&mut self, name: &str) -> ReadWriteBuffer {
        let &buffer = self
            .buffer_reads
            .contains_key(name)
            .then(|| self.buffer_writes.get(name))
            .flatten()
            .unwrap_or_else(|| panic!("no inout buffer named {name} available"));
        ReadWriteBuffer(buffer)
    }

    pub fn compute_pipeline(&self, name: &str) -> ComputePipelineHandle {
        self.compute_pipelines
            .get(name)
            .cloned()
            .unwrap_or_else(|| panic!("no compute pipeline named `{name}` available"))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ResourceUse {
    Buffer {
        size: u64,
        usage: BufferUsages,
        mapped: bool,
    },
}

impl ResourceUse {
    pub fn default_from_handle(handle: ResourceHandle) -> Self {
        match handle {
            ResourceHandle::Buffer(_) => ResourceUse::Buffer {
                size: 0,
                usage: BufferUsages::empty(),
                mapped: false,
            },
        }
    }

    pub fn set_uniform_buffer(&mut self) {
        match self {
            ResourceUse::Buffer {
                size,
                usage,
                mapped,
            } => *usage |= BufferUsages::UNIFORM,
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }

    pub fn set_storage_buffer(&mut self) {
        match self {
            ResourceUse::Buffer {
                size,
                usage,
                mapped,
            } => *usage |= BufferUsages::STORAGE,
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum RWMode {
    Read,
    Write,
    ReadWrite,
}
