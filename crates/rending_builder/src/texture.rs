//! This module includes the [`TextureExt`] extension which allows creation of textures with a [`Device`]
//! via the builder pattern.

use wgpu::{
    Device, Extent3d, Label, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};

/// The extension trait which gives [`Device`] a method to create textures using the builder pattern.
pub trait TextureExt {
    /// Create a texture using the builder pattern.
    ///
    /// 1. First, call this.
    /// 2. Then, call any of the following methods in a chain, like so:
    /// ```rust
    /// device
    /// .texture()
    /// // The following are mandatory
    /// .size(Extent3d { ... })
    /// .dimension(TextureDimension::...)
    /// .format(TextureFormat::...)
    /// // The following are optional
    /// .label("foo")
    /// .texture_binding()
    /// .storage_binding()
    /// .render_attachment()
    /// .copy_src()
    /// .copy_dst()
    /// .sample_count(n)
    /// .mip_level_count(n)
    /// .view_formats(&[TextureFormat::..., ...])
    /// ```
    /// 3. Finally, call `.finish()` at the end of the chain. This will produce the texture.
    ///
    /// See each method for more details on what settings they affect.
    fn texture(&self) -> TextureBuilder;
}

impl TextureExt for Device {
    fn texture(&self) -> TextureBuilder {
        TextureBuilder {
            device: self,
            label: None,
            size: None,
            dimension: None,
            format: None,
            usage: TextureUsages::empty(),
            sample_count: 1,
            mip_level_count: 1,
            view_formats: &[],
        }
    }
}

/// A builder that creates textures.
///
/// See its methods or [`TextureExt::texture()`](TextureExt::texture()) for more information.
///
/// (`TextureExt` is implemented for [`wgpu::Device`])
pub struct TextureBuilder<'d, 'l, 'v> {
    device: &'d Device,
    label: Label<'l>,
    size: Option<Extent3d>,
    dimension: Option<TextureDimension>,
    format: Option<TextureFormat>,
    usage: TextureUsages,
    sample_count: u32,
    mip_level_count: u32,
    view_formats: &'v [TextureFormat],
}

impl<'d, 'l, 'v> TextureBuilder<'d, 'l, 'v> {
    /// Add a label to the texture, which can be visible in debugging tools.
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the texture's size. This method is mandatory to construct a texture.
    ///
    /// For 1D or 2D textures, the [`depth_or_array_layers`](Extent3d::depth_or_array_layers) field
    /// will be 1. For array textures or 3D textures, it will probably not be 1.
    pub fn size(mut self, size: Extent3d) -> Self {
        self.size = Some(size);
        self
    }

    /// Set the texture's dimension. This method is mandatory to construct a texture.
    ///
    /// The options should be self explanatory; the texture is either 1 dimensional (a line),
    /// 2 dimensional (a rectangle, most often a square),
    /// or 3 dimensional (a rectangular prism, most often a cube). These are denoted D1, D2, and D3
    /// respectively.
    pub fn dimension(mut self, dimension: TextureDimension) -> Self {
        self.dimension = Some(dimension);
        self
    }

    /// Set the texture's format. This method is mandatory to construct a texture.
    ///
    /// There are many valid settings for the format of a texture and several invalid settings.
    ///
    /// For a basic, colored texture, Rgba8Unorm is a reasonable choice.
    ///
    /// Depth and stencil textures have their own formats.
    ///
    /// There are also compressed formats, which are too complex to summarize here. It is recommended
    /// to read the specs ([wgpu](https://www.w3.org/TR/webgpu/), [wgsl](https://www.w3.org/TR/WGSL/))
    /// to understand more.
    pub fn format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Allows the texture to be used as a normal bound texture in a shader. This is a very common
    /// option, and allows reading and sampling from the texture. If you want to write to a texture,
    /// use [`storage_binding()`] instead.
    pub fn texture_binding(mut self) -> Self {
        self.usage |= TextureUsages::TEXTURE_BINDING;
        self
    }

    /// Allows the texture to be used as a storage texture in a shader. This is less common, but
    /// very useful. It allows read *and* write access to the texture. If you only want to read from
    /// the texture, use [`texture_binding()`] instead.
    pub fn storage_binding(mut self) -> Self {
        self.usage |= TextureUsages::STORAGE_BINDING;
        self
    }

    /// Allows the texture to be used as a render attachment for a render pass. This is what a render
    /// pass actually draws to which can then be displayed to the screen. In normal circumstances you
    /// will only have one of these for a program, but you can also make more.
    pub fn render_attachment(mut self) -> Self {
        self.usage |= TextureUsages::RENDER_ATTACHMENT;
        self
    }

    /// Allows the texture to be copied from. This allows copying a slice of this texture into a
    /// buffer or another texture.
    pub fn copy_src(mut self) -> Self {
        self.usage |= TextureUsages::COPY_SRC;
        self
    }

    /// Allows the texture to be copied into. Most textures will need this set in order to have
    /// their data copied from disk into them.
    pub fn copy_dst(mut self) -> Self {
        self.usage |= TextureUsages::COPY_DST;
        self
    }

    /// Change the number of samples of this texture. If this is not 1, requires multisampling
    /// to be enabled when binding a view of the texture.
    ///
    /// Multisampling is a technique primarily used for Multisample Antialiasing, or MSAA, which
    /// is an optimization on superscale antialiasing, where a higher resolution image is downscaled
    /// to reduce aliasing. Aliasing is the jagged edges that are often produced by simple rendering
    /// processes, such as along the edges of geometry.
    pub fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    /// Change the mip level count of this texture. For a non-mipmapped texture, this will be 1.
    ///
    /// The mip level is the number of mip maps of the texture. A mip map is a smaller version of the
    /// texture, generally by a factor of 2. By expanding the memory use of a texture by about 1/3,
    /// a texture can have a 1/2, 1/4, 1/8, etc. series of copies of itself. Then, when the texture
    /// is far away from the camera, the smaller version can be sampled to avoid graphical artifacts
    /// from the high level of noise of a distant, detailed texture.
    pub fn mip_level_count(mut self, count: u32) -> Self {
        self.mip_level_count = count;
        self
    }

    /// Set the view formats of this image. This allows the use of alternative formats when binding
    /// a texture than it was actually declared as. Currently, this only allows changing the "sRGB-ness"
    /// of an image, such as binding an Rgba8Unorm image as an Rgba8UnormSrgb view.
    pub fn view_formats(mut self, formats: &'v [TextureFormat]) -> Self {
        self.view_formats = formats;
        self
    }

    /// Finish the builder and create the defined texture.
    pub fn finish(self) -> Texture {
        let Self {
            device,
            label,
            size,
            dimension,
            format,
            usage,
            sample_count,
            mip_level_count,
            view_formats,
        } = self;

        device.create_texture(&TextureDescriptor {
            label,
            size: size.expect("must provide size to all textures"),
            dimension: dimension.expect("must provide dimension to all textures"),
            format: format.expect("must provide format to all textures"),
            usage,
            sample_count,
            mip_level_count,
            view_formats,
        })
    }
}
