use std::ops::{Bound, RangeBounds};

use slotmap::{new_key_type, SecondaryMap};
use thiserror::Error;
use wgpu::{Extent3d, Origin3d, TextureDimension, TextureFormat, TextureUsages};

use super::ResourceBinding;

new_key_type! { pub struct TextureHandle; }

impl TextureHandle {
    pub fn view(self) -> TextureView {
        TextureView {
            handle: self,
            // dimensions: None,
            aspect: TextureAspect::All,
            base_mip: 0,
            mip_count: None,
            base_layer: 0,
            layer_count: None,
        }
    }

    pub fn copy_view(self, mip_level: u32, origin: Origin3d) -> TextureCopyView {
        TextureCopyView {
            handle: self,
            mip_level,
            origin,
            aspect: TextureAspect::All,
        }
    }
}

#[derive(Debug)]
pub struct Texture {
    pub inner: wgpu::Texture,
    pub size: TextureSize,
    pub format: TextureFormat,
    pub usage: TextureUsages,
    pub mip_level_count: u32,
    pub sample_count: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct TextureView {
    handle: TextureHandle,
    aspect: TextureAspect,
    base_mip: u32,
    mip_count: Option<u32>,
    base_layer: u32,
    layer_count: Option<u32>,
}

impl TextureView {
    pub fn create(&self) -> ResourceBinding {
        let Self {
            handle,
            aspect,
            base_mip,
            mip_count,
            base_layer,
            layer_count,
        } = *self;
        ResourceBinding::Texture {
            handle,
            dimension: None,
            aspect,
            base_mip,
            mip_count,
            base_layer,
            layer_count,
        }
    }

    pub fn depth_only(&mut self) -> &mut Self {
        self.aspect = TextureAspect::DepthOnly;
        self
    }

    pub fn stencil_only(&mut self) -> &mut Self {
        self.aspect = TextureAspect::StencilOnly;
        self
    }

    pub fn slice_mips(&mut self, range: impl RangeBounds<u32>) -> &mut Self {
        let base = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let count = match range.end_bound() {
            Bound::Included(&end) => Some(end - base + 1),
            Bound::Excluded(&end) => Some(end - base),
            Bound::Unbounded => None,
        };
        self.base_mip = base;
        self.mip_count = count;
        self
    }

    pub fn slice_layers(&mut self, range: impl RangeBounds<u32>) -> &mut Self {
        let base = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let count = match range.end_bound() {
            Bound::Included(&end) => Some(end - base + 1),
            Bound::Excluded(&end) => Some(end - base),
            Bound::Unbounded => None,
        };
        self.base_layer = base;
        self.layer_count = count;
        self
    }
}

#[derive(Debug)]
pub struct TextureCopyView {
    pub(crate) handle: TextureHandle,
    pub(crate) mip_level: u32,
    pub(crate) origin: Origin3d,
    pub(crate) aspect: TextureAspect,
}

impl TextureCopyView {
    pub fn stencil_only(self) -> Self {
        Self {
            handle: self.handle,
            mip_level: self.mip_level,
            origin: self.origin,
            aspect: TextureAspect::StencilOnly,
        }
    }

