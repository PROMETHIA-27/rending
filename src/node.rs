use std::borrow::Cow;
use std::collections::HashSet;

use naga::FastHashSet;
use slotmap::new_key_type;

use crate::commands::RenderCommands;

new_key_type! { pub struct NodeKey; }

pub struct RenderNodeMeta {
    pub(crate) name: Cow<'static, str>,
    pub(crate) before: FastHashSet<Cow<'static, str>>,
    pub(crate) after: FastHashSet<Cow<'static, str>>,
    pub(crate) run_fn: Box<dyn FnMut(&mut RenderCommands)>,
}

impl std::fmt::Debug for RenderNodeMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderNodeMeta")
            .field("name", &self.name)
            .field("before", &self.before)
            .field("after", &self.after)
            .finish()
    }
}

pub struct FunctionNode {
    pub(crate) name: Cow<'static, str>,
    pub(crate) before: FastHashSet<Cow<'static, str>>,
    pub(crate) after: FastHashSet<Cow<'static, str>>,
    pub(crate) run_fn: Box<dyn FnMut(&mut RenderCommands)>,
}

impl FunctionNode {
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        run: impl FnMut(&mut RenderCommands) + 'static,
    ) -> Self {
        FunctionNode {
            name: name.into(),
            before: HashSet::default(),
            after: HashSet::default(),
            run_fn: Box::new(run),
        }
    }

    pub fn before(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.before.insert(name.into());
        self
    }

    pub fn after(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.after.insert(name.into());
        self
    }
}

impl From<FunctionNode> for RenderNodeMeta {
    fn from(val: FunctionNode) -> Self {
        RenderNodeMeta {
            name: val.name,
            before: val.before,
            after: val.after,
            run_fn: val.run_fn,
        }
    }
}
