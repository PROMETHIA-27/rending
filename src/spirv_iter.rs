use std::iter::Copied;

use crate::util::{U8IterExt, U8ToU32Iterator};

/// This trait describes how a type can be converted into SPIR-V shader code. Any iterator over `u8`, `u32`,
/// `&u8`, or `&u32` implement `SpirvIterator`. All types that implement [`IntoIterator`] into one of those types
/// are also `SpirvIterator`s.
///
/// See [`ShaderSource`](crate::ShaderSource).
pub trait SpirvIterator {
    type SpirvIter: Iterator<Item = u32>;

    fn into_spirv(self) -> Self::SpirvIter;
}

/// You probably don't need to worry about this trait. It's a hack to get around a long-standing compiler limitation
/// regarding blanket impls with associated types, to make [`SpirvIterator`] work.
pub trait InnerSpirvIterator {
    type This;
    type Iter: Iterator<Item = u32>;

    fn into_spirv_inner(iter: Self::This) -> Self::Iter;
}

impl<I: IntoIterator<Item = u8>> InnerSpirvIterator for (I, u8) {
    type This = I;
    type Iter = U8ToU32Iterator<<I as IntoIterator>::IntoIter>;

    fn into_spirv_inner(iter: I) -> Self::Iter {
        iter.into_iter().to_u32_iter()
    }
}

impl<'a, I: IntoIterator<Item = &'a u8>> InnerSpirvIterator for (I, &'a u8) {
    type This = I;

    type Iter = U8ToU32Iterator<Copied<<I as IntoIterator>::IntoIter>>;

    fn into_spirv_inner(iter: I) -> Self::Iter {
        iter.into_iter().copied().to_u32_iter()
    }
}

impl<I: IntoIterator<Item = u32>> InnerSpirvIterator for (I, u32) {
    type This = I;
    type Iter = <I as IntoIterator>::IntoIter;

    fn into_spirv_inner(iter: I) -> Self::Iter {
        iter.into_iter()
    }
}

impl<'a, I: IntoIterator<Item = &'a u32>> InnerSpirvIterator for (I, &'a u32) {
    type This = I;

    type Iter = Copied<<I as IntoIterator>::IntoIter>;

    fn into_spirv_inner(iter: I) -> Self::Iter {
        iter.into_iter().copied()
    }
}

impl<T, I: IntoIterator<Item = T>> SpirvIterator for I
where
    (I, T): InnerSpirvIterator<This = I>,
{
    type SpirvIter = <(I, T) as InnerSpirvIterator>::Iter;

    fn into_spirv(self) -> Self::SpirvIter {
        <(I, T)>::into_spirv_inner(self)
    }
}
