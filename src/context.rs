use std::path::Path;

use wgpu::{Device, Label, Queue, TextureDescriptor, TextureFormat, TextureUsages};

use crate::spirv_iter::SpirvIterator;
use crate::{reflect, PipelineError};
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
        let module = reflect::module_from_source(self, shader)?;

        let pipeline = reflect::compute_pipeline_from_module(self, &module, entry_point, label)?;

        Ok(pipeline)
    }
}
