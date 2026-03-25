//! Scene graph — manages all active glyphs, entities, particles, and force fields.
//! Full implementation with BVH spatial index, scene queries, serialization,
//! transform hierarchy, layer management, event system, and portals.

pub mod node;
pub mod field_manager;
pub mod spawn_system;
pub mod bvh;
pub mod query;
pub mod events;

use crate::glyph::{Glyph, GlyphId, GlyphPool};
use crate::entity::{AmorphousEntity, EntityId};
use crate::particle::ParticlePool;
use crate::math::ForceField;
use glam::{Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

pub use bvh::{Bvh, BvhNode, Aabb};
pub use query::{SceneQuery, RaycastHit, FrustumQuery, SphereQuery};
pub use events::{SceneEvent, SceneEventQueue, EventKind};

// ─── IDs and handles ──────────────────────────────────────────────────────────

/// Opaque ID for a force field in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId(pub u32);

/// Opaque ID for a scene node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Opaque ID for a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub u8);

impl LayerId {
    pub const BACKGROUND: Self = LayerId(0);
    pub const TERRAIN:    Self = LayerId(1);
    pub const WORLD:      Self = LayerId(2);
    pub const ENTITIES:   Self = LayerId(3);
    pub const PARTICLES:  Self = LayerId(4);
    pub const UI:         Self = LayerId(5);
    pub const DEBUG:      Self = LayerId(6);
}

/// Opaque ID for a portal (scene transition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortalId(pub u32);

// ─── Scene node (transform hierarchy) ────────────────────────────────────────

/// A node in the scene hierarchy, holding a local transform.
#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id:       NodeId,
    pub parent:   Option<NodeId>,
    pub children: Vec<NodeId>,
    pub local:    Transform3D,
    pub world:    Mat4,  // cached world transform
    pub dirty:    bool,
    pub name:     String,
    pub visible:  bool,
    pub layer:    LayerId,
    pub tag:      u32,
    pub glyph_ids: Vec<GlyphId>,
}

impl SceneNode {
    pub fn new(id: NodeId, name: impl Into<String>) -> Self {
        Self {
            id,
            parent:    None,
            children:  Vec::new(),
            local:     Transform3D::identity(),
            world:     Mat4::IDENTITY,
            dirty:     true,
            name:      name.into(),
            visible:   true,
            layer:     LayerId::WORLD,
            tag:       0,
            glyph_ids: Vec::new(),
        }
    }

    pub fn with_position(mut self, pos: Vec3) -> Self {
        self.local.position = pos;
        self.dirty = true;
        self
    }

    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.local.scale = scale;
        self.dirty = true;
        self
    }

    pub fn with_rotation(mut self, rot: Quat) -> Self {
        self.local.rotation = rot;
        self.dirty = true;
        self
    }

    pub fn local_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.local.scale,
            self.local.rotation,
            self.local.position,
        )
    }
}

/// Decomposed 3-D transform.
#[derive(Debug, Clone, PartialEq)]
pub struct Transform3D {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale:    Vec3,
}

impl Transform3D {
    pub fn identity() -> Self {
        Self { position: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE }
    }

    pub fn from_position(p: Vec3) -> Self {
        Self { position: p, ..Self::identity() }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            position: self.position.lerp(other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale:    self.scale.lerp(other.scale, t),
        }
    }

    pub fn inverse(&self) -> Self {
        let inv_scale = Vec3::new(1.0 / self.scale.x, 1.0 / self.scale.y, 1.0 / self.scale.z);
        let inv_rot   = self.rotation.inverse();
        let inv_pos   = inv_rot * (-self.position * inv_scale);
        Self { position: inv_pos, rotation: inv_rot, scale: inv_scale }
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.position + self.rotation * (self.scale * p)
    }

    pub fn transform_direction(&self, d: Vec3) -> Vec3 {
        self.rotation * d
    }
}

// ─── Scene layer ──────────────────────────────────────────────────────────────

