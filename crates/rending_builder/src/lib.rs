#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]
#![warn(rustdoc::all)]
#![doc = include_str!("../README.md")]

pub mod buffer;
pub mod sampler;
pub mod tex_ext;
pub mod texture;

/// Exposes all extension traits at once for use.
/// This is the most convenient way to use the library.
pub mod prelude {
    use super::*;
    pub use buffer::BufferExt;
    pub use sampler::SamplerExt;
    pub use tex_ext::{TextureBindingExt, TextureViewExt};
    pub use texture::TextureExt;
}
