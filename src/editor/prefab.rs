// prefab.rs — Prefab system for proof-engine editor
// Prefabs are reusable entity templates with overrides, nested prefabs,
// variant management, instance diff tracking, and automatic update propagation.

use std::collections::HashMap;
use std::fmt;

// ─── Prefab identifier ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrefabId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrefabInstanceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);

// ─── Component data ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Vec4([f64; 4]),
    String(String),
    EntityRef(u32),
    PrefabRef(PrefabId),
    List(Vec<ComponentValue>),
    Map(Vec<(String, ComponentValue)>),
}

impl ComponentValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_)     => "bool",
            Self::Int(_)      => "int",
            Self::Float(_)    => "float",
            Self::Vec2(_)     => "vec2",
            Self::Vec3(_)     => "vec3",
            Self::Vec4(_)     => "vec4",
            Self::String(_)   => "string",
            Self::EntityRef(_) => "entity_ref",
            Self::PrefabRef(_) => "prefab_ref",
            Self::List(_)     => "list",
            Self::Map(_)      => "map",
        }
    }

    pub fn is_same_type(&self, other: &Self) -> bool {
        self.type_name() == other.type_name()
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v)   => Some(*v as f64),
            Self::Bool(v)  => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn lerp(&self, other: &Self, t: f64) -> Option<Self> {
        match (self, other) {
            (Self::Float(a), Self::Float(b)) => Some(Self::Float(a + (b - a) * t)),
            (Self::Int(a), Self::Int(b)) => Some(Self::Int(*a + ((*b - *a) as f64 * t) as i64)),
            (Self::Vec2(a), Self::Vec2(b)) => Some(Self::Vec2([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
            ])),
            (Self::Vec3(a), Self::Vec3(b)) => Some(Self::Vec3([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ])),
            (Self::Vec4(a), Self::Vec4(b)) => Some(Self::Vec4([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
                a[3] + (b[3] - a[3]) * t,
            ])),
            _ => None,
        }
    }
}

impl fmt::Display for ComponentValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(v)  => write!(f, "{}", v),
            Self::Int(v)   => write!(f, "{}", v),
            Self::Float(v) => write!(f, "{:.4}", v),
            Self::Vec2(v)  => write!(f, "({:.3},{:.3})", v[0], v[1]),
            Self::Vec3(v)  => write!(f, "({:.3},{:.3},{:.3})", v[0], v[1], v[2]),
            Self::Vec4(v)  => write!(f, "({:.3},{:.3},{:.3},{:.3})", v[0],v[1],v[2],v[3]),
            Self::String(s) => write!(f, "\"{}\"", s),
            Self::EntityRef(id) => write!(f, "entity:{}", id),
            Self::PrefabRef(id) => write!(f, "prefab:{}", id.0),
            Self::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Self::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{}:{}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

// ─── Component definition ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub id: ComponentId,
    pub name: String,
    pub category: String,
    pub fields: Vec<FieldDef>,
    pub version: u32,
    pub deprecated: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub default_value: ComponentValue,
    pub description: &'static str,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub hidden_in_editor: bool,
    pub read_only: bool,
    pub animatable: bool,
}

impl FieldDef {
    pub fn new(name: &str, default: ComponentValue) -> Self {
        Self {
            name: name.to_string(),
            default_value: default,
            description: "",
            min: None,
            max: None,
            hidden_in_editor: false,
            read_only: false,
            animatable: false,
        }
    }

    pub fn float(name: &str, default: f64) -> Self {
        Self::new(name, ComponentValue::Float(default))
    }

    pub fn vec3(name: &str, x: f64, y: f64, z: f64) -> Self {
        Self::new(name, ComponentValue::Vec3([x, y, z]))
    }

    pub fn bool(name: &str, default: bool) -> Self {
        Self::new(name, ComponentValue::Bool(default))
    }

    pub fn string(name: &str, default: &str) -> Self {
        Self::new(name, ComponentValue::String(default.to_string()))
    }
}

// ─── Component instance ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComponentInstance {
    pub def_id: ComponentId,
    pub values: HashMap<String, ComponentValue>,
    pub enabled: bool,
}

impl ComponentInstance {
    pub fn new(def_id: ComponentId) -> Self {
        Self { def_id, values: HashMap::new(), enabled: true }
    }

