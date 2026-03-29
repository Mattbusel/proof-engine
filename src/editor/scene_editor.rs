
//! Scene editor — entity management, component inspector, scene serialization, prefab integration.

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Component system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Transform,
    MeshRenderer,
    SkinnedMeshRenderer,
    Camera,
    DirectionalLight,
    PointLight,
    SpotLight,
    AreaLight,
    AudioSource,
    AudioListener,
    Rigidbody,
    BoxCollider,
    SphereCollider,
    CapsuleCollider,
    MeshCollider,
    TerrainCollider,
    Animator,
    ParticleSystem,
    TrailRenderer,
    LineRenderer,
    ReflectionProbe,
    LightProbe,
    NavMeshAgent,
    NavMeshObstacle,
    LodGroup,
    DecalProjector,
    PostProcessVolume,
    BillboardRenderer,
    SpeedTreeRenderer,
    OcclusionCullingProxy,
    SortingGroup,
    Canvas,
    TextMesh,
    Sprite,
    Script,
    Custom,
}

impl ComponentKind {
    pub fn label(self) -> &'static str {
        match self {
            ComponentKind::Transform => "Transform",
            ComponentKind::MeshRenderer => "Mesh Renderer",
            ComponentKind::SkinnedMeshRenderer => "Skinned Mesh Renderer",
            ComponentKind::Camera => "Camera",
            ComponentKind::DirectionalLight => "Directional Light",
            ComponentKind::PointLight => "Point Light",
            ComponentKind::SpotLight => "Spot Light",
            ComponentKind::AreaLight => "Area Light",
            ComponentKind::AudioSource => "Audio Source",
            ComponentKind::AudioListener => "Audio Listener",
            ComponentKind::Rigidbody => "Rigidbody",
            ComponentKind::BoxCollider => "Box Collider",
            ComponentKind::SphereCollider => "Sphere Collider",
            ComponentKind::CapsuleCollider => "Capsule Collider",
            ComponentKind::MeshCollider => "Mesh Collider",
            ComponentKind::TerrainCollider => "Terrain Collider",
            ComponentKind::Animator => "Animator",
            ComponentKind::ParticleSystem => "Particle System",
            ComponentKind::TrailRenderer => "Trail Renderer",
            ComponentKind::LineRenderer => "Line Renderer",
            ComponentKind::ReflectionProbe => "Reflection Probe",
            ComponentKind::LightProbe => "Light Probe Group",
            ComponentKind::NavMeshAgent => "Nav Mesh Agent",
            ComponentKind::NavMeshObstacle => "Nav Mesh Obstacle",
            ComponentKind::LodGroup => "LOD Group",
            ComponentKind::DecalProjector => "Decal Projector",
            ComponentKind::PostProcessVolume => "Post Process Volume",
            ComponentKind::BillboardRenderer => "Billboard Renderer",
            ComponentKind::SpeedTreeRenderer => "SpeedTree Renderer",
            ComponentKind::OcclusionCullingProxy => "Occlusion Culling Proxy",
            ComponentKind::SortingGroup => "Sorting Group",
            ComponentKind::Canvas => "Canvas",
            ComponentKind::TextMesh => "Text Mesh",
            ComponentKind::Sprite => "Sprite",
            ComponentKind::Script => "Script",
            ComponentKind::Custom => "Custom",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            ComponentKind::Transform => "Core",
            ComponentKind::MeshRenderer | ComponentKind::SkinnedMeshRenderer
            | ComponentKind::BillboardRenderer | ComponentKind::SpeedTreeRenderer => "Rendering",
            ComponentKind::Camera => "Camera",
            ComponentKind::DirectionalLight | ComponentKind::PointLight | ComponentKind::SpotLight | ComponentKind::AreaLight => "Lighting",
            ComponentKind::AudioSource | ComponentKind::AudioListener => "Audio",
            ComponentKind::Rigidbody | ComponentKind::BoxCollider | ComponentKind::SphereCollider
            | ComponentKind::CapsuleCollider | ComponentKind::MeshCollider | ComponentKind::TerrainCollider => "Physics",
            ComponentKind::Animator | ComponentKind::ParticleSystem | ComponentKind::TrailRenderer
            | ComponentKind::LineRenderer => "Animation",
            ComponentKind::ReflectionProbe | ComponentKind::LightProbe => "Lighting",
            ComponentKind::NavMeshAgent | ComponentKind::NavMeshObstacle => "AI",
            ComponentKind::LodGroup | ComponentKind::OcclusionCullingProxy | ComponentKind::SortingGroup => "Optimization",
            ComponentKind::DecalProjector | ComponentKind::PostProcessVolume => "Effects",
            ComponentKind::Canvas | ComponentKind::TextMesh | ComponentKind::Sprite => "UI",
            ComponentKind::Script | ComponentKind::Custom => "Scripts",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ComponentKind::Camera => "📷",
            ComponentKind::DirectionalLight | ComponentKind::PointLight | ComponentKind::SpotLight => "💡",
            ComponentKind::MeshRenderer | ComponentKind::SkinnedMeshRenderer => "⬡",
            ComponentKind::Rigidbody => "⚙",
            ComponentKind::AudioSource | ComponentKind::AudioListener => "♪",
            ComponentKind::ParticleSystem => "✦",
            _ => "▪",
        }
    }
}

