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
pub use self::texture::{TextureAspect, TextureSize};
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
pub enum ResourceMeta {
    Buffer {
        size: u64,
        usage: BufferUsages,
        mapped: bool,
    },
    Texture {
        size: Option<TextureSize>,
        mip_level_count: u32,
        sample_count: u32,
        format: Option<TextureFormat>,
        usage: TextureUsages,
        multisampled: bool,
    },
}

impl ResourceMeta {
    pub fn default_from_handle(handle: ResourceHandle) -> Self {
        match handle {
            ResourceHandle::Buffer(_) => ResourceMeta::Buffer {
                size: 0,
                usage: BufferUsages::empty(),
                mapped: false,
            },
            ResourceHandle::Texture(_) => ResourceMeta::Texture {
                size: None,
                mip_level_count: 1,
                sample_count: 1,
                format: None,
                usage: TextureUsages::empty(),
                multisampled: false,
            },
        }
    }

    pub fn set_buffer_size(&mut self, size: u64) {
        match self {
            ResourceMeta::Buffer { size: buf_size, .. } => *buf_size = (*buf_size).max(size),
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }

    pub fn set_uniform_buffer(&mut self) {
        match self {
            ResourceMeta::Buffer { usage, .. } => *usage |= BufferUsages::UNIFORM,
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }

    pub fn set_storage_buffer(&mut self) {
        match self {
            ResourceMeta::Buffer { usage, .. } => *usage |= BufferUsages::STORAGE,
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }

    pub fn set_format(&mut self, format: TextureFormat) {
        let new_format = format;
        match self {
            ResourceMeta::Texture { format, .. } => {
                if let Some(format) = format {
                    assert_eq!(*format, new_format, "conflicting texture formats detected; texture constrained or bound with formats {format:?} and {new_format:?}");
                } else {
                    *format = Some(new_format);
                }
            }
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_mip_count(&mut self, count: u32) {
        match self {
            ResourceMeta::Texture {
                mip_level_count, ..
            } => *mip_level_count = (*mip_level_count).max(count),
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_sample_count(&mut self, count: u32) {
        match self {
            ResourceMeta::Texture { sample_count, .. } => {
                *sample_count = (*sample_count).max(count)
            }
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_multisampled(&mut self) {
        match self {
            ResourceMeta::Texture { multisampled, .. } => *multisampled = true,
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_texture_binding(&mut self) {
        match self {
            ResourceMeta::Texture { usage, .. } => *usage |= TextureUsages::TEXTURE_BINDING,
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_storage_binding(&mut self) {
        match self {
            ResourceMeta::Texture { usage, .. } => *usage |= TextureUsages::STORAGE_BINDING,
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_render_attachment(&mut self) {
        match self {
            ResourceMeta::Texture { usage, .. } => *usage |= TextureUsages::RENDER_ATTACHMENT,
            _ => panic!("attempted to bind a non-texture resource to a texture slot"),
        }
    }

    pub fn set_copy_src(&mut self) {
        match self {
            ResourceMeta::Buffer { usage, .. } => *usage |= BufferUsages::COPY_SRC,
            ResourceMeta::Texture { usage, .. } => *usage |= TextureUsages::COPY_SRC,
        }
    }

    pub fn set_copy_dst(&mut self) {
        match self {
            ResourceMeta::Buffer { usage, .. } => *usage |= BufferUsages::COPY_DST,
            ResourceMeta::Texture { usage, .. } => *usage |= TextureUsages::COPY_DST,
        }
    }
}