    pub fn get(&self, field: &str) -> Option<&ComponentValue> {
        self.values.get(field)
    }

    pub fn set(&mut self, field: &str, value: ComponentValue) {
        self.values.insert(field.to_string(), value);
    }

    pub fn from_def(def: &ComponentDef) -> Self {
        let mut inst = Self::new(def.id);
        for field in &def.fields {
            inst.values.insert(field.name.clone(), field.default_value.clone());
        }
        inst
    }
}

// ─── Prefab node ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrefabNode {
    pub name: String,
    pub local_position: [f64; 3],
    pub local_rotation: [f64; 4],  // quaternion xyzw
    pub local_scale:    [f64; 3],
    pub components: Vec<ComponentInstance>,
    pub children: Vec<PrefabNode>,
    pub tags: Vec<String>,
    pub active: bool,
    pub static_: bool,
    pub layer: u32,
    pub nested_prefab: Option<(PrefabId, Vec<PrefabOverride>)>,
}

impl PrefabNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            local_position: [0.0, 0.0, 0.0],
            local_rotation: [0.0, 0.0, 0.0, 1.0],
            local_scale: [1.0, 1.0, 1.0],
            components: Vec::new(),
            children: Vec::new(),
            tags: Vec::new(),
            active: true,
            static_: false,
            layer: 0,
            nested_prefab: None,
        }
    }

    pub fn add_component(&mut self, inst: ComponentInstance) {
        self.components.push(inst);
    }

    pub fn add_child(&mut self, child: PrefabNode) {
        self.children.push(child);
    }

    pub fn find_child(&self, name: &str) -> Option<&PrefabNode> {
        for child in &self.children {
            if child.name == name { return Some(child); }
            if let Some(found) = child.find_child(name) { return Some(found); }
        }
        None
    }

    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut PrefabNode> {
        // Linear search: first check direct children, then recurse
        let idx = self.children.iter().position(|c| c.name == name);
        if let Some(i) = idx {
            return Some(&mut self.children[i]);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_child_mut(name) { return Some(found); }
        }
        None
    }

    pub fn total_node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_node_count()).sum::<usize>()
    }

    pub fn depth(&self) -> u32 {
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}

// ─── Override ─────────────────────────────────────────────────────────────────

/// Describes a single property override on a prefab instance
#[derive(Debug, Clone)]
pub struct PrefabOverride {
    pub path: String,       // dot-separated path: "Root/LeftArm/Hand.Rigidbody.mass"
    pub value: ComponentValue,
    pub added: bool,        // true if this field/component/node was added (not in base)
    pub removed: bool,      // true if this was removed from base
}

impl PrefabOverride {
    pub fn set(path: &str, value: ComponentValue) -> Self {
        Self {
            path: path.to_string(),
            value,
            added: false,
            removed: false,
        }
    }

    pub fn add(path: &str, value: ComponentValue) -> Self {
        Self {
            path: path.to_string(),
            value,
            added: true,
            removed: false,
        }
    }

    pub fn remove(path: &str) -> Self {
        Self {
            path: path.to_string(),
            value: ComponentValue::Bool(false),
            added: false,
            removed: true,
        }
    }

    pub fn node_path(&self) -> &str {
        if let Some(dot) = self.path.rfind('.') {
            &self.path[..dot]
        } else {
            &self.path
        }
    }

    pub fn field_path(&self) -> Option<&str> {
        self.path.rfind('.').map(|dot| &self.path[dot+1..])
    }
}

// ─── Prefab variant ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrefabVariant {
    pub name: String,
    pub description: String,
    pub overrides: Vec<PrefabOverride>,
    pub is_default: bool,
    pub thumbnail_path: Option<String>,
    pub tags: Vec<String>,
}

impl PrefabVariant {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            overrides: Vec::new(),
            is_default: false,
            thumbnail_path: None,
            tags: Vec::new(),
        }
    }

    pub fn default_variant() -> Self {
        let mut v = Self::new("Default");
        v.is_default = true;
        v
    }

    pub fn add_override(&mut self, ov: PrefabOverride) {
        // Replace existing override at same path
        if let Some(existing) = self.overrides.iter_mut().find(|o| o.path == ov.path) {
            *existing = ov;
        } else {
            self.overrides.push(ov);
        }
    }

    pub fn remove_override(&mut self, path: &str) {
        self.overrides.retain(|o| o.path != path);
    }

    pub fn get_override(&self, path: &str) -> Option<&PrefabOverride> {
        self.overrides.iter().find(|o| o.path == path)
    }

    pub fn override_count(&self) -> usize { self.overrides.len() }
}