// ---------------------------------------------------------------------------
// Component value store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ComponentField {
    Float(f32),
    Float2(Vec2),
    Float3(Vec3),
    Float4(Vec4),
    Int(i32),
    Uint(u32),
    Bool(bool),
    String(String),
    Color(Vec4),
    AssetRef(Option<u64>),
    Enum(u32, Vec<String>),
    LayerMask(u32),
}

impl ComponentField {
    pub fn type_label(&self) -> &'static str {
        match self {
            ComponentField::Float(_) => "Float",
            ComponentField::Float2(_) => "Vector2",
            ComponentField::Float3(_) => "Vector3",
            ComponentField::Float4(_) => "Vector4",
            ComponentField::Int(_) => "Int",
            ComponentField::Uint(_) => "UInt",
            ComponentField::Bool(_) => "Bool",
            ComponentField::String(_) => "String",
            ComponentField::Color(_) => "Color",
            ComponentField::AssetRef(_) => "AssetRef",
            ComponentField::Enum(_, _) => "Enum",
            ComponentField::LayerMask(_) => "LayerMask",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub id: u32,
    pub kind: ComponentKind,
    pub enabled: bool,
    pub fields: HashMap<String, ComponentField>,
    pub script_name: Option<String>,
    pub expanded: bool,
}

impl Component {
    pub fn new(id: u32, kind: ComponentKind) -> Self {
        let mut comp = Self {
            id, kind, enabled: true, fields: HashMap::new(), script_name: None, expanded: true,
        };
        comp.populate_defaults();
        comp
    }

    fn populate_defaults(&mut self) {
        match self.kind {
            ComponentKind::Transform => {
                self.fields.insert("position".into(), ComponentField::Float3(Vec3::ZERO));
                self.fields.insert("rotation".into(), ComponentField::Float3(Vec3::ZERO));
                self.fields.insert("scale".into(), ComponentField::Float3(Vec3::ONE));
            }
            ComponentKind::Camera => {
                self.fields.insert("fov".into(), ComponentField::Float(60.0));
                self.fields.insert("near_clip".into(), ComponentField::Float(0.1));
                self.fields.insert("far_clip".into(), ComponentField::Float(1000.0));
                self.fields.insert("orthographic".into(), ComponentField::Bool(false));
                self.fields.insert("depth".into(), ComponentField::Int(0));
            }
            ComponentKind::MeshRenderer => {
                self.fields.insert("mesh_id".into(), ComponentField::AssetRef(None));
                self.fields.insert("material_id".into(), ComponentField::AssetRef(None));
                self.fields.insert("cast_shadows".into(), ComponentField::Bool(true));
                self.fields.insert("receive_shadows".into(), ComponentField::Bool(true));
            }
            ComponentKind::PointLight => {
                self.fields.insert("color".into(), ComponentField::Color(Vec4::ONE));
                self.fields.insert("intensity".into(), ComponentField::Float(1.0));
                self.fields.insert("range".into(), ComponentField::Float(10.0));
                self.fields.insert("shadow_type".into(), ComponentField::Enum(0, vec!["No Shadows".into(), "Hard Shadows".into(), "Soft Shadows".into()]));
            }
            ComponentKind::Rigidbody => {
                self.fields.insert("mass".into(), ComponentField::Float(1.0));
                self.fields.insert("drag".into(), ComponentField::Float(0.0));
                self.fields.insert("angular_drag".into(), ComponentField::Float(0.05));
                self.fields.insert("use_gravity".into(), ComponentField::Bool(true));
                self.fields.insert("is_kinematic".into(), ComponentField::Bool(false));
            }
            ComponentKind::AudioSource => {
                self.fields.insert("clip_id".into(), ComponentField::AssetRef(None));
                self.fields.insert("volume".into(), ComponentField::Float(1.0));
                self.fields.insert("pitch".into(), ComponentField::Float(1.0));
                self.fields.insert("loop_audio".into(), ComponentField::Bool(false));
                self.fields.insert("play_on_awake".into(), ComponentField::Bool(true));
                self.fields.insert("spatial_blend".into(), ComponentField::Float(1.0));
            }
            _ => {}
        }
    }

