use std::num::NonZeroU8;

use fixed::types::extra::U26;
use fixed::FixedU32;
use slotmap::{new_key_type, SecondaryMap};
use thiserror::Error;
use wgpu::{AddressMode, CompareFunction, FilterMode, SamplerBindingType, SamplerBorderColor};

use super::ResourceBinding;

new_key_type! { pub struct SamplerHandle; }

impl SamplerHandle {
    pub fn bind(self) -> ResourceBinding {
        ResourceBinding::Sampler { handle: self }
    }
}

#[derive(Debug)]
pub struct Sampler {
    pub(crate) wgpu: wgpu::Sampler,
    pub(crate) address_mode_u: AddressMode,
    pub(crate) address_mode_v: AddressMode,
    pub(crate) address_mode_w: AddressMode,
    pub(crate) mag_filter: FilterMode,
    pub(crate) min_filter: FilterMode,
    pub(crate) mipmap_filter: FilterMode,
    pub(crate) lod_min_clamp: f32,
    pub(crate) lod_max_clamp: f32,
    pub(crate) compare: Option<CompareFunction>,
    pub(crate) anisotropy_clamp: Option<NonZeroU8>,
    pub(crate) border_color: Option<SamplerBorderColor>,
}

impl Sampler {
    pub fn is_filtering(&self) -> bool {
        match (self.mag_filter, self.min_filter, self.mipmap_filter) {
            (FilterMode::Nearest, FilterMode::Nearest, FilterMode::Nearest) => false,
            _ => true,
        }
    }

    pub fn is_comparison(&self) -> bool {
        self.compare.is_some()
    }
}

pub(crate) enum SamplerBinding<'s> {
    Retained(&'s Sampler),
    Transient(Sampler),
}

impl<'s> AsRef<Sampler> for SamplerBinding<'s> {
    fn as_ref(&self) -> &Sampler {
        match self {
            SamplerBinding::Retained(texture) => texture,
            SamplerBinding::Transient(texture) => texture,
        }
    }
}

pub(crate) type SamplerBindings<'s> = SecondaryMap<SamplerHandle, SamplerBinding<'s>>;

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) enum SamplerTypeConstraint {
    Constrained(SamplerBindingType),
    Unconstrained,
    Conflicted(SamplerBindingType, SamplerBindingType),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct SamplerConstraints {
    pub(crate) address_modes: [Option<AddressMode>; 3],
    pub(crate) mag_filter: Option<FilterMode>,
    pub(crate) min_filter: Option<FilterMode>,
    pub(crate) mipmap_filter: Option<FilterMode>,
    // These are fixed so as to play nice with hash/eq.
    // They use 6 bits for integer and 26 for fraction, and each
    // fraction is a multiple of 1/2^26. That should be both
    // sufficient layers (up to 64, with a size difference between layers 0 and 63 of 18446744073700000000)
    // and sufficient fractional precision (representing any multiple of 0.0000000149011611938), which is quite close
    // to maximum float precision
    pub(crate) lod_min_clamp: FixedU32<U26>,
    pub(crate) lod_max_clamp: FixedU32<U26>,
    pub(crate) compare: Option<CompareFunction>,
    pub(crate) anisotropy_clamp: Option<NonZeroU8>,
    pub(crate) border_color: Option<SamplerBorderColor>,
    pub(crate) ty: SamplerTypeConstraint,
}

impl SamplerConstraints {
    pub fn set_type(&mut self, ty: SamplerBindingType) {
        match self.ty {
            SamplerTypeConstraint::Constrained(old) => match (old, ty) {
                (SamplerBindingType::NonFiltering, SamplerBindingType::Filtering) => (),
                (o, n) if o == n => (),
                _ => self.ty = SamplerTypeConstraint::Conflicted(old, ty),
            },
            SamplerTypeConstraint::Unconstrained => {
                self.ty = SamplerTypeConstraint::Constrained(ty)
            }
            SamplerTypeConstraint::Conflicted(_, _) => (),
        }
    }

    pub fn verify(&self, name: &str) {
        match self.ty {
            SamplerTypeConstraint::Constrained(ty) => match ty {
                SamplerBindingType::Filtering => todo!(),
                SamplerBindingType::NonFiltering => {
                    match (self.mag_filter, self.min_filter, self.mipmap_filter) {
                        (
                            None | Some(FilterMode::Nearest),
                            None | Some(FilterMode::Nearest),
                            None | Some(FilterMode::Nearest),
                        ) => todo!(),
                        _ => todo!(),
                    }
                }
                SamplerBindingType::Comparison => todo!(),
            },
            SamplerTypeConstraint::Unconstrained => todo!(),
            SamplerTypeConstraint::Conflicted(_, _) => todo!(),
        }
    }
}

impl Default for SamplerConstraints {
    fn default() -> Self {
        Self {
            address_modes: [None; 3],
            mag_filter: None,
            min_filter: None,
            mipmap_filter: None,
            lod_min_clamp: FixedU32::unwrapped_from_str("0"),
            // TODO: Just keep an eye on this in the spec. Spec currently says 32 is default. Who knows if that'll change.
            // wgpu is still using infinity as default as of now.
            lod_max_clamp: FixedU32::unwrapped_from_str("32"),
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
            ty: SamplerTypeConstraint::Unconstrained,
        }
    }
}

#[derive(Debug, Error)]
pub enum SamplerError {
    // transient

    // retained
    #[error("retained sampler `{0}` does not fulfill its constraints. Expected values: {1:?} | Received values: {2:?}")]
    ConstraintsUnfulfilled(String, SamplerConstraints, SamplerConstraints),
}
