use std::{collections::HashSet, error::Error, fmt};

use crate::{
    FigureId, PropertyKey, PropertyValueType,
    figure::{PreparationBudgetError, PreparationBudgetHandle},
    runtime::update::ErasedPropertyKey,
};

/// Rejection while admitting a manager's immutable child dependency declarations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutDependencyRegistrationError {
    Duplicate {
        child: Option<FigureId>,
        property: ErasedPropertyKey,
    },
    InvalidChild {
        container: FigureId,
        child: FigureId,
    },
    Budget(PreparationBudgetError),
    Allocation,
}

impl fmt::Display for LayoutDependencyRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout dependency registration rejected: {self:?}")
    }
}

impl Error for LayoutDependencyRegistrationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Budget(error) => Some(error),
            _ => None,
        }
    }
}

type Entry = (Option<FigureId>, ErasedPropertyKey);

/// Admission-only collector. A rejected declaration poisons the entire collection,
/// even when the extension discards its error.
///
/// Independence includes output, measurements, derived state and failure behavior,
/// including the manager's invalidation callback. It applies only while the child's
/// external geometry and measurement inputs remain unchanged.
pub struct LayoutDependencyDeclaration {
    entries: HashSet<Entry>,
    budget: PreparationBudgetHandle,
    failure: Option<LayoutDependencyRegistrationError>,
}

impl LayoutDependencyDeclaration {
    pub(crate) fn new(budget: PreparationBudgetHandle) -> Self {
        Self {
            entries: HashSet::new(),
            budget,
            failure: None,
        }
    }

    /// Declare independence for every direct child of this manager's container.
    pub fn independent_child_property<V: PropertyValueType>(
        &mut self,
        key: PropertyKey<V>,
    ) -> Result<(), LayoutDependencyRegistrationError> {
        self.insert(None, key.erase())
    }

    /// Declare independence for one direct child, checked before publication.
    pub fn independent_child_property_for<V: PropertyValueType>(
        &mut self,
        child: FigureId,
        key: PropertyKey<V>,
    ) -> Result<(), LayoutDependencyRegistrationError> {
        self.insert(Some(child), key.erase())
    }

    fn insert(
        &mut self,
        child: Option<FigureId>,
        property: ErasedPropertyKey,
    ) -> Result<(), LayoutDependencyRegistrationError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            self.budget
                .consume("layout dependency registration", 1)
                .map_err(LayoutDependencyRegistrationError::Budget)?;
            if self.entries.contains(&(child, property)) {
                return Err(LayoutDependencyRegistrationError::Duplicate { child, property });
            }
            self.budget
                .allocate("layout dependency retained entries", 1)
                .map_err(LayoutDependencyRegistrationError::Budget)?;
            self.entries
                .try_reserve(1)
                .map_err(|_| LayoutDependencyRegistrationError::Allocation)?;
            self.entries.insert((child, property));
            Ok(())
        })();
        if let Err(error) = &result {
            self.failure = Some(error.clone());
        }
        result
    }

    pub(crate) fn finish(
        mut self,
        container: FigureId,
        is_child: impl Fn(FigureId) -> bool,
    ) -> Result<Option<Box<LayoutDependencyTable>>, LayoutDependencyRegistrationError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        self.budget
            .consume(
                "layout dependency child admission",
                self.entries.len() as u64,
            )
            .map_err(LayoutDependencyRegistrationError::Budget)?;
        for (child, _) in &self.entries {
            if let Some(child) = child
                && !is_child(*child)
            {
                return Err(LayoutDependencyRegistrationError::InvalidChild {
                    container,
                    child: *child,
                });
            }
        }
        self.budget
            .check()
            .map_err(LayoutDependencyRegistrationError::Budget)?;
        if self.entries.is_empty() {
            return Ok(None);
        }
        let wildcard_count = self
            .entries
            .iter()
            .filter(|(child, _)| child.is_none())
            .count();
        if wildcard_count != 0 {
            self.budget
                .allocate("layout dependency overlap index", wildcard_count as u64)
                .map_err(LayoutDependencyRegistrationError::Budget)?;
            let mut wildcards = HashSet::new();
            wildcards
                .try_reserve(wildcard_count)
                .map_err(|_| LayoutDependencyRegistrationError::Allocation)?;
            wildcards.extend(
                self.entries
                    .iter()
                    .filter_map(|(child, key)| child.is_none().then_some(*key)),
            );
            self.entries
                .retain(|(child, key)| child.is_none() || !wildcards.contains(key));
        }
        self.budget
            .allocate("layout dependency table metadata", 1)
            .map_err(LayoutDependencyRegistrationError::Budget)?;
        Ok(Some(Box::new(LayoutDependencyTable(self.entries))))
    }
}

pub(crate) struct LayoutDependencyTable(HashSet<Entry>);
impl LayoutDependencyTable {
    pub(crate) fn permits(&self, child: FigureId, property: ErasedPropertyKey) -> bool {
        self.0.contains(&(None, property)) || self.0.contains(&(Some(child), property))
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.0.len()
    }
}
