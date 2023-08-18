//! This module includes the [`SamplerExt`] extension which allows creation of samplers with a [`Device`]
//! via the builder pattern.

use wgpu::{
    AddressMode, CompareFunction, Device, FilterMode, Label, Sampler, SamplerBorderColor,
    SamplerDescriptor,
};

/// The extension trait which gives [`Device`] a method to create samplers using the builder pattern.
pub trait SamplerExt {
    /// Create a sampler using the builder pattern.
    ///
    /// 1. First, call this.
    /// 2. Then, call any of the following methods in a chain, like so:
    /// ```rust
    /// device
    /// .buffer()
    /// // None of these are mandatory
    /// .label("foo")
    /// .address_mode(AddressMode::...)
    /// .address_mode_u(AddressMode::...)
    /// .address_mode_v(AddressMode::...)
    /// .address_mode_w(AddressMode::...)
    /// .mag_filter(FilterMode::...)
    /// .min_filter(FilterMode::...)
    /// .mipmap_filter(FilterMode::...)
    /// .lod_min_clamp(n)
    /// .lod_max_clamp(n)
    /// .compare(CompareFunction::...)
    /// .anisotropy_clamp(n)
    /// .border_color(SamplerBorderColor::...)
    /// ```
    /// 3. Finally, call `.finish()` at the end of the chain. This will produce the sampler.
    ///
    /// See each method for more details on what settings they affect.
    fn sampler(&self) -> SamplerBuilder;
}

impl SamplerExt for Device {
    fn sampler(&self) -> SamplerBuilder {
        SamplerBuilder {
            device: self,
            label: None,
            address_mode_u: AddressMode::default(),
            address_mode_v: AddressMode::default(),
            address_mode_w: AddressMode::default(),
            mag_filter: FilterMode::default(),
            min_filter: FilterMode::default(),
            mipmap_filter: FilterMode::default(),
            lod_min_clamp: 0.0,
            lod_max_clamp: f32::INFINITY,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

/// A builder that creates samplers.
///
/// See its methods or [`SamplerExt::sampler()`](SamplerExt::sampler()) for more information.
///
/// (`SamplerExt` is implemented for [`wgpu::Device`])
pub struct SamplerBuilder<'d, 'l> {
    device: &'d Device,
    label: Label<'l>,
    address_mode_u: AddressMode,
    address_mode_v: AddressMode,
    address_mode_w: AddressMode,
    mag_filter: FilterMode,
    min_filter: FilterMode,
    mipmap_filter: FilterMode,
    lod_min_clamp: f32,
    lod_max_clamp: f32,
    compare: Option<CompareFunction>,
    anisotropy_clamp: u16,
    border_color: Option<SamplerBorderColor>,
}

impl<'d, 'l> SamplerBuilder<'d, 'l> {
    /// Add a label to the buffer, which can be visible in debugging tools.
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    /// Sets address_mode_u, address_mode_v, and address_mode_w to the given value.
    ///
    /// The address mode is how to deal with an out of bounds read from a texture. u is the horizontal
    /// direction, v is vertical, and w is depth.
    pub fn address_mode(mut self, mode: AddressMode) -> Self {
        self.address_mode_u = mode;
        self.address_mode_v = mode;
        self.address_mode_w = mode;
        self
    }

    /// Set the address mode for the u direction. See [`address_mode()`] for more information.
    pub fn address_mode_u(mut self, mode: AddressMode) -> Self {
        self.address_mode_u = mode;
        self
    }

    /// Set the address mode for the v direction. See [`address_mode()`] for more information.
    pub fn address_mode_v(mut self, mode: AddressMode) -> Self {
        self.address_mode_v = mode;
        self
    }

    /// Set the address mode for the w direction. See [`address_mode()`] for more information.
    pub fn address_mode_w(mut self, mode: AddressMode) -> Self {
        self.address_mode_w = mode;
        self
    }

    /// Set the filter mode to use when magnifying a texture. For a more pixelated style, the nearest
    /// neighbor filter mode is probably best, as it will not blur the image. For most other styles,
    /// linear will be better, as it will naturally smooth detail that could otherwise look ugly to
    /// scale up.
    pub fn mag_filter(mut self, filter: FilterMode) -> Self {
        self.mag_filter = filter;
        self
    }

    /// Set the filter mode to use when minifying a texture. This is not as important as the mag filter,
    /// as the image will *mostly* look the same with any filter. However, for a pixelated art style
    /// it will still probably be best to use nearest neighbor.
    pub fn min_filter(mut self, filter: FilterMode) -> Self {
        self.min_filter = filter;
        self
    }

    /// Set the filter mode to use for blending between mipmap layers. See
    /// [`TextureBuilder::mipmap_count`](crate::texture::TextureBuilder::mip_level_count()) for
    /// more information about mipmaps.
    pub fn mipmap_filter(mut self, filter: FilterMode) -> Self {
        self.mipmap_filter = filter;
        self
    }

    /// Set the minimum level of detail (i.e. mipmap level) to use. This allows preventing a texture
    /// from being made too low resolution even if it would otherwise be sampled at a very
    /// low level of detail.
    pub fn lod_min_clamp(mut self, clamp: f32) -> Self {
        self.lod_min_clamp = clamp;
        self
    }

    /// Set the maximum level of detail (i.e. mipmap level) to use. This allows preventing a texture
    /// from being made too high resolution even if it would otherwise be sampled at a very
    /// high level of detail.
    pub fn lod_max_clamp(mut self, clamp: f32) -> Self {
        self.lod_max_clamp = clamp;
        self
    }

    /// Set the comparison function for a comparison sampler. The comparison function is used to
    /// compare the values of depth and stencil textures, and "pass" or "fail" pixels in them
    /// based on that comparison. This can be used to do depth testing, for example, which ensures
    /// that objects that are closer to the camera are rendered instead of ones further behind them.
    pub fn compare(mut self, compare: CompareFunction) -> Self {
        self.compare = Some(compare);
        self
    }

    /// Full explanation TODO: I'm not sure exactly what this does, but if must be at least 1 and
    /// if it is greater than 1 then all filters must be linear.
    pub fn anisotropy_clamp(mut self, clamp: u16) -> Self {
        assert!(clamp >= 1, "anisotropy clamp must always be >= 1");
        self.anisotropy_clamp = clamp;
        self
    }

    /// Sets the border color chosen when address mode is set to ClampToBorder. See
    /// [`address_mode()`] for more information.
    pub fn border_color(mut self, color: SamplerBorderColor) -> Self {
        self.border_color = Some(color);
        self
    }

    /// Finish constructing and produce the sampler.
    pub fn finish(self) -> Sampler {
        let Self {
            device,
            label,
            address_mode_u,
            address_mode_v,
            address_mode_w,
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_min_clamp,
            lod_max_clamp,
            compare,
            anisotropy_clamp,
            border_color,
        } = self;

        assert!(
            anisotropy_clamp == 1
                || matches!(
                    (mag_filter, min_filter, mipmap_filter),
                    (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear)
                ),
            "if anisotropy clamp is not 1, all filter modes must be linear"
        );

        device.create_sampler(&SamplerDescriptor {
            label,
            address_mode_u,
            address_mode_v,
            address_mode_w,
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_min_clamp,
            lod_max_clamp,
            compare,
            anisotropy_clamp,
            border_color,
        })
    }
}
