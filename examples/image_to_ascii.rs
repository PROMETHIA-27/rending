use std::collections::HashSet;
use std::mem::size_of;
use std::num::NonZeroU32;

use encase::{StorageBuffer, UniformBuffer};
use glam::{uvec2, UVec2, UVec4};
use image::EncodableLayout;
use rending::*;
use wgpu::{ImageCopyTexture, TextureAspect, TextureUsages};

#[allow(clippy::read_zero_byte_vec)]
fn main() {
    let image_name = std::env::args().nth(1).expect("Must provide an image name");
    let mut path = std::path::PathBuf::new();
    path.push("assets/images/");
    path.push(image_name + ".jpg");

    println!("Converting image `{}` to ascii...", path.display());
    let img = image::io::Reader::open(path).unwrap().decode().unwrap();

    // Actual resolution of the image
    let resolution = uvec2(img.width(), img.height());
    // Resolution split into horizontal UVec4s
    let vec_resolution = uvec2(
        div_ceil(resolution.x, size_of::<UVec4>() as u32),
        resolution.y / 2,
    );
    // Actual byte dimensions of resolution after splitting into horizontal UVec4s
    let output_resolution = uvec2(
        vec_resolution.x * size_of::<UVec4>() as u32,
        vec_resolution.y,
    );

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
    graph.add_node(FunctionNode::new(
        "compute_levels",
        compute_levels(vec_resolution),
    ));
    graph.add_node(
        FunctionNode::new("copy_to_staging", copy_to_staging(output_resolution))
            .after("compute_levels"),
    );

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

    let mut compiled = graph.compile(&pipelines, None).unwrap();

    let mut resources = RenderResources::new();

    let ascii = context
        .buffer()
        .size(size_of::<[UVec4; 16]>() as u64)
        .copy_dst()
        .uniform()
        .create();
    let ascii_data: Vec<u8> = include_str!("../assets/text/levels.txt")
        .chars()
        .filter(|&c| c != '\n')
        .map(u32::from)
        .map(u8::try_from)
        .map(Result::unwrap)
        .collect();
    let ascii_data: [UVec4; 256 / (4 * 4)] = bytemuck::cast_slice(&ascii_data).try_into().unwrap();
    let mut ascii_uniform = UniformBuffer::new(vec![]);
    ascii_uniform.write(&ascii_data).unwrap();
    context.write_buffer(&ascii, &ascii_uniform);
    resources.insert_buffer("ascii_table", ascii);

    let input = context.texture(
        Some("dog"),
        TextureSize::D2 {
            x: resolution.x,
            y: resolution.y,
        },
        TextureFormat::Rgba8Unorm,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        1,
        1,
    );
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
            bytes_per_row: Some(NonZeroU32::new(4 * resolution.x).unwrap()),
            rows_per_image: None,
        },
        Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        },
    );
    resources.insert_texture("input", input);

    let staging = context
        .buffer()
        .size((output_resolution.x * output_resolution.y) as u64)
        .copy_dst()
        .map_read()
        .create();
    resources.insert_buffer("staging", staging);

    compiled.run(context, &resources).unwrap();

    let staging = resources.get_buffer("staging").unwrap();
    let slice = staging.slice(..);
    let buffer = context.read_map_buffer(&slice);
    let output = StorageBuffer::new(&buffer[..]);
    let mut out: Vec<UVec4> = vec![];
    output.read(&mut out).unwrap();
    let mut bytes = vec![];
    for y in 0..output_resolution.y {
        bytes.extend_from_slice(
            &bytemuck::cast_slice(
                &out[(vec_resolution.x * y) as usize..][..vec_resolution.x as usize],
            )[..resolution.x as usize],
        );
        bytes.push(b'\n');
    }

    std::fs::write("assets/text/output.txt", bytes).unwrap();

    println!("Image conversion success!");
}

fn compute_levels(vec_resolution: UVec2) -> impl Fn(&mut RenderCommands) {
    move |commands| {
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
            .dispatch(vec_resolution.x, vec_resolution.y, 1);
    }
}

fn copy_to_staging(output_resolution: UVec2) -> impl Fn(&mut RenderCommands) {
    move |commands| {
        let buffer = commands.buffer("output");
        let staging = commands.buffer("staging");
        commands.copy_buffer_to_buffer(
            buffer,
            0,
            staging,
            0,
            (output_resolution.x * output_resolution.y) as u64,
        );
    }
}

/* Helpers because stdlib has a bunch of unstable goodies that I can't use >:( */
pub const fn div_ceil(lhs: u32, rhs: u32) -> u32 {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 && rhs > 0 {
        d + 1
    } else {
        d
    }
}

pub fn flatten_slice<T, const N: usize>(slice: &[[T; N]]) -> &[T] {
    let len = if size_of::<T>() == 0 {
        slice.len().checked_mul(N).expect("slice len overflow")
    } else {
        // SAFETY: `self.len() * N` cannot overflow because `self` is
        // already in the address space.
        slice.len().checked_mul(N).unwrap()
    };
    // SAFETY: `[T]` is layout-identical to `[T; N]`
    unsafe { std::slice::from_raw_parts(slice.as_ptr().cast(), len) }
}
