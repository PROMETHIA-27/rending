use std::borrow::Cow;
use std::path::Path;
use std::str::Utf8Error;

use naga::front::spv::Options as SpvOptions;
use naga::valid::{Capabilities, ValidationFlags};
use thiserror::Error;
use wgpu::ShaderModuleDescriptor;
use wgpu_core::pipeline::CreateShaderModuleError;

use crate::spirv_iter::SpirvIterator;
use crate::RenderContext;

#[derive(Debug)]
pub struct ShaderModule {
    pub(crate) wgpu: wgpu::ShaderModule,
    pub(crate) module: naga::Module,
    pub(crate) info: naga::valid::ModuleInfo,
}

// TODO: Wrap this in a struct so that variants aren't visible
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
pub enum ModuleError {
    #[error(transparent)]
    SpvParsing(#[from] naga::front::spv::Error),
    #[error(transparent)]
    Naga(#[from] CreateShaderModuleError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
}

impl std::fmt::Debug for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleError::SpvParsing(arg0) => f.debug_tuple("ModuleError").field(arg0).finish(),
            ModuleError::Io(arg0) => f.debug_tuple("ModuleError").field(arg0).finish(),
            ModuleError::Utf8(arg0) => f.debug_tuple("ModuleError").field(arg0).finish(),
            ModuleError::Naga(CreateShaderModuleError::Validation(err)) => {
                write!(f, "\n{}", err.inner.emit_to_string(&err.source))
            }
            ModuleError::Naga(err) => std::fmt::Display::fmt(err, f),
        }
    }
}

pub fn module_from_source<I: SpirvIterator, P: AsRef<Path>>(
    ctx: &RenderContext,
    source: ShaderSource<I, P>,
) -> Result<ShaderModule, ModuleError> {
    let (module, info) = match source {
        ShaderSource::Spirv(spirv) => {
            let module = naga::front::spv::Parser::new(spirv.into_spirv(), &SpvOptions::default())
                .parse()?;
            let info = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
                .validate(&module)
                .map_err(|err| {
                    CreateShaderModuleError::from(wgpu_core::pipeline::ShaderError {
                        source: String::new(),
                        label: None,
                        inner: Box::new(err),
                    })
                })?;
            (module, info)
        }
        ShaderSource::FilePath(path) => {
            let bytes = std::fs::read(path)?;
            let module = naga::front::spv::Parser::new(bytes.into_spirv(), &SpvOptions::default())
                .parse()?;
            let info = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
                .validate(&module)
                .map_err(|err| {
                    CreateShaderModuleError::from(wgpu_core::pipeline::ShaderError {
                        source: String::new(),
                        label: None,
                        inner: Box::new(err),
                    })
                })?;
            (module, info)
        }
        ShaderSource::WgslFilePath(path) => {
            let bytes = std::fs::read(path)?;
            let source = std::str::from_utf8(&bytes[..])?;
            let module = naga::front::wgsl::parse_str(source).map_err(|err| {
                CreateShaderModuleError::from(wgpu_core::pipeline::ShaderError {
                    source: source.to_string(),
                    label: None,
                    inner: Box::new(err),
                })
            })?;
            let info = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
                .validate(&module)
                .map_err(|err| {
                    CreateShaderModuleError::from(wgpu_core::pipeline::ShaderError {
                        source: source.to_string(),
                        label: None,
                        inner: Box::new(err),
                    })
                })?;
            (module, info)
        }
    };

    let wgpu = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(Cow::Owned(module.clone())),
    });

    Ok(ShaderModule { wgpu, module, info })
}