    pub fn depth_only(self) -> Self {
        Self {
            handle: self.handle,
            mip_level: self.mip_level,
            origin: self.origin,
            aspect: TextureAspect::DepthOnly,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureViewDimension {
    D1,
    D2,
    D2Array,
    Cube,
    CubeArray,
    D3,
}

impl TextureViewDimension {
    pub fn from_wgpu(wgpu: wgpu::TextureViewDimension) -> Self {
        match wgpu {
            wgpu::TextureViewDimension::D1 => TextureViewDimension::D1,
            wgpu::TextureViewDimension::D2 => TextureViewDimension::D2,
            wgpu::TextureViewDimension::D2Array => TextureViewDimension::D2Array,
            wgpu::TextureViewDimension::Cube => TextureViewDimension::Cube,
            wgpu::TextureViewDimension::CubeArray => TextureViewDimension::CubeArray,
            wgpu::TextureViewDimension::D3 => TextureViewDimension::D3,
        }
    }

    pub fn into_wgpu(self) -> wgpu::TextureViewDimension {
        match self {
            TextureViewDimension::D1 => wgpu::TextureViewDimension::D1,
            TextureViewDimension::D2 => wgpu::TextureViewDimension::D2,
            TextureViewDimension::D2Array => wgpu::TextureViewDimension::D2Array,
            TextureViewDimension::Cube => wgpu::TextureViewDimension::Cube,
            TextureViewDimension::CubeArray => wgpu::TextureViewDimension::CubeArray,
            TextureViewDimension::D3 => wgpu::TextureViewDimension::D3,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TextureSize {
    D1 { x: u32 },
    D2 { x: u32, y: u32 },
    D3 { x: u32, y: u32, z: u32 },
    D2Array { x: u32, y: u32, layers: u32 },
}

impl TextureSize {
    pub fn into_wgpu(self) -> (TextureDimension, Extent3d) {
        match self {
            TextureSize::D1 { x } => (
                TextureDimension::D1,
                Extent3d {
                    width: x,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            ),
            TextureSize::D2 { x, y } => (
                TextureDimension::D2,
                Extent3d {
                    width: x,
                    height: y,
                    depth_or_array_layers: 1,
                },
            ),
            TextureSize::D3 { x, y, z } => (
                TextureDimension::D3,
                Extent3d {
                    width: x,
                    height: y,
                    depth_or_array_layers: z,
                },
            ),
            TextureSize::D2Array { x, y, layers } => (
                TextureDimension::D2,
                Extent3d {
                    width: x,
                    height: y,
                    depth_or_array_layers: layers,
                },
            ),
        }
    }
}

pub(crate) enum TextureBinding<'t> {
    Retained(&'t Texture),
    Transient(Texture),
}

impl<'t> AsRef<Texture> for TextureBinding<'t> {
    fn as_ref(&self) -> &Texture {
        match self {
            TextureBinding::Retained(texture) => texture,
            TextureBinding::Transient(texture) => texture,
        }
    }
}

pub(crate) type TextureBindings<'t> = SecondaryMap<TextureHandle, TextureBinding<'t>>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureAspect {
    All,
    StencilOnly,
    DepthOnly,
}

impl TextureAspect {
    pub fn into_wgpu(self) -> wgpu::TextureAspect {
        match self {
            TextureAspect::All => wgpu::TextureAspect::All,
            TextureAspect::StencilOnly => wgpu::TextureAspect::StencilOnly,
            TextureAspect::DepthOnly => wgpu::TextureAspect::DepthOnly,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureSampleType {
    Float { filterable: bool },
    Depth,
    Uint,
    Sint,
}

impl TextureSampleType {
    pub fn from_wgpu(wgpu: wgpu::TextureSampleType) -> Self {
        match wgpu {
            wgpu::TextureSampleType::Float { filterable } => {
                TextureSampleType::Float { filterable }
            }
            wgpu::TextureSampleType::Depth => TextureSampleType::Depth,
            wgpu::TextureSampleType::Uint => TextureSampleType::Uint,
            wgpu::TextureSampleType::Sint => TextureSampleType::Sint,
        }
    }

    pub fn to_wgpu(self) -> wgpu::TextureSampleType {
        match self {
            TextureSampleType::Float { filterable } => {
                wgpu::TextureSampleType::Float { filterable }
            }
            TextureSampleType::Depth => wgpu::TextureSampleType::Depth,
            TextureSampleType::Uint => wgpu::TextureSampleType::Uint,
            TextureSampleType::Sint => wgpu::TextureSampleType::Sint,
        }
    }
}

#[derive(Debug, Error)]
pub enum TextureError {
    // Transient
    #[error(
        "transient texture `{0}` never has a size specified; 
    try using `RenderCommands::texture_constraints()` and `TextureConstraints::has_size()`"
    )]
    UnconstrainedTextureSize(String),
    #[error(
        "transient texture `{0}` is constrained to a size of {2:?} 
        but a minimum size of {1:?}; try constraining to a larger size with `TextureConstraints::has_size()`
        or find and reduce the excessive usage"
    )]
    SizeLessThanMinSize(String, Extent3d, TextureSize),
    #[error(
        "transient texture `{0}` never has a format specified; 
    try using `RenderCommands::texture_constraints()` and `TextureConstraints::has_format()`"
    )]
    UnconstrainedTextureFormat(String),
    #[error(
        "transient texture `{0}`'s format does not allow being used as a storage texture,
    but the texture is used as one"
    )]
    FormatNotStorageCompatible(String),
    #[error(
        "transient texture `{0}`'s format does not allow being used as a render attachment,
    but the texture is used as one"
    )]
    FormatNotRenderCompatible(String),
    #[error(
        "transient texture `{0}`'s format does not allow being multisampled,
    but the texture is being multisampled"
    )]
    FormatNotMultisampleCompatible(String),
    #[error("transient texture `{0}` has format `{1:?}` that does not allow sample type `{2:?}`")]
    FormatNotSampleTypeCompatible(String, TextureFormat, TextureSampleType),
    #[error(
        "transient texture `{0}` was used with conflicting texture sample types {1:?} and {2:?}"
    )]
    ConflictingTextureSampleTypes(String, TextureSampleType, TextureSampleType),
    #[error("transient texture `{0}` was used with a depth aspect but its format {1:?} has no depth aspect")]
    FormatNotDepth(String, TextureFormat),
    #[error("transient texture `{0}` was used with a stencil aspect but its format {1:?} has no stencil aspect")]
    FormatNotStencil(String, TextureFormat),
    #[error("transient texture `{0}` is used multisampled, but has fewer than 2 samples")]
    TooFewSamples(String),
    // Retained
    #[error("retained texture `{0}` is constrained to a size of {1:?} but was provided with a size of {2:?}")]
    SizeMismatch(String, TextureSize, TextureSize),
    #[error("retained texture `{0}` is constrained to a format of {1:?} but was provided with a format of {2:?}")]
    FormatMismatch(String, TextureFormat, TextureFormat),
    #[error(
        "retained texture `{0}` is used with usages {1:?} but was not created with those flags"
    )]
    MissingUsages(String, TextureUsages),
    #[error("retained texture `{0}` is used with {1} mip levels but was created with {2}")]
    InsufficientMipLevels(String, u32, u32),
    #[error("retained texture `{0}` is used with {1} samples but was created with {2}")]
    InsufficientSamples(String, u32, u32),
}
