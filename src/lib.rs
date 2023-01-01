use std::borrow::Cow;
use std::path::Path;

use commands::RenderCommands;
use node::{NodeInput, NodeOutput, RenderNode};
use reflect::{ModuleError, ReflectedComputePipeline};
use resources::{RWMode, TextureSize};
use spirv_iter::SpirvIterator;
use thiserror::Error;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Adapter, BufferDescriptor, BufferUsages, Device, Instance, Label, MapMode, Queue, TextureFormat,
};

use crate::resources::{PipelineStorage, RenderResources};

mod bitset;
mod commands;
mod graph;
mod named_slotmap;
mod node;
mod reflect;
mod resources;
mod spirv_iter;
mod util;

pub mod prelude;

#[derive(Copy, Clone)]
pub struct RenderContext<'i, 'a, 'd, 'q> {
    pub instance: &'i Instance,
    pub adapter: &'a Adapter,
    pub device: &'d Device,
    pub queue: &'q Queue,
}

#[non_exhaustive]
pub enum ShaderSource<I: SpirvIterator, P: AsRef<Path>> {
    Spirv(I),
    FilePath(P),
    WgslFilePath(P),
}

impl ShaderSource<&'static [u32], &'static str> {
    pub fn spirv<I: SpirvIterator>(iter: I) -> ShaderSource<I, &'static str> {
        ShaderSource::Spirv(iter)
    }

    pub fn spirv_file_path<P: AsRef<Path>>(path: P) -> ShaderSource<&'static [u32], P> {
        ShaderSource::FilePath(path)
    }

    pub fn wgsl_file_path<P: AsRef<Path>>(path: P) -> ShaderSource<&'static [u32], P> {
        ShaderSource::WgslFilePath(path)
    }
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    ModuleError(#[from] ModuleError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    PipelineError(#[from] reflect::PipelineError),
}

impl<'i, 'a, 'd, 'q> RenderContext<'i, 'a, 'd, 'q> {
    pub fn new(
        instance: &'i Instance,
        adapter: &'a Adapter,
        device: &'d Device,
        queue: &'q Queue,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    pub fn compute_pipeline<I, P>(
        &self,
        label: Label,
        shader: ShaderSource<I, P>,
        entry_point: &str,
    ) -> Result<ReflectedComputePipeline, PipelineError>
    where
        P: AsRef<Path>,
        I: SpirvIterator,
    {
        let module = reflect::module_from_source(self, shader)?;

        let pipeline = reflect::compute_pipeline_from_module(self, &module, entry_point, label)?;

        Ok(pipeline)
    }
}

struct ComputeLevels;

impl RenderNode for ComputeLevels {
    fn name() -> Cow<'static, str> {
        "compute_levels".into()
    }

    fn run(commands: &mut RenderCommands) {
        let compute_levels = commands.compute_pipeline("compute_levels");
        let ascii = commands.buffer("ascii_buffer");
        let tex = commands.texture("tex");
        commands
            .texture_constraints(tex)
            .has_size(TextureSize::D2 { x: 256, y: 256 })
            .has_format(TextureFormat::Rgba8Unorm);

        // commands.write_buffer(ascii, 0, &[0xDE, 0xAD, 0xBE, 0xEF]);

        commands
            .compute_pass(Some("pass"))
            .pipeline(compute_levels)
            .bind_group(
                0,
                [
                    (0, ascii.slice(..).storage(RWMode::READWRITE)),
                    (1, tex.view().create()),
                ],
            )
            .dispatch(256, 1, 1);
    }
}

struct CopyToStaging;

impl RenderNode for CopyToStaging {
    fn name() -> Cow<'static, str> {
        "copy_to_staging".into()
    }

    fn after() -> Vec<Cow<'static, str>> {
        vec![ComputeLevels::name()]
    }

    fn run(commands: &mut RenderCommands) {
        let buffer = commands.buffer("ascii_buffer");
        let staging = commands.buffer("staging");
        commands.copy_buffer_to_buffer(buffer, 0, staging, 0, 4);
    }
}

#[test]
fn test() {
    use crate::graph::RenderGraph;
    use wgpu::{
        Backends, DeviceDescriptor, Features, Limits, PowerPreference, RequestAdapterOptions,
    };

    let instance = Instance::new(Backends::PRIMARY);
    let adapter =
        futures_lite::future::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .unwrap();
    let (device, queue) = futures_lite::future::block_on(adapter.request_device(
        &DeviceDescriptor {
            label: Some("RenderDevice"),
            features: Features::default(),
            limits: Limits::default(),
        },
        None,
    ))
    .unwrap();

    let ctx = RenderContext::new(&instance, &adapter, &device, &queue);

    let staging = ctx.device.create_buffer(&BufferDescriptor {
        label: None,
        size: 4,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let pipeline = ctx
        .compute_pipeline(
            Some("compute_levels pipeline"),
            ShaderSource::wgsl_file_path("test.wgsl"),
            "main",
        )
        .unwrap();

    let mut graph = RenderGraph::new();
    graph.add_node::<ComputeLevels>();
    graph.add_node::<CopyToStaging>();

    let mut resources = RenderResources::new();
    resources.insert_buffer("staging", staging);

    let mut pipelines = PipelineStorage::new();
    pipelines.insert_compute_pipeline("compute_levels", pipeline);

    println!("{graph:#?}");

    let mut comp = graph.compile(ctx, &pipelines).unwrap();

    println!("{comp:#?}");

    comp.run(ctx, &resources).unwrap();

    let staging = resources.get_buffer("staging").unwrap();
    let slice = staging.slice(0..4);
    slice.map_async(MapMode::Read, |_| ());
    ctx.device.poll(wgpu::MaintainBase::Wait);
    println!("New bytes: {:?}", &slice.get_mapped_range()[..]);
}
