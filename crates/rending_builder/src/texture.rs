use wgpu::{
    Device, Extent3d, Label, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};

pub trait TextureExt {
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
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn size(mut self, size: Extent3d) -> Self {
        self.size = Some(size);
        self
    }

    pub fn dimension(mut self, dimension: TextureDimension) -> Self {
        self.dimension = Some(dimension);
        self
    }

    pub fn format(mut self, format: TextureFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn texture_binding(mut self) -> Self {
        self.usage |= TextureUsages::TEXTURE_BINDING;
        self
    }

    pub fn storage_binding(mut self) -> Self {
        self.usage |= TextureUsages::STORAGE_BINDING;
        self
    }

    pub fn render_attachment(mut self) -> Self {
        self.usage |= TextureUsages::RENDER_ATTACHMENT;
        self
    }

    pub fn copy_src(mut self) -> Self {
        self.usage |= TextureUsages::COPY_SRC;
        self
    }

    pub fn copy_dst(mut self) -> Self {
        self.usage |= TextureUsages::COPY_DST;
        self
    }

    pub fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    pub fn mip_level_count(mut self, count: u32) -> Self {
        self.mip_level_count = count;
        self
    }

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
