use std::borrow::Cow;
use std::num::NonZeroU64;

use naga::{
    AddressSpace, FastHashSet, GlobalVariable, Handle, ImageClass, ImageDimension, ResourceBinding,
    ShaderStage, StorageAccess, StorageFormat, TypeInner,
};
use slotmap::{new_key_type, SlotMap};
use thiserror::Error;
use wgpu::{
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BufferBindingType,
    ComputePipelineDescriptor, Label, PipelineLayoutDescriptor, ShaderStages, StorageTextureAccess,
    TextureFormat,
};

use crate::named_slotmap::NamedSlotMap;
use crate::RenderContext;

use super::layout::PipelineLayoutHandle;
use super::module::ModuleError;
use super::{BindGroupLayout, BindGroupLayoutHandle, PipelineLayout, ShaderModule};

new_key_type! { pub struct ComputePipelineHandle; }

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) wgpu: wgpu::ComputePipeline,
    pub(crate) layout: PipelineLayoutHandle,
}

pub(crate) type ComputePipelines = NamedSlotMap<ComputePipelineHandle, ComputePipeline>;
pub(crate) type BindGroupLayouts = SlotMap<BindGroupLayoutHandle, BindGroupLayout>;
pub(crate) type PipelineLayouts = SlotMap<PipelineLayoutHandle, PipelineLayout>;

#[derive(Debug)]
pub struct PipelineStorage {
    pub(crate) compute_pipelines: ComputePipelines,
    pub(crate) bind_group_layouts: BindGroupLayouts,
    pub(crate) pipeline_layouts: PipelineLayouts,
}

impl PipelineStorage {
    pub fn new() -> Self {
        Self {
            compute_pipelines: NamedSlotMap::new(),
            bind_group_layouts: SlotMap::with_key(),
            pipeline_layouts: SlotMap::with_key(),
        }
    }

    pub fn insert_compute_pipeline(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        ReflectedComputePipeline {
            pipeline,
            layout,
            group_layouts,
        }: ReflectedComputePipeline,
    ) -> ComputePipelineHandle {
        let groups = group_layouts
            .into_iter()
            .map(|(layout, entries)| {
                self.bind_group_layouts.insert(BindGroupLayout {
                    wgpu: layout,
                    entries,
                })
            })
            .collect();

        let layout = self.pipeline_layouts.insert(PipelineLayout {
            wgpu: layout,
            groups,
        });

        self.compute_pipelines.insert(
            name,
            ComputePipeline {
                wgpu: pipeline,
                layout,
            },
        )
    }
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("entry point `{0}` is missing from module")]
    MissingEntryPoint(String),
    #[error("entry point `{0}` is not a compute shader")]
    NotComputeShader(String),
    #[error("bind group {0} is greater than the maximum amount of bind groups")]
    BindGroupTooHigh(u32),
    #[error(transparent)]
    ModuleError(#[from] ModuleError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
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
    nonfiltering_samplers: &FastHashSet<ResourceBinding>,
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
        (global.binding.is_some()).then_some((handle, global))
    });

    let filtered: FastHashSet<Handle<GlobalVariable>> = point_info
        .sampling_set
        .iter()
        .filter_map(|key| {
            let sampler = &module.module.global_variables[key.sampler];
            let ty = &module.module.types[sampler.ty];
            match ty.inner {
                TypeInner::Sampler { .. } => (!nonfiltering_samplers
                    .contains(&sampler.binding.clone().unwrap()))
                .then_some(key.image),
                _ => unreachable!(),
            }
        })
        .collect();

    let mut groups: [Vec<BindGroupLayoutEntry>; wgpu_core::MAX_BIND_GROUPS] =
        std::array::from_fn(|_| vec![]);

    for (handle, resource) in resources {
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
                } => match_image(dim, arrayed, class, filtered.contains(handle)),
                TypeInner::Sampler { comparison } => BindingType::Sampler(match comparison {
                    true => wgpu::SamplerBindingType::Comparison,
                    false => {
                        match nonfiltering_samplers.contains(&resource.binding.clone().unwrap()) {
                            true => wgpu::SamplerBindingType::NonFiltering,
                            false => wgpu::SamplerBindingType::Filtering,
                        }
                    }
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

    let layouts: Vec<(wgpu::BindGroupLayout, Vec<BindGroupLayoutEntry>)> = groups
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

fn match_image(
    dim: ImageDimension,
    arrayed: bool,
    class: ImageClass,
    filtered: bool,
) -> BindingType {
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
                naga::ScalarKind::Float => wgpu::TextureSampleType::Float {
                    filterable: filtered,
                },
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
        naga::StorageFormat::R16Unorm => TextureFormat::R16Unorm,
        naga::StorageFormat::R16Snorm => TextureFormat::R16Snorm,
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
        naga::StorageFormat::Rg16Unorm => TextureFormat::Rg16Unorm,
        naga::StorageFormat::Rg16Snorm => TextureFormat::Rg16Snorm,
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
        naga::StorageFormat::Rgba16Unorm => TextureFormat::Rgba16Unorm,
        naga::StorageFormat::Rgba16Snorm => TextureFormat::Rgba16Snorm,
        naga::StorageFormat::Rgba16Uint => TextureFormat::Rgba16Uint,
        naga::StorageFormat::Rgba16Sint => TextureFormat::Rgba16Sint,
        naga::StorageFormat::Rgba16Float => TextureFormat::Rgba16Float,
        naga::StorageFormat::Rgba32Uint => TextureFormat::Rgba32Uint,
        naga::StorageFormat::Rgba32Sint => TextureFormat::Rgba32Sint,
        naga::StorageFormat::Rgba32Float => TextureFormat::Rgba32Float,
    }
}
