@group(0) @binding(0)
var<storage, read_write> chunk: vec4<f32>;
@group(0) @binding(1)
var tex: texture_2d<f32>;

@compute @workgroup_size(1)
fn main() {
    chunk = textureLoad(tex, vec2(0, 0), 0);
}