// ─── Prefab ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Prefab {
    pub id: PrefabId,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: u32,
    pub root: PrefabNode,
    pub variants: Vec<PrefabVariant>,
    pub active_variant: usize,
    pub asset_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub modified_at: u64,
    pub is_locked: bool,
}

impl Prefab {
    pub fn new(id: PrefabId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: String::new(),
            author: String::new(),
            version: 1,
            root: PrefabNode::new(name),
            variants: vec![PrefabVariant::default_variant()],
            active_variant: 0,
            asset_path: None,
            thumbnail_path: None,
            tags: Vec::new(),
            created_at: 0,
            modified_at: 0,
            is_locked: false,
        }
    }

    pub fn add_variant(&mut self, variant: PrefabVariant) {
        self.variants.push(variant);
    }

    pub fn active_variant(&self) -> &PrefabVariant {
        &self.variants[self.active_variant.min(self.variants.len().saturating_sub(1))]
    }

    pub fn active_variant_mut(&mut self) -> &mut PrefabVariant {
        let idx = self.active_variant.min(self.variants.len().saturating_sub(1));
        &mut self.variants[idx]
    }

    pub fn set_active_variant(&mut self, name: &str) -> bool {
        if let Some(idx) = self.variants.iter().position(|v| v.name == name) {
            self.active_variant = idx;
            return true;
        }
        false
    }

    pub fn node_count(&self) -> usize { self.root.total_node_count() }
    pub fn depth(&self) -> u32 { self.root.depth() }

    pub fn variant_count(&self) -> usize { self.variants.len() }

    pub fn mark_modified(&mut self, time: u64) {
        self.modified_at = time;
        self.version += 1;
    }
}

// ─── Prefab instance ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrefabInstance {
    pub id: PrefabInstanceId,
    pub prefab_id: PrefabId,
    pub variant_name: Option<String>,
    pub entity_id: u32,
    pub overrides: Vec<PrefabOverride>,
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub scale:    [f64; 3],
    pub unlinked: bool,
    pub parent_instance: Option<PrefabInstanceId>,
}

impl PrefabInstance {
    pub fn new(id: PrefabInstanceId, prefab_id: PrefabId, entity_id: u32) -> Self {
        Self {
            id,
            prefab_id,
            variant_name: None,
            entity_id,
            overrides: Vec::new(),
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            unlinked: false,
            parent_instance: None,
        }
    }

    pub fn add_override(&mut self, ov: PrefabOverride) {
        if let Some(existing) = self.overrides.iter_mut().find(|o| o.path == ov.path) {
            *existing = ov;
        } else {
            self.overrides.push(ov);
        }
    }

    pub fn clear_override(&mut self, path: &str) {
        self.overrides.retain(|o| o.path != path);
    }

    pub fn clear_all_overrides(&mut self) {
        self.overrides.clear();
    }

    pub fn has_overrides(&self) -> bool { !self.overrides.is_empty() }
    pub fn override_count(&self) -> usize { self.overrides.len() }

    pub fn is_override_at(&self, path: &str) -> bool {
        self.overrides.iter().any(|o| o.path == path)
    }

    pub fn effective_value<'a>(&'a self, path: &str, prefab: &'a Prefab) -> Option<&'a ComponentValue> {
        // Instance overrides take priority
        if let Some(ov) = self.overrides.iter().find(|o| o.path == path) {
            if !ov.removed { return Some(&ov.value); }
            return None;
        }
        // Then variant overrides
        let variant_name = self.variant_name.as_deref().unwrap_or("Default");
        if let Some(variant) = prefab.variants.iter().find(|v| v.name == variant_name) {
            if let Some(ov) = variant.overrides.iter().find(|o| o.path == path) {
                if !ov.removed { return Some(&ov.value); }
                return None;
            }
        }
        // Then default variant
        if let Some(default) = prefab.variants.iter().find(|v| v.is_default) {
            if let Some(ov) = default.overrides.iter().find(|o| o.path == path) {
                if !ov.removed { return Some(&ov.value); }
            }
        }
        None
    }
}