/// A render/logic layer controlling visibility and sort order.
#[derive(Debug, Clone)]
pub struct SceneLayer {
    pub id:       LayerId,
    pub name:     String,
    pub visible:  bool,
    pub z_order:  i32,
    pub opaque:   bool,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
}

impl SceneLayer {
    pub fn new(id: LayerId, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), visible: true, z_order: 0, opaque: true,
               cast_shadows: true, receive_shadows: true }
    }
}

// ─── Scene portal ─────────────────────────────────────────────────────────────

/// A portal connects two points in space and renders the view from the other side.
#[derive(Debug, Clone)]
pub struct Portal {
    pub id:        PortalId,
    pub origin:    Vec3,
    pub target:    Vec3,
    pub normal:    Vec3,
    pub extent:    Vec2,
    pub active:    bool,
    pub linked:    Option<PortalId>,
}

use glam::Vec2;

// ─── Ambient zone ─────────────────────────────────────────────────────────────

/// Axis-aligned region with ambient light/audio/fog properties.
#[derive(Debug, Clone)]
pub struct AmbientZone {
    pub min:             Vec3,
    pub max:             Vec3,
    pub ambient_color:   Vec4,
    pub fog_density:     f32,
    pub fog_color:       Vec4,
    pub reverb_wet:      f32,
    pub wind_strength:   f32,
    pub gravity_scale:   f32,
    pub name:            String,
}

impl AmbientZone {
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn blend_factor(&self, p: Vec3) -> f32 {
        // Smooth fade near edges (within 2 units)
        let margin = 2.0;
        let dx = ((p.x - self.min.x).min(self.max.x - p.x) / margin).clamp(0.0, 1.0);
        let dy = ((p.y - self.min.y).min(self.max.y - p.y) / margin).clamp(0.0, 1.0);
        let dz = ((p.z - self.min.z).min(self.max.z - p.z) / margin).clamp(0.0, 1.0);
        dx.min(dy).min(dz)
    }
}

// ─── Scene serialization ──────────────────────────────────────────────────────

/// Serializable snapshot of the scene state.
#[derive(Debug, Clone)]
pub struct SceneSnapshot {
    pub time:       f32,
    pub glyph_count: usize,
    pub entity_count: usize,
    pub field_count: usize,
    pub node_count:  usize,
    /// Flat list of active glyph positions + colors for quick diffing.
    pub glyph_positions: Vec<[f32; 3]>,
    pub entity_positions: Vec<[f32; 3]>,
    pub field_positions:  Vec<[f32; 3]>,
}

