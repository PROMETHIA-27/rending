use std::borrow::Cow;
use std::collections::BTreeMap;

use slotmap::SecondaryMap;
use wgpu::Buffer;

use crate::bitset::Bitset;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub(crate) use self::buffer::{BufferBinding, BufferBindings, BufferConstraints, BufferUse};
pub use self::buffer::{BufferError, BufferHandle, BufferSlice};
pub use self::layout::BindGroupLayout;
pub use self::layout::{BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle};
pub use self::module::ShaderModule;
pub use self::pipeline::{ComputePipeline, ComputePipelineHandle, PipelineStorage};
pub use self::texture::{Texture, TextureAspect, TextureCopyView, TextureError, TextureSize};
pub(crate) use self::texture::{
    TextureBinding, TextureBindings, TextureConstraints, TextureHandle, TextureViewDimension,
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

#[derive(Debug, Default)]
pub(crate) struct ResourceConstraints {
    pub buffers: SecondaryMap<BufferHandle, BufferConstraints>,
    pub textures: SecondaryMap<TextureHandle, TextureConstraints>,
}
