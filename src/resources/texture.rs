use std::num::NonZeroU32;
use std::ops::{Bound, RangeBounds};

use slotmap::{new_key_type, SecondaryMap};
use thiserror::Error;
use wgpu::{
    Extent3d, Origin3d, TextureDimension, TextureFormat, TextureFormatFeatureFlags, TextureUsages,
};

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
    mip_count: Option<NonZeroU32>,
    base_layer: u32,
    layer_count: Option<NonZeroU32>,
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
        }
        .map(|c| NonZeroU32::new(c).expect("mips slice must be at least 1 element long"));
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
        }
        .map(|c| NonZeroU32::new(c).expect("layer slice must be at least 1 element long"));
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

#[derive(Debug)]
pub(crate) enum TextureSampleTypeConstraint {
    Unconstrained,
    Conflicted(TextureSampleType, TextureSampleType),
    Constrained(TextureSampleType),
}

#[derive(Debug)]
pub(crate) struct TextureConstraints {
    pub size: Option<TextureSize>,
    pub min_size: Extent3d,
    pub format: Option<TextureFormat>,
    pub has_depth: bool,
    pub has_stencil: bool,
    pub min_mip_level_count: u32,
    pub min_sample_count: u32,
    pub min_usages: TextureUsages,
    pub multisampled: bool,
    pub sample_type: TextureSampleTypeConstraint,
}

impl TextureConstraints {
    pub fn verify(&self, name: &str) -> Option<TextureError> {
        if let Some(size) = self.size {
            let (x, y, z) = match size {
                TextureSize::D1 { x } => (x, 1, 1),
                TextureSize::D2 { x, y } => (x, y, 1),
                TextureSize::D3 { x, y, z } => (x, y, z),
                TextureSize::D2Array { x, y, layers } => (x, y, layers),
            };
            if x < self.min_size.width
                || y < self.min_size.height
                || z < self.min_size.depth_or_array_layers
            {
                return Some(TextureError::SizeLessThanMinSize(
                    name.into(),
                    self.min_size,
                    size,
                ));
            }
        } else {
            return Some(TextureError::UnconstrainedTextureSize(name.into()));
        }

        if let Some(format) = self.format {
            let info = format.describe();

            if self.min_usages.contains(TextureUsages::STORAGE_BINDING)
                && !info
                    .guaranteed_format_features
                    .allowed_usages
                    .contains(TextureUsages::STORAGE_BINDING)
            {
                return Some(TextureError::FormatNotStorageCompatible(name.into()));
            }

            if self.min_usages.contains(TextureUsages::RENDER_ATTACHMENT)
                && !info
                    .guaranteed_format_features
                    .allowed_usages
                    .contains(TextureUsages::RENDER_ATTACHMENT)
            {
                return Some(TextureError::FormatNotRenderCompatible(name.into()));
            }

            match self.min_sample_count {
                1 => {}
                2 if info
                    .guaranteed_format_features
                    .flags
                    .contains(TextureFormatFeatureFlags::MULTISAMPLE_X2) => {}
                4 if info
                    .guaranteed_format_features
                    .flags
                    .contains(TextureFormatFeatureFlags::MULTISAMPLE_X4) => {}
                8 if info
                    .guaranteed_format_features
                    .flags
                    .contains(TextureFormatFeatureFlags::MULTISAMPLE_X8) => {}
                _ => return Some(TextureError::FormatNotMultisampleCompatible(name.into())),
            }

            if self.has_depth {
                match format {
                    TextureFormat::Depth16Unorm
                    | TextureFormat::Depth24Plus
                    | TextureFormat::Depth24PlusStencil8
                    | TextureFormat::Depth32Float
                    | TextureFormat::Depth32FloatStencil8 => (),
                    _ => return Some(TextureError::FormatNotDepth(name.into(), format)),
                }
            }

            match self.sample_type {
                TextureSampleTypeConstraint::Unconstrained => (),
                TextureSampleTypeConstraint::Conflicted(left, right) => {
                    return Some(TextureError::ConflictingTextureSampleTypes(
                        name.into(),
                        left,
                        right,
                    ))
                }
                TextureSampleTypeConstraint::Constrained(sample_type) => {
                    match (info.sample_type, sample_type) {
                        (wgpu::TextureSampleType::Depth, TextureSampleType::Depth)
                        | (
                            wgpu::TextureSampleType::Depth,
                            TextureSampleType::Float { filterable: false },
                        )
                        | (
                            wgpu::TextureSampleType::Float { filterable: true },
                            TextureSampleType::Float { .. },
                        )
                        | (
                            wgpu::TextureSampleType::Float { filterable: false },
                            TextureSampleType::Float { filterable: false },
                        )
                        | (wgpu::TextureSampleType::Sint, TextureSampleType::Sint)
                        | (wgpu::TextureSampleType::Uint, TextureSampleType::Uint) => (),
                        _ => {
                            return Some(TextureError::FormatNotSampleTypeCompatible(
                                name.into(),
                                format,
                                sample_type,
                            ))
                        }
                    }
                }
            }

            if self.has_stencil {
                match format {
                    TextureFormat::Depth24PlusStencil8 | TextureFormat::Depth32FloatStencil8 => (),
                    _ => return Some(TextureError::FormatNotStencil(name.into(), format)),
                }
            }
        } else {
            return Some(TextureError::UnconstrainedTextureFormat(name.into()));
        }

        None
    }

