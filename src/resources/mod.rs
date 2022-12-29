use std::borrow::{Borrow, Cow};
use std::collections::BTreeMap;

use wgpu::{Buffer, BufferUsages};

use crate::bitset::Bitset;
use crate::commands::{ReadBuffer, WriteBuffer};
use crate::named_slotmap::NamedSlotMap;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub(crate) use self::buffer::{BufferBinding, BufferBindings, BufferUse, VirtualBuffer};
pub use self::buffer::{BufferHandle, BufferSlice};
pub use self::layout::BindGroupLayout;
pub use self::layout::{BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle};
pub use self::module::ShaderModule;
use self::pipeline::ComputePipelines;
pub use self::pipeline::{ComputePipeline, ComputePipelineHandle, PipelineStorage};

mod bindgroup;
mod buffer;
mod layout;
mod module;
mod pipeline;
mod texture;

pub(crate) type Buffers = BTreeMap<Cow<'static, str>, Buffer>;

#[derive(Debug)]
pub struct RenderResources {
    pub(crate) buffers: Buffers,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            buffers: Buffers::new(),
        }
    }

    pub fn insert_buffer(&mut self, name: impl Into<Cow<'static, str>>, buffer: Buffer) {
        self.buffers.insert(name.into(), buffer);
    }

    pub fn get_buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.get(name)
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

bitflags::bitflags! {
    pub struct RWMode : u8 {
        const READ = 0b01;
        const WRITE = 0b10;
        const READWRITE = Self::READ.bits | Self::WRITE.bits;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct NodeResourceAccess {
    pub reads: Bitset,
    pub writes: Bitset,
}

impl NodeResourceAccess {
    pub fn new() -> Self {
        Self {
            reads: Bitset::new(0),
            writes: Bitset::new(0),
        }
    }
}

pub(crate) type ResourceList = Vec<(Cow<'static, str>, ResourceHandle)>;
pub(crate) type ResourceRev = BTreeMap<Cow<'static, str>, (usize, ResourceHandle)>;
pub(crate) type ResourceAccesses = Vec<NodeResourceAccess>;
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, VirtualBuffer>;

#[derive(Debug)]
pub struct Resources<'s> {
    pub(crate) node_index: usize,
    pub(crate) resources: ResourceList,
    pub(crate) resource_rev: ResourceRev,
    pub(crate) resource_accesses: ResourceAccesses,
    pub(crate) virtual_buffers: VirtualBuffers,
    pub(crate) compute_pipelines: &'s ComputePipelines,
}

impl Resources<'_> {
    pub fn read_buffer(&mut self, name: impl Into<Cow<'static, str>> + Borrow<str>) -> ReadBuffer {
        match self.resource_rev.get(name.borrow()) {
            Some(&(index, handle)) => match handle {
                ResourceHandle::Buffer(handle) => {
                    let accesses = &mut self.resource_accesses[self.node_index];
                    accesses.reads.insert(index);
                    ReadBuffer(handle)
                } /*
                  TODO: Fail for textures
                  */
            },
            None => {
                let name = name.into();
                let handle = self.virtual_buffers.insert(name.clone(), VirtualBuffer);
                let index = self.resources.len();
                self.resources
                    .push((name.clone(), ResourceHandle::Buffer(handle)));
                self.resource_rev
                    .insert(name, (index, ResourceHandle::Buffer(handle)));
                self.resource_accesses[self.node_index].reads.insert(index);
                ReadBuffer(handle)
            }
        }
    }

    pub fn write_buffer(
        &mut self,
        name: impl Into<Cow<'static, str>> + Borrow<str>,
    ) -> WriteBuffer {
        match self.resource_rev.get(name.borrow()) {
            Some(&(index, handle)) => match handle {
                ResourceHandle::Buffer(handle) => {
                    let accesses = &mut self.resource_accesses[self.node_index];
                    accesses.writes.insert(index);
                    WriteBuffer(handle)
                }
            },
            None => {
                let name = name.into();
                let handle = self.virtual_buffers.insert(name.clone(), VirtualBuffer);
                let index = self.resources.len();
                self.resources
                    .push((name.clone(), ResourceHandle::Buffer(handle)));
                self.resource_rev
                    .insert(name, (index, ResourceHandle::Buffer(handle)));
                self.resource_accesses[self.node_index].writes.insert(index);
                WriteBuffer(handle)
            }
        }
    }

    // pub fn readwrite_buffer(&mut self, name: &str) -> ReadWriteBuffer {
    //     let &buffer = self
    //         .buffer_reads
    //         .contains_key(name)
    //         .then(|| self.buffer_writes.get(name))
    //         .flatten()
    //         .unwrap_or_else(|| panic!("no inout buffer named {name} available"));
    //     ReadWriteBuffer(buffer)
    // }

    pub fn compute_pipeline(&self, name: &str) -> ComputePipelineHandle {
        self.compute_pipelines
            .get_key(name)
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

    pub fn set_buffer_size(&mut self, size: u64) {
        match self {
            ResourceUse::Buffer { size: buf_size, .. } => *buf_size = (*buf_size).max(size),
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
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
