use std::borrow::Cow;
use std::collections::BTreeMap;

use slotmap::SecondaryMap;
use wgpu::Buffer;

use crate::bitset::Bitset;

pub(crate) use self::bindgroup::{BindGroupCache, BindGroupHandle, ResourceBinding};
pub(crate) use self::buffer::{BufferBinding, BufferBindings, BufferConstraints, BufferUse};
pub use self::buffer::{BufferError, BufferHandle, BufferSlice};
pub use self::layout::{
    BindGroupLayout, BindGroupLayoutHandle, PipelineLayout, PipelineLayoutHandle,
};
pub use self::module::{module_from_source, ModuleError, ShaderModule, ShaderSource};
pub use self::pipeline::{
    compute_pipeline_from_module, ComputePipeline, ComputePipelineHandle, PipelineError,
    PipelineStorage, ReflectedComputePipeline,
};
// use self::sampler::SamplerTypeConstraint;
// pub use self::sampler::{Sampler, SamplerError, SamplerHandle};
// pub(crate) use self::sampler::{SamplerBinding, SamplerBindings, SamplerConstraints};
pub use self::texture::{Texture, TextureAspect, TextureCopyView, TextureError, TextureSize};
pub(crate) use self::texture::{
    TextureBinding, TextureBindings, TextureConstraints, TextureHandle, TextureSampleType,
    TextureViewDimension,
};

mod bindgroup;
mod buffer;
mod layout;
mod module;
mod pipeline;
// mod sampler;
mod texture;

pub(crate) type Buffers = BTreeMap<Cow<'static, str>, Buffer>;
pub(crate) type Textures = BTreeMap<Cow<'static, str>, Texture>;
// pub(crate) type Samplers = BTreeMap<Cow<'static, str>, Sampler>;
// pub(crate) type SamplersConstraints = FastHashMap<SamplerConstraints, Cow<'static, str>>;

#[derive(Debug)]
pub struct RenderResources {
    pub(crate) buffers: Buffers,
    pub(crate) textures: Textures,
    // pub(crate) samplers: Samplers,
    // pub(crate) samplers_constraints: SamplersConstraints,
}

impl RenderResources {
    pub fn new() -> Self {
        Self {
            buffers: Buffers::new(),
            textures: Textures::new(),
            // samplers: Samplers::new(),
            // samplers_constraints: SamplersConstraints::default(),
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

    pub fn get_texture(&self, name: &str) -> Option<&Texture> {
        self.textures.get(name)
    }

    // pub fn insert_sampler(&mut self, name: impl Into<Cow<'static, str>>, sampler: Sampler) {
    //     let name = name.into();
    //     self.samplers_constraints.insert(
    //         SamplerConstraints {
    //             address_modes: [
    //                 Some(sampler.address_mode_u),
    //                 Some(sampler.address_mode_v),
    //                 Some(sampler.address_mode_w),
    //             ],
    //             mag_filter: Some(sampler.mag_filter),
    //             min_filter: Some(sampler.min_filter),
    //             mipmap_filter: Some(sampler.mipmap_filter),
    //             lod_min_clamp: FixedU32::from_num(sampler.lod_min_clamp),
    //             lod_max_clamp: FixedU32::from_num(sampler.lod_max_clamp),
    //             compare: sampler.compare,
    //             anisotropy_clamp: sampler.anisotropy_clamp,
    //             border_color: sampler.border_color,
    //             ty: SamplerTypeConstraint::Unconstrained,
    //         },
    //         name.clone(),
    //     );
    //     self.samplers.insert(name, sampler);
    // }

    // pub fn get_sampler(&self, name: &str) -> Option<&Sampler> {
    //     self.samplers.get(name)
    // }
}

impl Default for RenderResources {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceHandle {
    Buffer(BufferHandle),
    Texture(TextureHandle),
    // Sampler(SamplerHandle),
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

// impl From<SamplerHandle> for ResourceHandle {
//     fn from(handle: SamplerHandle) -> Self {
//         Self::Sampler(handle)
//     }
// }

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
    // pub samplers: SecondaryMap<SamplerHandle, SamplerConstraints>,
}

impl ResourceConstraints {
    pub fn clear(&mut self) {
        self.buffers.clear();
        self.textures.clear();
    }
}
