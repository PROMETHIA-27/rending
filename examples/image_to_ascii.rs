use std::collections::HashSet;
use std::mem::size_of;
use std::num::NonZeroU32;

use encase::{impl_vector, ShaderType, StorageBuffer, UniformBuffer};
use glam::{uvec2, uvec4, UVec2, UVec4, Vec4Swizzles};
use image::EncodableLayout;
use rending::*;
use wgpu::{ImageCopyTexture, TextureAspect, TextureUsages};

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

    let img = image::io::Reader::open("assets/images/dog.jpg")
        .unwrap()
        .decode()
        .unwrap();
    let resolution = uvec2(img.width(), img.height());

    let mut graph = RenderGraph::new();
    graph.add_node(FunctionNode::new(
        "compute_levels",
        compute_levels(resolution),
    ));
    graph.add_node(
        FunctionNode::new("copy_to_staging", copy_to_staging(resolution)).after("compute_levels"),
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

    let mut compiled = graph.compile(&pipelines).unwrap();

    let mut resources = RenderResources::new();

    let ascii = context
        .buffer()
        .size(size_of::<[UVec4; 16]>() as u64)
        .copy_dst()
        .uniform()
        .create();
    let ascii_data: [UVec4; 16] = include_str!("../assets/text/levels.txt")
        .chars()
        .filter(|&c| c != '\n')
        .map(u32::from)
        .map(u8::try_from)
        .map(Result::unwrap)
        .collect::<Vec<u8>>()
        .chunks_exact(16)
        .map(|chunk| {
            UVec4::from_slice(
                &chunk
                    .chunks_exact(4)
                    .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
                    .collect::<Vec<u32>>(),
            )
        })
        .collect::<Vec<UVec4>>()
        .try_into()
        .unwrap_or_else(|_| panic!());
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

    let output_res = uvec2(
        div_ceil(resolution.x, size_of::<UVec4>() as u32) * size_of::<UVec4>() as u32,
        resolution.y / 2,
    );
    let staging = context
        .buffer()
        .size((output_res.x * output_res.y) as u64)
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
    for y in 0..output_res.y {
        bytes.extend(
            out[(output_res.x * y) as usize..][..output_res.x as usize]
                .into_iter()
                .flat_map(UVec4::to_array)
                .flat_map(u32::to_ne_bytes)
                .take(resolution.x as usize),
        );
        bytes.push(b'\n');
    }

    std::fs::write("assets/text/output.txt", bytes).unwrap();
}

fn compute_levels(resolution: UVec2) -> impl Fn(&mut RenderCommands) {
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
            .dispatch(resolution.x, resolution.y, 1);
    }
}

fn copy_to_staging(resolution: UVec2) -> impl Fn(&mut RenderCommands) {
    move |commands| {
        let buffer = commands.buffer("output");
        let staging = commands.buffer("staging");
        commands.copy_buffer_to_buffer(
            buffer,
            0,
            staging,
            0,
            (resolution.x * (resolution.y / 2)) as u64,
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
