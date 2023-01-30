// webgpu requires an alignment of 16 for arrays in uniform space because *Reasons*
struct AlignedU32 {
    @size(16) value: u32,
}

// Contains ASCII codes for each character corresponding to a lightness value
// which should be used as an index
@group(0) @binding(0)
var<uniform> ascii_table: array<AlignedU32, 256>;

// Contains RGB data
@group(0) @binding(1)
var input: texture_2d<f32>;

// Output ascii values. Assume size is equivalent to `width * height` of input
@group(0) @binding(2)
var<storage, read_write> output: array<AlignedU32>;

// Returns the L component of the HSL form of the input color
fn lightness(rgb: vec4<f32>) -> f32 {
    let max = max(rgb.r, max(rgb.b, rgb.g));
    let min = min(rgb.r, min(rgb.b, rgb.g));

    return min + ((max - min) / 2.0);
}

@compute
@workgroup_size(1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>, 
    @builtin(local_invocation_id) local_id: vec3<u32>
) {
    let width = u32(textureDimensions(input).x);
    let coords = vec2(global_id.x, global_id.y * 2u);
    let l0 = lightness(textureLoad(input, coords, 0));
    let l1 = lightness(textureLoad(input, coords + vec2(0u, 1u), 0));
    let l = u32(((l0 + l1) / 2.0) * 255.0);
    let index = global_id.x + (global_id.y * width);
    output[index] = ascii_table[l];
}