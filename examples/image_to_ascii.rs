use std::collections::HashSet;
use std::num::NonZeroU32;

use image::EncodableLayout;
use rending::*;
use wgpu::{ImageCopyTexture, TextureAspect, TextureUsages};

struct Point<T> {
    x: T,
    y: T,
}
const RESOLUTION: Point<u32> = Point { x: 256, y: 256 };

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

    let ascii = context
        .buffer()
        .size((16 * 256) as u64)
        .copy_dst()
        .uniform()
        .create();
    let ascii_bytes: Vec<u8> = include_bytes!("../assets/text/levels.txt")
        .iter()
        .copied()
        .filter(|&c| c != b'\n')
        .flat_map(|byte| {
            let mut arr = [0; 16];
            arr[0] = byte;
            arr
        })
        .collect();
    context.queue.write_buffer(&ascii, 0, &ascii_bytes);
    resources.insert_buffer("ascii_table", ascii);

    let input = context.texture(
        Some("dog"),
        TextureSize::D2 {
            x: RESOLUTION.x,
            y: RESOLUTION.y,
        },
        TextureFormat::Rgba8Unorm,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        1,
        1,
    );
    let img = image::io::Reader::open("assets/images/dog.jpg")
        .unwrap()
        .decode()
        .unwrap();
    context.queue.write_texture(
        ImageCopyTexture {
            texture: &input.inner,
            mip_level: 0,
            origin: Origin3d::default(),
            aspect: TextureAspect::default(),
        },
        img.into_rgba8().as_bytes(),
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(NonZeroU32::new(4 * RESOLUTION.x).unwrap()),
            rows_per_image: None,
        },
        Extent3d {
            width: RESOLUTION.x,
            height: RESOLUTION.y,
            depth_or_array_layers: 1,
        },
    );
    resources.insert_texture("input", input);

    let staging = context
        .buffer()
        .size((RESOLUTION.x * RESOLUTION.y * 16) as u64)
        .copy_dst()
        .map_read()
        .create();
    resources.insert_buffer("staging", staging);

    compiled.run(context, &resources).unwrap();

    let staging = resources.get_buffer("staging").unwrap();
    let mut output = context
        .read_map_buffer(&staging.slice(..))
        .into_iter()
        .step_by(16)
        .copied()
        .collect::<Vec<_>>();
    for i in (0..256).rev() {
        output.insert(RESOLUTION.x as usize * i, b'\n');
    }
    std::fs::write("assets/text/output.txt", output).unwrap();
}

fn compute_levels(commands: &mut RenderCommands) {
    let ascii = commands.buffer("ascii_table");
    let input = commands.texture("input");
    let output = commands.buffer("output");
    let pipeline = commands.compute_pipeline("compute_levels_pipeline");

    commands
        .compute_pass(Some("compute_levels"))
        .bind_group(
            0,
            [
                (0, ascii.slice(..).uniform()),
                (1, input.view().create()),
                (2, output.slice(..).infer()),
            ],
        )
        .pipeline(pipeline)
        .dispatch(RESOLUTION.x, RESOLUTION.y, 1);
}

fn copy_to_staging(commands: &mut RenderCommands) {
    let buffer = commands.buffer("output");
    let staging = commands.buffer("staging");
    commands.copy_buffer_to_buffer(
        buffer,
        0,
        staging,
        0,
        (RESOLUTION.x * RESOLUTION.y * 16) as u64,
    );
}
