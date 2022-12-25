use slotmap::new_key_type;

use super::layout::PipelineLayoutHandle;

new_key_type! { pub struct ComputePipelineHandle; }

#[derive(Debug)]
pub struct ComputePipeline {
    pub(crate) wgpu: wgpu::ComputePipeline,
    pub(crate) layout: PipelineLayoutHandle,
}