impl SceneSnapshot {
    pub fn diff(&self, other: &SceneSnapshot) -> SnapshotDiff {
        SnapshotDiff {
            glyph_delta:  other.glyph_count as i32  - self.glyph_count as i32,
            entity_delta: other.entity_count as i32 - self.entity_count as i32,
            field_delta:  other.field_count as i32  - self.field_count as i32,
            time_delta:   other.time - self.time,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    pub glyph_delta:  i32,
    pub entity_delta: i32,
    pub field_delta:  i32,
    pub time_delta:   f32,
}

// ─── Scene statistics ─────────────────────────────────────────────────────────

/// Frame statistics for the scene.
#[derive(Debug, Clone, Default)]
pub struct SceneStats {
    pub glyph_count:    usize,
    pub particle_count: usize,
    pub entity_count:   usize,
    pub field_count:    usize,
    pub node_count:     usize,
    pub portal_count:   usize,
    pub zone_count:     usize,
    pub visible_glyphs: usize,
    pub culled_glyphs:  usize,
    pub tick_count:     u64,
    pub elapsed_secs:   f32,
}

// ─── Scene ────────────────────────────────────────────────────────────────────

/// The complete scene: all renderable objects and active forces.
pub struct Scene {
    pub glyphs:    GlyphPool,
    pub particles: ParticlePool,
    pub entities:  Vec<(EntityId, AmorphousEntity)>,
    pub fields:    Vec<(FieldId, ForceField)>,
    next_field_id:  u32,
    next_entity_id: u32,
    next_node_id:   u32,
    next_portal_id: u32,
    pub time:       f32,

    // Hierarchy
    pub nodes:     HashMap<NodeId, SceneNode>,
    pub root_nodes: Vec<NodeId>,

    // Layers
    pub layers:    [SceneLayer; 8],

    // Ambient zones
    pub zones:     Vec<AmbientZone>,

    // Portals
    pub portals:   Vec<Portal>,

    // Spatial index (rebuilt on demand)
    bvh_dirty:     bool,
    pub bvh:       Option<Bvh>,

    // Events
    pub events:    SceneEventQueue,

    // Stats
    pub stats:     SceneStats,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            glyphs:       GlyphPool::new(8192),
            particles:    ParticlePool::new(4096),
            entities:     Vec::new(),
            fields:       Vec::new(),
            next_field_id:  0,
            next_entity_id: 0,
            next_node_id:   1,
            next_portal_id: 0,
            time:           0.0,
            nodes:          HashMap::new(),
            root_nodes:     Vec::new(),
            layers:         [
                SceneLayer::new(LayerId::BACKGROUND, "Background"),
                SceneLayer::new(LayerId::TERRAIN,    "Terrain"),
                SceneLayer::new(LayerId::WORLD,      "World"),
                SceneLayer::new(LayerId::ENTITIES,   "Entities"),
                SceneLayer::new(LayerId::PARTICLES,  "Particles"),
                SceneLayer::new(LayerId::UI,         "UI"),
                SceneLayer::new(LayerId::DEBUG,      "Debug"),
                SceneLayer::new(LayerId(7),          "Overlay"),
            ],
            zones:          Vec::new(),
            portals:        Vec::new(),
            bvh_dirty:      false,
            bvh:            None,
            events:         SceneEventQueue::new(),
            stats:          SceneStats::default(),
        }
    }

    // ── Glyph management ────────────────────────────────────────────────────

    pub fn spawn_glyph(&mut self, glyph: Glyph) -> GlyphId {
        self.bvh_dirty = true;
        self.glyphs.spawn(glyph)
    }

    pub fn despawn_glyph(&mut self, id: GlyphId) {
        self.glyphs.despawn(id);
        self.bvh_dirty = true;
    }

    pub fn get_glyph(&self, id: GlyphId) -> Option<&Glyph> {
        self.glyphs.get(id)
    }

    pub fn get_glyph_mut(&mut self, id: GlyphId) -> Option<&mut Glyph> {
        self.bvh_dirty = true;
        self.glyphs.get_mut(id)
    }

    // ── Entity management ────────────────────────────────────────────────────

