use std::borrow::Cow;

use naga::{FastHashMap, FastHashSet};
use slotmap::new_key_type;

use crate::commands::RenderCommands;
use crate::resources::{ResourceProvider, ResourceType};

new_key_type! { pub struct NodeKey; }

pub trait RenderNode {
    fn name() -> Cow<'static, str>;

    fn reads() -> Vec<NodeInput> {
        vec![]
    }

    fn writes() -> Vec<NodeOutput> {
        vec![]
    }

    // TODO: Ergonomics of this are garbage. Fix it
    fn before() -> Vec<Cow<'static, str>> {
        vec![]
    }

    fn after() -> Vec<Cow<'static, str>> {
        vec![]
    }

    fn run(commands: &mut RenderCommands, resources: &ResourceProvider);
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeInput {
    pub resource: Cow<'static, str>,
}

impl NodeInput {
    pub fn new(resource: impl Into<Cow<'static, str>>) -> Self {
        Self {
            resource: resource.into(),
        }
    }

    // pub fn retained(resource: impl Into<Cow<'static, str>>) -> Self {
    //     Self {
    //         resource: resource.into(),
    //         source: ResourceSource::Retained,
    //     }
    // }

    // pub fn node(
    //     resource: impl Into<Cow<'static, str>>,
    //     node: impl Into<Cow<'static, str>>,
    // ) -> Self {
    //     Self {
    //         resource: resource.into(),
    //         source: ResourceSource::Node(node.into()),
    //     }
    // }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeOutput {
    pub resource: Cow<'static, str>,
    pub ty: ResourceType,
}

impl NodeOutput {
    pub fn buffer(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            resource: name.into(),
            ty: ResourceType::Buffer,
        }
    }

    pub fn texture(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            resource: name.into(),
            ty: ResourceType::Texture,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum OrderingList {
    Names(FastHashSet<Cow<'static, str>>),
    Keys(FastHashSet<NodeKey>),
}

impl OrderingList {
    pub fn is_names(&self) -> bool {
        match self {
            OrderingList::Names(_) => true,
            OrderingList::Keys(_) => false,
        }
    }

    pub fn is_keys(&self) -> bool {
        match self {
            OrderingList::Names(_) => false,
            OrderingList::Keys(_) => true,
        }
    }

    pub fn unwrap_keys(&self) -> &FastHashSet<NodeKey> {
        match self {
            OrderingList::Keys(keys) => keys,
            _ => panic!("unwrapped names"),
        }
    }

    pub fn unwrap_keys_mut(&mut self) -> &mut FastHashSet<NodeKey> {
        match self {
            OrderingList::Keys(keys) => keys,
            _ => panic!("unwrapped names"),
        }
    }
}

#[derive(Clone)]
pub struct RenderNodeMeta {
    pub(crate) reads: FastHashSet<Cow<'static, str>>,
    pub(crate) writes: FastHashMap<Cow<'static, str>, ResourceType>,
    pub(crate) before: OrderingList,
    pub(crate) after: OrderingList,
    pub(crate) run_fn: fn(&mut RenderCommands, &ResourceProvider),
    pub(crate) type_name: Option<&'static str>,
}

impl std::fmt::Debug for RenderNodeMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fn_name = match &self.type_name {
            Some(name) => Some(format!("{}::run()", name)),
            None => None,
        };
        f.debug_struct("RenderNodeMeta")
            .field("inputs", &self.reads)
            .field("outputs", &self.writes)
            .field("before", &self.before)
            .field("after", &self.after)
            .field(
                "run_fn",
                &fn_name
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("custom fn"),
            )
            .finish()
    }
}

impl RenderNodeMeta {
    pub fn conflicts_with(&self, other: &RenderNodeMeta) -> bool {
        for read in self.reads.iter() {
            if other.writes.contains_key(&read[..]) {
                return true;
            }
        }
        for (write, _) in self.writes.iter() {
            if other.writes.contains_key(&write[..]) {
                return true;
            }
        }

        for read in other.reads.iter() {
            if self.writes.contains_key(&read[..]) {
                return true;
            }
        }
        for (write, _) in other.writes.iter() {
            if self.writes.contains_key(&write[..]) {
                return true;
            }
        }

        false
    }
}
