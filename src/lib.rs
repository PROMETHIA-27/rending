// mod bitset;
// mod commands;
// mod context;
// mod named_slotmap;
// mod resources;
// mod spirv_iter;
// mod util;

pub use rending_builder as builder;
pub use rending_reflect as reflect;

pub mod prelude {
    pub use rending_builder::prelude::*;
    pub use rending_reflect::ReflectedComputePipeline;
}

// pub use commands::RenderCommands;
// pub use context::{BufferBuilder, RenderContext};
// use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
// pub use resources::{
//     compute_pipeline_from_module, module_from_source, ComputePipeline, ModuleError, PipelineError,
//     Pipelines, RWMode, ReflectedComputePipeline, RenderResources, ShaderSource, Texture,
//     TextureSize,
// };
// pub use wgpu::{
//     Backends, Extent3d, Features, ImageDataLayout, Limits, MaintainBase, MapMode, Origin3d,
//     PowerPreference, TextureFormat,
// };
// use wgpu::{
//     Device, DeviceDescriptor, Dx12Compiler, Instance, InstanceDescriptor, Queue,
//     RequestAdapterOptions, Surface,
// };

// /// This is a convenience struct so that users of `rending` don't have to
// /// depend on wgpu directly to get started. It barely provides enough
// /// functionality to get started, but should be adequate for basic
// /// setups.
// pub struct GPUInstance {
//     device: Device,
//     queue: Queue,
// }

// impl GPUInstance {
//     /// Attempt to construct a new GPUInstance from the given settings.
//     /// For more control, you can import `wgpu` and construct an `Instance`,
//     /// `Adapter`, `Device`, and `Queue` manually.
//     /// To construct without a surface, you can use [`GPUInstance::new_headless()`].
//     ///
//     /// # Safety
//     /// - The window that the surface handle points to must remain valid for
//     /// the duration of this function. It should be difficult to violate this invariant,
//     /// but failure to keep it alive is Undefined Behavior.
//     /// This would likely require deliberately invalidating the window during this function call.
//     /// In most cases this shouldn't be possible without unsafe code.
//     pub fn new(
//         backends: Backends,
//         power: PowerPreference,
//         surface: impl HasRawWindowHandle + HasRawDisplayHandle,
//         device_label: Option<&str>,
//         features: Features,
//         limits: Limits,
//         force_fallback_adapter: bool,
//     ) -> Option<Self> {
//         let instance = Instance::new(InstanceDescriptor {
//             backends,
//             // TODO: Do something about this
//             dx12_shader_compiler: Dx12Compiler::default(),
//         });
//         // SAFETY:
//         // - The safety invariants of `HawRawWindowHandle` and `HasRawDisplayHandle` guarantee
//         // that they will be valid (a contract the implementor of those traits must uphold)
//         // - The handle will outlive the surface and nothing should cause the backing window
//         // to stop being valid before then.
//         // *Ok technically there might be some *really* funky threading stuff you could do to break this
//         // but at least most window handles should be !Send + !Sync which should prevent that
//         let surface = unsafe { instance.create_surface(&surface) }.ok()?;

//         Self::new_inner(
//             instance,
//             power,
//             device_label,
//             features,
//             limits,
//             force_fallback_adapter,
//             Some(&surface),
//         )
//     }

//     /// Create a new surface-less `GPUInstance`.
//     /// For more control, you can import `wgpu` and construct an `Instance`,
//     /// `Adapter`, `Device`, and `Queue` manually.
//     /// To construct with a surface, you can use [`GPUInstance::new()`].
//     pub fn new_headless(
//         backends: Backends,
//         power: PowerPreference,
//         device_label: Option<&str>,
//         features: Features,
//         limits: Limits,
//         force_fallback_adapter: bool,
//     ) -> Option<Self> {
//         let instance = Instance::new(InstanceDescriptor {
//             backends,
//             // TODO: Do something about this
//             dx12_shader_compiler: Dx12Compiler::default(),
//         });

//         Self::new_inner(
//             instance,
//             power,
//             device_label,
//             features,
//             limits,
//             force_fallback_adapter,
//             None,
//         )
//     }

//     fn new_inner(
//         instance: Instance,
//         power: PowerPreference,
//         device_label: Option<&str>,
//         features: Features,
//         limits: Limits,
//         force_fallback_adapter: bool,
//         surface: Option<&Surface>,
//     ) -> Option<Self> {
//         let (device, queue) = futures_lite::future::block_on(async {
//             let adapter = instance
//                 .request_adapter(&RequestAdapterOptions {
//                     power_preference: power,
//                     force_fallback_adapter,
//                     compatible_surface: surface,
//                 })
//                 .await?;
//             let (device, queue) = adapter
//                 .request_device(
//                     &DeviceDescriptor {
//                         label: device_label,
//                         features,
//                         limits,
//                     },
//                     None,
//                 )
//                 .await
//                 .ok()?;
//             Some((device, queue))
//         })?;

//         Some(Self { device, queue })
//     }

//     /// Create a `RenderContext`. This is the connection to the GPU that
//     /// rending actually uses.
//     pub fn create_render_context(&self) -> RenderContext {
//         RenderContext::new(&self.device, &self.queue)
//     }
// }
