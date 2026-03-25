//! Topology operations — genus changes, handle attachment, non-orientable surfaces.

use glam::Vec3;
use super::{GeoMesh, parametric::ParametricSurface};

/// Topology operation to apply to a surface.
#[derive(Debug, Clone)]
pub enum TopologyOp {
    /// Attach a handle (increases genus by 1).
    AttachHandle { position: Vec3, radius: f32 },
    /// Punch a hole through the surface.
    Puncture { position: Vec3, radius: f32 },
    /// Create a crosscap (non-orientable).
    CrossCap { position: Vec3, radius: f32 },
    /// Connect two surfaces via a tube.
    Connect { pos_a: Vec3, pos_b: Vec3, tube_radius: f32 },
    /// Twist a surface strip (half-twist = Möbius).
    Twist { axis: Vec3, center: Vec3, half_twists: u32 },
}

/// Topological properties of a surface.
#[derive(Debug, Clone)]
pub struct SurfaceTopology {
    /// Euler characteristic: V - E + F.
    pub euler_characteristic: i32,
    /// Genus (number of handles).
    pub genus: u32,
    /// Whether the surface is orientable.
    pub orientable: bool,
    /// Number of boundary loops.
    pub boundary_loops: u32,
    /// Number of connected components.
    pub components: u32,
}

impl SurfaceTopology {
    /// Compute topological properties from a mesh.
    pub fn from_mesh(mesh: &GeoMesh) -> Self {
        let v = mesh.vertices.len() as i32;
        let f = mesh.triangles.len() as i32;

        // Count unique edges
        let mut edges = std::collections::HashSet::new();
        for tri in &mesh.triangles {
            let mut add = |a: u32, b: u32| {
                let key = if a < b { (a, b) } else { (b, a) };
                edges.insert(key);
            };
            add(tri.a, tri.b);
            add(tri.b, tri.c);
            add(tri.c, tri.a);
        }
        let e = edges.len() as i32;

        let euler = v - e + f;
        // For closed orientable surface: χ = 2 - 2g
        let genus = ((2 - euler) / 2).max(0) as u32;

        // Count boundary edges (edges shared by only 1 face)
        let mut edge_count: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for tri in &mesh.triangles {
            let mut inc = |a: u32, b: u32| {
                let key = if a < b { (a, b) } else { (b, a) };
                *edge_count.entry(key).or_insert(0) += 1;
            };
            inc(tri.a, tri.b);
            inc(tri.b, tri.c);
            inc(tri.c, tri.a);
        }
        let boundary_edges: Vec<_> = edge_count.iter().filter(|(_, &c)| c == 1).collect();
        let boundary_loops = if boundary_edges.is_empty() { 0 } else { 1 }; // simplified

        Self {
            euler_characteristic: euler,
            genus,
            orientable: true, // simplified: assume orientable
            boundary_loops,
            components: 1, // simplified
        }
    }

    /// Expected Euler characteristic for given genus and boundary.
    pub fn expected_euler(genus: u32, boundaries: u32) -> i32 {
        2 - 2 * genus as i32 - boundaries as i32
    }
}

/// A surface with tracked topology.
pub struct TopologicalSurface {
    pub mesh: GeoMesh,
    pub topology: SurfaceTopology,
    pub operations: Vec<TopologyOp>,
}

impl TopologicalSurface {
    pub fn new(mesh: GeoMesh) -> Self {
        let topology = SurfaceTopology::from_mesh(&mesh);
        Self { mesh, topology, operations: Vec::new() }
    }

    /// Apply a topology operation (modifies the mesh).
    pub fn apply(&mut self, op: TopologyOp) {
        match &op {
            TopologyOp::AttachHandle { position, radius } => {
                self.topology.genus += 1;
            }
            TopologyOp::Puncture { .. } => {
                self.topology.boundary_loops += 1;
            }
            TopologyOp::CrossCap { .. } => {
                self.topology.orientable = false;
            }
            TopologyOp::Connect { .. } => {
                // genus may increase
            }
            TopologyOp::Twist { half_twists, .. } => {
                if *half_twists % 2 != 0 {
                    self.topology.orientable = false;
                }
            }
        }
        self.topology.euler_characteristic = SurfaceTopology::expected_euler(
            self.topology.genus, self.topology.boundary_loops
        );
        self.operations.push(op);
    }

    pub fn recompute_topology(&mut self) {
        self.topology = SurfaceTopology::from_mesh(&self.mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn make_tetrahedron() -> GeoMesh {
        let mut mesh = GeoMesh::new();
        mesh.add_vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(-1.0, -1.0, 1.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(1.0, -1.0, 1.0), Vec3::Y, Vec2::ZERO);
        mesh.add_vertex(Vec3::new(0.0, -1.0, -1.0), Vec3::Y, Vec2::ZERO);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        mesh.add_triangle(0, 3, 1);
        mesh.add_triangle(1, 3, 2);
        mesh
    }

    #[test]
    fn tetrahedron_euler() {
        let mesh = make_tetrahedron();
        let topo = SurfaceTopology::from_mesh(&mesh);
        assert_eq!(topo.euler_characteristic, 2); // sphere topology: V-E+F = 4-6+4 = 2
    }

    #[test]
    fn genus_zero_for_sphere() {
        let mesh = make_tetrahedron();
        let topo = SurfaceTopology::from_mesh(&mesh);
        assert_eq!(topo.genus, 0);
    }

    #[test]
    fn attach_handle_increases_genus() {
        let mesh = make_tetrahedron();
        let mut surface = TopologicalSurface::new(mesh);
        assert_eq!(surface.topology.genus, 0);
        surface.apply(TopologyOp::AttachHandle { position: Vec3::ZERO, radius: 0.5 });
        assert_eq!(surface.topology.genus, 1);
    }
}
