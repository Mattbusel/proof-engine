use glam::{Vec2, Vec4};
use std::collections::HashMap;
use super::graph_core::{Graph, GraphKind, NodeId, EdgeId};

#[derive(Debug, Clone)]
pub struct NeuronNode {
    pub layer: usize,
    pub index: usize,
    pub activation: f32,
    pub bias: f32,
}

#[derive(Debug, Clone)]
pub struct SynapseEdge {
    pub weight: f32,
    pub gradient: f32,
}

#[derive(Debug, Clone)]
pub struct NeuralNetGraph {
    pub graph: Graph<NeuronNode, SynapseEdge>,
    /// layer_index -> Vec<NodeId> of neurons in that layer
    pub layers: Vec<Vec<NodeId>>,
    pub layer_count: usize,
}

impl NeuralNetGraph {
    pub fn neuron_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn synapse_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn get_neuron(&self, id: NodeId) -> Option<&NeuronNode> {
        self.graph.get_node(id).map(|nd| &nd.data)
    }

    pub fn get_synapse(&self, id: EdgeId) -> Option<&SynapseEdge> {
        self.graph.get_edge(id).map(|ed| &ed.data)
    }

    /// Set activations for a specific layer.
    pub fn set_activations(&mut self, layer: usize, values: &[f32]) {
        if layer >= self.layers.len() { return; }
        let layer_nodes = &self.layers[layer];
        for (i, &nid) in layer_nodes.iter().enumerate() {
            if i < values.len() {
                if let Some(nd) = self.graph.get_node_mut(nid) {
                    nd.data.activation = values[i];
                }
            }
        }
    }

    /// Set all weights for edges between two layers.
    pub fn set_weights(&mut self, from_layer: usize, weights: &[Vec<f32>]) {
        if from_layer + 1 >= self.layers.len() { return; }
        let from_nodes = &self.layers[from_layer].clone();
        let to_nodes = &self.layers[from_layer + 1].clone();

        for (i, &from_nid) in from_nodes.iter().enumerate() {
            if i >= weights.len() { break; }
            for (j, &to_nid) in to_nodes.iter().enumerate() {
                if j >= weights[i].len() { break; }
                if let Some(eid) = self.graph.find_edge(from_nid, to_nid) {
                    if let Some(ed) = self.graph.get_edge_mut(eid) {
                        ed.data.weight = weights[i][j];
                    }
                }
            }
        }
    }

    /// Forward pass: compute activations using sigmoid.
    pub fn forward(&mut self, inputs: &[f32]) {
        self.set_activations(0, inputs);

        for l in 1..self.layer_count {
            let prev_layer = self.layers[l - 1].clone();
            let curr_layer = self.layers[l].clone();

            for &to_nid in &curr_layer {
                let bias = self.graph.get_node(to_nid).map(|n| n.data.bias).unwrap_or(0.0);
                let mut sum = bias;
                for &from_nid in &prev_layer {
                    if let Some(eid) = self.graph.find_edge(from_nid, to_nid) {
                        let w = self.graph.get_edge(eid).map(|e| e.data.weight).unwrap_or(0.0);
                        let a = self.graph.get_node(from_nid).map(|n| n.data.activation).unwrap_or(0.0);
                        sum += w * a;
                    }
                }
                // Sigmoid activation
                let activation = 1.0 / (1.0 + (-sum).exp());
                if let Some(nd) = self.graph.get_node_mut(to_nid) {
                    nd.data.activation = activation;
                }
            }
        }
    }

