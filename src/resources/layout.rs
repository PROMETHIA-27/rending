use naga::FastHashMap;
use slotmap::new_key_type;
use wgpu::BindGroupLayoutEntry;

new_key_type! { pub struct BindGroupLayoutHandle; }

#[derive(Debug)]
pub struct BindGroupLayout {
    pub(crate) wgpu: wgpu::BindGroupLayout,
    pub(crate) entries: FastHashMap<u32, BindGroupLayoutEntry>,
}

new_key_type! { pub struct PipelineLayoutHandle; }

#[derive(Debug)]
pub struct PipelineLayout {
    pub(crate) wgpu: wgpu::PipelineLayout,
    pub(crate) groups: Vec<BindGroupLayoutHandle>,
}
