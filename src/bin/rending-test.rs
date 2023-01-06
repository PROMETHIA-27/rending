use std::borrow::Cow;
use std::num::NonZeroU32;

use rending::*;
use wgpu::{
    Backends, BufferDescriptor, BufferUsages, DeviceDescriptor, Extent3d, Features,
    ImageDataLayout, Instance, Limits, MapMode, Origin3d, PowerPreference, RequestAdapterOptions,
    TextureFormat,
};

fn main() {
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

    let ctx = RenderContext::new(&device, &queue);

    // TODO:
    // ctx.buffer()
    //    .size(4)
    //    .copy_dst()
    //    .map_read()
    //    .create();
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
    let slice = staging.slice(0..16);
    slice.map_async(MapMode::Read, |_| ());
    ctx.device.poll(wgpu::MaintainBase::Wait);
    println!(
        "New bytes: {:?}",
        &slice.get_mapped_range()[..]
            .chunks(4)
            .map(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>()
    );
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
            .has_size(TextureSize::D2 { x: 1, y: 1 })
            .has_format(TextureFormat::Rgba8Unorm);

        commands.write_texture(
            tex.copy_view(0, Origin3d::ZERO),
            &[0xDE, 0xAD, 0xBE, 0xEF],
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(NonZeroU32::new(4).unwrap()),
                rows_per_image: None,
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

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
            .dispatch(1, 1, 1);
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
        commands.copy_buffer_to_buffer(buffer, 0, staging, 0, 16);
    }
}
