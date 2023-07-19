use std::mem::MaybeUninit;

pub trait U8IterExt: Iterator<Item = u8> + Sized {
    fn to_u32_iter(self) -> U8ToU32Iterator<Self>;
}

impl<I: Iterator<Item = u8>> U8IterExt for I {
    fn to_u32_iter(self) -> U8ToU32Iterator<Self> {
        U8ToU32Iterator { iter: self }
    }
}

pub struct U8ToU32Iterator<I: Iterator<Item = u8>> {
    iter: I,
}

// TODO: Replace with try_from_fn
impl<I: Iterator<Item = u8>> Iterator for U8ToU32Iterator<I> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let chunk: [Option<u8>; 4] = std::array::from_fn(|_| self.iter.next());
        let chunk: [u8; 4] = array_from_iter(
            &mut chunk
                .into_iter()
                .take_while(Option::is_some)
                .map(Option::unwrap),
        )?;
        Some(u32::from_le_bytes(chunk))
    }
}

// Will be removed when try_from_fn goes in
#[allow(clippy::needless_range_loop)]
fn array_from_iter<T, I: Iterator<Item = T>, const N: usize>(iter: &mut I) -> Option<[T; N]> {
    // SAFETY:
    // - This is very scary, but supported by the docs!
    // https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#initializing-an-array-element-by-element
    let mut uninit: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };

    for i in 0..N {
        match iter.next() {
            Some(next) => uninit[i] = MaybeUninit::new(next),
            None => {
                for j in 0..i {
                    // SAFETY:
                    // - We know that this element is initialized, as it came prior in the loop
                    // - i is uninit[i] is *not* initialized because that would've happened only if we *didn't*
                    //   hit this case
                    unsafe { uninit[j].assume_init_drop() };
                }
                return None;
            }
        }
    }

    // SAFETY:
    // - Must use transmute_copy due to a compiler bug related to transmute's size check
    // - The types have the same size
    // - The array is fully initialized and thus valid for [T; N]
    Some(unsafe { std::mem::transmute_copy::<[MaybeUninit<T>; N], [T; N]>(&uninit) })
}

pub trait IterCombinations {
    type Item;

    fn iter_combinations(&self) -> Combinations<Self::Item>;
}

pub struct Combinations<'a, T> {
    slice: &'a [T],
    left_index: usize,
    right_index: usize,
}

impl<'a, T> IterCombinations for &'a [T] {
    type Item = T;

    fn iter_combinations(&self) -> Combinations<Self::Item> {
        Combinations {
            slice: self,
            left_index: 0,
            right_index: 0,
        }
    }
}

impl<'a, T> Iterator for Combinations<'a, T> {
    type Item = (&'a T, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.right_index == self.left_index {
            self.right_index += 1;
        }

        if self.right_index == self.slice.len() {
            self.left_index += 1;
            self.right_index = 0;
        }

        let val = if self.left_index == self.slice.len() {
            return None;
        } else {
            Some((&self.slice[self.left_index], &self.slice[self.right_index]))
        };

        self.right_index += 1;

        val
    }
}