    pub fn get_float(&self, field: &str) -> f32 {
        match self.fields.get(field) {
            Some(ComponentField::Float(v)) => *v,
            _ => 0.0,
        }
    }
    pub fn get_bool(&self, field: &str) -> bool {
        match self.fields.get(field) {
            Some(ComponentField::Bool(v)) => *v,
            _ => false,
        }
    }
    pub fn get_vec3(&self, field: &str) -> Vec3 {
        match self.fields.get(field) {
            Some(ComponentField::Float3(v)) => *v,
            _ => Vec3::ZERO,
        }
    }
}

// ---------------------------------------------------------------------------
// Scene entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SceneEntity {
    pub id: u64,
    pub name: String,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub components: Vec<Component>,
    pub active: bool,
    pub static_flags: StaticFlags,
    pub layer: u8,
    pub tag: String,
    pub prefab_id: Option<u64>,
    pub is_prefab_root: bool,
    pub scene_id: Option<u32>,
    pub next_component_id: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StaticFlags {
    pub batching: bool,
    pub navigation: bool,
    pub occluder: bool,
    pub occludee: bool,
    pub lightmap: bool,
    pub reflection: bool,
}

impl StaticFlags {
    pub fn all() -> Self { Self { batching: true, navigation: true, occluder: true, occludee: true, lightmap: true, reflection: true } }
    pub fn is_any_static(self) -> bool { self.batching || self.navigation || self.occluder || self.occludee || self.lightmap || self.reflection }
}

impl SceneEntity {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        let mut entity = Self {
            id, name: name.into(), parent: None, children: Vec::new(),
            components: Vec::new(), active: true,
            static_flags: StaticFlags::default(), layer: 0,
            tag: "Untagged".into(), prefab_id: None, is_prefab_root: false,
            scene_id: None, next_component_id: 1,
        };
        entity.add_component(ComponentKind::Transform);
        entity
    }

    pub fn add_component(&mut self, kind: ComponentKind) -> u32 {
        let id = self.next_component_id;
        self.next_component_id += 1;
        self.components.push(Component::new(id, kind));
        id
    }

    pub fn remove_component(&mut self, id: u32) {
        if self.components.len() <= 1 { return; } // Can't remove last
        self.components.retain(|c| c.id != id);
    }

    pub fn get_component(&self, kind: ComponentKind) -> Option<&Component> {
        self.components.iter().find(|c| c.kind == kind)
    }

    pub fn get_component_mut(&mut self, kind: ComponentKind) -> Option<&mut Component> {
        self.components.iter_mut().find(|c| c.kind == kind)
    }

    pub fn has_component(&self, kind: ComponentKind) -> bool {
        self.components.iter().any(|c| c.kind == kind)
    }

