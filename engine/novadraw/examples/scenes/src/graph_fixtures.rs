//! Readonly source inputs from Draw2D GraphTests at 4463d9d0.
//! Pair order is the literal declaration order. The upstream shuffle changes no topology
//! and its signed-overflow expression can produce negative array indices.

use novadraw::geometry::{Dimension, Insets};
use novadraw::layout::graph::{
    AttachmentOffset, DIRECTED_GRAPH, DirectedGraphConfig, GraphAttachment, GraphDirection,
    GraphEdgeId, GraphLayoutError, GraphLayoutRequest, GraphNodeId, GraphRequestBuilder, GraphSize,
    GraphSolveContext, LAYERED_EDGE, LAYERED_RESULT, LayeredEdgeConstraint,
};

pub const DIRECTED_FIXTURES: &[&str] = &[
    "offsetTest",
    "anotherTour",
    "balanceThis1",
    "balanceThis2",
    "balanceThis3",
    "fourLevelBinaryTree",
    "graph1",
    "graph2",
    "graph3",
    "simpleGraph",
    "test1",
    "test2",
    "test3",
    "test4",
    "tinyGraph",
    "unstableGraph",
];

pub struct GraphFixture {
    pub request: GraphLayoutRequest,
    /// Labels follow the immutable node input order.
    pub labels: Vec<String>,
}

struct FixtureBuilder<'a> {
    builder: GraphRequestBuilder,
    nodes: Vec<GraphNodeId>,
    edge_ids: Vec<GraphEdgeId>,
    labels: Vec<String>,
    direction: GraphDirection,
    context: &'a mut GraphSolveContext,
}
impl<'a> FixtureBuilder<'a> {
    fn new(
        direction: GraphDirection,
        context: &'a mut GraphSolveContext,
    ) -> Result<Self, GraphLayoutError> {
        let mut builder = GraphRequestBuilder::new(
            direction,
            DIRECTED_GRAPH,
            DirectedGraphConfig::default(),
            context,
        )?;
        builder.default_padding(Insets::uniform(16.0));
        builder.require(LAYERED_RESULT, context)?;
        Ok(Self {
            builder,
            nodes: Vec::new(),
            edge_ids: Vec::new(),
            labels: Vec::new(),
            direction,
            context,
        })
    }
    fn node(&mut self, label: &str, width: f64) -> Result<usize, GraphLayoutError> {
        let index = self.nodes.len();
        self.nodes.push(self.builder.add_node(
            GraphSize::directed(Dimension::new(width, 40.0), self.direction),
            self.context,
        )?);
        self.labels.push(label.to_owned());
        Ok(index)
    }
    fn named(&mut self, labels: &[&str]) -> Result<(), GraphLayoutError> {
        for label in labels {
            self.node(label, 50.0)?;
        }
        Ok(())
    }
    fn edge(
        &mut self,
        source: usize,
        target: usize,
        span: u32,
        weight: u32,
    ) -> Result<(), GraphLayoutError> {
        let id = self
            .builder
            .add_edge(self.nodes[source], self.nodes[target], self.context)?;
        self.edge_ids.push(id);
        let edge = self.builder.edge_mut(id)?;
        edge.width = 1.0;
        edge.padding = 10.0;
        edge.constraints.insert(
            LAYERED_EDGE,
            LayeredEdgeConstraint {
                minimum_span: span,
                weight,
            },
            self.context,
        )?;
        Ok(())
    }
    fn edges(&mut self, pairs: &[(usize, usize)]) -> Result<(), GraphLayoutError> {
        for (source, target) in pairs {
            self.edge(*source, *target, 1, 1)?;
        }
        Ok(())
    }
    fn width(&mut self, index: usize, width: f64) -> Result<(), GraphLayoutError> {
        self.builder.node_mut(self.nodes[index])?.size =
            GraphSize::directed(Dimension::new(width, 40.0), self.direction);
        Ok(())
    }
    fn join(
        &mut self,
        first: &[Option<usize>],
        pairs: &[usize],
    ) -> Result<Vec<Option<usize>>, GraphLayoutError> {
        let count = pairs.chunks_exact(2).map(|pair| pair[1]).max().unwrap_or(0) + 1;
        let mut second = vec![None; count];
        // Upstream creates nodes during edge insertion, then adds nodes sorted by row number.
        // Preallocate in row-number order to reproduce the public NodeList identity order.
        for (index, node) in second.iter_mut().enumerate().skip(1) {
            if pairs.chunks_exact(2).any(|pair| pair[1] == index) {
                *node = Some(self.node(&format!("node{index}"), 78.0)?);
            }
        }
        for pair in pairs.chunks_exact(2) {
            self.edge(
                first[pair[0]].expect("source fixture row"),
                second[pair[1]].expect("target fixture row"),
                1,
                1,
            )?;
        }
        Ok(second)
    }
    fn finish(self) -> Result<GraphFixture, GraphLayoutError> {
        Ok(GraphFixture {
            request: self.builder.build(self.context)?,
            labels: self.labels,
        })
    }
}