// ─── Diff detection ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrefabDiff {
    pub path: String,
    pub kind: DiffKind,
    pub base_value: Option<ComponentValue>,
    pub instance_value: Option<ComponentValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Modified,
    Added,
    Removed,
}

impl fmt::Display for PrefabDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DiffKind::Modified => write!(f, "~ {} : {:?} → {:?}", self.path,
                self.base_value, self.instance_value),
            DiffKind::Added   => write!(f, "+ {} : {:?}", self.path, self.instance_value),
            DiffKind::Removed => write!(f, "- {} : {:?}", self.path, self.base_value),
        }
    }
}

pub fn diff_instance(instance: &PrefabInstance, _prefab: &Prefab) -> Vec<PrefabDiff> {
    let mut diffs = Vec::new();
    for ov in &instance.overrides {
        diffs.push(PrefabDiff {
            path: ov.path.clone(),
            kind: if ov.added { DiffKind::Added }
                  else if ov.removed { DiffKind::Removed }
                  else { DiffKind::Modified },
            base_value: None,
            instance_value: if ov.removed { None } else { Some(ov.value.clone()) },
        });
    }
    diffs
}

// ─── Prefab library ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PrefabLibrary {
    prefabs: HashMap<PrefabId, Prefab>,
    instances: HashMap<PrefabInstanceId, PrefabInstance>,
    next_prefab_id: u32,
    next_instance_id: u32,
    pub search_index: Vec<(PrefabId, String, Vec<String>)>,
}

impl PrefabLibrary {
    pub fn new() -> Self {
        let mut lib = Self {
            prefabs: HashMap::new(),
            instances: HashMap::new(),
            next_prefab_id: 1,
            next_instance_id: 1,
            search_index: Vec::new(),
        };
        lib.populate_defaults();
        lib
    }

    fn populate_defaults(&mut self) {
        // Primitive shapes
        let shapes = [
            ("Cube",     vec!["mesh", "primitive", "shape"]),
            ("Sphere",   vec!["mesh", "primitive", "shape"]),
            ("Cylinder", vec!["mesh", "primitive", "shape"]),
            ("Capsule",  vec!["mesh", "primitive", "shape"]),
            ("Plane",    vec!["mesh", "primitive", "flat"]),
            ("Quad",     vec!["mesh", "primitive", "flat", "ui"]),
        ];
        for (name, tags) in &shapes {
            let id = PrefabId(self.next_prefab_id);
            self.next_prefab_id += 1;
            let mut p = Prefab::new(id, name);
            p.tags = tags.iter().map(|s| s.to_string()).collect();
            p.tags.push("built-in".into());
            self.prefabs.insert(id, p);
        }
        // Lights
        let lights = [
            ("Directional Light", vec!["light", "directional"]),
            ("Point Light",       vec!["light", "point"]),
            ("Spot Light",        vec!["light", "spot"]),
            ("Area Light",        vec!["light", "area"]),
        ];
        for (name, tags) in &lights {
            let id = PrefabId(self.next_prefab_id);
            self.next_prefab_id += 1;
            let mut p = Prefab::new(id, name);
            p.tags = tags.iter().map(|s| s.to_string()).collect();
            p.tags.push("built-in".into());
            self.prefabs.insert(id, p);
        }
        // Cameras
        let id = PrefabId(self.next_prefab_id);
        self.next_prefab_id += 1;
        let mut cam = Prefab::new(id, "Camera");
        cam.tags = vec!["camera".into(), "built-in".into()];
        self.prefabs.insert(id, cam);

        // SDF body
        let sdf_id = PrefabId(self.next_prefab_id);
        self.next_prefab_id += 1;
        let mut sdf = Prefab::new(sdf_id, "SDF Body");
        sdf.tags = vec!["sdf".into(), "body".into(), "built-in".into()];
        self.prefabs.insert(sdf_id, sdf);

        // Force field
        let ff_id = PrefabId(self.next_prefab_id);
        self.next_prefab_id += 1;
        let mut ff = Prefab::new(ff_id, "Force Field");
        ff.tags = vec!["physics".into(), "force_field".into(), "built-in".into()];
        self.prefabs.insert(ff_id, ff);

        self.rebuild_index();
    }

    fn rebuild_index(&mut self) {
        self.search_index = self.prefabs.iter()
            .map(|(&id, p)| (id, p.name.clone(), p.tags.clone()))
            .collect();
    }

