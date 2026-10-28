//! Serialized metadata admission. This module never owns a live Figure or Runtime.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::{advanced::RuntimeNamespace, tree::FigureId};

use super::SceneSourceId;

/// Failure before any local structure, binding or resource is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneAdmissionError {
    /// Another participant committed after this candidate was prepared.
    StaleSceneAdmission,
    /// The proposed structural dependencies contain a direct or indirect cycle.
    CyclicSceneViewBinding,
    /// The source or participant does not belong to this group.
    UnknownSceneMember,
    /// A relation version cannot advance without reusing an old version.
    Exhausted,
    /// A failed internal commit made this group unavailable.
    GroupUnavailable,
}

impl std::fmt::Display for SceneAdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scene dependency admission failed: {self:?}")
    }
}
impl std::error::Error for SceneAdmissionError {}

/// Composition-root-owned group; only Core can commit dependency metadata.
#[derive(Clone, Default)]
pub struct SceneBindingGroup {
    inner: Arc<Mutex<GroupState>>,
}

#[derive(Clone, Default)]
struct GroupState {
    revision: u64,
    members: HashMap<RuntimeNamespace, Member>,
    // Retired sources remain known identities until the group is dropped.
    sources: HashMap<SceneSourceId, Option<FigureId>>,
}

#[derive(Clone, Default)]
struct Member {
    generation: u64,
    parents: HashMap<FigureId, Option<FigureId>>,
    views: HashMap<FigureId, SceneSourceId>,
}

/// Frozen evidence, never an admission ticket. It is separate from drawing identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneDependencyEvidence {
    /// Historical graph revision at capture.
    pub graph_revision: u64,
    /// Producer's topology and binding generation, excluding pure geometry.
    pub member_generation: u64,
    /// Every direct structural dependency, including hidden and pending views.
    pub direct_sources: Vec<SceneSourceId>,
}

pub(crate) struct SceneAdmission {
    group: SceneBindingGroup,
    base_revision: u64,
    proposed: GroupState,
}