    pub fn spawn_entity(&mut self, entity: AmorphousEntity) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        self.events.push(SceneEvent { kind: EventKind::EntitySpawned(id), time: self.time });
        self.entities.push((id, entity));
        self.bvh_dirty = true;
        id
    }

    pub fn despawn_entity(&mut self, id: EntityId) {
        self.entities.retain(|(eid, _)| *eid != id);
        self.events.push(SceneEvent { kind: EventKind::EntityDespawned(id), time: self.time });
        self.bvh_dirty = true;
    }

    pub fn get_entity(&self, id: EntityId) -> Option<&AmorphousEntity> {
        self.entities.iter().find(|(eid, _)| *eid == id).map(|(_, e)| e)
    }

    pub fn get_entity_mut(&mut self, id: EntityId) -> Option<&mut AmorphousEntity> {
        self.entities.iter_mut().find(|(eid, _)| *eid == id).map(|(_, e)| e)
    }

    // ── Field management ─────────────────────────────────────────────────────

    pub fn add_field(&mut self, field: ForceField) -> FieldId {
        let id = FieldId(self.next_field_id);
        self.next_field_id += 1;
        self.fields.push((id, field));
        id
    }

    pub fn remove_field(&mut self, id: FieldId) {
        self.fields.retain(|(fid, _)| *fid != id);
    }

    pub fn get_field(&self, id: FieldId) -> Option<&ForceField> {
        self.fields.iter().find(|(fid, _)| *fid == id).map(|(_, f)| f)
    }

    pub fn get_field_mut(&mut self, id: FieldId) -> Option<&mut ForceField> {
        self.fields.iter_mut().find(|(fid, _)| *fid == id).map(|(_, f)| f)
    }

    // ── Node hierarchy ───────────────────────────────────────────────────────

    pub fn create_node(&mut self, name: impl Into<String>) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let node = SceneNode::new(id, name);
        self.nodes.insert(id, node);
        self.root_nodes.push(id);
        id
    }

    pub fn destroy_node(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.remove(&id) {
            // Detach glyphs
            for glyph_id in &node.glyph_ids {
                self.glyphs.despawn(*glyph_id);
            }
            // Detach from parent
            if let Some(parent_id) = node.parent {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.children.retain(|c| *c != id);
                }
            } else {
                self.root_nodes.retain(|r| *r != id);
            }
            // Recursively destroy children
            let children: Vec<NodeId> = node.children.clone();
            for child in children {
                self.destroy_node(child);
            }
        }
    }

    pub fn attach_node(&mut self, child: NodeId, parent: NodeId) {
        // Remove from current root/parent
        if let Some(node) = self.nodes.get(&child) {
            if let Some(old_parent) = node.parent {
                if let Some(p) = self.nodes.get_mut(&old_parent) {
                    p.children.retain(|c| *c != child);
                }
            } else {
                self.root_nodes.retain(|r| *r != child);
            }
        }
        // Set new parent
        if let Some(node) = self.nodes.get_mut(&child) {
            node.parent = Some(parent);
            node.dirty  = true;
        }
        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.push(child);
        }
    }

    pub fn detach_node(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.parent = None;
            node.dirty  = true;
        }
        self.root_nodes.push(id);
    }

    pub fn get_node(&self, id: NodeId) -> Option<&SceneNode> {
        self.nodes.get(&id)
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut SceneNode> {
        self.nodes.get_mut(&id)
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<NodeId> {
        self.nodes.values().find(|n| n.name == name).map(|n| n.id)
    }

    pub fn find_nodes_by_tag(&self, tag: u32) -> Vec<NodeId> {
        self.nodes.values().filter(|n| n.tag == tag).map(|n| n.id).collect()
    }

    /// Attach a glyph to a node so it moves with it.
    pub fn attach_glyph_to_node(&mut self, glyph_id: GlyphId, node_id: NodeId) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.glyph_ids.push(glyph_id);
        }
    }

    // ── Layers ───────────────────────────────────────────────────────────────

    pub fn set_layer_visible(&mut self, layer: LayerId, visible: bool) {
        if let Some(l) = self.layers.get_mut(layer.0 as usize) {
            l.visible = visible;
        }
    }

    pub fn is_layer_visible(&self, layer: LayerId) -> bool {
        self.layers.get(layer.0 as usize).map(|l| l.visible).unwrap_or(true)
    }

    // ── Ambient zones ────────────────────────────────────────────────────────

    pub fn add_zone(&mut self, zone: AmbientZone) { self.zones.push(zone); }

    pub fn remove_zone(&mut self, name: &str) { self.zones.retain(|z| z.name != name); }

    /// Find the ambient zone at a world position (first match, blended weight).
    pub fn zone_at(&self, pos: Vec3) -> Option<(&AmbientZone, f32)> {
        self.zones.iter()
            .filter(|z| z.contains(pos))
            .map(|z| (z, z.blend_factor(pos)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }

    // ── Portals ──────────────────────────────────────────────────────────────

    pub fn add_portal(&mut self, origin: Vec3, target: Vec3, normal: Vec3, extent: Vec2) -> PortalId {
        let id = PortalId(self.next_portal_id);
        self.next_portal_id += 1;
        self.portals.push(Portal { id, origin, target, normal, extent, active: true, linked: None });
        id
    }

    pub fn link_portals(&mut self, a: PortalId, b: PortalId) {
        if let Some(pa) = self.portals.iter_mut().find(|p| p.id == a) { pa.linked = Some(b); }
        if let Some(pb) = self.portals.iter_mut().find(|p| p.id == b) { pb.linked = Some(a); }
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    /// Advance the scene by `dt` seconds: step physics, age glyphs/particles, apply fields.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.stats.tick_count += 1;
        self.stats.elapsed_secs += dt;

        // Apply force fields to glyphs
        for (_, glyph) in self.glyphs.iter_mut() {
            let mut total_force = Vec3::ZERO;
            for (_, field) in &self.fields {
                total_force += field.force_at(glyph.position, glyph.mass, glyph.charge, self.time);
            }
            glyph.acceleration = total_force / glyph.mass.max(0.001);
        }

        // Tick glyph pool
        self.glyphs.tick(dt);

        // Tick particle pool
        self.particles.tick(dt);

        // Apply fields to particles
        for (_, field) in &self.fields {
            self.particles.apply_field(field, self.time);
        }

        // Tick entities
        for (_, entity) in &mut self.entities {
            entity.tick(dt, self.time);
        }

        // Update node world transforms
        self.flush_transforms();

        // Sync node-attached glyph positions
        self.sync_node_glyphs();

        // Update stats
        self.stats.glyph_count    = self.glyphs.count();
        self.stats.particle_count = self.particles.count();
        self.stats.entity_count   = self.entities.len();
        self.stats.field_count    = self.fields.len();
        self.stats.node_count     = self.nodes.len();
        self.stats.portal_count   = self.portals.len();
        self.stats.zone_count     = self.zones.len();
    }

    // ── Transform propagation ─────────────────────────────────────────────────

    fn flush_transforms(&mut self) {
        let roots: Vec<NodeId> = self.root_nodes.clone();
        for root in roots {
            self.update_world_transform(root, Mat4::IDENTITY);
        }
    }

    fn update_world_transform(&mut self, id: NodeId, parent_world: Mat4) {
        let local_mat = if let Some(node) = self.nodes.get(&id) {
            if !node.dirty { return; }
            node.local_matrix()
        } else {
            return;
        };

        let world = parent_world * local_mat;
        let children: Vec<NodeId> = if let Some(node) = self.nodes.get_mut(&id) {
            node.world = world;
            node.dirty = false;
            node.children.clone()
        } else {
            return;
        };

        for child in children {
            self.update_world_transform(child, world);
        }
    }

    fn sync_node_glyphs(&mut self) {
        // Collect (node_world, glyph_id) pairs to avoid borrow conflicts
        let updates: Vec<(Mat4, GlyphId)> = self.nodes.values()
            .flat_map(|n| {
                let w = n.world;
                n.glyph_ids.iter().map(move |&gid| (w, gid)).collect::<Vec<_>>()
            })
            .collect();

        for (world, glyph_id) in updates {
            if let Some(glyph) = self.glyphs.get_mut(glyph_id) {
                let pos = world.transform_point3(Vec3::ZERO);
                glyph.position = pos;
            }
        }
    }

    // ── Spatial queries ──────────────────────────────────────────────────────

    /// Rebuild the BVH if dirty. Call before performing spatial queries.
    pub fn rebuild_bvh(&mut self) {
        if !self.bvh_dirty { return; }
        let aabbs: Vec<(GlyphId, Aabb)> = self.glyphs.iter()
            .map(|(id, g)| (id, Aabb::from_point(g.position, 1.0)))
            .collect();
        self.bvh = Some(Bvh::build(&aabbs));
        self.bvh_dirty = false;
    }

    /// Find all glyphs within `radius` of `center`.
    pub fn glyphs_in_sphere(&self, center: Vec3, radius: f32) -> Vec<GlyphId> {
        if let Some(ref bvh) = self.bvh {
            bvh.sphere_query(center, radius)
        } else {
            self.glyphs.iter()
                .filter(|(_, g)| (g.position - center).length() <= radius)
                .map(|(id, _)| id)
                .collect()
        }
    }

    /// Raycast against all glyphs, return nearest hit.
    pub fn raycast_glyphs(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<RaycastHit> {
        let dir = direction.normalize_or_zero();
        let mut best: Option<RaycastHit> = None;
        for (id, glyph) in self.glyphs.iter() {
            let delta = glyph.position - origin;
            let t = delta.dot(dir);
            if t < 0.0 || t > max_dist { continue; }
            let perp = delta - dir * t;
            let hit_radius = 0.6;
            if perp.length_squared() <= hit_radius * hit_radius {
                if best.as_ref().map(|b: &RaycastHit| t < b.distance).unwrap_or(true) {
                    best = Some(RaycastHit {
                        glyph_id:  id,
                        distance:  t,
                        point:     origin + dir * t,
                        normal:    -dir,
                    });
                }
            }
        }
        best
    }

    /// Return all entities within radius.
    pub fn entities_in_sphere(&self, center: Vec3, radius: f32) -> Vec<EntityId> {
        self.entities.iter()
            .filter(|(_, e)| (e.position - center).length() <= radius)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Return the nearest entity to `point`, if any.
    pub fn nearest_entity(&self, point: Vec3) -> Option<EntityId> {
        self.entities.iter()
            .min_by(|a, b| {
                let da = (a.1.position - point).length_squared();
                let db = (b.1.position - point).length_squared();
                da.partial_cmp(&db).unwrap()
            })
            .map(|(id, _)| *id)
    }

    /// Return all entities with a given tag.
    pub fn entities_with_tag(&self, tag: &str) -> Vec<EntityId> {
        self.entities.iter()
            .filter(|(_, e)| e.tags.contains(&tag.to_string()))
            .map(|(id, _)| *id)
            .collect()
    }

    // ── Snapshot ─────────────────────────────────────────────────────────────

    pub fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            time:             self.time,
            glyph_count:      self.glyphs.count(),
            entity_count:     self.entities.len(),
            field_count:      self.fields.len(),
            node_count:       self.nodes.len(),
            glyph_positions:  self.glyphs.iter().map(|(_, g)| g.position.to_array()).collect(),
            entity_positions: self.entities.iter().map(|(_, e)| e.position.to_array()).collect(),
            field_positions:  Vec::new(),
        }
    }

    // ── Bulk operations ──────────────────────────────────────────────────────

    /// Remove all expired glyphs/particles and despawned entities. Already handled by tick,
    /// but can be called manually to force GC.
    pub fn gc(&mut self) {
        self.entities.retain(|(_, e)| !e.despawn_requested);
        self.bvh_dirty = true;
    }

    /// Clear the entire scene.
    pub fn clear(&mut self) {
        self.glyphs   = GlyphPool::new(8192);
        self.particles = ParticlePool::new(4096);
        self.entities.clear();
        self.fields.clear();
        self.nodes.clear();
        self.root_nodes.clear();
        self.zones.clear();
        self.portals.clear();
        self.bvh = None;
        self.bvh_dirty = false;
        self.events.clear();
        self.stats = SceneStats::default();
    }

    /// Spawn a burst of glyphs in a grid pattern.
    pub fn spawn_glyph_grid(
        &mut self,
        origin: Vec3,
        cols: u32, rows: u32,
        spacing: f32,
        glyph_fn: impl Fn(u32, u32) -> Glyph,
    ) -> Vec<GlyphId> {
        let mut ids = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let mut g = glyph_fn(col, row);
                g.position = origin + Vec3::new(col as f32 * spacing, 0.0, row as f32 * spacing);
                ids.push(self.spawn_glyph(g));
            }
        }
        ids
    }

    /// Spawn a ring of glyphs.
    pub fn spawn_glyph_ring(
        &mut self,
        center: Vec3,
        radius: f32,
        count: u32,
        glyph_fn: impl Fn(u32) -> Glyph,
    ) -> Vec<GlyphId> {
        let mut ids = Vec::with_capacity(count as usize);
        for i in 0..count {
            let angle = i as f32 / count as f32 * std::f32::consts::TAU;
            let mut g = glyph_fn(i);
            g.position = center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
            ids.push(self.spawn_glyph(g));
        }
        ids
    }

    /// Despawn all glyphs in a list.
    pub fn despawn_glyphs(&mut self, ids: &[GlyphId]) {
        for &id in ids { self.despawn_glyph(id); }
    }

    // ── Event accessors ──────────────────────────────────────────────────────

    pub fn drain_events(&mut self) -> Vec<SceneEvent> { self.events.drain() }
    pub fn push_event(&mut self, e: SceneEvent) { self.events.push(e); }

    // ── Diagnostics ──────────────────────────────────────────────────────────

    pub fn diagnostics(&self) -> String {
        format!(
            "Scene t={:.2}s | glyphs={} particles={} entities={} fields={} nodes={} zones={} portals={}",
            self.time,
            self.stats.glyph_count,
            self.stats.particle_count,
            self.stats.entity_count,
            self.stats.field_count,
            self.stats.node_count,
            self.stats.zone_count,
            self.stats.portal_count,
        )
    }
}

