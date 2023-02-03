// Contains ASCII codes for each character corresponding to a lightness value
// which should be used as an index
@group(0) @binding(0)
var<uniform> ascii_table: array<vec4<u32>, 16>;

// Contains RGB data
@group(0) @binding(1)
var input: texture_2d<f32>;

// Output ascii values. Assume size is equivalent to `(width * height) / 16`  of input
@group(0) @binding(2)
var<storage, read_write> output: array<vec4<u32>>;

// Returns the L component of the HSL form of the input color
fn lightness(rgb: vec4<f32>) -> f32 {
    let max = max(rgb.r, max(rgb.b, rgb.g));
    let min = min(rgb.r, min(rgb.b, rgb.g));

    return min + ((max - min) / 2.0);
}

@compute
@workgroup_size(1)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
) {
    let width = u32(textureDimensions(input).x);
    let base_coords = vec2(id.x, id.y * 2u);

    var out_ints = output[id.x];

    for (var i = 0u; i < 4u; i++) {
        var out_chars = unpack4x8unorm(out_ints)[i];

        for (var j = 0u; j < 4u; j++) {
            let tex_coords = base_coords + vec2((4u * i) + j, 0u);

            let l0 = lightness(textureLoad(input, tex_coords, 0));
            let l1 = lightness(textureLoad(input, tex_coords + vec2(0u, 1u), 0));
            let l = u32(((l0 + l1) / 2.0) * 255.0);

            let ascii_arr_index = l / 16u;
            let ascii_ints_index = (l % 16u) / 4u;
            let ascii_chars_index = l % 4u;
            let char = unpack4x8unorm(
                ascii_table[ascii_arr_index][ascii_ints_index]
            )[ascii_chars_index];

            out_chars[j] = char;
        }

        out_ints[i] = pack4x8unorm(out_chars);
    }
    output[id.x] = out_ints;
}