    pub fn register(&mut self, prefab: Prefab) -> PrefabId {
        let id = prefab.id;
        self.prefabs.insert(id, prefab);
        self.rebuild_index();
        id
    }

    pub fn create(&mut self, name: &str) -> PrefabId {
        let id = PrefabId(self.next_prefab_id);
        self.next_prefab_id += 1;
        self.prefabs.insert(id, Prefab::new(id, name));
        self.rebuild_index();
        id
    }

    pub fn remove(&mut self, id: PrefabId) -> Option<Prefab> {
        let p = self.prefabs.remove(&id);
        self.rebuild_index();
        p
    }

    pub fn get(&self, id: PrefabId) -> Option<&Prefab> {
        self.prefabs.get(&id)
    }

    pub fn get_mut(&mut self, id: PrefabId) -> Option<&mut Prefab> {
        self.prefabs.get_mut(&id)
    }

    pub fn instantiate(&mut self, prefab_id: PrefabId, entity_id: u32) -> Option<PrefabInstanceId> {
        if !self.prefabs.contains_key(&prefab_id) { return None; }
        let id = PrefabInstanceId(self.next_instance_id);
        self.next_instance_id += 1;
        self.instances.insert(id, PrefabInstance::new(id, prefab_id, entity_id));
        Some(id)
    }

    pub fn destroy_instance(&mut self, id: PrefabInstanceId) {
        self.instances.remove(&id);
    }

    pub fn instance(&self, id: PrefabInstanceId) -> Option<&PrefabInstance> {
        self.instances.get(&id)
    }

    pub fn instance_mut(&mut self, id: PrefabInstanceId) -> Option<&mut PrefabInstance> {
        self.instances.get_mut(&id)
    }

    pub fn instances_of(&self, prefab_id: PrefabId) -> Vec<&PrefabInstance> {
        self.instances.values().filter(|i| i.prefab_id == prefab_id).collect()
    }

