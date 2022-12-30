use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};

use naga::FastHashMap;
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BufferBinding, Texture,
};

use crate::RenderContext;

use super::buffer::BufferUse;
use super::pipeline::PipelineStorage;
use super::{
    BindGroupLayoutHandle, BufferBindings, BufferHandle, RenderResources, ResourceHandle,
    ResourceUse, TextureBindings, TextureHandle, TextureViewDimension,
};

pub(crate) type BindGroups = SecondaryMap<BindGroupHandle, BindGroup>;

new_key_type! { pub(crate) struct BindGroupHandle; }

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
        resources: &RenderResources,
        bound_buffers: &BufferBindings,
        bound_textures: &TextureBindings,
        meta: &FastHashMap<ResourceHandle, ResourceUse>,
    ) -> BindGroups {
        let mut bind_groups = BindGroups::with_capacity(self.groups.len());
        for (handle, (layout, bindings)) in &self.groups {
            let layout = pipelines
                .bind_group_layouts
                .get(*layout)
                .expect("bind group layouts should not be invalidated before bind group creation");

            let entries = bindings
                .iter()
                .map(|&(index, binding)| {
                    let &meta = meta
                        .get(&binding.handle())
                        .expect("all bound resources should have meta associated");

                    BindGroupEntry {
                        binding: index,
                        resource: match (binding, meta) {
                            (
                                ResourceBinding::Buffer {
                                    handle,
                                    offset,
                                    size,
                                    ..
                                },
                                ResourceUse::Buffer { .. },
                            ) => BindingResource::Buffer(BufferBinding {
                                buffer: bound_buffers.get(handle).expect(
                                    "buffers should not be invalidated before bind group creation",
                                ).as_ref(),
                                offset,
                                size,
                            }),
                            _ => panic!("resource use and resource type must match"),
                        },
                    }
                })
                .collect::<Vec<_>>();

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
        dimensions: TextureViewDimension,
        base_mip: u32,
        mip_count: Option<NonZeroU32>,
        base_layer: u32,
        layer_count: Option<NonZeroU32>,
    },
}

impl ResourceBinding {
    pub(crate) fn handle(&self) -> ResourceHandle {
        match self {
            &ResourceBinding::Buffer { handle, .. } => ResourceHandle::Buffer(handle),
            &ResourceBinding::Texture { handle, .. } => ResourceHandle::Texture(handle),
        }
    }

    pub(crate) fn buffer_use(&self) -> BufferUse {
        match self {
            &ResourceBinding::Buffer { usage, .. } => usage,
            _ => panic!("attempted to bind a non-buffer resource to a buffer slot"),
        }
    }
}