impl SceneBindingGroup {
    /// Returns the currently committed metadata revision.
    pub fn graph_revision(&self) -> Result<u64, SceneAdmissionError> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?
            .revision)
    }

    fn prepare(
        &self,
        change: impl FnOnce(&mut GroupState) -> Result<(), SceneAdmissionError>,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        let mut proposed = self
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?
            .clone();
        let base_revision = proposed.revision;
        proposed.revision = base_revision
            .checked_add(1)
            .ok_or(SceneAdmissionError::Exhausted)?;
        change(&mut proposed)?;
        validate_graph(&proposed)?;
        Ok(SceneAdmission {
            group: self.clone(),
            base_revision,
            proposed,
        })
    }

    pub(crate) fn join(
        &self,
        namespace: RuntimeNamespace,
        parents: HashMap<FigureId, Option<FigureId>>,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        self.prepare(|state| {
            if state.members.contains_key(&namespace) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            state.members.insert(
                namespace,
                Member {
                    parents,
                    ..Member::default()
                },
            );
            Ok(())
        })
    }

    pub(crate) fn topology(
        &self,
        namespace: RuntimeNamespace,
        parents: HashMap<FigureId, Option<FigureId>>,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        self.prepare(|state| {
            let member = state
                .members
                .get_mut(&namespace)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            member.generation = member
                .generation
                .checked_add(1)
                .ok_or(SceneAdmissionError::Exhausted)?;
            member
                .views
                .retain(|figure, _| parents.contains_key(figure));
            member.parents = parents;
            for (source, root) in &mut state.sources {
                if source.namespace == namespace
                    && root.is_some_and(|root| !member.parents.contains_key(&root))
                {
                    *root = None;
                }
            }
            Ok(())
        })
    }

    pub(crate) fn source(
        &self,
        source: SceneSourceId,
        root: Option<FigureId>,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        self.prepare(|state| {
            let member = state
                .members
                .get_mut(&source.namespace)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            if root.is_some_and(|root| !member.parents.contains_key(&root)) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            member.generation = member
                .generation
                .checked_add(1)
                .ok_or(SceneAdmissionError::Exhausted)?;
            state.sources.insert(source, root);
            Ok(())
        })
    }

    pub(crate) fn binding(
        &self,
        namespace: RuntimeNamespace,
        figure: FigureId,
        source: Option<SceneSourceId>,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        self.prepare(|state| {
            if source.is_some_and(|source| !matches!(state.sources.get(&source), Some(Some(_)))) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            let member = state
                .members
                .get_mut(&namespace)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            if !member.parents.contains_key(&figure) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            member.generation = member
                .generation
                .checked_add(1)
                .ok_or(SceneAdmissionError::Exhausted)?;
            if let Some(source) = source {
                member.views.insert(figure, source);
            } else {
                member.views.remove(&figure);
            }
            Ok(())
        })
    }

    pub(crate) fn evidence(
        &self,
        source: SceneSourceId,
    ) -> Result<SceneDependencyEvidence, SceneAdmissionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?;
        if !matches!(state.sources.get(&source), Some(Some(_))) {
            return Err(SceneAdmissionError::UnknownSceneMember);
        }
        let mut direct_sources: Vec<_> = edges(&state)[&source].iter().copied().collect();
        direct_sources.sort_by_key(|id| (format!("{:?}", id.namespace), id.sequence));
        Ok(SceneDependencyEvidence {
            graph_revision: state.revision,
            member_generation: state.members[&source.namespace].generation,
            direct_sources,
        })
    }

    pub(crate) fn install(
        &self,
        namespace: RuntimeNamespace,
        figure: FigureId,
        source: SceneSourceId,
        evidence: &SceneDependencyEvidence,
    ) -> Result<SceneAdmission, SceneAdmissionError> {
        self.prepare(|state| {
            if !matches!(state.sources.get(&source), Some(Some(_))) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            let producer = state
                .members
                .get(&source.namespace)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            let direct: HashSet<_> = evidence.direct_sources.iter().copied().collect();
            if producer.generation != evidence.member_generation || edges(state)[&source] != direct
            {
                return Err(SceneAdmissionError::StaleSceneAdmission);
            }
            let destination = state
                .members
                .get_mut(&namespace)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            if destination.views.get(&figure) != Some(&source) {
                return Err(SceneAdmissionError::UnknownSceneMember);
            }
            destination.generation = destination
                .generation
                .checked_add(1)
                .ok_or(SceneAdmissionError::Exhausted)?;
            Ok(())
        })
    }

    pub(crate) fn check_snapshot(
        &self,
        source: SceneSourceId,
        evidence: &SceneDependencyEvidence,
    ) -> Result<(), SceneAdmissionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?;
        if !matches!(state.sources.get(&source), Some(Some(_))) {
            return Err(SceneAdmissionError::UnknownSceneMember);
        }
        let producer = state
            .members
            .get(&source.namespace)
            .ok_or(SceneAdmissionError::UnknownSceneMember)?;
        let direct: HashSet<_> = evidence.direct_sources.iter().copied().collect();
        if producer.generation != evidence.member_generation || edges(&state)[&source] != direct {
            return Err(SceneAdmissionError::StaleSceneAdmission);
        }
        validate_graph(&state)
    }

    pub(crate) fn member_generation(
        &self,
        source: SceneSourceId,
    ) -> Result<u64, SceneAdmissionError> {
        let state = self
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?;
        if !matches!(state.sources.get(&source), Some(Some(_))) {
            return Err(SceneAdmissionError::UnknownSceneMember);
        }
        Ok(state.members[&source.namespace].generation)
    }

    pub(crate) fn retire_member(&self, namespace: RuntimeNamespace) {
        // Disposal cannot run user callbacks while holding this lock. A poisoned or
        // exhausted group must not subsequently accept any new admission.
        if let Ok(mut state) = self.inner.lock() {
            state.members.remove(&namespace);
            for (source, root) in &mut state.sources {
                if source.namespace == namespace {
                    *root = None;
                }
            }
            if let Some(revision) = state.revision.checked_add(1) {
                state.revision = revision;
            }
        }
    }
}

