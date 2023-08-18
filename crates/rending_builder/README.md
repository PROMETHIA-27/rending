# rending_builder

This crate provides helper builder APIs for use with WGPU. It currently provides APIs for:
- building buffers, textures, and samplers
- building texture views tersely

Here are some examples:
```rust
let buffer = device
    .label("some data")
    .buffer()
    .size(128)
    .uniform()
    .copy_dst()
    .finish();
```

```rust
let texture = device
    .texture()
    .size(128 * 128)
    .format(TextureFormat::Rgba8Unorm)
    .dimension(TextureDimension::D2)
    .texture_binding()
    .copy_dst()
    .finish();
```

```rust
// create a BindingResource to create bind groups with quickly
let binding = texture.as_entire().binding();

// customize a little more, for less straightforward usecases
let binding = texture
    .view_builder()
    .aspect(TextureAspect::Stencil)
    .dimension(TextureViewDimension::D1)
    .mip_levels(2, Some(1))
    .finish();
```