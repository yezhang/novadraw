use super::ContainerEditError;
use std::{collections::HashSet, hash::Hash};

/// Position interpreted after removing this batch's local sources from the target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertionPosition<I> {
    /// Before all remaining children.
    Start,
    /// After all remaining children.
    End,
    /// Immediately before one remaining target child.
    Before(I),
    /// Immediately after one remaining target child.
    After(I),
}

/// Computes a complete ordered target sequence without executing partial model changes.
pub fn ordered_insertion<I: Copy + Eq + Hash>(
    children: &[I],
    sources: &[I],
    position: InsertionPosition<I>,
) -> Result<Vec<I>, ContainerEditError> {
    let source_set: HashSet<_> = sources.iter().copied().collect();
    if sources.is_empty() || source_set.len() != sources.len() {
        return Err(ContainerEditError::Batch(
            "empty or duplicated ordered sources",
        ));
    }
    if children.iter().copied().collect::<HashSet<_>>().len() != children.len() {
        return Err(ContainerEditError::Batch("duplicate target child"));
    }
    let mut remaining: Vec<_> = children
        .iter()
        .copied()
        .filter(|id| !source_set.contains(id))
        .collect();
    let index = match position {
        InsertionPosition::Start => 0,
        InsertionPosition::End => remaining.len(),
        InsertionPosition::Before(reference) | InsertionPosition::After(reference) => {
            if source_set.contains(&reference) {
                return Err(ContainerEditError::Batch(
                    "insertion reference belongs to the source batch",
                ));
            }
            let index = remaining.iter().position(|id| *id == reference).ok_or(
                ContainerEditError::Batch("insertion reference is not a remaining target child"),
            )?;
            index + usize::from(matches!(position, InsertionPosition::After(_)))
        }
    };
    remaining.splice(index..index, sources.iter().copied());
    Ok(remaining)
}
