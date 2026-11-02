//! Literal inputs from Draw2D CompoundGraphTests at 4463d9d0. Java is read-only.
use novadraw::geometry::{Dimension, Insets};
use novadraw::layout::graph::*;

pub const COMPOUND_FIXTURES: &[&str] = &[
    "aaaapull",
    "aaaflowEditor",
    "chains",
    "flowChart",
    "flowEditor1",
    "flowEditor2",
    "ShortestPassCase",
    "tangledSubgraphs",
    "test1",
    "test2",
];

pub struct CompoundFixture {
    pub request: GraphLayoutRequest,
    pub labels: Vec<(GraphEndpoint, &'static str)>,
}

struct Inputs {
    builder: GraphRequestBuilder,
    labels: Vec<(GraphEndpoint, &'static str)>,
}
impl Inputs {
    fn group(
        &mut self,
        label: &'static str,
        parent: Option<GraphGroupId>,
        ctx: &mut GraphSolveContext,
    ) -> Result<GraphGroupId, GraphLayoutError> {
        let id = self.builder.add_group(Dimension::new(50.0, 40.0), ctx)?;
        let group = self.builder.group_mut(id)?;
        group.parent = parent;
        group.border_insets = Insets::uniform(1.0);
        ctx.allocate(GraphStage::Request, 1)?;
        self.labels.push((id.into(), label));
        Ok(id)
    }
    fn node(
        &mut self,
        label: &'static str,
        parent: GraphGroupId,
        width: f64,
        direction: GraphDirection,
        ctx: &mut GraphSolveContext,
    ) -> Result<GraphNodeId, GraphLayoutError> {
        let id = self.builder.add_node(
            GraphSize::directed(Dimension::new(width, 40.0), direction),
            ctx,
        )?;
        self.builder.node_mut(id)?.parent = Some(parent);
        ctx.allocate(GraphStage::Request, 1)?;
        self.labels.push((id.into(), label));
        Ok(id)
    }
    fn edge(
        &mut self,
        a: impl Into<GraphEndpoint>,
        b: impl Into<GraphEndpoint>,
        ctx: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        let edge = self.builder.add_edge(a, b, ctx)?;
        let edge = self.builder.edge_mut(edge)?;
        edge.width = 1.0;
        edge.padding = 10.0;
        Ok(())
    }
    fn pairs(
        &mut self,
        nodes: &[GraphNodeId],
        pairs: &[(usize, usize)],
        ctx: &mut GraphSolveContext,
    ) -> Result<(), GraphLayoutError> {
        for (a, b) in pairs {
            self.edge(nodes[*a], nodes[*b], ctx)?;
        }
        Ok(())
    }
}

pub fn compound_fixture(
    name: &str,
    direction: GraphDirection,
    ctx: &mut GraphSolveContext,
) -> Result<CompoundFixture, GraphLayoutError> {
    let mut inputs = Inputs {
        builder: GraphRequestBuilder::new(direction, COMPOUND_GRAPH, CompoundGraphConfig, ctx)?,
        labels: Vec::new(),
    };
    inputs.builder.default_padding(Insets::uniform(16.0));
    inputs.builder.require(LAYERED_RESULT, ctx)?;
    match name {
        "aaaapull" => {
            let s1 = inputs.group("Subgraph 1", None, ctx)?;
            let s2 = inputs.group("Subgraph 2", None, ctx)?;
            let r = inputs.node("r", s2, 50.0, direction, ctx)?;
            let t = inputs.node("t", s2, 50.0, direction, ctx)?;
            let a = inputs.node("a", s1, 50.0, direction, ctx)?;
            let b = inputs.node("b", s1, 50.0, direction, ctx)?;
            let e = inputs.node("e", s1, 50.0, direction, ctx)?;
            let j = inputs.node("j", s1, 50.0, direction, ctx)?;
            let m = inputs.node("m", s1, 50.0, direction, ctx)?;
            inputs.pairs(
                &[r, t, a, b, e, j, m],
                &[(2, 3), (3, 4), (4, 5), (6, 1), (5, 0), (2, 0)],
                ctx,
            )?;
            inputs.edge(s1, s2, ctx)?;
        }
        "aaaflowEditor" => {
            let diagram = inputs.group("Editor", None, ctx)?;
            let flow = inputs.group("Flow", Some(diagram), ctx)?;
            let sub1 = inputs.group("Sub1", Some(flow), ctx)?;
            let sub2 = inputs.group("Sub2", Some(flow), ctx)?;
            let a1 = inputs.node("a1", diagram, 200.0, direction, ctx)?;
            let a2 = inputs.node("a2", sub1, 200.0, direction, ctx)?;
            let a3 = inputs.node("a3", sub2, 200.0, direction, ctx)?;
            inputs.builder.node_mut(a1)?.outgoing.offset = AttachmentOffset::Distance(1.0);
            inputs.edge(a1, flow, ctx)?;
            inputs.edge(a1, a2, ctx)?;
            inputs.edge(a2, a3, ctx)?;
        }
        "chains" => {
            let s1 = inputs.group("S1", None, ctx)?;
            let s2 = inputs.group("S2", None, ctx)?;
            let s3 = inputs.group("S3", None, ctx)?;
            let sb = inputs.group("SB", None, ctx)?;
            let group = inputs.builder.group_mut(s1)?;
            group.padding = Some(Insets::uniform(10.0));
            group.inner_padding = Insets::uniform(1.0);
            group.border_insets = Insets::uniform(9.0);
            for (a, b) in [(s1, s2), (s1, sb), (sb, s3), (s2, s3)] {
                inputs.edge(a, b, ctx)?;
            }
            let mut nodes = Vec::new();
            for (label, parent, width) in [
                ("0", s1, 50.0),
                ("x", s1, 50.0),
                ("1", s1, 60.0),
                ("2", s1, 70.0),
                ("3", s2, 100.0),
                ("4", s2, 150.0),
                ("5", s3, 64.0),
                ("6", s3, 64.0),
                ("a", sb, 70.0),
                ("b", sb, 50.0),
            ] {
                nodes.push(inputs.node(label, parent, width, direction, ctx)?);
            }
            inputs.pairs(
                &nodes,
                &[(1, 3), (0, 3), (2, 3), (4, 5), (6, 7), (8, 9)],
                ctx,
            )?;
        }
        "flowEditor1" => {
            let diagram = inputs.group("Editor", None, ctx)?;
            let flow = inputs.group("Flow", Some(diagram), ctx)?;
            let mut nodes = Vec::new();
            for label in ["a1", "a2", "a4", "a5", "a6", "a7", "a8"] {
                nodes.push(inputs.node(label, diagram, 50.0, direction, ctx)?);
            }
            inputs.pairs(&nodes, &[(0, 1), (1, 2), (1, 3), (1, 4)], ctx)?;
            inputs.edge(nodes[4], flow, ctx)?;
            let x = inputs.node("x", flow, 50.0, direction, ctx)?;
            let y = inputs.node("y", flow, 50.0, direction, ctx)?;
            inputs.edge(x, y, ctx)?;
        }
        "flowEditor2" => {
            let diagram = inputs.group("Editor", None, ctx)?;
            let flow = inputs.group("Flow", Some(diagram), ctx)?;
            let a1 = inputs.node("a1", diagram, 50.0, direction, ctx)?;
            let a2 = inputs.node("a2", diagram, 50.0, direction, ctx)?;
            let a3 = inputs.node("a3", flow, 50.0, direction, ctx)?;
            let a4 = inputs.node("a4", flow, 50.0, direction, ctx)?;
            inputs.pairs(&[a1, a2, a3, a4], &[(0, 1), (1, 3), (2, 1)], ctx)?;
        }
        "ShortestPassCase" => {
            let parent = inputs.group("parent", None, ctx)?;
            let mut nodes = Vec::new();
            for label in ["a", "b", "c", "d", "e"] {
                nodes.push(inputs.node(label, parent, 50.0, direction, ctx)?);
            }
            inputs.pairs(
                &nodes,
                &[(0, 3), (0, 2), (1, 2), (1, 3), (1, 4), (2, 3), (2, 4)],
                ctx,
            )?;
        }
        "tangledSubgraphs" => {
            let mut groups = Vec::new();
            for label in ["Subgraph A", "Subgraph B", "Subgraph C", "Subgraph D"] {
                groups.push(inputs.group(label, None, ctx)?);
            }
            let mut nodes = Vec::new();
            for (i, labels) in [["a1", "a2"], ["b1", "b2"], ["c1", "c2"], ["d1", "d2"]]
                .iter()
                .enumerate()
            {
                for label in labels {
                    nodes.push(inputs.node(label, groups[i], 50.0, direction, ctx)?);
                }
                inputs.edge(nodes[i * 2], nodes[i * 2 + 1], ctx)?;
            }
            inputs.pairs(&nodes, &[(0, 7), (6, 5), (6, 3)], ctx)?;
        }
        "test1" | "test2" => {
            let s1 = inputs.group("Subgraph 1", None, ctx)?;
            let s2 = inputs.group("Subgraph 2", None, ctx)?;
            let nested = if name == "test2" {
                Some(inputs.group("Subgraph 1.1", Some(s1), ctx)?)
            } else {
                None
            };
            let mut nodes = Vec::new();
            for (label, parent, width) in [
                ("1", s1, 60.0),
                ("2", s1, 70.0),
                ("3", s1, 100.0),
                ("4", s2, 90.0),
                ("5", s2, 64.0),
                ("6", s2, 64.0),
                ("7", s2, 90.0),
            ] {
                nodes.push(inputs.node(label, parent, width, direction, ctx)?);
            }
            if let Some(parent) = nested {
                nodes.push(inputs.node("8", parent, 80.0, direction, ctx)?);
                inputs.pairs(
                    &nodes,
                    &[
                        (0, 1),
                        (1, 2),
                        (0, 7),
                        (0, 4),
                        (7, 2),
                        (3, 4),
                        (3, 5),
                        (5, 6),
                        (4, 6),
                        (1, 4),
                    ],
                    ctx,
                )?;
            } else {
                inputs.pairs(
                    &nodes,
                    &[(0, 2), (1, 2), (3, 4), (3, 5), (5, 6), (4, 6), (1, 4)],
                    ctx,
                )?;
            }
        }
        "flowChart" => {
            let diagram = inputs.group("diagram", None, ctx)?;
            let s1 = inputs.group("s1", Some(diagram), ctx)?;
            let mut groups = vec![s1];
            for label in ["s2", "s3", "s4", "s5", "s6", "s7", "s8"] {
                groups.push(inputs.group(label, Some(s1), ctx)?);
            }
            let outer1 = inputs.node("asdf", diagram, 50.0, direction, ctx)?;
            let outer2 = inputs.node("asfasdf", diagram, 50.0, direction, ctx)?;
            let outer3 = inputs.node("a3", diagram, 50.0, direction, ctx)?;
            for (a, b) in [(2, 5), (3, 6), (5, 7)] {
                inputs.edge(groups[a], groups[b], ctx)?;
            }
            inputs.edge(outer1, outer3, ctx)?;
            inputs.edge(outer3, s1, ctx)?;
            inputs.edge(outer2, s1, ctx)?;
            for (i, labels, pairs) in [
                (1, &["BMW", "Hawking", "Smurfy"][..], &[(0, 1), (0, 2)][..]),
                (2, &["Jammin", "This is it"][..], &[(0, 1)][..]),
                (3, &["catDog"][..], &[][..]),
                (4, &["a1", "a2"][..], &[(0, 1)][..]),
                (
                    5,
                    &["Hoop it up", "Streeball", "Downtown"][..],
                    &[(1, 2), (0, 1)][..],
                ),
                (6, &["Thing 1", "Thing 2"][..], &[(0, 1)][..]),
                (
                    7,
                    &["a1", "a2", "a3", "a4", "a5"][..],
                    &[(0, 2), (0, 3), (1, 2), (1, 4), (2, 4)][..],
                ),
            ] {
                let mut nodes = Vec::new();
                for label in labels {
                    nodes.push(inputs.node(label, groups[i], 50.0, direction, ctx)?);
                }
                inputs.pairs(&nodes, pairs, ctx)?;
            }
            let mut inner = Vec::new();
            for label in [
                "buckyball",
                "d",
                "cheese",
                "dingleberry",
                "dinosaur",
                "foobar",
                "t30",
                "a21",
                "katarina",
                "zig zag",
                "a16",
                "a23",
                "a17",
                "a20",
                "a19",
                "a24",
            ] {
                inner.push(inputs.node(label, s1, 50.0, direction, ctx)?);
            }
            inputs.pairs(
                &inner,
                &[(0, 2), (1, 3), (1, 2), (2, 4), (3, 4), (3, 5)],
                ctx,
            )?;
            inputs.edge(inner[5], groups[5], ctx)?;
            inputs.pairs(&inner, &[(4, 6), (6, 7)], ctx)?;
            inputs.edge(inner[7], groups[4], ctx)?;
            inputs.edge(groups[2], inner[8], ctx)?;
            inputs.edge(groups[3], inner[8], ctx)?;
            inputs.edge(inner[8], inner[9], ctx)?;
            inputs.edge(groups[6], inner[10], ctx)?;
            inputs.edge(groups[6], inner[11], ctx)?;
            inputs.pairs(
                &inner,
                &[(10, 12), (10, 13), (10, 14), (11, 14), (11, 15)],
                ctx,
            )?;
        }
        _ => return Err(GraphLayoutError::InvalidPayload("compound fixture")),
    }
    Ok(CompoundFixture {
        request: inputs.builder.build(ctx)?,
        labels: inputs.labels,
    })
}
