//! This module contains extensions related to texture binding for bind groups. It mainly includes:
//! - The `as_entire().binding()` method(s), much like the one on buffers, which binds the whole texture,
//! including all mip layers and array layers, with the `All` aspect, and other settings set to `None`.
//! - The `view_builder` API, which is a terser way to build a texture view and only specify what you want to.

use wgpu::{
    BindingResource, Label, Texture, TextureAspect, TextureFormat, TextureView,
    TextureViewDescriptor, TextureViewDimension,
};

/// The extension trait giving [`Texture`] more convenient APIs for creating a [`TextureView`].
pub trait TextureBindingExt {
    /// Bind the entire texture as a texture view. This does not label the view, uses
    /// the same format as the texture, uses the same dimension as the texture, uses all aspects,
    /// and uses all mip layers and array layers. If you need a different binding, you can use
    /// [`view_builder()`] to be more specific, without needing to specify everything verbosely.
    fn as_entire(&self) -> TextureView;

    /// Create a view builder which can conveniently create a texture view without having to specify
    /// all parts of the view. By default, it will produce the same binding as [`as_entire()`], but
    /// it can be modified either partially or completely.
    ///
    /// The builder's methods are as follows:
    /// ```rust
    /// label("foo")
    /// format(TextureFormat::...)
    /// dimension(TextureViewDimension::...)
    /// aspect(TextureAspect::...)
    /// mip_levels(base, count)
    /// array_layers(base, count)
    /// finish()
    /// ```
    ///
    /// See them individually for more information.
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

/// The extension trait allowing a quick way to wrap a [`TextureView`] in a [`BindingResource`].
pub trait TextureViewExt {
    /// Wrap this [`TextureView`] in [`BindingResource::TextureView`].
    fn binding(&self) -> BindingResource;
}

impl TextureViewExt for TextureView {
    fn binding(&self) -> BindingResource {
        BindingResource::TextureView(self)
    }
}

/// A builder that can construct a [`TextureView`].
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
    /// Label the texture view, which can be seen in a debugger.
    pub fn label(mut self, label: Label<'l>) -> Self {
        self.label = label;
        self
    }

    /// Override the format of the [`TextureView`]. Must be one of the options added to
    /// the [`Texture`]'s `view_formats` setting.
    pub fn format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Override the dimension of the [`TextureView`]. For 1D and 3D textures, there is no reason to
    /// use this. For 2D textures, this can be used to read the texture as a single texture, array,
    /// cubemap, or cubemap array.
    pub fn dimension(mut self, dimension: TextureViewDimension) -> Self {
        self.dimension = Some(dimension);
        self
    }

    /// Override the aspect of the view. This allows reading just the depth or stencil of a texture,
    /// for instance.
    pub fn aspect(mut self, aspect: TextureAspect) -> Self {
        self.aspect = aspect;
        self
    }

    /// Select a range of mip levels to view. `base` must be >= 1, and if `count` is none then it will
    /// select all remaining mip maps.
    pub fn mip_levels(mut self, base: u32, count: Option<u32>) -> Self {
        self.base_mip_level = base;
        self.mip_level_count = count;
        self
    }

    /// Select a range of array layers to view. `base` must be >= 1, and if `count` is none then it
    /// will select all remaining array layers.
    pub fn array_layers(mut self, base: u32, count: Option<u32>) -> Self {
        self.base_array_layer = base;
        self.array_layer_count = count;
        self
    }

    /// Finish and create the texture view.
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
