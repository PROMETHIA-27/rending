use std::borrow::Cow;
use std::path::Path;

use commands::RenderCommands;
use node::{NodeInput, NodeOutput, RenderNode};
use reflect::{ModuleError, ReflectedComputePipeline};
use resources::Resources;
use spirv_iter::SpirvIterator;
use thiserror::Error;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Adapter, BufferDescriptor, BufferUsages, Device, Instance, Label, MapMode, Queue};

mod bitset;
mod commands;
mod graph;
mod named_slotmap;
mod node;
mod reflect;
mod resources;
mod spirv_iter;
mod util;

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

    fn reads() -> Vec<NodeInput> {
        vec![NodeInput::new("ascii_buffer")]
    }

    fn writes() -> Vec<NodeOutput> {
        vec![
            // NodeOutput::buffer("levels"),
            NodeOutput::new("ascii_buffer"),
        ]
    }

    fn run(commands: &mut RenderCommands, res: &mut Resources) {
        // let ascii = res.write_buffer("ascii_buffer");

        // commands.write_buffer(ascii, 0, &[0xDE, 0xAD, 0xBE, 0xEF]);

        // commands
        //     .compute_pass(Some("pass"))
        //     .pipeline(res.compute_pipeline("compute_levels"))
        //     .bind_group(0, [(0, ascii.slice(..))])
        //     .dispatch(256, 1, 1);
    }
}

struct CopyToStaging;

impl RenderNode for CopyToStaging {
    fn name() -> Cow<'static, str> {
        "copy_to_staging".into()
    }

    fn reads() -> Vec<NodeInput> {
        vec![NodeInput::new("ascii_buffer"), NodeInput::new("staging")]
    }

    fn writes() -> Vec<NodeOutput> {
        vec![NodeOutput::new("staging")]
    }

    fn after() -> Vec<Cow<'static, str>> {
        vec![ComputeLevels::name()]
    }

    fn run(commands: &mut RenderCommands, res: &mut Resources) {
        let buffer = res.read_buffer("ascii_buffer");
        // let staging = res.write_buffer("staging");
        // commands.copy_buffer_to_buffer(buffer, 0, staging, 0, 4);
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
            features: Features::TEXTURE_BINDING_ARRAY,
            limits: Limits::default(),
        },
        None,
    ))
    .unwrap();

    let ctx = RenderContext::new(&instance, &adapter, &device, &queue);

    let ascii_buffer = ctx.device.create_buffer(&BufferDescriptor {
        label: None,
        size: 16,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = ctx.device.create_buffer(&BufferDescriptor {
        label: None,
        size: 16,
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

    // graph.insert_buffer("ascii_buffer", ascii_buffer);
    // graph.insert_buffer("staging", staging);
    // graph.insert_compute_pipeline("compute_levels", pipeline);

    graph.add_node::<ComputeLevels>();
    graph.add_node::<CopyToStaging>();
    println!("{graph:#?}");
    let mut comp = graph.compile(ctx).unwrap();
    // comp.run(ctx).unwrap();

    // let staging = graph.get_buffer_named("staging").unwrap();
    // let slice = staging.slice(0..4);
    // slice.map_async(MapMode::Read, |_| ());
    // ctx.device.poll(wgpu::MaintainBase::Wait);
    // println!("New bytes: {:?}", &slice.get_mapped_range()[..]);
}