    pub fn world_transform(&self) -> Mat4 {
        let transform = self.get_component(ComponentKind::Transform);
        if let Some(t) = transform {
            let pos = t.get_vec3("position");
            let rot_euler = t.get_vec3("rotation");
            let scale = t.get_vec3("scale");
            let scale = if scale == Vec3::ZERO { Vec3::ONE } else { scale };
            let rot = Quat::from_euler(
                glam::EulerRot::XYZ,
                rot_euler.x.to_radians(),
                rot_euler.y.to_radians(),
                rot_euler.z.to_radians(),
            );
            Mat4::from_scale_rotation_translation(scale, rot, pos)
        } else {
            Mat4::IDENTITY
        }
    }

    pub fn set_position(&mut self, pos: Vec3) {
        if let Some(t) = self.get_component_mut(ComponentKind::Transform) {
            t.fields.insert("position".into(), ComponentField::Float3(pos));
        }
    }

    pub fn position(&self) -> Vec3 {
        self.get_component(ComponentKind::Transform)
            .map(|t| t.get_vec3("position"))
            .unwrap_or(Vec3::ZERO)
    }
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Scene {
    pub name: String,
    pub path: Option<String>,
    pub entities: Vec<SceneEntity>,
    pub is_dirty: bool,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    pub skybox_material: Option<u64>,
    pub fog_enabled: bool,
    pub fog_color: Vec3,
    pub fog_density: f32,
    pub gravity: Vec3,
    pub time_scale: f32,
    pub physics_layer_matrix: [[bool; 32]; 32],
    pub next_entity_id: u64,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: None,
            entities: Vec::new(),
            is_dirty: false,
            ambient_color: Vec3::new(0.2, 0.2, 0.3),
            ambient_intensity: 1.0,
            skybox_material: None,
            fog_enabled: false,
            fog_color: Vec3::new(0.5, 0.6, 0.7),
            fog_density: 0.01,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            time_scale: 1.0,
            physics_layer_matrix: [[true; 32]; 32],
            next_entity_id: 1,
        }
    }

    pub fn create_entity(&mut self, name: impl Into<String>) -> u64 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let entity = SceneEntity::new(id, name);
        self.entities.push(entity);
        self.is_dirty = true;
        id
    }

    pub fn destroy_entity(&mut self, id: u64) {
        // Collect children to destroy recursively
        let children: Vec<u64> = self.entities.iter()
            .find(|e| e.id == id)
            .map(|e| e.children.clone())
            .unwrap_or_default();
        for child in children {
            self.destroy_entity(child);
        }
        self.entities.retain(|e| e.id != id);
        // Remove from parent's children list
        for entity in &mut self.entities {
            entity.children.retain(|&c| c != id);
        }
        self.is_dirty = true;
    }

    pub fn set_parent(&mut self, child_id: u64, parent_id: Option<u64>) {
        // Remove from old parent
        let old_parent = self.entities.iter().find(|e| e.id == child_id).and_then(|e| e.parent);
        if let Some(op) = old_parent {
            if let Some(op_entity) = self.entities.iter_mut().find(|e| e.id == op) {
                op_entity.children.retain(|&c| c != child_id);
            }
        }
        // Set new parent
        if let Some(pid) = parent_id {
            if let Some(parent) = self.entities.iter_mut().find(|e| e.id == pid) {
                parent.children.push(child_id);
            }
        }
        if let Some(child) = self.entities.iter_mut().find(|e| e.id == child_id) {
            child.parent = parent_id;
        }
        self.is_dirty = true;
    }

    pub fn find_entity(&self, id: u64) -> Option<&SceneEntity> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn find_entity_mut(&mut self, id: u64) -> Option<&mut SceneEntity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SceneEntity> {
        self.entities.iter().find(|e| e.name == name)
    }

    pub fn root_entities(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.iter().filter(|e| e.parent.is_none())
    }

    pub fn entity_count(&self) -> usize { self.entities.len() }

    pub fn entities_with_component(&self, kind: ComponentKind) -> impl Iterator<Item = &SceneEntity> {
        self.entities.iter().filter(move |e| e.has_component(kind))
    }

    pub fn populate_default_scene(&mut self) {
        // Camera
        let cam = self.create_entity("Main Camera");
        if let Some(e) = self.find_entity_mut(cam) {
            e.add_component(ComponentKind::Camera);
            e.add_component(ComponentKind::AudioListener);
            e.set_position(Vec3::new(0.0, 1.0, -10.0));
            e.tag = "MainCamera".into();
        }
        // Directional light
        let light = self.create_entity("Directional Light");
        if let Some(e) = self.find_entity_mut(light) {
            e.add_component(ComponentKind::DirectionalLight);
        }
        // Ground plane
        let ground = self.create_entity("Ground");
        if let Some(e) = self.find_entity_mut(ground) {
            e.add_component(ComponentKind::MeshRenderer);
            e.add_component(ComponentKind::BoxCollider);
            e.set_position(Vec3::ZERO);
            e.static_flags = StaticFlags::all();
        }
        // Test object
        let cube = self.create_entity("Cube");
        if let Some(e) = self.find_entity_mut(cube) {
            e.add_component(ComponentKind::MeshRenderer);
            e.add_component(ComponentKind::BoxCollider);
            e.add_component(ComponentKind::Rigidbody);
            e.set_position(Vec3::new(0.0, 2.0, 0.0));
        }
    }

    pub fn serialize_to_json(&self) -> String {
        // Simplified JSON-like representation
        let mut json = format!("{{\"scene\":\"{}\",\"entities\":[", self.name);
        for (i, e) in self.entities.iter().enumerate() {
            if i > 0 { json.push(','); }
            let pos = e.position();
            json.push_str(&format!("{{\"id\":{},\"name\":\"{}\",\"active\":{},\"components\":{},\"position\":[{:.3},{:.3},{:.3}]}}",
                e.id, e.name, e.active, e.components.len(), pos.x, pos.y, pos.z));
        }
        json.push_str("]}");
        json
    }
}

