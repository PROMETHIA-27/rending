use std::error::Error;
use std::path::Path;

use reflect::ReflectedComputePipeline;
use spirv_iter::SpirvIterator;
use thiserror::Error;
use wgpu::{Device, Label, Queue, TextureDescriptor, TextureFormat, TextureUsages};

pub use prelude::*;
use wgpu_core::pipeline::CreateShaderModuleError;

mod bitset;
mod commands;
mod graph;
mod named_slotmap;
mod node;
mod prelude;
mod reflect;
mod resources;
mod spirv_iter;
mod util;

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

#[non_exhaustive]
pub enum ShaderSource<I: SpirvIterator, P: AsRef<Path>> {
    Spirv(I),
    FilePath(P),
    WgslFilePath(P),
}

impl ShaderSource<&'static [u32], &'static str> {
    pub fn spirv<I: SpirvIterator>(iter: I) -> ShaderSource<I, &'static str> {
        ShaderSource::Spirv(iter)
    }

    pub fn spirv_file_path<P: AsRef<Path>>(path: P) -> ShaderSource<&'static [u32], P> {
        ShaderSource::FilePath(path)
    }

    pub fn wgsl_file_path<P: AsRef<Path>>(path: P) -> ShaderSource<&'static [u32], P> {
        ShaderSource::WgslFilePath(path)
    }
}

#[derive(Error)]
pub enum PipelineError {
    #[error(transparent)]
    ModuleError(#[from] ModuleError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    PipelineError(#[from] reflect::PipelineError),
}

impl std::fmt::Debug for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error = match self {
            Self::IoError(arg0) => return f.debug_tuple("IoError").field(arg0).finish(),
            Self::PipelineError(arg0) => {
                return f.debug_tuple("PipelineError").field(arg0).finish()
            }
            Self::ModuleError(err) => err,
        };

        let error = match error {
            ModuleError::SpvParsing(arg0) => {
                return f.debug_tuple("ModuleError").field(arg0).finish()
            }
            ModuleError::Io(arg0) => return f.debug_tuple("ModuleError").field(arg0).finish(),
            ModuleError::Utf8(arg0) => return f.debug_tuple("ModuleError").field(arg0).finish(),
            ModuleError::Naga(err) => err,
        };

        use codespan_reporting::diagnostic::Diagnostic;
        use codespan_reporting::files::SimpleFile;
        use codespan_reporting::term;

        let error = match error {
            CreateShaderModuleError::Validation(err) => err,
            err => return f.debug_tuple("ModuleError").field(err).finish(),
        };

        let files = SimpleFile::new("wgpu", &error.source);
        let config = term::Config::default();
        let mut writer = term::termcolor::Ansi::new(vec![]);
        let diagnostic = Diagnostic::error()
            .with_message(error.inner.to_string())
            .with_labels(
                error
                    .inner
                    .spans()
                    .map(|&(span, ref desc)| {
                        codespan_reporting::diagnostic::Label::primary((), span.to_range().unwrap())
                            .with_message(desc.to_owned())
                    })
                    .collect(),
            )
            .with_notes({
                let mut notes = Vec::new();
                let mut source: &dyn Error = error.inner.as_inner();
                while let Some(next) = Error::source(source) {
                    notes.push(next.to_string());
                    source = next;
                }
                notes
            });

        term::emit(&mut writer, &config, &files, &diagnostic).expect("could not write error");

        f.write_str(&String::from_utf8_lossy(&writer.into_inner()))
    }
}
