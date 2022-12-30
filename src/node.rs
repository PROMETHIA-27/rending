use std::borrow::Cow;

use naga::FastHashSet;
use slotmap::new_key_type;

use crate::commands::RenderCommands;

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

    fn run(commands: &mut RenderCommands);
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

pub(crate) type OrderingList = FastHashSet<Cow<'static, str>>;

#[derive(Clone)]
pub struct RenderNodeMeta {
    pub(crate) before: OrderingList,
    pub(crate) after: OrderingList,
    pub(crate) run_fn: fn(&mut RenderCommands),
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