// ---------------------------------------------------------------------------
// Scene editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneEditorMode { Normal, Prefab, Terrain, Animation }

#[derive(Debug, Clone)]
pub struct MultiSelection {
    pub entity_ids: Vec<u64>,
    pub pivot: Vec3,
    pub bounding_box: [Vec3; 2],
}

impl MultiSelection {
    pub fn empty() -> Self {
        Self { entity_ids: Vec::new(), pivot: Vec3::ZERO, bounding_box: [Vec3::ZERO, Vec3::ZERO] }
    }
    pub fn is_empty(&self) -> bool { self.entity_ids.is_empty() }
    pub fn contains(&self, id: u64) -> bool { self.entity_ids.contains(&id) }
}

#[derive(Debug, Clone)]
pub struct SceneEditorState {
    pub scene: Scene,
    pub mode: SceneEditorMode,
    pub selection: MultiSelection,
    pub clipboard: Vec<SceneEntity>,
    pub undo_stack: Vec<String>,  // JSON snapshots
    pub undo_pos: usize,
    pub search_query: String,
    pub filter_by_component: Option<ComponentKind>,
    pub expand_all: bool,
    pub show_active_only: bool,
    pub show_gizmos: bool,
    pub gizmo_size: f32,
    pub rename_target: Option<u64>,
    pub rename_buffer: String,
}

impl SceneEditorState {
    pub fn new() -> Self {
        let mut scene = Scene::new("SampleScene");
        scene.populate_default_scene();
        Self {
            scene,
            mode: SceneEditorMode::Normal,
            selection: MultiSelection::empty(),
            clipboard: Vec::new(),
            undo_stack: Vec::new(),
            undo_pos: 0,
            search_query: String::new(),
            filter_by_component: None,
            expand_all: false,
            show_active_only: false,
            show_gizmos: true,
            gizmo_size: 1.0,
            rename_target: None,
            rename_buffer: String::new(),
        }
    }

    pub fn select(&mut self, id: u64) {
        self.selection.entity_ids = vec![id];
        self.update_selection_pivot();
    }

    pub fn add_to_selection(&mut self, id: u64) {
        if !self.selection.contains(id) {
            self.selection.entity_ids.push(id);
        }
        self.update_selection_pivot();
    }

