use std::borrow::Cow;
use std::num::NonZeroU64;
use std::path::Path;
use std::string::FromUtf8Error;

use naga::front::spv::Options as SpvOptions;
use naga::valid::{Capabilities, GlobalUse, ValidationError, ValidationFlags};
use naga::{
    AddressSpace, FastHashSet, GlobalVariable, Handle, ImageClass, ImageDimension, ShaderStage,
    StorageAccess, StorageFormat, TypeInner, WithSpan,
};
use thiserror::Error;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
    BufferBindingType, ComputePipelineDescriptor, Label, PipelineLayoutDescriptor,
    ShaderModuleDescriptor, ShaderStages, StorageTextureAccess, TextureFormat,
};

use crate::resources::ShaderModule;
use crate::spirv_iter::SpirvIterator;
use crate::{RenderContext, ShaderSource};

#[derive(Debug, Error)]
pub enum ModuleError {
    #[error(transparent)]
    SpvParsing(#[from] naga::front::spv::Error),
    #[error(transparent)]
    WgslParsing(#[from] naga::front::wgsl::ParseError),
    #[error(transparent)]
    Validation(#[from] WithSpan<ValidationError>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Utf8(#[from] FromUtf8Error),
}

pub fn module_from_source<I: SpirvIterator, P: AsRef<Path>>(
    ctx: &RenderContext,
    source: ShaderSource<I, P>,
) -> Result<ShaderModule, ModuleError> {
    let module = match source {
        ShaderSource::Spirv(spirv) => {
            naga::front::spv::Parser::new(spirv.into_spirv(), &SpvOptions::default()).parse()?
        }
        ShaderSource::FilePath(path) => {
            let bytes = std::fs::read(path)?;
            naga::front::spv::Parser::new(bytes.into_spirv(), &SpvOptions::default()).parse()?
        }
        ShaderSource::WgslFilePath(path) => {
            let bytes = std::fs::read(path)?;
            naga::front::wgsl::parse_str(&String::from_utf8(bytes)?[..])?
        }
    };

    let info = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)?;

    let wgpu = ctx.device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(Cow::Owned(module.clone())),
    });

    Ok(ShaderModule { wgpu, module, info })
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("entry point `{0}` is missing from module")]
    MissingEntryPoint(String),
    #[error("entry point `{0}` is not a compute shader")]
    NotComputeShader(String),
    #[error("bind group {0} is greater than the maximum amount of bind groups")]
    BindGroupTooHigh(u32),
}

#[derive(Debug)]
pub struct ReflectedComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub layout: wgpu::PipelineLayout,
    pub group_layouts: Vec<(wgpu::BindGroupLayout, Vec<BindGroupLayoutEntry>)>,
}

// TODO: Investigate a way to explicitly reuse superset pipelinelayouts
pub fn compute_pipeline_from_module(
    ctx: &RenderContext,
    module: &ShaderModule,
    entry_point: &str,
    label: Label,
) -> Result<ReflectedComputePipeline, PipelineError> {
    let (point_index, point) = module
        .module
        .entry_points
        .iter()
        .enumerate()
        .find(|point| point.1.name == entry_point)
        .ok_or_else(|| PipelineError::MissingEntryPoint(entry_point.to_string()))?;

    if point.stage != ShaderStage::Compute {
        return Err(PipelineError::NotComputeShader(entry_point.to_string()));
    };

    let point_info = module.info.get_entry_point(point_index);

    let globals: FastHashSet<_> = module
        .module
        .global_variables
        .iter()
        .filter_map(|(handle, _)| (!point_info[handle].is_empty()).then_some(handle))
        .collect();

    let resources = globals.iter().filter_map(|handle| {
        let global = module.module.global_variables.try_get(*handle).unwrap();
        (global.binding.is_some()).then_some(global)
    });

    let mut groups: [Vec<BindGroupLayoutEntry>; wgpu_core::MAX_BIND_GROUPS] =
        std::array::from_fn(|_| vec![]);

    for resource in resources {
        let binding = resource.binding.as_ref().unwrap();

        if binding.group as usize >= wgpu_core::MAX_BIND_GROUPS {
            return Err(PipelineError::BindGroupTooHigh(binding.group));
        }

        let ty = module.module.types.get_handle(resource.ty).unwrap();
        let size = ty.inner.size(&module.module.constants);

        let binding_ty = match resource.space {
            AddressSpace::Uniform => BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(
                    NonZeroU64::new(size as u64).expect("buffers should be non-zero sized types"),
                ),
            },
            AddressSpace::Storage { access } => BindingType::Buffer {
                ty: BufferBindingType::Storage {
                    read_only: !access.contains(StorageAccess::LOAD),
                },
                has_dynamic_offset: false,
                min_binding_size: Some(
                    NonZeroU64::new(size as u64).expect("buffers should be non-zero sized types"),
                ),
            },
            AddressSpace::Handle => match ty.inner {
                TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                } => match_image(dim, arrayed, class),
                TypeInner::Sampler { comparison } => BindingType::Sampler(match comparison {
                    true => wgpu::SamplerBindingType::Comparison,
                    false => wgpu::SamplerBindingType::Filtering, // TODO: Add options to select NonFiltering instead if desired
                }),
                _ => unreachable!("a handle should be an image or sampler"),
            },
            AddressSpace::PushConstant => todo!(),
            _ => unreachable!("resources should not be private, function, or workgroup variables"),
        };

        groups[binding.group as usize].push(BindGroupLayoutEntry {
            binding: binding.binding,
            visibility: ShaderStages::COMPUTE,
            ty: binding_ty,
            count: None,
        })
    }

    let last_active_group = groups
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, group)| (group.len() != 0).then_some(idx));

    let layouts: Vec<(BindGroupLayout, Vec<BindGroupLayoutEntry>)> = groups
        .into_iter()
        .take(last_active_group.map(|i| i + 1).unwrap_or(0))
        .map(|entries| {
            let group = ctx
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries[..],
                });

            (group, entries)
        })
        .collect();

    // TODO: This is an unnecessary allocation that can hopefully be fixed later
    let borrows: Vec<_> = layouts.iter().map(|(group, _)| group).collect();

    let layout = ctx
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &borrows[..],
            push_constant_ranges: &[],
        });

    let pipeline = ctx
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label,
            layout: Some(&layout),
            module: &module.wgpu,
            entry_point,
        });

    Ok(ReflectedComputePipeline {
        pipeline,
        layout,
        group_layouts: layouts,
    })
}

