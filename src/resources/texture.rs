use slotmap::{new_key_type, SecondaryMap};
use wgpu::{Extent3d, Texture, TextureDimension};

new_key_type! { pub struct TextureHandle; }

#[derive(Debug)]
pub(crate) struct VirtualTexture;

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

#[derive(Copy, Clone, Debug)]
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
