use std::borrow::Cow;
use std::collections::BTreeMap;

use slotmap::{Key, SlotMap};

#[derive(Clone, Debug)]
pub struct NamedSlotMap<K: Key, V> {
    slotmap: SlotMap<K, V>,
    names: BTreeMap<Cow<'static, str>, K>,
}

impl<K: Key, V> NamedSlotMap<K, V> {
    pub fn new() -> Self {
        Self {
            slotmap: SlotMap::with_key(),
            names: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.slotmap.len()
    }

    pub fn insert(&mut self, name: impl Into<Cow<'static, str>>, value: V) -> K {
        let key = self.slotmap.insert(value);
        // TODO: This is silently "working" in the case that something was already there
        self.names.insert(name.into(), key);
        key
    }

    pub fn get(&self, key: K) -> Option<&V> {
        self.slotmap.get(key)
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.slotmap.get_mut(key)
    }

    pub fn get_key(&self, name: &str) -> Option<K> {
        self.names.get(name).cloned()
    }

    pub fn get_named(&self, name: &str) -> Option<&V> {
        let &key = self.names.get(name)?;
        self.slotmap.get(key)
    }

    /// Get the name of the value assigned to a given key.
    ///
    /// **WARNING**: This is an expensive function. This should only be used on cold paths, such as errors and such.
    pub fn get_name(&self, key: K) -> Option<&str> {
        self.names
            .iter()
            .find(|(_, &k)| k == key)
            .map(|(name, _)| &name[..])
    }

    pub fn get_key_value(&self, name: &str) -> Option<(K, &V)> {
        let &handle = self.names.get(name)?;
        let value = self.slotmap.get(handle)?;
        Some((handle, value))
    }

    pub fn iter_key_value(&self) -> impl Iterator<Item = (K, &V)> {
        self.slotmap.iter()
    }

    pub fn iter_key_value_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.slotmap.iter_mut()
    }

    pub fn iter_keys(&self) -> impl Iterator<Item = K> + '_ {
        self.slotmap.keys()
    }

    pub fn iter_values(&self) -> impl Iterator<Item = &V> {
        self.slotmap.values()
    }

    pub fn drain_key_value(&mut self) -> impl Iterator<Item = (K, V)> + '_ {
        self.slotmap.drain()
    }

    pub fn iter_names(&self) -> impl Iterator<Item = (&str, K)> {
        self.names.iter().map(|(name, &key)| (&name[..], key))
    }

    pub fn split_mut(&mut self) -> (KeyMap<K, V>, NameMap<K>) {
        (
            KeyMap {
                map: &mut self.slotmap,
            },
            NameMap {
                map: &mut self.names,
            },
        )
    }
}

pub struct KeyMap<'m, K: Key, V> {
    map: &'m mut SlotMap<K, V>,
}

impl<K: Key, V> KeyMap<'_, K, V> {
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    pub fn iter_key_value(&self) -> impl Iterator<Item = (K, &V)> {
        self.map.iter()
    }

    pub fn iter_key_value_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.map.iter_mut()
    }
}

pub struct NameMap<'m, K: Key> {
    map: &'m mut BTreeMap<Cow<'static, str>, K>,
}

impl<K: Key> NameMap<'_, K> {
    pub fn get(&self, name: &str) -> Option<K> {
        self.map.get(name).copied()
    }
}
