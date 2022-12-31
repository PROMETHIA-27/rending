use std::num::NonZeroU32;
use std::ops::{Bound, RangeBounds};

use slotmap::{new_key_type, SecondaryMap};
use wgpu::{Extent3d, Texture, TextureDimension};

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
}

#[derive(Debug, Copy, Clone)]
pub struct TextureView {
    handle: TextureHandle,
    // dimensions: Option<TextureViewDimension>,
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

    // pub fn d2(&mut self) -> &mut Self {
    //     self.dimensions = Some(TextureViewDimension::D2);
    //     self
    // }

    // pub fn d2_array(&mut self) -> &mut Self {
    //     self.dimensions = Some(TextureViewDimension::D2Array);
    //     self
    // }

    // pub fn cube(&mut self) -> &mut Self {
    //     self.dimensions = Some(TextureViewDimension::Cube);
    //     self
    // }

    // pub fn cube_array(&mut self) -> &mut Self {
    //     self.dimensions = Some(TextureViewDimension::CubeArray);
    //     self
    // }

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
