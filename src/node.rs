use std::borrow::Cow;

use naga::{FastHashMap, FastHashSet};
use slotmap::new_key_type;

use crate::commands::RenderCommands;
use crate::resources::{ResourceType, Resources};

new_key_type! { pub struct NodeKey; }

pub trait RenderNode {
    fn name() -> Cow<'static, str>;

    // TODO: Ergonomics of this are garbage. Fix it
    fn before() -> Vec<Cow<'static, str>> {
        vec![]
    }

    fn after() -> Vec<Cow<'static, str>> {
        vec![]
    }

    fn run(commands: &mut RenderCommands, resources: &mut Resources);
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
}

impl NodeOutput {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            resource: name.into(),
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
    pub(crate) before: OrderingList,
    pub(crate) after: OrderingList,
    pub(crate) run_fn: fn(&mut RenderCommands, &mut Resources),
    pub(crate) type_name: Option<&'static str>,
}

impl std::fmt::Debug for RenderNodeMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fn_name = match &self.type_name {
            Some(name) => Some(format!("{}::run()", name)),
            None => None,
        };
        f.debug_struct("RenderNodeMeta")
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
