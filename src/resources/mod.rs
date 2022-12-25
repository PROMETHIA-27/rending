use std::borrow::Cow;
use std::collections::BTreeMap;

use slotmap::{SecondaryMap, SlotMap};
use wgpu::{BindGroup, BufferUsages};

use crate::commands::{InOutBuffer, InputBuffer, OutputBuffer};
use crate::named_slotmap::NamedSlotMap;
use crate::reflect::ReflectedComputePipeline;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub use self::buffer::BufferHandle;
pub(crate) use self::buffer::{Buffer, BufferUse};
pub use self::layout::BindGroupLayout;
pub use self::layout::{BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle};
pub use self::module::ShaderModule;
pub use self::pipeline::{ComputePipeline, ComputePipelineHandle};

mod bindgroup;
mod buffer;
mod layout;
mod module;
mod pipeline;

#[derive(Debug)]
pub(crate) struct RenderResources {
    // TODO: Consider whether resources should be stored in a different slotmap type. Probably not.
    pub data_resources: BTreeMap<Cow<'static, str>, ResourceHandle>,
    pub buffers: NamedSlotMap<BufferHandle, Buffer>,
    pub compute_pipelines: NamedSlotMap<ComputePipelineHandle, ComputePipeline>,
    pub bind_groups: SecondaryMap<BindGroupHandle, BindGroup>,
    pub bind_group_layouts: SlotMap<BindGroupLayoutHandle, BindGroupLayout>,
    pub pipeline_layouts: SlotMap<PipelineLayoutHandle, PipelineLayout>,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            data_resources: BTreeMap::new(),
            buffers: NamedSlotMap::new(),
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
        let handle = self.buffers.insert(name.clone(), buffer);
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
    pub(crate) in_buffers: BTreeMap<&'s str, BufferHandle>,
    pub(crate) out_buffers: BTreeMap<&'s str, BufferHandle>,
    pub(crate) compute_pipelines: BTreeMap<&'s str, ComputePipelineHandle>,
}

impl ResourceProvider<'_> {
    pub(crate) fn new() -> Self {
        Self {
            in_buffers: BTreeMap::new(),
            out_buffers: BTreeMap::new(),
            compute_pipelines: BTreeMap::new(),
        }
    }

    pub fn input_buffer(&self, name: &str) -> InputBuffer {
        InputBuffer(
            self.in_buffers
                .get(name)
                .copied()
                .unwrap_or_else(|| panic!("no buffer named `{name}` available")),
        )
    }

    pub fn output_buffer(&self, name: &str) -> OutputBuffer {
        OutputBuffer(
            self.out_buffers
                .get(name)
                .copied()
                .unwrap_or_else(|| panic!("no buffer named `{name}` available")),
        )
    }

    pub fn inout_buffer(&self, name: &str) -> InOutBuffer {
        let &buffer = self
            .in_buffers
            .contains_key(name)
            .then(|| self.out_buffers.get(name))
            .flatten()
            .unwrap_or_else(|| panic!("no inout buffer named {name} available"));
        InOutBuffer(buffer)
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
