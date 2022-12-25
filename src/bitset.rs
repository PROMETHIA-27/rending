use std::fmt::Debug;
use std::ops::Index;

use smallvec::SmallVec;

#[derive(Clone)]
pub struct Bitset {
    words: SmallVec<[u64; 1]>,
    len: usize,
}

impl Bitset {
    pub fn new(len: usize) -> Self {
        let word_len = len / 64 + if len % 64 != 0 { 1 } else { 0 };
        Self {
            words: std::iter::repeat(0).take(word_len).collect(),
            len,
        }
    }

    pub fn resize(&mut self, len: usize) {
        if len <= self.len {
            return;
        }

        let old_word_len = self.word_len();
        let new_word_len = len / 64 + if len % 64 != 0 { 1 } else { 0 };
        for _ in old_word_len..new_word_len {
            self.words.push(0);
        }
        self.len = len;
    }

    fn word_len(&self) -> usize {
        let len = self.len;
        len / 64 + if len % 64 != 0 { 1 } else { 0 }
    }

    pub fn contains(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }

        let word = self.words[index / 64];
        let bit = word & ((1 << 63) >> (index % 64));
        Some(bit == 1)
    }

    pub fn insert(&mut self, index: usize) -> Result<(), ()> {
        if index >= self.len {
            return Err(());
        }

        let word = &mut self.words[index / 64];
        let mask = (1 << 63) >> (index % 64);
        *word |= mask;
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<(), ()> {
        if index >= self.len {
            return Err(());
        }

        let word = &mut self.words[index / 64];
        let mask = (1 << 63) >> (index % 64);
        *word &= !mask;
        Ok(())
    }

    pub fn union_with(&mut self, other: &Bitset) {
        if self.len < other.len {
            self.resize(other.len);
        }

        for i in 0..self.word_len() {
            self.words[i] |= other.words[i];
        }
    }

    pub fn iter(&self) -> Iter {
        Iter {
            bitset: self,
            index: 0,
        }
    }
}

impl Debug for Bitset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bitstring = String::new();
        let word_len = self.word_len();
        for &word in &self.words[..word_len - 1] {
            bitstring.push_str(&format!("{:064b}", word)[..]);
        }
        if word_len > 0 {
            for char in format!("{:064b}", self.words[word_len - 1])
                .chars()
                .take(self.len % 64)
            {
                bitstring.push(char);
            }
        }
        f.write_fmt(format_args!("Bitset {{ {} }}", bitstring))
    }
}

pub struct Iter<'a> {
    bitset: &'a Bitset,
    index: usize,
}

impl Iterator for Iter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.bitset.len {
            return None;
        }
        let val = if self.bitset.contains(self.index).unwrap() {
            Some(self.index)
        } else {
            None
        };
        self.index += 1;
        val
    }
}

#[test]
fn bitset_fmt() {
    let mut bitset = Bitset::new(8);
    bitset.insert(0);
    bitset.insert(5);
    bitset.insert(7);
    assert_eq!(&format!("{bitset:?}")[..], "Bitset { 10000101 }");
}

#[test]
fn bitset_union() {
    let mut x = Bitset::new(4);
    x.insert(1).unwrap();
    x.insert(3).unwrap();

    assert_eq!(&format!("{x:?}")[..], "Bitset { 0101 }");

    let mut y = Bitset::new(8);
    y.insert(6).unwrap();
    y.insert(7).unwrap();

    assert_eq!(&format!("{y:?}")[..], "Bitset { 00000011 }");

    x.union_with(&y);

    assert_eq!(&format!("{x:?}")[..], "Bitset { 01010011 }");
}
