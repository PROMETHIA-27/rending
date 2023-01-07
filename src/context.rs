use std::path::Path;

use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Device, Label, Queue, TextureDescriptor, TextureFormat,
    TextureUsages,
};

use crate::spirv_iter::SpirvIterator;
use crate::PipelineError;
use crate::{ReflectedComputePipeline, ShaderSource, Texture, TextureSize};

#[derive(Copy, Clone)]
pub struct RenderContext<'d, 'q> {
    pub device: &'d Device,
    pub queue: &'q Queue,
}

impl<'d, 'q> RenderContext<'d, 'q> {
    pub fn new(device: &'d Device, queue: &'q Queue) -> Self {
        Self { device, queue }
    }

    pub fn buffer<'a>(self) -> BufferBuilder<'d, 'q, 'a> {
        BufferBuilder {
            ctx: self,
            label: None,
            size: None,
            usages: BufferUsages::empty(),
            mapped: false,
        }
    }

    // TODO: Builder pattern textures
    pub fn texture(
        &self,
        label: Label,
        size: TextureSize,
        format: TextureFormat,
        usage: TextureUsages,
        mip_level_count: u32,
        sample_count: u32,
    ) -> Texture {
        let inner = {
            let (dimension, size) = size.into_wgpu();
            self.device.create_texture(&TextureDescriptor {
                label,
                size,
                mip_level_count,
                sample_count,
                dimension,
                format,
                usage,
            })
        };
        Texture {
            inner,
            size,
            format,
            usage,
            mip_level_count,
            sample_count,
        }
    }

    pub fn compute_pipeline<I, P>(
        &self,
        label: Label,
        shader: ShaderSource<I, P>,
        entry_point: &str,
    ) -> Result<ReflectedComputePipeline, PipelineError>
    where
        P: AsRef<Path>,
        I: SpirvIterator,
    {
        let module = crate::resources::module_from_source(self, shader)?;

        let pipeline =
            crate::resources::compute_pipeline_from_module(self, &module, entry_point, label)?;

        Ok(pipeline)
    }
}

pub struct BufferBuilder<'d, 'q, 'a> {
    ctx: RenderContext<'d, 'q>,
    label: Label<'a>,
    size: Option<u64>,
    usages: BufferUsages,
    mapped: bool,
}

impl<'a> BufferBuilder<'_, '_, 'a> {
    pub fn label(mut self, l: Label<'a>) -> Self {
        self.label = l;
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn map_read(mut self) -> Self {
        self.usages |= BufferUsages::MAP_READ;
        self
    }

    pub fn map_write(mut self) -> Self {
        self.usages |= BufferUsages::MAP_WRITE;
        self
    }

    pub fn copy_src(mut self) -> Self {
        self.usages |= BufferUsages::COPY_SRC;
        self
    }

    pub fn copy_dst(mut self) -> Self {
        self.usages |= BufferUsages::COPY_DST;
        self
    }

    pub fn uniform(mut self) -> Self {
        self.usages |= BufferUsages::UNIFORM;
        self
    }

    pub fn storage(mut self) -> Self {
        self.usages |= BufferUsages::STORAGE;
        self
    }

    pub fn mapped(mut self) -> Self {
        self.mapped = true;
        self
    }

    pub fn create(self) -> Buffer {
        self.ctx.device.create_buffer(&BufferDescriptor {
            label: self.label,
            size: self
                .size
                .expect("must specify a size when creating a buffer using `BufferBuilder::size()`"),
            usage: self.usages,
            mapped_at_creation: self.mapped,
        })
    }
}
