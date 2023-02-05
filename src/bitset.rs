use std::fmt::Debug;

use smallvec::SmallVec;

// TODO: Do performance comparisons of this with various lengths of smallvec and `FixedBitset`
#[derive(Clone)]
pub struct Bitset {
    words: SmallVec<[u64; 1]>,
    len: usize,
}

impl Bitset {
    pub fn new(len: usize) -> Self {
        Self {
            words: std::iter::repeat(0)
                .take(Self::compute_word_len(len))
                .collect(),
            len,
        }
    }

    pub fn resize(&mut self, len: usize) {
        if len <= self.len {
            return;
        }

        let old_word_len = self.word_len();
        let new_word_len = Self::compute_word_len(len);
        for _ in old_word_len..new_word_len {
            self.words.push(0);
        }
        self.len = len;
    }

    fn word_len(&self) -> usize {
        Self::compute_word_len(self.len)
    }

    fn compute_word_len(len: usize) -> usize {
        len / 64 + if len % 64 != 0 { 1 } else { 0 }
    }

    pub fn contains(&self, index: usize) -> Option<bool> {
        if index >= self.len {
            return None;
        }

        let word = self.words[index / 64];
        let bit = word & ((1 << 63) >> (index % 64));
        Some(bit != 0)
    }

    pub fn insert(&mut self, index: usize) {
        if index >= self.len {
            self.resize(index + 1);
        }

        let word = &mut self.words[index / 64];
        let mask = (1 << 63) >> (index % 64);
        *word |= mask;
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }

        let word = &mut self.words[index / 64];
        let mask = (1 << 63) >> (index % 64);
        let value = *word & mask != 0;
        *word &= !mask;
        value
    }

    pub fn invert(&mut self) {
        for i in 0..self.word_len() {
            self.words[i] ^= !0;
        }
    }

    pub fn inverted(&self) -> Bitset {
        let mut new = self.clone();

        for i in 0..new.word_len() {
            new.words[i] ^= !0;
        }

        new
    }

    pub fn union_with(&mut self, other: &Bitset) {
        if self.len < other.len {
            self.resize(other.len);
        }

        for i in 0..self.word_len() {
            self.words[i] |= other.words[i];
        }
    }

    pub fn difference_with(&mut self, other: &Bitset) {
        if self.len < other.len {
            self.resize(other.len)
        }

        for i in 0..self.word_len() {
            self.words[i] &= !other.words[i];
        }
    }

    pub fn intersects_with(&self, other: &Bitset) -> bool {
        for i in 0..self.word_len().min(other.word_len()) {
            if self.words[i] & other.words[i] != 0 {
                return true;
            }
        }
        false
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
        loop {
            match self.bitset.contains(self.index) {
                Some(val) => {
                    if val {
                        self.index += 1;
                        return Some(self.index - 1);
                    }
                }
                None => return None,
            }
            self.index += 1;
        }
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
    x.insert(1);
    x.insert(3);

    assert_eq!(&format!("{x:?}")[..], "Bitset { 0101 }");

    let mut y = Bitset::new(8);
    y.insert(6);
    y.insert(7);

    assert_eq!(&format!("{y:?}")[..], "Bitset { 00000011 }");

    x.union_with(&y);

    assert_eq!(&format!("{x:?}")[..], "Bitset { 01010011 }");
}

#[test]
fn bitset_iter() {
    let mut bitset = Bitset::new(8);
    bitset.insert(0);
    bitset.insert(2);
    bitset.insert(5);
    bitset.insert(6);
    bitset.insert(7);

    let mut string = "".to_string();
    for elem in bitset.iter() {
        string.push_str(&elem.to_string()[..]);
    }

    assert_eq!(&string[..], "02567");
}
