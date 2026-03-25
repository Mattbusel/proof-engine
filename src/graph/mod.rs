pub mod graph_core;
pub mod layout;
pub mod community;
pub mod pathfinding;
pub mod flow;
pub mod generators;
pub mod partition;
pub mod dynamics;
pub mod hypergraph;
pub mod level_gen;
pub mod neural_viz;

pub use graph_core::{
    Graph, GraphKind, NodeId, EdgeId, NodeData, EdgeData,
    AdjacencyMatrix, EdgeList,
};
pub use layout::{LayoutAlgorithm, LayoutConfig, compute_layout, ForceDirectedLayout};
pub use community::{Community, CommunityResult, louvain, label_propagation, modularity};
pub use pathfinding::{dijkstra, astar, bellman_ford, all_pairs_shortest, Path, PathVisualizer};
pub use flow::{FlowNetwork, FlowResult, ford_fulkerson, FlowVisualizer};
pub use generators::{
    watts_strogatz, barabasi_albert, erdos_renyi,
    complete_graph, cycle_graph, path_graph, star_graph,
    grid_graph, binary_tree, petersen_graph, complete_bipartite,
};
pub use partition::{spectral_partition, kernighan_lin, recursive_bisection, partition_quality};
pub use dynamics::{GraphAnimator, AnimationState};
pub use hypergraph::{Hypergraph, HyperedgeId};
pub use level_gen::{LevelGraph, RoomNode, generate_dungeon, corridor_path};
pub use neural_viz::{NeuralNetGraph, NeuronNode, SynapseEdge, build_feedforward};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_imports_work() {
        let g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        assert_eq!(g.node_count(), 0);
    }

    #[test]
    fn all_submodules_accessible() {
        let _id = NodeId(0);
        let _eid = EdgeId(0);
        let _hid = HyperedgeId(0);
    }
}
