use std::borrow::Cow;
use std::collections::HashSet;
use std::mem::size_of;

use glam::{uvec2, UVec2, UVec4};
use image::EncodableLayout;
use rending::prelude::*;
use wgpu::{
    Backends, BindGroup, Buffer, CommandEncoder, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, DeviceDescriptor, Extent3d, Features, ImageCopyTexture, ImageDataLayout,
    InstanceDescriptor, Limits, MaintainBase, MapMode, Origin3d, PowerPreference,
    RequestAdapterOptions, ShaderSource, TextureAspect, TextureDimension, TextureFormat,
};

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

    let instance = wgpu::Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });

    let adapter =
        futures_lite::future::block_on(instance.request_adapter(&RequestAdapterOptions {
            force_fallback_adapter: false,
            compatible_surface: None,
            power_preference: PowerPreference::HighPerformance,
        }))
        .unwrap();

    let (device, queue) = futures_lite::future::block_on(adapter.request_device(
        &DeviceDescriptor {
            features: Features::empty(),
            limits: Limits::downlevel_defaults(),
            label: None,
        },
        None,
    ))
    .unwrap();

    let start = std::time::Instant::now();

    let pipeline = ReflectedComputePipeline::new(
        &device,
        ShaderSource::Wgsl(Cow::Borrowed(include_str!(
            "../assets/shaders/image_to_ascii.wgsl"
        ))),
        "main",
        &HashSet::default(),
        Some("compute_levels"),
    )
    .unwrap();

    let ascii_data: Vec<u8> = include_str!("../assets/text/levels.txt")
        .chars()
        .filter(|&c| c != '\n')
        .map(u32::from)
        .map(u8::try_from)
        .map(Result::unwrap)
        .collect();

    let ascii_table = device
        .buffer()
        .mapped()
        .data(&ascii_data)
        .uniform()
        .copy_dst()
        .finish();

    let input = device
        .texture()
        .label("input image")
        .size(Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        })
        .dimension(TextureDimension::D2)
        .format(TextureFormat::Rgba8Unorm)
        .texture_binding()
        .copy_dst()
        .finish();

    queue.write_texture(
        ImageCopyTexture {
            texture: &input,
            mip_level: 0,
            origin: Origin3d::default(),
            aspect: TextureAspect::default(),
        },
        img.into_rgba8().as_bytes(),
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * resolution.x),
            rows_per_image: None,
        },
        Extent3d {
            width: resolution.x,
            height: resolution.y,
            depth_or_array_layers: 1,
        },
    );

    let staging_size = (output_resolution.x * output_resolution.y) as u64;

    let staging = device
        .buffer()
        .size(staging_size)
        .copy_dst()
        .map_read()
        .finish();

    let output = device
        .buffer()
        .label("output")
        .size(staging_size)
        .storage()
        .copy_src()
        .finish();

    let mut commands = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    let bindgroup = pipeline
        .bind_group(
            &device,
            None,
            0,
            [
                (0, ascii_table.as_entire_binding()),
                (1, input.as_entire().binding()),
                (2, output.as_entire_binding()),
            ],
        )
        .unwrap();

    let exe_start = std::time::Instant::now();

    compute_levels(&mut commands, vec_resolution, pipeline.pipeline, bindgroup);
    copy_to_staging(&mut commands, output_resolution, &output, &staging);

    let commands = commands.finish();
    queue.submit([commands]);

    let exe = std::time::Instant::now()
        .duration_since(exe_start)
        .as_micros();

    let slice = staging.slice(..);
    slice.map_async(MapMode::Read, |_| ());
    device.poll(MaintainBase::Wait);
    let buffer = slice.get_mapped_range();
    let out: &[UVec4] = bytemuck::cast_slice(&buffer);

    let mut bytes = vec![];
    for y in 0..output_resolution.y {
        bytes.extend_from_slice(
            &bytemuck::cast_slice(
                &out[(vec_resolution.x * y) as usize..][..vec_resolution.x as usize],
            )[..resolution.x as usize],
        );
        bytes.push(b'\n');
    }

    std::fs::write("../../assets/text/output.txt", bytes).unwrap();

    let duration = std::time::Instant::now().duration_since(start).as_micros();

    println!(
        "Image conversion success! Took {duration}us total after init, and {exe}us to execute"
    );
}

fn compute_levels(
    commands: &mut CommandEncoder,
    vec_resolution: UVec2,
    pipeline: ComputePipeline,
    bindgroup: BindGroup,
) {
    let mut pass = commands.begin_compute_pass(&ComputePassDescriptor { label: None });
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &bindgroup, &[]);
    pass.dispatch_workgroups(vec_resolution.x, vec_resolution.y, 1);
}

fn copy_to_staging(
    commands: &mut CommandEncoder,
    output_resolution: UVec2,
    output: &Buffer,
    staging: &Buffer,
) {
    commands.copy_buffer_to_buffer(
        output,
        0,
        staging,
        0,
        (output_resolution.x * output_resolution.y) as u64,
    );
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
