use std::borrow::{Borrow, Cow};
use std::collections::{BTreeMap, BTreeSet};

use naga::FastHashMap;
use slotmap::{SecondaryMap, SlotMap};
use wgpu::{BindGroup, Buffer, BufferUsages};

use crate::bitset::Bitset;
use crate::commands::{ReadBuffer, ReadWriteBuffer, WriteBuffer};
use crate::named_slotmap::NamedSlotMap;
use crate::reflect::ReflectedComputePipeline;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub use self::buffer::BufferHandle;
pub(crate) use self::buffer::{BufferUse, VirtualBuffer};
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

pub(crate) type DataResources = BTreeMap<Cow<'static, str>, ResourceHandle>;
pub(crate) type VirtualBuffers = NamedSlotMap<BufferHandle, VirtualBuffer>;
pub(crate) type Buffers = SecondaryMap<BufferHandle, Buffer>;

#[derive(Debug)]
pub struct RenderResources {
    // TODO: Consider whether resources should be stored in a different slotmap type. Probably not.
    pub(crate) data_resources: DataResources,
    pub(crate) buffers: Buffers,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            data_resources: BTreeMap::new(),
            buffers: SecondaryMap::new(),
        }
    }

    // pub fn insert_buffer(
    //     &mut self,
    //     name: impl Into<Cow<'static, str>>,
    //     buffer: Buffer,
    // ) -> BufferHandle {
    //     let name: Cow<str> = name.into();
    //     let handle = self
    //         .virtual_buffers
    //         .insert(name.clone(), VirtualBuffer { retained: true });
    //     self.buffers.insert(handle, buffer);
    //     self.data_resources
    //         .insert(name, ResourceHandle::Buffer(handle));
    //     handle
    // }
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
    pub(crate) struct ResourceAccess : u8 {
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

type ResourceList = Vec<(Cow<'static, str>, ResourceHandle)>;
type ResourceRev = BTreeMap<Cow<'static, str>, (usize, ResourceHandle)>;
type ResourceAccesses = Vec<NodeResourceAccess>;

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
    pub fn read_buffer(
        &mut self,
        name: impl Into<Cow<'static, str>> + Borrow<str> + Clone,
    ) -> ReadBuffer {
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
                let handle = self
                    .virtual_buffers
                    .insert(name.clone().into(), VirtualBuffer);
                let index = self.resources.len();
                self.resources
                    .push((name.clone().into(), ResourceHandle::Buffer(handle)));
                self.resource_rev
                    .insert(name.into(), (index, ResourceHandle::Buffer(handle)));
                self.resource_accesses[self.node_index].reads.insert(index);
                ReadBuffer(handle)
            }
        }
    }

    // pub fn write_buffer(&mut self, name: &str) -> WriteBuffer {
    //     WriteBuffer(
    //         self.buffer_writes
    //             .get(name)
    //             .copied()
    //             .or_else(|| {
    //                 self.transient_writes.contains(name).then(|| {
    //                     let handle = self
    //                         .virtual_buffers
    //                         .insert(name, VirtualBuffer { retained: false });
    //                     self.buffer_writes.insert(name, handle);
    //                     handle
    //                 })
    //             })
    //             .unwrap_or_else(|| panic!("no buffer named `{name}` available")),
    //     )
    // }

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