impl Default for Scene {
    fn default() -> Self { Self::new() }
}

/// Backward compat alias used in lib.rs.
pub type SceneGraph = Scene;

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glyph(pos: Vec3) -> Glyph {
        Glyph { position: pos, ..Default::default() }
    }

    #[test]
    fn scene_spawn_despawn_glyph() {
        let mut s = Scene::new();
        let id = s.spawn_glyph(make_glyph(Vec3::ZERO));
        assert_eq!(s.glyphs.count(), 1);
        s.despawn_glyph(id);
        assert_eq!(s.glyphs.count(), 0);
    }

    #[test]
    fn scene_spawn_entity() {
        let mut s = Scene::new();
        let e = AmorphousEntity { position: Vec3::ONE, ..Default::default() };
        let id = s.spawn_entity(e);
        assert!(s.get_entity(id).is_some());
        s.despawn_entity(id);
        assert!(s.get_entity(id).is_none());
    }

    #[test]
    fn scene_fields() {
        let mut s = Scene::new();
        let field = ForceField::Gravity { center: Vec3::ZERO, strength: 9.81, falloff: crate::math::Falloff::InverseSquare };
        let id = s.add_field(field);
        assert!(s.get_field(id).is_some());
        s.remove_field(id);
        assert!(s.get_field(id).is_none());
    }

    #[test]
    fn scene_node_hierarchy() {
        let mut s = Scene::new();
        let parent = s.create_node("parent");
        let child  = s.create_node("child");
        s.attach_node(child, parent);
        assert_eq!(s.get_node(child).unwrap().parent, Some(parent));
        assert!(s.get_node(parent).unwrap().children.contains(&child));
    }

    #[test]
    fn scene_node_find_by_name() {
        let mut s = Scene::new();
        s.create_node("the_node");
        let id = s.find_node_by_name("the_node");
        assert!(id.is_some());
    }

    #[test]
    fn scene_tick_advances_time() {
        let mut s = Scene::new();
        s.tick(0.016);
        assert!((s.time - 0.016).abs() < 1e-5);
    }

    #[test]
    fn scene_glyphs_in_sphere() {
        let mut s = Scene::new();
        s.spawn_glyph(make_glyph(Vec3::ZERO));
        s.spawn_glyph(make_glyph(Vec3::new(100.0, 0.0, 0.0)));
        let hits = s.glyphs_in_sphere(Vec3::ZERO, 5.0);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn scene_raycast_glyphs() {
        let mut s = Scene::new();
        s.spawn_glyph(make_glyph(Vec3::new(0.0, 0.0, 10.0)));
        let hit = s.raycast_glyphs(Vec3::ZERO, Vec3::Z, 100.0);
        assert!(hit.is_some());
        assert!((hit.unwrap().distance - 10.0).abs() < 1.0);
    }

    #[test]
    fn scene_layer_visibility() {
        let mut s = Scene::new();
        s.set_layer_visible(LayerId::ENTITIES, false);
        assert!(!s.is_layer_visible(LayerId::ENTITIES));
        s.set_layer_visible(LayerId::ENTITIES, true);
        assert!(s.is_layer_visible(LayerId::ENTITIES));
    }

    #[test]
    fn scene_ambient_zone() {
        let mut s = Scene::new();
        s.add_zone(AmbientZone {
            min: Vec3::splat(-5.0), max: Vec3::splat(5.0),
            ambient_color: Vec4::ONE, fog_density: 0.0,
            fog_color: Vec4::ZERO, reverb_wet: 0.0,
            wind_strength: 0.0, gravity_scale: 1.0,
            name: "test_zone".into(),
        });
        let result = s.zone_at(Vec3::ZERO);
        assert!(result.is_some());
        let outside = s.zone_at(Vec3::new(100.0, 0.0, 0.0));
        assert!(outside.is_none());
    }

    #[test]
    fn scene_portal() {
        let mut s = Scene::new();
        let a = s.add_portal(Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0), Vec3::Z, Vec2::splat(2.0));
        let b = s.add_portal(Vec3::new(100.0, 0.0, 0.0), Vec3::ZERO, Vec3::NEG_Z, Vec2::splat(2.0));
        s.link_portals(a, b);
        assert_eq!(s.portals[0].linked, Some(b));
        assert_eq!(s.portals[1].linked, Some(a));
    }

    #[test]
    fn scene_snapshot_diff() {
        let mut s = Scene::new();
        let snap1 = s.snapshot();
        s.spawn_glyph(make_glyph(Vec3::ZERO));
        let snap2 = s.snapshot();
        let diff = snap1.diff(&snap2);
        assert_eq!(diff.glyph_delta, 1);
    }

    #[test]
    fn scene_spawn_glyph_ring() {
        let mut s = Scene::new();
        let ids = s.spawn_glyph_ring(Vec3::ZERO, 5.0, 8, |_| make_glyph(Vec3::ZERO));
        assert_eq!(ids.len(), 8);
    }

    #[test]
    fn scene_spawn_glyph_grid() {
        let mut s = Scene::new();
        let ids = s.spawn_glyph_grid(Vec3::ZERO, 4, 4, 1.0, |_, _| make_glyph(Vec3::ZERO));
        assert_eq!(ids.len(), 16);
    }

    #[test]
    fn scene_clear() {
        let mut s = Scene::new();
        s.spawn_glyph(make_glyph(Vec3::ZERO));
        s.spawn_entity(AmorphousEntity::default());
        s.clear();
        assert_eq!(s.stats.glyph_count, 0);
    }

    #[test]
    fn scene_diagnostics_string() {
        let s = Scene::new();
        let d = s.diagnostics();
        assert!(d.contains("Scene"));
    }

    #[test]
    fn transform3d_lerp() {
        let a = Transform3D::identity();
        let b = Transform3D::from_position(Vec3::new(10.0, 0.0, 0.0));
        let mid = a.lerp(&b, 0.5);
        assert!((mid.position.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn transform3d_transform_point() {
        let t = Transform3D { position: Vec3::new(1.0, 2.0, 3.0), ..Transform3D::identity() };
        let p = t.transform_point(Vec3::ZERO);
        assert_eq!(p, Vec3::new(1.0, 2.0, 3.0));
    }
}
