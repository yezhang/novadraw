//! Public handles carry ownership; only the arena sees local SlotMap keys.

use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use slotmap::{DefaultKey, Key, KeyData, SlotMap};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeNamespace(Uuid);

impl RuntimeNamespace {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

pub(crate) trait RuntimeHandle: Copy {
    fn from_local(namespace: RuntimeNamespace, local: KeyData) -> Self;
    fn namespace(self) -> RuntimeNamespace;
    fn local(self) -> KeyData;
}

macro_rules! runtime_handle {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            namespace: RuntimeNamespace,
            local: KeyData,
        }

        impl $name {
            pub fn namespace(self) -> RuntimeNamespace {
                self.namespace
            }

            #[allow(dead_code)]
            pub fn null() -> Self {
                Self {
                    namespace: RuntimeNamespace(Uuid::nil()),
                    local: DefaultKey::null().data(),
                }
            }

            #[allow(dead_code)]
            pub fn is_null(self) -> bool {
                DefaultKey::from(self.local).is_null()
            }
        }

        impl RuntimeHandle for $name {
            fn from_local(namespace: RuntimeNamespace, local: KeyData) -> Self {
                Self { namespace, local }
            }

            fn namespace(self) -> RuntimeNamespace {
                self.namespace
            }

            fn local(self) -> KeyData {
                self.local
            }
        }

        // Synthetic test IDs are never valid in a live Runtime.
        #[cfg(test)]
        impl From<KeyData> for $name {
            fn from(local: KeyData) -> Self {
                Self::from_local(RuntimeNamespace(Uuid::nil()), local)
            }
        }
    };
}

runtime_handle!(FigureId);
runtime_handle!(AnchorId);
runtime_handle!(RouterId);
runtime_handle!(AnimationId);
runtime_handle!(AnimationBehaviorId);
runtime_handle!(AnimationChannelId);
runtime_handle!(TemporaryVisualId);

pub(crate) struct RuntimeArena<I, T> {
    namespace: RuntimeNamespace,
    entries: SlotMap<DefaultKey, T>,
    identity: PhantomData<I>,
}

impl<I: RuntimeHandle, T> RuntimeArena<I, T> {
    pub(crate) fn new(namespace: RuntimeNamespace) -> Self {
        Self {
            namespace,
            entries: SlotMap::with_key(),
            identity: PhantomData,
        }
    }

    pub(crate) fn namespace(&self) -> RuntimeNamespace {
        self.namespace
    }

    pub(crate) fn insert(&mut self, value: T) -> I {
        self.insert_with_key(|_| value)
    }

    pub(crate) fn insert_with_key(&mut self, make: impl FnOnce(I) -> T) -> I {
        let namespace = self.namespace;
        let key = self
            .entries
            .insert_with_key(|key| make(I::from_local(namespace, key.data())));
        I::from_local(namespace, key.data())
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        if id.namespace() != self.namespace {
            return None;
        }
        self.entries.get(DefaultKey::from(id.local()))
    }

    pub(crate) fn get_mut(&mut self, id: I) -> Option<&mut T> {
        if id.namespace() != self.namespace {
            return None;
        }
        self.entries.get_mut(DefaultKey::from(id.local()))
    }

    pub(crate) fn contains_key(&self, id: I) -> bool {
        self.get(id).is_some()
    }

    pub(crate) fn remove(&mut self, id: I) -> Option<T> {
        if id.namespace() != self.namespace {
            return None;
        }
        self.entries.remove(DefaultKey::from(id.local()))
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        let namespace = self.namespace;
        self.entries
            .iter()
            .map(move |(key, value)| (I::from_local(namespace, key.data()), value))
    }
}

impl<I: RuntimeHandle, T> Index<I> for RuntimeArena<I, T> {
    type Output = T;

    fn index(&self, index: I) -> &Self::Output {
        self.get(index).expect("invalid Runtime arena handle")
    }
}

impl<I: RuntimeHandle, T> IndexMut<I> for RuntimeArena<I, T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.get_mut(index).expect("invalid Runtime arena handle")
    }
}
