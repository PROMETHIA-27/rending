#[derive(Debug)]
pub struct ShaderModule {
    pub(crate) wgpu: wgpu::ShaderModule,
    pub(crate) module: naga::Module,
    pub(crate) info: naga::valid::ModuleInfo,
}