pub fn directed_fixture(
    name: &str,
    direction: GraphDirection,
    context: &mut GraphSolveContext,
) -> Result<GraphFixture, GraphLayoutError> {
    let mut f = FixtureBuilder::new(direction, context)?;
    match name {
        "offsetTest" => {
            f.named(&[
                "Head", "node a1", "node a2", "node a3", "node a4", "node b1", "node b2",
                "node b3", "node b4",
            ])?;
            f.width(0, 100.0)?;
            f.edges(&[
                (0, 1),
                (0, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (1, 2),
                (2, 3),
                (3, 4),
            ])?;
            let attachment = |value| GraphAttachment {
                side: None,
                offset: AttachmentOffset::Distance(value),
            };
            // Edge creation order exactly matches the explicit Java edge sequence.
            for (index, value) in [(0, 10.0), (1, 90.0), (6, 10.0)] {
                f.builder.edge_mut(f.edge_ids[index])?.source_attachment = Some(attachment(value));
            }
            f.builder.edge_mut(f.edge_ids[6])?.target_attachment = Some(attachment(40.0));
            f.builder.node_mut(f.nodes[1])?.incoming = attachment(40.0);
            f.builder.node_mut(f.nodes[5])?.incoming = attachment(10.0);
            f.builder.node_mut(f.nodes[1])?.outgoing = attachment(10.0);
            f.builder.node_mut(f.nodes[2])?.incoming = attachment(40.0);
            f.builder.node_mut(f.nodes[3])?.outgoing = attachment(10.0);
            f.builder.node_mut(f.nodes[4])?.incoming = attachment(40.0);
        }
        "balanceThis1" => {
            f.named(&[
                "node b1", "node b2", "node b3", "node b4", "node b5", "node c", "node d",
                "node e", "node f", "node g", "node a",
            ])?;
            f.width(1, 90.0)?;
            f.width(2, 90.0)?;
            f.edges(&[
                (5, 6),
                (5, 7),
                (7, 8),
                (7, 9),
                (10, 0),
                (10, 1),
                (10, 2),
                (10, 3),
                (10, 4),
                (2, 6),
                (3, 8),
                (10, 5),
            ])?;
        }
        "balanceThis2" => {
            f.named(&[
                "node b2", "node b3", "node b4", "node c", "node d", "node e", "node f", "node g",
                "node a",
            ])?;
            f.edges(&[(4, 3), (5, 3), (6, 5), (7, 5), (0, 8), (1, 8)])?;
            f.edge(2, 8, 1, 2)?;
            f.edges(&[(4, 1), (6, 2), (3, 8)])?;
        }
        "balanceThis3" => {
            f.named(&[
                "Top 1", "Top 2", "node a1", "node a2", "node a3", "node a4", "node b1", "node b2",
                "node b3", "node b4", "node b5", "node b6", "node c1", "node c2", "node c3",
                "node c4", "Bottom 1", "Bottom 2",
            ])?;
            for index in [0, 1, 16, 17] {
                f.width(index, 400.0)?;
            }
            f.edges(&[
                (0, 2),
                (1, 5),
                (2, 6),
                (3, 6),
                (3, 8),
                (4, 8),
                (4, 10),
                (5, 10),
                (5, 11),
                (6, 12),
                (7, 12),
                (7, 13),
                (9, 13),
                (9, 14),
                (11, 14),
                (11, 15),
                (12, 16),
                (15, 17),
            ])?;
        }
        "simpleGraph" => {
            f.named(&[
                "node a", "node e", "node b", "node c", "node d", "node X", "node f", "node g",
                "node h", "node i",
            ])?;
            f.edges(&[
                (0, 9),
                (9, 7),
                (9, 8),
                (3, 9),
                (0, 2),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 1),
                (0, 6),
                (6, 7),
                (6, 8),
                (7, 1),
                (8, 1),
                (2, 7),
                (6, 4),
            ])?;
        }
        "test1" => {
            f.named(&[
                "node a", "node b", "node c", "node d", "node e", "node f", "node g",
            ])?;
            f.edges(&[(0, 3), (1, 3), (2, 3), (3, 4), (3, 5), (3, 6), (2, 6)])?;
        }
        "test2" => {
            f.named(&["node a", "node b", "node c", "node d", "node e", "node f"])?;
            f.edges(&[(0, 1), (0, 2), (3, 1), (0, 5), (4, 1)])?;
        }
        "test3" => {
            f.named(&["node a", "node b", "node c", "node d"])?;
            f.width(0, 80.0)?;
            f.width(3, 75.0)?;
            f.edges(&[(0, 1), (0, 2), (2, 3)])?;
        }
        "test4" => {
            f.named(&[
                "node a", "node b", "node c", "node d", "node e", "node f", "node g", "node h",
                "node i", "node j", "node k",
            ])?;
            f.edges(&[
                (0, 1),
                (0, 2),
                (0, 3),
                (1, 4),
                (2, 4),
                (3, 4),
                (3, 5),
                (3, 6),
                (4, 7),
                (5, 7),
                (6, 7),
                (6, 8),
                (6, 9),
                (7, 10),
                (8, 10),
                (9, 10),
            ])?;
        }
        "tinyGraph" => {
            f.named(&["node a", "node b", "node c", "node d", "node e", "node g"])?;
            f.edges(&[(0, 3), (2, 1)])?;
            f.edge(1, 5, 1, 3)?;
            f.edges(&[(2, 3), (3, 4), (3, 5)])?;
        }
        "unstableGraph" => {
            f.named(&[
                "node a0", "node b0", "node c0", "node a1", "node b1", "node c1", "node a2",
                "node b2", "node c2",
            ])?;
            for (source, target) in [(0, 3), (3, 6), (1, 4), (4, 7), (2, 5), (5, 8)] {
                f.edge(source, target, 1, 9)?;
            }
            f.edges(&[(0, 7), (1, 8), (2, 6)])?;
        }
        "fourLevelBinaryTree" => {
            let root = f.node("root", 50.0)?;
            let row = f.join(&[None, Some(root)], &[1, 1, 1, 2])?;
            let row = f.join(&row, &[1, 1, 1, 2, 2, 3, 2, 4])?;
            f.join(&row, &[1, 1, 1, 2, 2, 3, 2, 4, 3, 5, 3, 6, 4, 7, 4, 8])?;
        }
        "anotherTour" | "graph3" => {
            let width = if name == "graph3" { 120.0 } else { 50.0 };
            let row = [
                None,
                Some(f.node("h1", width)?),
                Some(f.node("h2", width)?),
                Some(f.node("h3", width)?),
            ];
            let row = f.join(&row, &[1, 1, 1, 2, 1, 3, 2, 3, 2, 4, 3, 3, 3, 4])?;
            let row = f.join(&row, &[1, 1, 3, 4, 2, 1, 2, 3, 3, 2, 4, 4])?;
            let row = f.join(&row, &[1, 1, 1, 2, 2, 1, 2, 4, 3, 3, 3, 4, 4, 3, 4, 5])?;
            if name == "graph3" {
                f.join(&row, &[1, 1, 2, 2, 4, 4, 5, 5, 1, 2, 4, 3, 5, 4])?;
            } else {
                f.join(&row, &[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 1, 2, 4, 3, 5, 4])?;
            }
        }
        "graph2" => {
            let row = [
                None,
                Some(f.node("h1", 160.0)?),
                Some(f.node("h2", 160.0)?),
                Some(f.node("h3", 160.0)?),
                Some(f.node("h4", 160.0)?),
            ];
            let row = f.join(&row, &[1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5])?;
            let row = f.join(&row, &[1, 1, 3, 3, 5, 2])?;
            let row = f.join(&row, &[1, 1, 2, 2])?;
            f.join(&row, &[1, 1, 1, 2, 2, 2, 2, 1])?;
        }
        "graph1" => {
            let row = [None, Some(f.node("h1", 50.0)?), Some(f.node("h2", 50.0)?)];
            let row = f.join(
                &row,
                &[1, 1, 1, 2, 1, 3, 1, 4, 1, 5, 1, 6, 2, 7, 2, 8, 2, 9, 2, 10],
            )?;
            let row = f.join(
                &row,
                &[
                    1, 1, 2, 2, 2, 3, 3, 1, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 9, 10, 10, 11,
                ],
            )?;
            let row = f.join(
                &row,
                &[
                    1, 1, 1, 2, 2, 3, 2, 6, 3, 6, 4, 4, 5, 5, 6, 7, 7, 6, 8, 8, 9, 6, 10, 9, 10,
                    10, 11, 11,
                ],
            )?;
            let row = f.join(
                &row,
                &[
                    1, 1, 1, 2, 1, 3, 2, 3, 3, 4, 4, 5, 5, 6, 7, 3, 8, 7, 9, 8, 10, 9, 10, 10, 11,
                    11,
                ],
            )?;
            let row = f.join(
                &row,
                &[1, 1, 2, 2, 4, 3, 5, 4, 6, 5, 7, 6, 8, 7, 9, 7, 10, 7, 11, 7],
            )?;
            let row = f.join(&row, &[1, 1, 2, 2, 3, 4, 4, 4, 5, 3, 6, 4, 7, 4, 7, 5])?;
            let row = f.join(&row, &[1, 2, 2, 1, 3, 2, 4, 2, 4, 3, 5, 4])?;
            f.join(&row, &[1, 1, 2, 1, 3, 1, 4, 1])?;
        }
        _ => return Err(GraphLayoutError::InvalidPayload("directed fixture name")),
    }
    f.finish()
}