    pub fn deselect_all(&mut self) {
        self.selection = MultiSelection::empty();
    }

    fn update_selection_pivot(&mut self) {
        if self.selection.is_empty() { return; }
        let positions: Vec<Vec3> = self.selection.entity_ids.iter()
            .filter_map(|&id| self.scene.find_entity(id))
            .map(|e| e.position())
            .collect();
        if positions.is_empty() { return; }
        let sum: Vec3 = positions.iter().sum();
        self.selection.pivot = sum / positions.len() as f32;
    }

    pub fn snapshot(&mut self) {
        let json = self.scene.serialize_to_json();
        self.undo_stack.truncate(self.undo_pos);
        self.undo_stack.push(json);
        self.undo_pos = self.undo_stack.len();
        self.scene.is_dirty = true;
    }

    pub fn delete_selection(&mut self) {
        self.snapshot();
        let ids: Vec<u64> = self.selection.entity_ids.drain(..).collect();
        for id in ids {
            self.scene.destroy_entity(id);
        }
    }

    pub fn duplicate_selection(&mut self) {
        self.snapshot();
        let ids: Vec<u64> = self.selection.entity_ids.clone();
        let mut new_ids = Vec::new();
        for id in ids {
            if let Some(entity) = self.scene.find_entity(id) {
                let mut new_entity = entity.clone();
                let new_id = self.scene.next_entity_id;
                self.scene.next_entity_id += 1;
                new_entity.id = new_id;
                new_entity.name = format!("{} (Copy)", new_entity.name);
                let pos = new_entity.position();
                new_entity.set_position(pos + Vec3::new(0.5, 0.0, 0.5));
                new_entity.parent = None;
                new_entity.children.clear();
                self.scene.entities.push(new_entity);
                new_ids.push(new_id);
            }
        }
        self.selection.entity_ids = new_ids;
    }

    pub fn create_empty(&mut self, name: &str) -> u64 {
        self.snapshot();
        self.scene.create_entity(name)
    }

    pub fn visible_entities(&self) -> Vec<&SceneEntity> {
        self.scene.entities.iter().filter(|e| {
            let active_ok = !self.show_active_only || e.active;
            let comp_ok = self.filter_by_component.map(|k| e.has_component(k)).unwrap_or(true);
            let search_ok = self.search_query.is_empty() ||
                e.name.to_lowercase().contains(&self.search_query.to_lowercase());
            active_ok && comp_ok && search_ok
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_create_entity() {
        let mut scene = Scene::new("Test");
        let id = scene.create_entity("TestEntity");
        assert!(scene.find_entity(id).is_some());
    }

    #[test]
    fn test_entity_components() {
        let mut entity = SceneEntity::new(1, "test");
        entity.add_component(ComponentKind::MeshRenderer);
        assert!(entity.has_component(ComponentKind::Transform));
        assert!(entity.has_component(ComponentKind::MeshRenderer));
    }

    #[test]
    fn test_scene_parenting() {
        let mut scene = Scene::new("Test");
        let parent = scene.create_entity("Parent");
        let child = scene.create_entity("Child");
        scene.set_parent(child, Some(parent));
        assert_eq!(scene.find_entity(child).unwrap().parent, Some(parent));
        assert!(scene.find_entity(parent).unwrap().children.contains(&child));
    }

    #[test]
    fn test_destroy_propagates() {
        let mut scene = Scene::new("Test");
        let parent = scene.create_entity("Parent");
        let child = scene.create_entity("Child");
        scene.set_parent(child, Some(parent));
        scene.destroy_entity(parent);
        assert!(scene.find_entity(parent).is_none());
        assert!(scene.find_entity(child).is_none());
    }

    #[test]
    fn test_scene_editor() {
        let mut ed = SceneEditorState::new();
        assert!(!ed.scene.entities.is_empty());
        let id = ed.create_empty("NewEntity");
        ed.select(id);
        assert_eq!(ed.selection.entity_ids.len(), 1);
        ed.duplicate_selection();
        assert!(ed.scene.entity_count() > 5);
    }
}
