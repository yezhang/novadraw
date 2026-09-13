//! Ordered Viewer selection and EditPart focus state.

/// Typed change emitted by a [`SelectionModel`] mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionDelta<I> {
    added: Vec<I>,
    removed: Vec<I>,
    previous_primary: Option<I>,
    primary: Option<I>,
    previous_focus: Option<I>,
    focus: Option<I>,
}

impl<I> SelectionDelta<I> {
    /// Returns items newly included in the selection.
    pub fn added(&self) -> &[I] {
        &self.added
    }

    /// Returns items removed from the selection.
    pub fn removed(&self) -> &[I] {
        &self.removed
    }

    /// Returns the primary selection before the mutation.
    pub const fn previous_primary(&self) -> Option<I>
    where
        I: Copy,
    {
        self.previous_primary
    }

    /// Returns the primary selection after the mutation.
    pub const fn primary(&self) -> Option<I>
    where
        I: Copy,
    {
        self.primary
    }

    /// Returns focus before the mutation.
    pub const fn previous_focus(&self) -> Option<I>
    where
        I: Copy,
    {
        self.previous_focus
    }

    /// Returns focus after the mutation.
    pub const fn focus(&self) -> Option<I>
    where
        I: Copy,
    {
        self.focus
    }
}

/// Ordered selection with the last item serving as primary selection.
#[derive(Clone, Debug)]
pub struct SelectionModel<I> {
    items: Vec<I>,
    focus: Option<I>,
}

impl<I> Default for SelectionModel<I> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            focus: None,
        }
    }
}

impl<I: Copy + Eq> SelectionModel<I> {
    /// Creates an empty selection with no focused part.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns selected parts in insertion order.
    pub fn items(&self) -> &[I] {
        &self.items
    }

    /// Returns whether the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the last selected item.
    pub fn primary(&self) -> Option<I> {
        self.items.last().copied()
    }

    /// Returns the independently tracked focused part.
    pub const fn focus(&self) -> Option<I> {
        self.focus
    }

    /// Replaces the current selection with one item.
    pub fn replace(&mut self, item: I) -> Option<SelectionDelta<I>> {
        self.apply(vec![item], self.focus)
    }

    /// Appends an item and makes it primary, moving an existing item to the end.
    pub fn append(&mut self, item: I) -> Option<SelectionDelta<I>> {
        if self.primary() == Some(item) {
            return None;
        }
        let mut next = self.items.clone();
        next.retain(|candidate| *candidate != item);
        next.push(item);
        self.apply(next, self.focus)
    }

    /// Adds an absent item or removes a selected item.
    pub fn toggle(&mut self, item: I) -> Option<SelectionDelta<I>> {
        let mut next = self.items.clone();
        if let Some(index) = next.iter().position(|candidate| *candidate == item) {
            next.remove(index);
        } else {
            next.push(item);
        }
        self.apply(next, self.focus)
    }

    /// Removes one item from the selection.
    pub fn remove(&mut self, item: I) -> Option<SelectionDelta<I>> {
        if !self.items.contains(&item) {
            return None;
        }
        let mut next = self.items.clone();
        next.retain(|candidate| *candidate != item);
        let next_focus = self.focus.filter(|focus| *focus != item);
        self.apply(next, next_focus)
    }

    /// Clears selection and EditPart focus.
    pub fn clear(&mut self) -> Option<SelectionDelta<I>> {
        self.apply(Vec::new(), None)
    }

    /// Changes EditPart focus independently from the selected list.
    pub fn set_focus(&mut self, focus: Option<I>) -> bool {
        if self.focus == focus {
            return false;
        }
        self.focus = focus;
        true
    }

    /// Removes selected and focused items that are no longer valid.
    pub fn reconcile(&mut self, mut retain: impl FnMut(I) -> bool) -> Option<SelectionDelta<I>> {
        let next = self
            .items
            .iter()
            .copied()
            .filter(|item| retain(*item))
            .collect();
        let next_focus = self.focus.filter(|focus| retain(*focus));
        self.apply(next, next_focus)
    }

    fn apply(&mut self, next: Vec<I>, next_focus: Option<I>) -> Option<SelectionDelta<I>> {
        if self.items == next && self.focus == next_focus {
            return None;
        }
        let previous_primary = self.primary();
        let previous_focus = self.focus;
        let removed = self
            .items
            .iter()
            .copied()
            .filter(|item| !next.contains(item))
            .collect();
        let added = next
            .iter()
            .copied()
            .filter(|item| !self.items.contains(item))
            .collect();
        self.items = next;
        self.focus = next_focus;
        Some(SelectionDelta {
            added,
            removed,
            previous_primary,
            primary: self.primary(),
            previous_focus,
            focus: self.focus,
        })
    }
}