    pub fn verify_retained(&self, tex: &Texture, name: &str) -> Option<TextureError> {
        let size = self.size.unwrap();
        let format = self.format.unwrap();

        if tex.size != size {
            return Some(TextureError::SizeMismatch(name.into(), size, tex.size));
        }
        if tex.format != format {
            return Some(TextureError::FormatMismatch(
                name.into(),
                format,
                tex.format,
            ));
        }
        if !tex.usage.contains(self.min_usages) {
            return Some(TextureError::MissingUsages(
                name.into(),
                self.min_usages.difference(tex.usage),
            ));
        }
        if tex.mip_level_count < self.min_mip_level_count {
            return Some(TextureError::InsufficientMipLevels(
                name.into(),
                self.min_mip_level_count,
                tex.mip_level_count,
            ));
        }
        if tex.sample_count < self.min_sample_count {
            return Some(TextureError::InsufficientSamples(
                name.into(),
                self.min_sample_count,
                tex.sample_count,
            ));
        }
        None
    }

    pub fn set_min_size(&mut self, size: Extent3d) {
        self.min_size.width = self.min_size.width.max(size.width);
        self.min_size.height = self.min_size.height.max(size.height);
        self.min_size.depth_or_array_layers = self
            .min_size
            .depth_or_array_layers
            .max(size.depth_or_array_layers);
    }

    pub fn set_format(&mut self, format: TextureFormat) {
        if let Some(old_format) = self.format {
            assert_eq!(old_format, format, "conflicting texture formats detected; texture constrained or bound with formats {old_format:?} and {format:?}");
        } else {
            self.format = Some(format);
        }
    }

    pub fn set_mip_count(&mut self, count: u32) {
        self.min_mip_level_count = self.min_mip_level_count.max(count);
    }

    pub fn set_multisampled(&mut self) {
        self.multisampled = true;
    }

    pub fn set_texture_binding(&mut self) {
        self.min_usages |= TextureUsages::TEXTURE_BINDING;
    }

    pub fn set_storage_binding(&mut self) {
        self.min_usages |= TextureUsages::STORAGE_BINDING;
    }

    pub fn set_render_attachment(&mut self) {
        self.min_usages |= TextureUsages::RENDER_ATTACHMENT;
    }

    pub fn set_copy_src(&mut self) {
        self.min_usages |= TextureUsages::COPY_SRC;
    }

    pub fn set_copy_dst(&mut self) {
        self.min_usages |= TextureUsages::COPY_DST;
    }

    pub fn set_sample_type(&mut self, ty: TextureSampleType) {
        match self.sample_type {
            TextureSampleTypeConstraint::Unconstrained => {
                self.sample_type = TextureSampleTypeConstraint::Constrained(ty)
            }
            TextureSampleTypeConstraint::Conflicted(_, _) => (),
            TextureSampleTypeConstraint::Constrained(old_ty) => match (old_ty, ty) {
                // Upgrade
                (
                    TextureSampleType::Float { filterable: false },
                    TextureSampleType::Float { filterable: true } | TextureSampleType::Depth,
                ) => self.sample_type = TextureSampleTypeConstraint::Constrained(ty),
                // Compatible
                (
                    TextureSampleType::Float { filterable: true },
                    TextureSampleType::Float { .. },
                )
                | (
                    TextureSampleType::Float { filterable: false },
                    TextureSampleType::Float { filterable: false },
                )
                | (TextureSampleType::Depth, TextureSampleType::Depth)
                | (TextureSampleType::Depth, TextureSampleType::Float { filterable: false })
                | (TextureSampleType::Sint, TextureSampleType::Sint)
                | (TextureSampleType::Uint, TextureSampleType::Uint) => (),
                // Incompatible
                _ => self.sample_type = TextureSampleTypeConstraint::Conflicted(old_ty, ty),
            },
        }
    }
}

impl Default for TextureConstraints {
    fn default() -> Self {
        Self {
            size: None,
            min_size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: None,
            has_depth: false,
            has_stencil: false,
            min_mip_level_count: 1,
            min_sample_count: 1,
            min_usages: TextureUsages::empty(),
            multisampled: false,
            sample_type: TextureSampleTypeConstraint::Unconstrained,
        }
    }
}
