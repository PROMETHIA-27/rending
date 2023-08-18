#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![warn(rustdoc::all)]
#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::num::NonZeroU64;

use naga::valid::{Capabilities, ValidationFlags};
use naga::{
    AddressSpace, FastHashSet, GlobalVariable, Handle, ImageClass, ImageDimension, ResourceBinding,
    ShaderStage, StorageAccess, StorageFormat, TypeInner, WithSpan,
};
use quickerr::error;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBindingType, ComputePipeline,
    ComputePipelineDescriptor, Device, Label, PipelineLayout, PipelineLayoutDescriptor,
    RenderPipeline, ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess,
    TextureFormat,
};

// TODO: Reflect everything in a module
// TODO: Allow choosing an override/neither with a FastHashMap<ResourceBinding, Option<String>>
/// A reflected compute pipeline. It contains a [`wgpu::ComputePipeline`], [`wgpu::PipelineLayout`],
/// and a collection representing the bind group layouts and their entries of the pipeline layout.
///
/// It can be created with the [`ReflectedComputePipeline::new()`] method, and bind groups can be
/// created off of it using the [`ReflectedComputePipeline::bind_group()`] and
/// [`ReflectedComputePipeline::bind_groups()`] methods.
#[derive(Debug)]
pub struct ReflectedComputePipeline {
    /// The compute pipeline itself.
    pub pipeline: ComputePipeline,
    /// The PipelineLayout of [`pipeline`].
    pub layout: PipelineLayout,
    /// The bind group layouts of [`layout`] and their corresponding entries.
    pub group_layouts: Vec<(BindGroupLayout, Vec<(u32, BindGroupLayoutEntry)>)>,
}

type SpirvError = naga::front::spv::Error;

type WgslError = naga::front::wgsl::ParseError;

error! {
    #[cfg(feature = "glsl")]
    /// The error that occurs when invalid glsl code is fed to a reflected pipeline new function.
    pub GlslError
    "errors in glsl shader"
    [naga::front::glsl::Error]
}

type ValidationError = WithSpan<naga::valid::ValidationError>;

error! {
    /// The error that occurs when the given entry point could not be found in a module.
    pub MissingEntryPoint
    "module is missing entry point `{point}`"
    point: String
}

error! {
    /// The error that occurs when the entry point is not of the correct type.
    pub WrongShaderType
    "module expected shader type `{ty}` but got shader type `{got}`"
    ty: &'static str,
    got: String,
}

error! {
    /// The error that occurs when a bind group index exceeds [`wgpu_core::MAX_BIND_GROUPS`].
    pub BindGroupTooHigh
    "binding index `{index}` is greater than `MAX_BIND_GROUPS`"
    index: u32
}

error! {
    /// The error that occurs when an attempt to reflect a pipeline from a module fails.
    pub ReflectError
    "failed to reflect pipeline"
    /// An error occurred while converting SPIR-V bytecode to a module.
    SpirvError,
    /// An error occurred while converting WGSL code to a module.
    WgslError,
    /// An error occurred while converting GLSL code to a module.
    #[cfg(feature = "glsl")]
    GlslError,
    /// An error occurred while validating the module.
    ValidationError,
    /// The module did not have the requested entry point.
    MissingEntryPoint,
    /// The entry point was not of the requested type.
    WrongShaderType,
    /// A bind group index exceeded MAX_BIND_GROUPS.
    BindGroupTooHigh,
}

