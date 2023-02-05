use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};

use slotmap::{new_key_type, SecondaryMap, SlotMap};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BufferBinding, TextureView,
    TextureViewDescriptor,
};

use crate::RenderContext;

use super::buffer::BufferUse;
use super::pipeline::PipelineStorage;
use super::{
    BindGroupLayoutHandle, BufferBindings,
    BufferHandle, /* Sampler, SamplerBindings, SamplerHandle,*/
    TextureAspect, TextureBindings, TextureHandle, TextureViewDimension,
};

pub(crate) type BindGroups = SecondaryMap<BindGroupHandle, BindGroup>;

new_key_type! { pub(crate) struct BindGroupHandle; }

// TODO: pool vecs here
#[derive(Debug)]
pub(crate) struct BindGroupCache {
    groups: SlotMap<BindGroupHandle, (BindGroupLayoutHandle, Vec<(u32, ResourceBinding)>)>,
    reverse: BTreeMap<Vec<(u32, ResourceBinding)>, BindGroupHandle>,
}

impl BindGroupCache {
    pub fn new() -> Self {
        Self {
            groups: SlotMap::with_key(),
            reverse: BTreeMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.groups.clear();
        self.reverse.clear();
    }

    pub fn get_handle(
        &mut self,
        layout: BindGroupLayoutHandle,
        resources: &[(u32, ResourceBinding)],
    ) -> BindGroupHandle {
        if let Some(&handle) = self.reverse.get(resources) {
            handle
        } else {
            let handle = self.groups.insert((layout, resources.to_vec()));
            self.reverse.insert(resources.to_vec(), handle);
            handle
        }
    }

    pub fn get_group(
        &self,
        handle: BindGroupHandle,
    ) -> Option<(BindGroupLayoutHandle, &[(u32, ResourceBinding)])> {
        self.groups
            .get(handle)
            .map(|(layout, group)| (*layout, &group[..]))
    }

    pub fn create_groups(
        &self,
        context: RenderContext,
        pipelines: &PipelineStorage,
        bound_buffers: &BufferBindings,
        bound_textures: &TextureBindings,
        // bound_samplers: &SamplerBindings,
    ) -> BindGroups {
        let mut bind_groups = BindGroups::with_capacity(self.groups.len());
        for (handle, (layout, bindings)) in &self.groups {
            let layout = pipelines
                .bind_group_layouts
                .get(*layout)
                .expect("bind group layouts should not be invalidated before bind group creation");

            let bindings: Vec<(u32, BoundResource)> = bindings
                .iter()
                .map(|&(index, binding)| {
                    let binding = match binding {
                        ResourceBinding::Buffer {
                            handle,
                            offset,
                            size,
                            ..
                        } => BoundResource::Buffer(BufferBinding {
                            buffer: bound_buffers
                                .get(handle)
                                .expect(
                                    "buffers should not be invalidated before bind group creation",
                                )
                                .as_ref(),
                            offset,
                            size,
                        }),
                        ResourceBinding::Texture {
                            handle,
                            dimension,
                            aspect,
                            base_mip,
                            mip_count,
                            base_layer,
                            layer_count,
                        } => {
                            let texture = bound_textures.get(handle).unwrap().as_ref();
                            BoundResource::Texture(texture.inner.create_view(
                                &TextureViewDescriptor {
                                    label: None,
                                    format: Some(texture.format),
                                    dimension: dimension.map(|dim| dim.into_wgpu()),
                                    aspect: aspect.into_wgpu(),
                                    base_mip_level: base_mip,
                                    mip_level_count: mip_count,
                                    base_array_layer: base_layer,
                                    array_layer_count: layer_count,
                                },
                            ))
                        } // ResourceBinding::Sampler { handle } => {
                          //     let sampler = bound_samplers.get(handle).unwrap().as_ref();
                          //     BoundResource::Sampler(sampler)
                          // }
                    };
                    (index, binding)
                })
                .collect();

            let entries: Vec<BindGroupEntry> = bindings
                .iter()
                .map(|(index, binding)| BindGroupEntry {
                    binding: *index,
                    resource: match binding {
                        BoundResource::Buffer(binding) => BindingResource::Buffer(binding.clone()),
                        BoundResource::Texture(view) => BindingResource::TextureView(view),
                        // BoundResource::Sampler(sampler) => BindingResource::Sampler(&sampler.wgpu),
                    },
                })
                .collect();

            let bind_group = context.device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &layout.wgpu,
                entries: &entries,
            });

            bind_groups.insert(handle, bind_group);
        }
        bind_groups
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceBinding {
    Buffer {
        handle: BufferHandle,
        offset: u64,
        size: Option<NonZeroU64>,
        usage: BufferUse,
    },
    Texture {
        handle: TextureHandle,
        dimension: Option<TextureViewDimension>,
        aspect: TextureAspect,
        base_mip: u32,
        mip_count: Option<NonZeroU32>,
        base_layer: u32,
        layer_count: Option<NonZeroU32>,
    },
    // Sampler {
    //     handle: SamplerHandle,
    // },
}

enum BoundResource<'a> {
    Buffer(BufferBinding<'a>),
    Texture(TextureView),
    // Sampler(&'a Sampler),
}
