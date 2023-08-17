use wgpu::{
    AddressMode, CompareFunction, Device, FilterMode, Label, Sampler, SamplerBorderColor,
    SamplerDescriptor,
};

pub trait SamplerExt {
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
    pub fn label(mut self, label: &'l str) -> Self {
        self.label = Some(label);
        self
    }

    /// Sets address_mode_u, address_mode_v, and address_mode_w to the given value.
    ///
    /// See the methods for each one for more information.
    pub fn address_mode(mut self, mode: AddressMode) -> Self {
        self.address_mode_u = mode;
        self.address_mode_v = mode;
        self.address_mode_w = mode;
        self
    }

    pub fn address_mode_u(mut self, mode: AddressMode) -> Self {
        self.address_mode_u = mode;
        self
    }

    pub fn address_mode_v(mut self, mode: AddressMode) -> Self {
        self.address_mode_v = mode;
        self
    }

    pub fn address_mode_w(mut self, mode: AddressMode) -> Self {
        self.address_mode_w = mode;
        self
    }

    pub fn mag_filter(mut self, filter: FilterMode) -> Self {
        self.mag_filter = filter;
        self
    }

    pub fn min_filter(mut self, filter: FilterMode) -> Self {
        self.min_filter = filter;
        self
    }

    pub fn mipmap_filter(mut self, filter: FilterMode) -> Self {
        self.mipmap_filter = filter;
        self
    }

    pub fn lod_min_clamp(mut self, clamp: f32) -> Self {
        self.lod_min_clamp = clamp;
        self
    }

    pub fn lod_max_clamp(mut self, clamp: f32) -> Self {
        self.lod_max_clamp = clamp;
        self
    }

    pub fn compare(mut self, compare: CompareFunction) -> Self {
        self.compare = Some(compare);
        self
    }

    pub fn anisotropy_clamp(mut self, clamp: u16) -> Self {
        assert!(clamp >= 1, "anisotropy clamp must always be >= 1");
        self.anisotropy_clamp = clamp;
        self
    }

    pub fn border_color(mut self, color: SamplerBorderColor) -> Self {
        self.border_color = Some(color);
        self
    }

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
