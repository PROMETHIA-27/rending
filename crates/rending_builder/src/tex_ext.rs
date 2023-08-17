//! This module contains extensions related to texture binding for bind groups. It mainly includes:
//! - The `as_entire().binding()` method(s), much like the one on buffers, which binds the whole texture,
//! including all mip layers and array layers, with the `All` aspect, and other settings set to `None`.
//! - The `view_builder` API, which is a terser way to build a texture view and only specify what you want to.

use wgpu::{
    BindingResource, Label, Texture, TextureAspect, TextureFormat, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

pub trait TextureBindingExt {
    fn as_entire(&self) -> TextureView;

    fn view_builder(&self) -> ViewBuilder;
}

impl TextureBindingExt for Texture {
    fn as_entire(&self) -> TextureView {
        self.create_view(&TextureViewDescriptor {
            label: None,
            format: None,
            dimension: None,
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        })
    }

    fn view_builder(&self) -> ViewBuilder {
        ViewBuilder {
            texture: self,
            label: None,
            format: None,
            dimension: None,
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        }
    }
}

pub trait TextureViewExt {
    fn binding(&self) -> BindingResource;
}

impl TextureViewExt for TextureView {
    fn binding(&self) -> BindingResource {
        BindingResource::TextureView(self)
    }
}

pub struct ViewBuilder<'t, 'l> {
    texture: &'t Texture,
    label: Label<'l>,
    format: Option<TextureFormat>,
    dimension: Option<TextureViewDimension>,
    aspect: TextureAspect,
    base_mip_level: u32,
    mip_level_count: Option<u32>,
    base_array_layer: u32,
    array_layer_count: Option<u32>,
}

impl<'t, 'l> ViewBuilder<'t, 'l> {
    pub fn label(mut self, label: Label<'l>) -> Self {
        self.label = label;
        self
    }

    pub fn format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn dimension(mut self, dimension: TextureViewDimension) -> Self {
        self.dimension = Some(dimension);
        self
    }

    pub fn aspect(mut self, aspect: TextureAspect) -> Self {
        self.aspect = aspect;
        self
    }

    pub fn mip_levels(mut self, base: u32, count: Option<u32>) -> Self {
        self.base_mip_level = base;
        self.mip_level_count = count;
        self
    }

    pub fn array_layers(mut self, base: u32, count: Option<u32>) -> Self {
        self.base_array_layer = base;
        self.array_layer_count = count;
        self
    }

    pub fn finish(self) -> TextureView {
        let Self {
            texture,
            label,
            format,
            dimension,
            aspect,
            base_mip_level,
            mip_level_count,
            base_array_layer,
            array_layer_count,
        } = self;

        texture.create_view(&TextureViewDescriptor {
            label,
            format,
            dimension,
            aspect,
            base_mip_level,
            mip_level_count,
            base_array_layer,
            array_layer_count,
        })
    }
}