impl SceneAdmission {
    /// The closure contains only prevalidated local writes. It must return retired
    /// user-owned values rather than dropping them or invoking callbacks under lock.
    pub(crate) fn commit<T>(self, local: impl FnOnce() -> T) -> Result<T, SceneAdmissionError> {
        let mut current = self
            .group
            .inner
            .lock()
            .map_err(|_| SceneAdmissionError::GroupUnavailable)?;
        if current.revision != self.base_revision {
            return Err(SceneAdmissionError::StaleSceneAdmission);
        }
        // Revalidate under the exclusive guard; the candidate is private and consumed.
        validate_graph(&self.proposed)?;
        let value = local();
        *current = self.proposed;
        Ok(value)
    }
}

fn edges(state: &GroupState) -> HashMap<SceneSourceId, HashSet<SceneSourceId>> {
    state
        .sources
        .iter()
        .map(|(&source, &root)| {
            let mut dependencies = HashSet::new();
            if let Some(root) = root
                && let Some(member) = state.members.get(&source.namespace)
            {
                for (&figure, &dependency) in &member.views {
                    let mut cursor = Some(figure);
                    while let Some(id) = cursor {
                        if id == root {
                            dependencies.insert(dependency);
                            break;
                        }
                        cursor = member.parents.get(&id).copied().flatten();
                    }
                }
            }
            (source, dependencies)
        })
        .collect()
}

