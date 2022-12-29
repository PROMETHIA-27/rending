use std::borrow::Cow;

use slotmap::{new_key_type, SecondaryMap, SlotMap};
use wgpu::BindGroup;

use crate::named_slotmap::NamedSlotMap;
use crate::reflect::ReflectedComputePipeline;

use super::layout::PipelineLayoutHandle;
use super::{BindGroupHandle, BindGroupLayout, BindGroupLayoutHandle, PipelineLayout};

new_key_type! { pub struct ComputePipelineHandle; }

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) wgpu: wgpu::ComputePipeline,
    pub(crate) layout: PipelineLayoutHandle,
}

pub(crate) type ComputePipelines = NamedSlotMap<ComputePipelineHandle, ComputePipeline>;
pub(crate) type BindGroups = SecondaryMap<BindGroupHandle, BindGroup>;
pub(crate) type BindGroupLayouts = SlotMap<BindGroupLayoutHandle, BindGroupLayout>;
pub(crate) type PipelineLayouts = SlotMap<PipelineLayoutHandle, PipelineLayout>;

#[derive(Debug)]
pub struct PipelineStorage {
    pub(crate) compute_pipelines: ComputePipelines,
    pub(crate) bind_groups: BindGroups,
    pub(crate) bind_group_layouts: BindGroupLayouts,
    pub(crate) pipeline_layouts: PipelineLayouts,
}

impl PipelineStorage {
    pub fn new() -> Self {
        Self {
            compute_pipelines: NamedSlotMap::new(),
            bind_groups: SecondaryMap::new(),
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
