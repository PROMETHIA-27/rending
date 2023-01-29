use std::collections::HashSet;
use std::num::NonZeroU32;

use rending::*;

fn main() {
    let instance = GPUInstance::new_headless(
        Backends::PRIMARY,
        PowerPreference::HighPerformance,
        None,
        Features::default(),
        Limits::default(),
        false,
    )
    .unwrap();
    let context = instance.create_render_context();

    let mut graph = RenderGraph::new();
    graph.add_node(FunctionNode::new("compute_levels", compute_levels));
    graph.add_node(FunctionNode::new("copy_to_staging", copy_to_staging).after("compute_levels"));

    let mut pipelines = PipelineStorage::new();
    let compute_levels = context
        .compute_pipeline(
            Some("compute_levels"),
            ShaderSource::wgsl_file_path("assets/shaders/image_to_ascii.wgsl"),
            "main",
            &HashSet::default(),
        )
        .unwrap();
    pipelines.insert_compute_pipeline("compute_levels_pipeline", compute_levels);

    let mut compiled = graph.compile(&pipelines).unwrap();

    let mut resources = RenderResources::new();
    let staging = context.buffer().size(16).copy_dst().map_read().create();
    resources.insert_buffer("staging", staging);

    compiled.run(context, &resources).unwrap();

    let slice = resources.get_buffer("staging").unwrap().slice(..);
    slice.map_async(MapMode::Read, |_| ());
    context.device.poll(MaintainBase::Wait);
    println!(
        "{:?}",
        &slice.get_mapped_range()[..]
            .chunks_exact(4)
            .map(|data| f32::from_ne_bytes(data.try_into().unwrap()))
            .collect::<Vec<_>>()
    );
}

fn compute_levels(commands: &mut RenderCommands) {
    let compute_levels = commands.compute_pipeline("compute_levels_pipeline");
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

fn copy_to_staging(commands: &mut RenderCommands) {
    let buffer = commands.buffer("ascii_buffer");
    let staging = commands.buffer("staging");
    commands.copy_buffer_to_buffer(buffer, 0, staging, 0, 16);
}