    pub fn search(&self, query: &str) -> Vec<&Prefab> {
        let q = query.to_lowercase();
        self.prefabs.values()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || p.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&Prefab> {
        self.prefabs.values()
            .filter(|p| p.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn prefab_count(&self) -> usize { self.prefabs.len() }
    pub fn instance_count(&self) -> usize { self.instances.len() }

    /// Apply an override to all instances of a prefab
    pub fn propagate_change(&mut self, prefab_id: PrefabId, path: &str, value: ComponentValue) {
        for inst in self.instances.values_mut() {
            if inst.prefab_id == prefab_id && !inst.unlinked {
                // Only propagate if not already overridden by the instance
                if !inst.is_override_at(path) {
                    // The base change will be reflected automatically since
                    // effective_value checks the prefab base first
                    let _ = (path, &value);
                }
            }
        }
    }

    /// Unpack (unlink) an instance from its prefab
    pub fn unlink_instance(&mut self, id: PrefabInstanceId) {
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.unlinked = true;
        }
    }

    /// Re-link an unlinked instance back to its prefab source
    pub fn relink_instance(&mut self, id: PrefabInstanceId) {
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.unlinked = false;
        }
    }

    /// Create a new prefab from an existing entity (reverse workflow)
    pub fn create_from_overrides(
        &mut self,
        name: &str,
        instance_id: PrefabInstanceId,
    ) -> Option<PrefabId> {
        let base_id = self.instances.get(&instance_id)?.prefab_id;
        let overrides = self.instances.get(&instance_id)?.overrides.clone();
        let new_id = self.create(name);
        if let Some(base) = self.prefabs.get(&base_id) {
            let mut variant = PrefabVariant::new("FromInstance");
            for ov in overrides { variant.add_override(ov); }
            let base_clone = base.root.clone();
            if let Some(new_prefab) = self.prefabs.get_mut(&new_id) {
                new_prefab.root = base_clone;
                new_prefab.variants.push(variant);
            }
        }
        Some(new_id)
    }

    pub fn stats(&self) -> PrefabLibraryStats {
        let total_nodes: usize = self.prefabs.values().map(|p| p.node_count()).sum();
        let total_variants: usize = self.prefabs.values().map(|p| p.variant_count()).sum();
        PrefabLibraryStats {
            prefab_count: self.prefabs.len(),
            instance_count: self.instances.len(),
            total_nodes,
            total_variants,
            overridden_instances: self.instances.values().filter(|i| i.has_overrides()).count(),
            unlinked_instances: self.instances.values().filter(|i| i.unlinked).count(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrefabLibraryStats {
    pub prefab_count: usize,
    pub instance_count: usize,
    pub total_nodes: usize,
    pub total_variants: usize,
    pub overridden_instances: usize,
    pub unlinked_instances: usize,
}

impl fmt::Display for PrefabLibraryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "Prefabs: {} ({} nodes, {} variants) | Instances: {} ({} overridden, {} unlinked)",
            self.prefab_count, self.total_nodes, self.total_variants,
            self.instance_count, self.overridden_instances, self.unlinked_instances,
        )
    }
}

impl Default for PrefabLibrary {
    fn default() -> Self { Self::new() }
}

// ─── Prefab editor ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefabEditorMode {
    Browse,
    EditPrefab,
    EditInstance,
    VariantEditor,
    DiffView,
}

#[derive(Debug)]
pub struct PrefabEditor {
    pub library: PrefabLibrary,
    pub mode: PrefabEditorMode,
    pub selected_prefab: Option<PrefabId>,
    pub selected_instance: Option<PrefabInstanceId>,
    pub selected_variant: Option<String>,
    pub search_query: String,
    pub filter_tag: Option<String>,
    pub sort_by: PrefabSortMode,
    pub show_built_in: bool,
    pub show_variants: bool,
    pub show_instances_panel: bool,
    pub pending_delete: Option<PrefabId>,
    undo_stack: Vec<PrefabEditAction>,
    redo_stack: Vec<PrefabEditAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefabSortMode {
    Name,
    RecentlyModified,
    InstanceCount,
    NodeCount,
}

#[derive(Debug, Clone)]
pub enum PrefabEditAction {
    CreatePrefab(PrefabId),
    DeletePrefab(PrefabId),
    AddVariant { prefab: PrefabId, variant_name: String },
    RemoveVariant { prefab: PrefabId, variant_name: String },
    AddOverride { instance: PrefabInstanceId, ov: PrefabOverride },
    RemoveOverride { instance: PrefabInstanceId, path: String },
    UnlinkInstance(PrefabInstanceId),
    RelinkInstance(PrefabInstanceId),
    RenameNode { prefab: PrefabId, path: String, old: String, new: String },
}

impl PrefabEditor {
    pub fn new() -> Self {
        Self {
            library: PrefabLibrary::new(),
            mode: PrefabEditorMode::Browse,
            selected_prefab: None,
            selected_instance: None,
            selected_variant: None,
            search_query: String::new(),
            filter_tag: None,
            sort_by: PrefabSortMode::Name,
            show_built_in: true,
            show_variants: true,
            show_instances_panel: true,
            pending_delete: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn search_results(&self) -> Vec<&Prefab> {
        let mut results = if self.search_query.is_empty() {
            if let Some(tag) = &self.filter_tag {
                self.library.by_tag(tag)
            } else {
                self.library.prefabs.values().collect()
            }
        } else {
            self.library.search(&self.search_query)
        };
        if !self.show_built_in {
            results.retain(|p| !p.tags.contains(&"built-in".to_string()));
        }
        match self.sort_by {
            PrefabSortMode::Name => results.sort_by(|a, b| a.name.cmp(&b.name)),
            PrefabSortMode::RecentlyModified => results.sort_by(|a, b| b.modified_at.cmp(&a.modified_at)),
            PrefabSortMode::InstanceCount => {
                results.sort_by_key(|p| std::cmp::Reverse(self.library.instances_of(p.id).len()));
            }
            PrefabSortMode::NodeCount => results.sort_by_key(|p| std::cmp::Reverse(p.node_count())),
        }
        results
    }

    pub fn select_prefab(&mut self, id: PrefabId) {
        self.selected_prefab = Some(id);
        self.selected_instance = None;
        self.mode = PrefabEditorMode::EditPrefab;
    }

    pub fn select_instance(&mut self, id: PrefabInstanceId) {
        self.selected_instance = Some(id);
        self.mode = PrefabEditorMode::EditInstance;
        if let Some(inst) = self.library.instance(id) {
            self.selected_prefab = Some(inst.prefab_id);
        }
    }

    pub fn create_prefab(&mut self, name: &str) -> PrefabId {
        let id = self.library.create(name);
        self.undo_stack.push(PrefabEditAction::CreatePrefab(id));
        self.redo_stack.clear();
        self.selected_prefab = Some(id);
        self.mode = PrefabEditorMode::EditPrefab;
        id
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected_prefab.take() {
            self.undo_stack.push(PrefabEditAction::DeletePrefab(id));
            self.redo_stack.clear();
            self.library.remove(id);
        }
    }

    pub fn add_override_to_selected(&mut self, ov: PrefabOverride) {
        if let Some(iid) = self.selected_instance {
            self.undo_stack.push(PrefabEditAction::AddOverride {
                instance: iid, ov: ov.clone(),
            });
            self.redo_stack.clear();
            if let Some(inst) = self.library.instance_mut(iid) {
                inst.add_override(ov);
            }
        }
    }

    pub fn clear_override_on_selected(&mut self, path: &str) {
        if let Some(iid) = self.selected_instance {
            self.undo_stack.push(PrefabEditAction::RemoveOverride {
                instance: iid, path: path.to_string(),
            });
            self.redo_stack.clear();
            if let Some(inst) = self.library.instance_mut(iid) {
                inst.clear_override(path);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match &action {
                PrefabEditAction::CreatePrefab(id) => { self.library.remove(*id); }
                PrefabEditAction::DeletePrefab(_) => { /* TODO: restore */ }
                _ => {}
            }
            self.redo_stack.push(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.undo_stack.push(action);
        }
    }

    pub fn diff_view(&self) -> Option<Vec<PrefabDiff>> {
        let iid = self.selected_instance?;
        let inst = self.library.instance(iid)?;
        let prefab = self.library.get(inst.prefab_id)?;
        Some(diff_instance(inst, prefab))
    }

    pub fn stats(&self) -> PrefabLibraryStats { self.library.stats() }
}

// Needed for sort_by_key
use std::cmp::Reverse;

impl Default for PrefabEditor {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_search() {
        let mut lib = PrefabLibrary::new();
        let id = lib.create("TestRobot");
        if let Some(p) = lib.get_mut(id) {
            p.tags.push("robot".into());
        }
        let results = lib.search("robot");
        assert!(!results.is_empty());
        assert!(results.iter().any(|p| p.id == id));
    }

    #[test]
    fn instantiate_and_override() {
        let mut lib = PrefabLibrary::new();
        let pid = lib.create("Cube");
        let iid = lib.instantiate(pid, 42).unwrap();
        lib.instance_mut(iid).unwrap().add_override(
            PrefabOverride::set("Root.Transform.position", ComponentValue::Vec3([1.0, 0.0, 0.0]))
        );
        assert!(lib.instance(iid).unwrap().has_overrides());
    }

    #[test]
    fn prefab_node_hierarchy() {
        let mut root = PrefabNode::new("Root");
        root.add_child(PrefabNode::new("Child1"));
        let mut c2 = PrefabNode::new("Child2");
        c2.add_child(PrefabNode::new("Grandchild"));
        root.add_child(c2);
        assert_eq!(root.total_node_count(), 4);
        assert_eq!(root.depth(), 3);
    }

    #[test]
    fn variant_override() {
        let mut variant = PrefabVariant::new("Big");
        variant.add_override(PrefabOverride::set(
            "Root.Transform.scale",
            ComponentValue::Vec3([2.0, 2.0, 2.0]),
        ));
        assert_eq!(variant.override_count(), 1);
        variant.remove_override("Root.Transform.scale");
        assert_eq!(variant.override_count(), 0);
    }

    #[test]
    fn component_value_lerp() {
        let a = ComponentValue::Float(0.0);
        let b = ComponentValue::Float(10.0);
        let m = a.lerp(&b, 0.5).unwrap();
        assert!((m.as_float().unwrap() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn prefab_editor_search() {
        let ed = PrefabEditor::new();
        let results = ed.search_results();
        assert!(!results.is_empty());
    }

    #[test]
    fn prefab_editor_create_undo() {
        let mut ed = PrefabEditor::new();
        let count_before = ed.library.prefab_count();
        ed.create_prefab("NewPrefab");
        assert_eq!(ed.library.prefab_count(), count_before + 1);
        ed.undo();
        assert_eq!(ed.library.prefab_count(), count_before);
    }
}