    /// Get output activations (last layer).
    pub fn outputs(&self) -> Vec<f32> {
        self.layers.last()
            .map(|layer| {
                layer.iter()
                    .map(|&nid| self.graph.get_node(nid).map(|n| n.data.activation).unwrap_or(0.0))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Generate layout: neurons in vertical columns per layer.
    pub fn compute_layout(&self, bounds: Vec2) -> HashMap<NodeId, Vec2> {
        let mut positions = HashMap::new();
        if self.layer_count == 0 { return positions; }

        let layer_spacing = bounds.x / (self.layer_count + 1) as f32;

        for (l, layer_nodes) in self.layers.iter().enumerate() {
            let n = layer_nodes.len();
            let neuron_spacing = bounds.y / (n + 1) as f32;
            let x = layer_spacing * (l + 1) as f32;
            for (i, &nid) in layer_nodes.iter().enumerate() {
                let y = neuron_spacing * (i + 1) as f32;
                positions.insert(nid, Vec2::new(x, y));
            }
        }
        positions
    }

    /// Generate rendering data: node brightness = activation, edge thickness = |weight|, edge color by sign.
    pub fn render_data(&self) -> NeuralRenderData {
        let mut node_data = Vec::new();
        for layer in &self.layers {
            for &nid in layer {
                if let Some(nd) = self.graph.get_node(nid) {
                    let brightness = nd.data.activation;
                    node_data.push(NeuronRender {
                        node_id: nid,
                        position: nd.position,
                        brightness,
                        radius: 8.0 + brightness * 4.0,
                        color: Vec4::new(brightness, brightness, 1.0, 1.0),
                    });
                }
            }
        }

        let mut edge_data = Vec::new();
        for edge in self.graph.edges() {
            let w = edge.data.weight;
            let thickness = w.abs().min(5.0);
            let color = if w >= 0.0 {
                Vec4::new(0.2, 0.8, 0.2, 0.8) // green = positive
            } else {
                Vec4::new(0.8, 0.2, 0.2, 0.8) // red = negative
            };
            edge_data.push(SynapseRender {
                edge_id: edge.id,
                from: edge.from,
                to: edge.to,
                thickness,
                color,
                weight: w,
            });
        }

        NeuralRenderData { neurons: node_data, synapses: edge_data }
    }
}

#[derive(Debug, Clone)]
pub struct NeuronRender {
    pub node_id: NodeId,
    pub position: Vec2,
    pub brightness: f32,
    pub radius: f32,
    pub color: Vec4,
}

#[derive(Debug, Clone)]
pub struct SynapseRender {
    pub edge_id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub thickness: f32,
    pub color: Vec4,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct NeuralRenderData {
    pub neurons: Vec<NeuronRender>,
    pub synapses: Vec<SynapseRender>,
}

/// Build a feedforward neural network graph.
/// `layer_sizes`: number of neurons in each layer, e.g. [3, 4, 2] = 3 inputs, 4 hidden, 2 outputs.
pub fn build_feedforward(layer_sizes: &[usize]) -> NeuralNetGraph {
    let mut graph = Graph::new(GraphKind::Directed);
    let mut layers: Vec<Vec<NodeId>> = Vec::new();

    let bounds = Vec2::new(800.0, 600.0);
    let layer_count = layer_sizes.len();
    let layer_spacing = bounds.x / (layer_count + 1) as f32;

    for (l, &size) in layer_sizes.iter().enumerate() {
        let mut layer_nodes = Vec::new();
        let neuron_spacing = bounds.y / (size + 1) as f32;
        let x = layer_spacing * (l + 1) as f32;

        for i in 0..size {
            let y = neuron_spacing * (i + 1) as f32;
            let neuron = NeuronNode {
                layer: l,
                index: i,
                activation: 0.0,
                bias: 0.0,
            };
            let nid = graph.add_node_with_pos(neuron, Vec2::new(x, y));
            layer_nodes.push(nid);
        }
        layers.push(layer_nodes);
    }

    // Connect consecutive layers (fully connected)
    let mut seed_counter: u64 = 42;
    for l in 0..(layer_count - 1) {
        let from_layer = layers[l].clone();
        let to_layer = layers[l + 1].clone();
        for &from in &from_layer {
            for &to in &to_layer {
                // Initialize with small random weights
                let w = (pseudo_random(seed_counter, 0) as f32 - 0.5) * 0.5;
                seed_counter += 1;
                let synapse = SynapseEdge { weight: w, gradient: 0.0 };
                graph.add_edge(from, to, synapse);
            }
        }
    }

    NeuralNetGraph {
        graph,
        layers,
        layer_count,
    }
}

fn pseudo_random(seed: u64, i: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(i.wrapping_mul(1442695040888963407));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_feedforward_structure() {
        let nn = build_feedforward(&[3, 4, 2]);
        assert_eq!(nn.layer_count, 3);
        assert_eq!(nn.layers[0].len(), 3);
        assert_eq!(nn.layers[1].len(), 4);
        assert_eq!(nn.layers[2].len(), 2);
        assert_eq!(nn.neuron_count(), 9);
        assert_eq!(nn.synapse_count(), 3 * 4 + 4 * 2); // 20
    }

    #[test]
    fn test_set_activations() {
        let mut nn = build_feedforward(&[2, 3, 1]);
        nn.set_activations(0, &[0.5, 0.8]);
        let n0 = nn.get_neuron(nn.layers[0][0]).unwrap();
        let n1 = nn.get_neuron(nn.layers[0][1]).unwrap();
        assert!((n0.activation - 0.5).abs() < 1e-6);
        assert!((n1.activation - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_forward_pass() {
        let mut nn = build_feedforward(&[2, 2, 1]);
        nn.forward(&[1.0, 1.0]);
        let outputs = nn.outputs();
        assert_eq!(outputs.len(), 1);
        // Output should be between 0 and 1 (sigmoid)
        assert!(outputs[0] >= 0.0 && outputs[0] <= 1.0);
    }

    #[test]
    fn test_layout() {
        let nn = build_feedforward(&[3, 4, 2]);
        let bounds = Vec2::new(800.0, 600.0);
        let layout = nn.compute_layout(bounds);
        assert_eq!(layout.len(), 9);
        for pos in layout.values() {
            assert!(pos.x > 0.0 && pos.x < bounds.x);
            assert!(pos.y > 0.0 && pos.y < bounds.y);
        }
    }

    #[test]
    fn test_render_data() {
        let mut nn = build_feedforward(&[2, 3, 1]);
        nn.forward(&[0.5, 0.5]);
        let render = nn.render_data();
        assert_eq!(render.neurons.len(), 6);
        assert_eq!(render.synapses.len(), 2 * 3 + 3 * 1);
        for n in &render.neurons {
            assert!(n.brightness >= 0.0 && n.brightness <= 1.0);
        }
    }

    #[test]
    fn test_single_layer() {
        let nn = build_feedforward(&[5]);
        assert_eq!(nn.layer_count, 1);
        assert_eq!(nn.neuron_count(), 5);
        assert_eq!(nn.synapse_count(), 0);
    }

    #[test]
    fn test_deep_network() {
        let nn = build_feedforward(&[4, 8, 8, 4, 2]);
        assert_eq!(nn.layer_count, 5);
        assert_eq!(nn.neuron_count(), 26);
        assert_eq!(nn.synapse_count(), 4 * 8 + 8 * 8 + 8 * 4 + 4 * 2);
    }

    #[test]
    fn test_set_weights() {
        let mut nn = build_feedforward(&[2, 2]);
        nn.set_weights(0, &[vec![1.0, -1.0], vec![0.5, 0.5]]);
        nn.forward(&[1.0, 0.0]);
        let outputs = nn.outputs();
        // neuron 0: sigmoid(1.0 * 1.0 + 0.0 * 0.5) = sigmoid(1.0) ~ 0.731
        // neuron 1: sigmoid(1.0 * -1.0 + 0.0 * 0.5) = sigmoid(-1.0) ~ 0.269
        assert!((outputs[0] - 0.731).abs() < 0.01);
        assert!((outputs[1] - 0.269).abs() < 0.01);
    }

    #[test]
    fn test_neuron_node_fields() {
        let nn = build_feedforward(&[3, 2]);
        let n = nn.get_neuron(nn.layers[0][1]).unwrap();
        assert_eq!(n.layer, 0);
        assert_eq!(n.index, 1);
        assert_eq!(n.activation, 0.0);
    }

    #[test]
    fn test_render_edge_colors() {
        let mut nn = build_feedforward(&[1, 1]);
        nn.set_weights(0, &[vec![1.0]]);
        let render = nn.render_data();
        // Positive weight -> green
        assert!(render.synapses[0].color.y > render.synapses[0].color.x);

        nn.set_weights(0, &[vec![-1.0]]);
        let render = nn.render_data();
        // Negative weight -> red
        assert!(render.synapses[0].color.x > render.synapses[0].color.y);
    }
}
