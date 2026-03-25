//! Geodesic distance computation on surfaces — shortest path along a curved surface.

use glam::Vec3;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use super::GeoMesh;

/// Geodesic distance solver using Dijkstra on mesh edges.
pub struct GeodesicSolver;

#[derive(Debug, Clone)]
struct DijkNode { vertex: u32, dist: f32 }
impl PartialEq for DijkNode { fn eq(&self, other: &Self) -> bool { self.dist == other.dist } }
impl Eq for DijkNode {}
impl PartialOrd for DijkNode { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { other.dist.partial_cmp(&self.dist) } }
impl Ord for DijkNode { fn cmp(&self, other: &Self) -> Ordering { self.partial_cmp(other).unwrap_or(Ordering::Equal) } }

impl GeodesicSolver {
    /// Compute geodesic distances from a source vertex to all other vertices.
    pub fn distances(mesh: &GeoMesh, source: u32) -> Vec<f32> {
        let n = mesh.vertices.len();
        let adj = Self::build_adjacency(mesh);
        let mut dist = vec![f32::MAX; n];
        let mut heap = BinaryHeap::new();

        dist[source as usize] = 0.0;
        heap.push(DijkNode { vertex: source, dist: 0.0 });

        while let Some(DijkNode { vertex, dist: d }) = heap.pop() {
            if d > dist[vertex as usize] { continue; }
            if let Some(neighbors) = adj.get(&vertex) {
                for &(nb, edge_len) in neighbors {
                    let new_dist = d + edge_len;
                    if new_dist < dist[nb as usize] {
                        dist[nb as usize] = new_dist;
                        heap.push(DijkNode { vertex: nb, dist: new_dist });
                    }
                }
            }
        }
        dist
    }

    /// Compute shortest path between two vertices (sequence of vertex indices).
    pub fn shortest_path(mesh: &GeoMesh, from: u32, to: u32) -> Vec<u32> {
        let n = mesh.vertices.len();
        let adj = Self::build_adjacency(mesh);
        let mut dist = vec![f32::MAX; n];
        let mut prev = vec![u32::MAX; n];
        let mut heap = BinaryHeap::new();

        dist[from as usize] = 0.0;
        heap.push(DijkNode { vertex: from, dist: 0.0 });

        while let Some(DijkNode { vertex, dist: d }) = heap.pop() {
            if vertex == to { break; }
            if d > dist[vertex as usize] { continue; }
            if let Some(neighbors) = adj.get(&vertex) {
                for &(nb, edge_len) in neighbors {
                    let new_dist = d + edge_len;
                    if new_dist < dist[nb as usize] {
                        dist[nb as usize] = new_dist;
                        prev[nb as usize] = vertex;
                        heap.push(DijkNode { vertex: nb, dist: new_dist });
                    }
                }
            }
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = to;
        while current != u32::MAX && current != from {
            path.push(current);
            current = prev[current as usize];
        }
        if current == from { path.push(from); }
        path.reverse();
        path
    }

    fn build_adjacency(mesh: &GeoMesh) -> HashMap<u32, Vec<(u32, f32)>> {
        let mut adj: HashMap<u32, Vec<(u32, f32)>> = HashMap::new();
        for tri in &mesh.triangles {
            let edges = [(tri.a, tri.b), (tri.b, tri.c), (tri.c, tri.a)];
            for (a, b) in edges {
                let d = (mesh.vertices[a as usize] - mesh.vertices[b as usize]).length();
                adj.entry(a).or_default().push((b, d));
                adj.entry(b).or_default().push((a, d));
            }
        }
        adj
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn simple_quad_mesh() -> GeoMesh {
        let mut mesh = GeoMesh::new();
        mesh.add_vertex(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(1.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(1.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        mesh
    }

    #[test]
    fn distance_to_self_is_zero() {
        let mesh = simple_quad_mesh();
        let dists = GeodesicSolver::distances(&mesh, 0);
        assert_eq!(dists[0], 0.0);
    }

    #[test]
    fn distance_to_neighbor() {
        let mesh = simple_quad_mesh();
        let dists = GeodesicSolver::distances(&mesh, 0);
        assert!((dists[1] - 1.0).abs() < 0.01); // direct edge
    }

    #[test]
    fn shortest_path_basic() {
        let mesh = simple_quad_mesh();
        let path = GeodesicSolver::shortest_path(&mesh, 0, 2);
        assert!(!path.is_empty());
        assert_eq!(*path.first().unwrap(), 0);
        assert_eq!(*path.last().unwrap(), 2);
    }
}
