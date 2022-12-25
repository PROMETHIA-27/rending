struct big_chunkus {
    data: vec4<u32>,
}

@group(0) @binding(0)
var<uniform> offset: big_chunkus;
@group(2) @binding(1)
var tex: texture_2d_array<f32>;

@compute @workgroup_size(1, 1, 1)
fn main() {
    _ = offset;
    // _ = tex;
}