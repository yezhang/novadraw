//! Exclusive assembly scratch storage. Published plans never borrow these buffers.
use std::collections::{HashMap, HashSet};

use crate::{
    FigureId,
    text::{TextContentError, TextContentLease, TextContentStore, TextError},
};

use super::FlowError;

#[derive(Default)]
pub(crate) struct AssemblyWorkspace {
    pub(crate) visited: HashSet<FigureId>,
    pub(crate) stack: Vec<FigureId>,
    pub(crate) source_offsets: HashMap<FigureId, usize>,
    store: Option<TextContentStore>,
    retention: Option<TextContentLease>,
    limit: usize,
}

impl AssemblyWorkspace {
    pub(crate) fn new(store: TextContentStore) -> Self {
        Self {
            store: Some(store),
            limit: usize::MAX,
            ..Self::default()
        }
    }

    fn clear(&mut self) {
        self.visited.clear();
        self.stack.clear();
        self.source_offsets.clear();
    }

    pub(crate) fn discard(&mut self) {
        self.visited = HashSet::new();
        self.stack = Vec::new();
        self.source_offsets = HashMap::new();
        self.retention = None;
    }

    // Logical capacity accounting, as for the assembly's retained maps.
    // Allocator overhead is outside the content ledger's units.
    fn units(&self) -> Option<usize> {
        let visited = self
            .visited
            .capacity()
            .checked_mul(std::mem::size_of::<FigureId>())?;
        let stack = self
            .stack
            .capacity()
            .checked_mul(std::mem::size_of::<FigureId>())?;
        let offsets = self
            .source_offsets
            .capacity()
            .checked_mul(std::mem::size_of::<(FigureId, usize)>())?;
        let buffers = visited.checked_add(stack)?.checked_add(offsets)?;
        if buffers == 0 {
            Some(0)
        } else {
            buffers
                .checked_add(std::mem::size_of::<Self>())?
                .checked_add(2 * std::mem::size_of::<usize>())
        }
    }

    pub(crate) fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        if self.units().is_none_or(|units| units > limit) {
            self.discard();
        }
    }

    pub(crate) fn begin(&mut self) -> AssemblyWorkspaceUse<'_> {
        self.clear();
        AssemblyWorkspaceUse(self)
    }

    fn retain(&mut self) -> Result<(), FlowError> {
        let Some(units) = self.units().filter(|&units| units <= self.limit) else {
            self.discard();
            return Ok(());
        };
        if units <= self.retention.as_ref().map_or(0, TextContentLease::units) {
            return Ok(());
        }
        let Some(store) = &self.store else {
            self.discard();
            return Ok(());
        };
        // Reserve the replacement while the previous lease remains live. A
        // capacity miss releases this optional cache; it never bypasses a
        // preparation-budget failure or turns a ledger failure into success.
        match store.reserve(units) {
            Ok(lease) => self.retention = Some(lease),
            Err(TextContentError::Capacity { .. }) => self.discard(),
            Err(error) => {
                self.discard();
                return Err(TextError::from(error).into());
            }
        }
        Ok(())
    }
}

pub(crate) struct AssemblyWorkspaceUse<'a>(&'a mut AssemblyWorkspace);

impl AssemblyWorkspaceUse<'_> {
    pub(crate) fn retain(&mut self) -> Result<(), FlowError> {
        self.0.retain()
    }
}
impl std::ops::Deref for AssemblyWorkspaceUse<'_> {
    type Target = AssemblyWorkspace;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
impl std::ops::DerefMut for AssemblyWorkspaceUse<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
impl Drop for AssemblyWorkspaceUse<'_> {
    fn drop(&mut self) {
        self.0.clear();
        // Errors and unwinds may leave newly grown buffers. Never retain
        // uncharged capacity after returning from an assembly call.
        if self.0.units().is_none_or(|units| {
            units > self.0.retention.as_ref().map_or(0, TextContentLease::units)
        }) {
            self.0.discard();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (FigureId, FigureId) {
        let mut tree = crate::tree::FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(
                crate::figure::RectangleFigure::new(0.0, 0.0, 0.0, 0.0).unwrap(),
            ))
            .unwrap();
        let child = tree
            .builder()
            .add_child(
                root,
                Box::new(crate::figure::RectangleFigure::new(0.0, 0.0, 0.0, 0.0).unwrap()),
            )
            .unwrap();
        (root, child)
    }

    #[test]
    fn workspace_clears_ids_offsets_and_retains_only_charged_capacity() {
        let store = TextContentStore::new(usize::MAX);
        let mut workspace = AssemblyWorkspace::new(store.clone());
        let (root, child) = ids();
        {
            let mut scratch = workspace.begin();
            scratch.visited.insert(root);
            scratch.stack.push(root);
            scratch.source_offsets.insert(root, 42);
            scratch.retain().unwrap();
        }
        let units = store.usage().unwrap().retained_units;
        assert!(units > 0);
        let capacities = (
            workspace.visited.capacity(),
            workspace.stack.capacity(),
            workspace.source_offsets.capacity(),
        );
        {
            let mut scratch = workspace.begin();
            assert!(scratch.visited.is_empty());
            assert!(scratch.stack.is_empty());
            assert!(scratch.source_offsets.is_empty());
            scratch.visited.insert(child);
            assert!(scratch.visited.insert(root));
            assert_eq!(*scratch.source_offsets.entry(root).or_default(), 0);
            scratch.retain().unwrap();
        }
        assert_eq!(store.usage().unwrap().retained_units, units);
        assert_eq!(
            capacities,
            (
                workspace.visited.capacity(),
                workspace.stack.capacity(),
                workspace.source_offsets.capacity(),
            )
        );
        workspace.set_limit(0);
        assert_eq!(store.usage().unwrap().retained_units, 0);
        assert_eq!(workspace.units(), Some(0));
    }

    #[test]
    fn workspace_error_and_capacity_miss_release_unaccounted_growth() {
        let store = TextContentStore::new(usize::MAX);
        let mut workspace = AssemblyWorkspace::new(store.clone());
        let (root, _) = ids();
        {
            let mut scratch = workspace.begin();
            scratch.stack.push(root);
            // Simulate a contribution error before finish/retention.
        }
        assert_eq!(workspace.stack.capacity(), 0);
        assert_eq!(store.usage().unwrap().retained_units, 0);
        store.set_limit(0).unwrap();
        {
            let mut scratch = workspace.begin();
            scratch.visited.insert(root);
            scratch.retain().unwrap();
        }
        assert_eq!(workspace.visited.capacity(), 0);
        assert_eq!(store.usage().unwrap().retained_units, 0);
        assert_eq!(AssemblyWorkspace::new(store).units(), Some(0));
    }

    #[test]
    fn workspace_unwind_releases_growth_and_does_not_share_state_with_another_instance() {
        let store = TextContentStore::new(usize::MAX);
        let mut workspace = AssemblyWorkspace::new(store.clone());
        let (root, _) = ids();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut scratch = workspace.begin();
            scratch.visited.insert(root);
            scratch.stack.push(root);
            panic!("contributor panic");
        }));
        assert!(result.is_err());
        assert_eq!(workspace.units(), Some(0));
        assert_eq!(store.usage().unwrap().retained_units, 0);
        let mut other = AssemblyWorkspace::new(TextContentStore::new(usize::MAX));
        {
            let mut scratch = workspace.begin();
            scratch.visited.insert(root);
            scratch.retain().unwrap();
        }
        assert!(other.begin().visited.is_empty());
        assert!(store.usage().unwrap().retained_units > 0);
        drop(workspace);
        assert_eq!(store.usage().unwrap().retained_units, 0);
    }
}
