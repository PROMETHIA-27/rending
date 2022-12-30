use std::borrow::{Borrow, Cow};
use std::collections::BTreeMap;

use wgpu::{
    Buffer, BufferUsages, Extent3d, Texture, TextureDimension, TextureFormat, TextureUsages,
};

use crate::bitset::Bitset;
use crate::named_slotmap::NamedSlotMap;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub(crate) use self::buffer::{BufferBinding, BufferBindings, BufferUse};
pub use self::buffer::{BufferHandle, BufferSlice};
pub use self::layout::BindGroupLayout;
pub use self::layout::{BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle};
pub use self::module::ShaderModule;
use self::pipeline::ComputePipelines;
pub use self::pipeline::{ComputePipeline, ComputePipelineHandle, PipelineStorage};
use self::texture::TextureSize;
pub(crate) use self::texture::{
    TextureBinding, TextureBindings, TextureHandle, TextureViewDimension,
};

mod bindgroup;
mod buffer;
mod layout;
mod module;
mod pipeline;
mod texture;

pub(crate) type Buffers = BTreeMap<Cow<'static, str>, Buffer>;
pub(crate) type Textures = BTreeMap<Cow<'static, str>, Texture>;

#[derive(Debug)]
pub struct RenderResources {
    pub(crate) buffers: Buffers,
    pub(crate) textures: Textures,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            buffers: Buffers::new(),
            textures: Textures::new(),
        }
    }

    pub fn insert_buffer(&mut self, name: impl Into<Cow<'static, str>>, buffer: Buffer) {
        self.buffers.insert(name.into(), buffer);
    }

    pub fn get_buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.get(name)
    }

    pub fn insert_texture(&mut self, name: impl Into<Cow<'static, str>>, texture: Texture) {
        self.textures.insert(name.into(), texture);
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
    Texture(TextureHandle),
}

impl From<BufferHandle> for ResourceHandle {
    fn from(handle: BufferHandle) -> Self {
        Self::Buffer(handle)
    }
}

impl From<TextureHandle> for ResourceHandle {
    fn from(handle: TextureHandle) -> Self {
        Self::Texture(handle)
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

#[derive(Copy, Clone, Debug)]
pub enum ResourceUse {
    Buffer {
        size: u64,
        usage: BufferUsages,
        mapped: bool,
    },
    Texture {
        size: TextureSize,
        mip_level_count: u32,
        sample_count: u32,
        format: TextureFormat,
        usage: TextureUsages,
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
            ResourceHandle::Texture(_) => ResourceUse::Texture {
                size: TextureSize::D1 { x: 0 },
                mip_level_count: 1,
                sample_count: 1,
                format: TextureFormat::Rgba32Float,
                usage: TextureUsages::empty(),
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
