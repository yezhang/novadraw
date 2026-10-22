use crate::{FigureId, figure::ResolvedStyle};

/// Exact frozen source inputs. Diagnostic hashes never grant snapshot authority.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct PreparationSource(pub(crate) Vec<PreparationSourceNode>);

#[derive(Clone, PartialEq)]
pub(crate) struct PreparationSourceNode {
    pub(crate) id: FigureId,
    pub(crate) component: u64,
    pub(crate) resources: Vec<(crate::render::ResourceId, Option<crate::ResourceStatus>)>,
    pub(crate) provider: Option<u64>,
    pub(crate) parent: Option<FigureId>,
    pub(crate) children: Vec<FigureId>,
    pub(crate) layout: u64,
    pub(crate) sizes: [Option<[u64; 2]>; 3],
    pub(crate) geometry: [u64; 8],
    pub(crate) style: ResolvedStyle,
}