impl ReflectedComputePipeline {
    // TODO: Investigate a way to explicitly reuse superset pipelinelayouts
    /// Reflect a module to produce a pipeline with its layout and bind groups automatically
    /// generated from the module.
    ///
    /// [`BindGroup`]s are how WGPU groups resources that are passed into [pipelines](wgpu::RenderPipeline)
    /// when they're invoked.
    /// A single bind group is atomic, and is created ahead of time. But a pipeline can have multiple
    /// bind group slots, and replace an entire bind group easily. A bind group can contain multiple
    /// resources. These resources can be [`Texture`](wgpu::Texture)s, [`Buffer`](wgpu::Buffer)s,
    /// [`Sampler`](wgpu::Sampler)s, etc. 
    /// 
    /// A pipeline has a given
    /// [`PipelineLayout`] which is a set of [`BindGroupLayout`]s. A bind group can only be bound to a
    /// slot in a pipeline if it shares a bind group layout with that slot in the pipeline layout
    /// of the pipeline it's being bound to. This crate's purpose is to automatically generate
    /// the bind group layouts and pipeline layout of a pipeline from a given shader module. 
    /// 
    /// A shader
    /// module is a piece of shader code that can contain some resource bindings, functions, and entry points
    /// for shaders, as well as a few other things. 
    /// Thus, a single module can contain multiple shaders, of different types.
    /// A [`RenderPipeline`] will correspond to two shaders, one vertex and one fragment,
    /// while a [`ComputePipeline`] will correspond to one shader, a compute shader.
    pub fn new(
        device: &Device,
        source: ShaderSource,
        entry_point: &str,
        nonfiltering_samplers: &FastHashSet<ResourceBinding>,
        label: Label,
    ) -> Result<ReflectedComputePipeline, ReflectError> {
        let module: naga::Module = match source {
            ShaderSource::SpirV(source) => {
                let options = naga::front::spv::Options {
                    adjust_coordinate_space: false,
                    strict_capabilities: true,
                    block_ctx_dump_prefix: None,
                };
                naga::front::spv::Frontend::new(source.iter().copied(), &options).parse()?
            }
            ShaderSource::Wgsl(source) => naga::front::wgsl::parse_str(&source)?,
            ShaderSource::Naga(source) => source.into_owned(),
            #[cfg(feature = "glsl")]
            ShaderSource::Glsl {
                shader,
                defines,
                stage,
            } => {
                let options = naga::front::glsl::Options { defines, stage };
                naga::front::glsl::Frontend::default()
                    .parse(&options, &shader)
                    .map_err(GlslError)?
            }
            _ => unreachable!(),
        };
        let info = naga::valid::Validator::new(ValidationFlags::all(), Capabilities::all())
            .validate(&module)?;

        let (point_index, point) = module
            .entry_points
            .iter()
            .enumerate()
            .find(|point| point.1.name == entry_point)
            .ok_or_else(|| MissingEntryPoint {
                point: entry_point.to_string(),
            })?;

        if point.stage != ShaderStage::Compute {
            return Err(WrongShaderType {
                ty: "compute",
                got: format!("{:?}", point.stage),
            })?;
        };

        let point_info = info.get_entry_point(point_index);

        let globals: FastHashSet<_> = module
            .global_variables
            .iter()
            .filter_map(|(handle, _)| (!point_info[handle].is_empty()).then_some(handle))
            .collect();

        let resources = globals.iter().filter_map(|handle| {
            let global = module.global_variables.try_get(*handle).unwrap();
            (global.binding.is_some()).then_some((handle, global))
        });

        let filtered: FastHashSet<Handle<GlobalVariable>> = point_info
            .sampling_set
            .iter()
            .filter_map(|key| {
                let sampler = &module.global_variables[key.sampler];
                let ty = &module.types[sampler.ty];
                match ty.inner {
                    TypeInner::Sampler { .. } => (!nonfiltering_samplers
                        .contains(&sampler.binding.clone().unwrap()))
                    .then_some(key.image),
                    _ => unreachable!(),
                }
            })
            .collect();

        let mut groups: [Vec<BindGroupLayoutEntry>; wgpu::core::MAX_BIND_GROUPS] =
            std::array::from_fn(|_| vec![]);

        for (handle, resource) in resources {
            let binding = resource.binding.as_ref().unwrap();

            if binding.group as usize >= wgpu::core::MAX_BIND_GROUPS {
                return Err(BindGroupTooHigh {
                    index: binding.group,
                })?;
            }

            let ty = module.types.get_handle(resource.ty).unwrap();
            let size = ty.inner.size(module.to_ctx());

            let binding_ty = match resource.space {
                AddressSpace::Uniform => BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        NonZeroU64::new(size as u64)
                            .expect("buffers should be non-zero sized types"),
                    ),
                },
                AddressSpace::Storage { access } => BindingType::Buffer {
                    ty: BufferBindingType::Storage {
                        read_only: !access.contains(StorageAccess::LOAD),
                    },
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        NonZeroU64::new(size as u64)
                            .expect("buffers should be non-zero sized types"),
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
                            match nonfiltering_samplers.contains(&resource.binding.clone().unwrap())
                            {
                                true => wgpu::SamplerBindingType::NonFiltering,
                                false => wgpu::SamplerBindingType::Filtering,
                            }
                        }
                    }),
                    _ => unreachable!("a handle should be an image or sampler"),
                },
                AddressSpace::PushConstant => todo!(),
                _ => unreachable!(
                    "resources should not be private, function, or workgroup variables"
                ),
            };

            groups[binding.group as usize].push(BindGroupLayoutEntry {
                binding: binding.binding,
                visibility: ShaderStages::COMPUTE,
                ty: binding_ty,
                count: None,
            });
        }

        let last_active_group = groups
            .iter()
            .enumerate()
            .rev()
            .find_map(|(idx, group)| (!group.is_empty()).then_some(idx));

        let layouts: Vec<(BindGroupLayout, Vec<(u32, BindGroupLayoutEntry)>)> = groups
            .into_iter()
            .take(last_active_group.map(|i| i + 1).unwrap_or(0))
            .map(|entries| {
                let group = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries[..],
                });

                let entries = entries
                    .into_iter()
                    .map(|entry| (entry.binding, entry))
                    .collect();

                (group, entries)
            })
            .collect();

        // TODO: This is an unnecessary allocation that can hopefully be fixed later
        let bind_group_layouts: Vec<_> = layouts.iter().map(|(group, _)| group).collect();

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts[..],
            push_constant_ranges: &[],
        });

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label,
            source: ShaderSource::Naga(Cow::Owned(module)),
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label,
            layout: Some(&layout),
            module: &module,
            entry_point,
        });

        Ok(ReflectedComputePipeline {
            pipeline,
            layout,
            group_layouts: layouts,
        })
    }

    /// Construct a bind group using the given layout of this pipeline.
    pub fn bind_group<'a>(
        &self,
        device: &Device,
        label: Label,
        group: usize,
        bindings: impl IntoIterator<Item = (u32, BindingResource<'a>)>,
    ) -> Option<BindGroup> {
        let entries: Vec<BindGroupEntry> = bindings
            .into_iter()
            .map(|(binding, resource)| BindGroupEntry { binding, resource })
            .collect();
        Some(device.create_bind_group(&BindGroupDescriptor {
            label,
            layout: &self.group_layouts.get(group)?.0,
            entries: &entries[..],
        }))
    }

    /// Construct all bind groups of this pipeline.
    pub fn bind_groups<
        'a,
        'l,
        BindGroups: IntoIterator<Item = (Label<'l>, usize, BindGroupEntries)>,
        BindGroupEntries: IntoIterator<Item = (u32, BindingResource<'a>)>,
    >(
        &self,
        device: &Device,
        groups: BindGroups,
    ) -> Option<Vec<(usize, BindGroup)>> {
        groups
            .into_iter()
            .map(|(label, index, group)| {
                Some((index, self.bind_group(device, label, index, group)?))
            })
            .collect::<Option<Vec<_>>>()
    }
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