fn match_image(dim: ImageDimension, arrayed: bool, class: ImageClass) -> BindingType {
    let view_dim = match (dim, arrayed) {
        (naga::ImageDimension::D1, false) => wgpu::TextureViewDimension::D1,
        (naga::ImageDimension::D2, false) => wgpu::TextureViewDimension::D2,
        (naga::ImageDimension::D2, true) => wgpu::TextureViewDimension::D2Array,
        (naga::ImageDimension::D3, false) => wgpu::TextureViewDimension::D3,
        (naga::ImageDimension::Cube, false) => wgpu::TextureViewDimension::Cube,
        (naga::ImageDimension::Cube, true) => wgpu::TextureViewDimension::CubeArray,
        _ => {
            unreachable!("incorrect texture dimension/arrayedness combination")
        }
    };

    match class {
        naga::ImageClass::Sampled { kind, multi } => BindingType::Texture {
            sample_type: match kind {
                naga::ScalarKind::Sint => wgpu::TextureSampleType::Sint,
                naga::ScalarKind::Uint => wgpu::TextureSampleType::Uint,
                naga::ScalarKind::Float => {
                    wgpu::TextureSampleType::Float { filterable: true }
                    // TODO: Only set this if any associated samplers are filtering
                }
                naga::ScalarKind::Bool => {
                    unreachable!("images cannot be of type bool")
                }
            },
            view_dimension: view_dim,
            multisampled: multi,
        },
        naga::ImageClass::Depth { multi } => BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: view_dim,
            multisampled: multi,
        },
        naga::ImageClass::Storage { format, access } => BindingType::StorageTexture {
            access: if access == StorageAccess::STORE {
                StorageTextureAccess::WriteOnly
            } else if access == StorageAccess::LOAD {
                StorageTextureAccess::ReadOnly
            } else if access == StorageAccess::LOAD | StorageAccess::STORE {
                StorageTextureAccess::ReadWrite
            } else {
                unreachable!("storage textures must be readonly, writeonly, or readwrite.");
            },
            format: match_format(format),
            view_dimension: view_dim,
        },
    }
}

fn match_format(format: StorageFormat) -> TextureFormat {
    match format {
        naga::StorageFormat::R8Unorm => TextureFormat::R8Unorm,
        naga::StorageFormat::R8Snorm => TextureFormat::R8Snorm,
        naga::StorageFormat::R8Uint => TextureFormat::R8Uint,
        naga::StorageFormat::R8Sint => TextureFormat::R8Sint,
        naga::StorageFormat::R16Uint => TextureFormat::R16Uint,
        naga::StorageFormat::R16Sint => TextureFormat::R16Sint,
        naga::StorageFormat::R16Float => TextureFormat::R16Float,
        naga::StorageFormat::Rg8Unorm => TextureFormat::Rg8Unorm,
        naga::StorageFormat::Rg8Snorm => TextureFormat::Rg8Snorm,
        naga::StorageFormat::Rg8Uint => TextureFormat::Rg8Uint,
        naga::StorageFormat::Rg8Sint => TextureFormat::Rg8Sint,
        naga::StorageFormat::R32Uint => TextureFormat::R32Uint,
        naga::StorageFormat::R32Sint => TextureFormat::R32Sint,
        naga::StorageFormat::R32Float => TextureFormat::R32Float,
        naga::StorageFormat::Rg16Uint => TextureFormat::Rg16Uint,
        naga::StorageFormat::Rg16Sint => TextureFormat::Rg16Sint,
        naga::StorageFormat::Rg16Float => TextureFormat::Rg16Float,
        naga::StorageFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        naga::StorageFormat::Rgba8Snorm => TextureFormat::Rgba8Snorm,
        naga::StorageFormat::Rgba8Uint => TextureFormat::Rgba8Uint,
        naga::StorageFormat::Rgba8Sint => TextureFormat::Rgba8Sint,
        naga::StorageFormat::Rgb10a2Unorm => TextureFormat::Rgb10a2Unorm,
        naga::StorageFormat::Rg11b10Float => TextureFormat::Rg11b10Float,
        naga::StorageFormat::Rg32Uint => TextureFormat::Rg32Uint,
        naga::StorageFormat::Rg32Sint => TextureFormat::Rg32Sint,
        naga::StorageFormat::Rg32Float => TextureFormat::Rg32Float,
        naga::StorageFormat::Rgba16Uint => TextureFormat::Rgba16Uint,
        naga::StorageFormat::Rgba16Sint => TextureFormat::Rgba16Sint,
        naga::StorageFormat::Rgba16Float => TextureFormat::Rgba16Float,
        naga::StorageFormat::Rgba32Uint => TextureFormat::Rgba32Uint,
        naga::StorageFormat::Rgba32Sint => TextureFormat::Rgba32Sint,
        naga::StorageFormat::Rgba32Float => TextureFormat::Rgba32Float,
    }
}