fn validate_graph(state: &GroupState) -> Result<(), SceneAdmissionError> {
    let mut graph = edges(state);
    // Kahn elimination avoids growing the call stack with the number of sources.
    let mut incoming: HashMap<_, usize> = graph.keys().map(|&source| (source, 0)).collect();
    for dependencies in graph.values() {
        for dependency in dependencies {
            let count = incoming
                .get_mut(dependency)
                .ok_or(SceneAdmissionError::UnknownSceneMember)?;
            *count += 1;
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .filter_map(|(&source, &count)| (count == 0).then_some(source))
        .collect();
    let mut visited = 0;
    while let Some(source) = ready.pop() {
        visited += 1;
        for dependency in graph.remove(&source).unwrap_or_default() {
            let count = incoming.get_mut(&dependency).expect("validated graph node");
            *count -= 1;
            if *count == 0 {
                ready.push(dependency);
            }
        }
    }
    if visited != incoming.len() {
        Err(SceneAdmissionError::CyclicSceneViewBinding)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::FigureTree;

    fn participant(group: &SceneBindingGroup) -> (FigureTree, SceneSourceId) {
        let tree = FigureTree::new();
        group
            .join(tree.namespace(), tree.scene_parents())
            .unwrap()
            .commit(|| ())
            .unwrap();
        let source = SceneSourceId {
            namespace: tree.namespace(),
            sequence: 1,
        };
        group
            .source(source, Some(tree.root_id()))
            .unwrap()
            .commit(|| ())
            .unwrap();
        (tree, source)
    }

    #[test]
    fn interleaved_candidates_revalidate_revision_before_any_local_write() {
        let group = SceneBindingGroup::default();
        let (a, source_a) = participant(&group);
        let (b, source_b) = participant(&group);
        let first = group
            .binding(a.namespace(), a.root_id(), Some(source_b))
            .unwrap();
        let stale = group
            .binding(b.namespace(), b.root_id(), Some(source_a))
            .unwrap();
        first.commit(|| ()).unwrap();
        let mut wrote = false;
        assert_eq!(
            stale.commit(|| wrote = true),
            Err(SceneAdmissionError::StaleSceneAdmission)
        );
        assert!(!wrote);
        assert!(matches!(
            group.binding(b.namespace(), b.root_id(), Some(source_a)),
            Err(SceneAdmissionError::CyclicSceneViewBinding)
        ));
    }

    #[test]
    fn exhausted_revision_rejects_all_new_metadata() {
        let group = SceneBindingGroup::default();
        group.inner.lock().unwrap().revision = u64::MAX;
        let tree = FigureTree::new();
        assert!(matches!(
            group.join(tree.namespace(), tree.scene_parents()),
            Err(SceneAdmissionError::Exhausted)
        ));
        assert!(group.inner.lock().unwrap().members.is_empty());
    }

    #[test]
    fn last_revision_commits_the_entire_replacement_and_rejects_later_insertion() {
        let group = SceneBindingGroup::default();
        let mut runtime = crate::runtime::Runtime::empty();
        let old = runtime
            .set_contents(Box::new(crate::figure::RectangleFigure::new(
                0.0, 0.0, 20.0, 20.0,
            )))
            .unwrap();
        runtime.scenes().join_group(group.clone()).unwrap();
        let source = runtime.scenes().register_source(old).unwrap();
        let (_, _, handoff) = runtime
            .scenes()
            .subscribe(
                source,
                super::super::SOURCE_LOCAL_CAPTURE,
                super::super::SourceLocalCapture::default(),
            )
            .unwrap();
        group.inner.lock().unwrap().revision = u64::MAX - 1;
        let new = runtime
            .set_contents(Box::new(crate::figure::RectangleFigure::new(
                0.0, 0.0, 30.0, 30.0,
            )))
            .unwrap();
        assert!(!runtime.tree().contains(old));
        assert!(runtime.tree().contains(new));
        assert_eq!(group.graph_revision().unwrap(), u64::MAX);
        assert!(matches!(
            handoff.take().unwrap(),
            Some(super::super::SceneEvent::Closed(_))
        ));
        assert!(
            runtime
                .figure(new)
                .unwrap()
                .add(Box::new(crate::figure::RectangleFigure::new(
                    0.0, 0.0, 5.0, 5.0
                )))
                .is_err()
        );
        assert!(runtime.tree().child_order(new).unwrap().is_empty());
    }

    #[test]
    fn last_revision_commits_both_nodes_in_an_atomic_append_batch() {
        let group = SceneBindingGroup::default();
        let mut runtime = crate::runtime::Runtime::empty();
        let root = runtime
            .set_contents(Box::new(crate::figure::RectangleFigure::new(
                0.0, 0.0, 20.0, 20.0,
            )))
            .unwrap();
        runtime.scenes().join_group(group.clone()).unwrap();
        group.inner.lock().unwrap().revision = u64::MAX - 1;
        let ids = runtime
            .add_figures(vec![
                (
                    root,
                    Box::new(crate::figure::RectangleFigure::new(0.0, 0.0, 5.0, 5.0)),
                    None,
                ),
                (
                    root,
                    Box::new(crate::figure::RectangleFigure::new(5.0, 0.0, 5.0, 5.0)),
                    None,
                ),
            ])
            .unwrap();
        assert_eq!(runtime.tree().child_order(root).unwrap(), ids);
        assert_eq!(group.graph_revision().unwrap(), u64::MAX);
    }

    #[test]
    fn nested_capture_roots_each_receive_the_entire_structural_dependency() {
        let group = SceneBindingGroup::default();
        let (mut tree, outer) = participant(&group);
        let root = tree.root_id();
        let inner = tree
            .builder()
            .add_child(
                root,
                Box::new(crate::figure::RectangleFigure::new(0.0, 0.0, 1.0, 1.0)),
            )
            .unwrap();
        group
            .topology(tree.namespace(), tree.scene_parents())
            .unwrap()
            .commit(|| ())
            .unwrap();
        let source_inner = SceneSourceId {
            namespace: tree.namespace(),
            sequence: 2,
        };
        group
            .source(source_inner, Some(inner))
            .unwrap()
            .commit(|| ())
            .unwrap();
        let (_, other) = participant(&group);
        group
            .binding(tree.namespace(), inner, Some(other))
            .unwrap()
            .commit(|| ())
            .unwrap();
        assert_eq!(group.evidence(outer).unwrap().direct_sources, vec![other]);
        assert_eq!(
            group.evidence(source_inner).unwrap().direct_sources,
            vec![other]
        );
    }
}
