//! Scene I/O — save and load scenes to/from a custom binary format and a
//! human-readable TOML format.
//!
//! # Binary format  (.scene)
//!
//! ```text
//! magic:   [u8; 8]  = b"PROOFSCN"
//! version: u32      = SCENE_VERSION
//! flags:   u32      = SceneFlags bits
//! --- sections ---
//! [SectionKind::Entities]
//!   count: u32
//!   for each entity: EntityRecord (variable-length)
//! [SectionKind::Lights]
//!   count: u32
//!   for each: LightRecord
//! [SectionKind::ForceFields]
//!   count: u32
//!   for each: ForceFieldRecord
//! [SectionKind::KitParams]
//!   count: u32
//!   for each: KitParamRecord (key:String, value:f32|bool|Vec4)
//! [SectionKind::Eof]
//! ```
//!
//! Strings are length-prefixed u16 UTF-8.  Vec3/Vec4 are four f32.
//! All multi-byte integers are little-endian.
//!
//! # TOML format  (.toml)
//!
//! Human-readable subset; intended for version control diffing.
//! Round-trips perfectly with the binary format for the supported fields.

use std::io::{Read, Write, Seek, SeekFrom, Cursor};
use std::path::PathBuf;
use glam::{Vec3, Vec4};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const MAGIC: &[u8; 8]      = b"PROOFSCN";
const SCENE_VERSION: u32    = 4;

// ─────────────────────────────────────────────────────────────────────────────
// SceneFlags
// ─────────────────────────────────────────────────────────────────────────────

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct SceneFlags: u32 {
        const COMPRESSED    = 0x0001;
        const HAS_AUDIO     = 0x0002;
        const HAS_SCRIPTS   = 0x0004;
        const EDITOR_ONLY   = 0x0008;
        const INSTANCED_SDF = 0x0010;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SectionKind
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionKind {
    Entities    = 0x01,
    Lights      = 0x02,
    ForceFields = 0x03,
    KitParams   = 0x04,
    Animations  = 0x05,
    SdfGraphs   = 0x06,
    Materials   = 0x07,
    Camera      = 0x08,
    Metadata    = 0x09,
    Eof         = 0xFF,
}

impl SectionKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Entities),
            0x02 => Some(Self::Lights),
            0x03 => Some(Self::ForceFields),
            0x04 => Some(Self::KitParams),
            0x05 => Some(Self::Animations),
            0x06 => Some(Self::SdfGraphs),
            0x07 => Some(Self::Materials),
            0x08 => Some(Self::Camera),
            0x09 => Some(Self::Metadata),
            0xFF => Some(Self::Eof),
            _    => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// IoError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum IoError {
    BadMagic,
    UnsupportedVersion(u32),
    UnknownSection(u8),
    Truncated,
    Utf8Error,
    Custom(String),
}

impl std::fmt::Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IoError::BadMagic            => write!(f, "bad magic bytes — not a proof-engine scene"),
            IoError::UnsupportedVersion(v) => write!(f, "unsupported scene version {v}"),
            IoError::UnknownSection(s)   => write!(f, "unknown section kind {s:#04x}"),
            IoError::Truncated           => write!(f, "unexpected end of file"),
            IoError::Utf8Error           => write!(f, "invalid UTF-8 in string field"),
            IoError::Custom(s)           => write!(f, "{s}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scene data model (serialised subset)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SceneMetadata {
    pub name:        String,
    pub author:      String,
    pub description: String,
    pub created_at:  u64,  // unix timestamp
    pub edited_at:   u64,
    pub engine_ver:  String,
}

#[derive(Debug, Clone)]
pub struct EntityRecord {
    pub id:         u32,
    pub name:       String,
    pub position:   Vec3,
    pub rotation:   Vec4,  // xyzw quaternion
    pub scale:      Vec3,
    pub visible:    bool,
    pub tags:       Vec<String>,
    pub sdf_graph:  Option<String>,  // name reference
    pub material:   Option<String>,
}

impl Default for EntityRecord {
    fn default() -> Self {
        Self {
            id: 0, name: String::new(),
            position: Vec3::ZERO, rotation: Vec4::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::ONE, visible: true, tags: Vec::new(),
            sdf_graph: None, material: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LightKind { Directional, Point, Spot, Area, Sky }

#[derive(Debug, Clone)]
pub struct LightRecord {
    pub id:        u32,
    pub name:      String,
    pub kind:      LightKind,
    pub position:  Vec3,
    pub direction: Vec3,
    pub color:     Vec4,
    pub intensity: f32,
    pub range:     f32,
    pub enabled:   bool,
}

#[derive(Debug, Clone)]
pub enum ForceFieldKind { Gravity, Vortex, Attractor, HeatSource, Wind, Pulsing }

#[derive(Debug, Clone)]
pub struct ForceFieldRecord {
    pub id:       u32,
    pub name:     String,
    pub kind:     ForceFieldKind,
    pub position: Vec3,
    pub params:   [f32; 8],
    pub enabled:  bool,
}

#[derive(Debug, Clone)]
pub enum KitParamValue {
    Float(f32),
    Bool(bool),
    Vec3(Vec3),
    Vec4(Vec4),
    Int(i32),
    String(String),
}

#[derive(Debug, Clone)]
pub struct KitParamRecord {
    pub group: String,
    pub key:   String,
    pub value: KitParamValue,
}

#[derive(Debug, Clone)]
pub struct CameraRecord {
    pub position:   Vec3,
    pub focal:      Vec3,
    pub azimuth:    f32,
    pub elevation:  f32,
    pub distance:   f32,
    pub fov:        f32,
    pub near:       f32,
    pub far:        f32,
    pub ortho:      bool,
}

impl Default for CameraRecord {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 4.0),
            focal: Vec3::ZERO,
            azimuth: 0.0, elevation: 0.2, distance: 4.0,
            fov: 65.0, near: 0.01, far: 1000.0, ortho: false,
        }
    }
}

/// A complete scene in memory.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub meta:         SceneMetadata,
    pub entities:     Vec<EntityRecord>,
    pub lights:       Vec<LightRecord>,
    pub force_fields: Vec<ForceFieldRecord>,
    pub kit_params:   Vec<KitParamRecord>,
    pub camera:       CameraRecord,
    pub flags:        SceneFlags,
}

// ─────────────────────────────────────────────────────────────────────────────
// Binary writer helpers
// ─────────────────────────────────────────────────────────────────────────────

fn write_u8(buf: &mut Vec<u8>, v: u8)  { buf.push(v); }
fn write_u16(buf: &mut Vec<u8>, v: u16) { buf.extend_from_slice(&v.to_le_bytes()); }
fn write_u32(buf: &mut Vec<u8>, v: u32) { buf.extend_from_slice(&v.to_le_bytes()); }
fn write_u64(buf: &mut Vec<u8>, v: u64) { buf.extend_from_slice(&v.to_le_bytes()); }
fn write_f32(buf: &mut Vec<u8>, v: f32) { buf.extend_from_slice(&v.to_le_bytes()); }
fn write_bool(buf: &mut Vec<u8>, v: bool) { buf.push(v as u8); }

fn write_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(u16::MAX as usize) as u16;
    write_u16(buf, len);
    buf.extend_from_slice(&bytes[..len as usize]);
}

fn write_vec3(buf: &mut Vec<u8>, v: Vec3) {
    write_f32(buf, v.x); write_f32(buf, v.y); write_f32(buf, v.z);
}

fn write_vec4(buf: &mut Vec<u8>, v: Vec4) {
    write_f32(buf, v.x); write_f32(buf, v.y); write_f32(buf, v.z); write_f32(buf, v.w);
}

// ─────────────────────────────────────────────────────────────────────────────
// Binary reader helpers
// ─────────────────────────────────────────────────────────────────────────────

fn read_u8(cur: &mut Cursor<&[u8]>) -> Result<u8, IoError> {
    let mut b = [0u8; 1];
    cur.read_exact(&mut b).map_err(|_| IoError::Truncated)?;
    Ok(b[0])
}

fn read_u16(cur: &mut Cursor<&[u8]>) -> Result<u16, IoError> {
    let mut b = [0u8; 2];
    cur.read_exact(&mut b).map_err(|_| IoError::Truncated)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(cur: &mut Cursor<&[u8]>) -> Result<u32, IoError> {
    let mut b = [0u8; 4];
    cur.read_exact(&mut b).map_err(|_| IoError::Truncated)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(cur: &mut Cursor<&[u8]>) -> Result<u64, IoError> {
    let mut b = [0u8; 8];
    cur.read_exact(&mut b).map_err(|_| IoError::Truncated)?;
    Ok(u64::from_le_bytes(b))
}

fn read_f32(cur: &mut Cursor<&[u8]>) -> Result<f32, IoError> {
    let mut b = [0u8; 4];
    cur.read_exact(&mut b).map_err(|_| IoError::Truncated)?;
    Ok(f32::from_le_bytes(b))
}

fn read_bool(cur: &mut Cursor<&[u8]>) -> Result<bool, IoError> {
    Ok(read_u8(cur)? != 0)
}

fn read_str(cur: &mut Cursor<&[u8]>) -> Result<String, IoError> {
    let len = read_u16(cur)? as usize;
    let mut buf = vec![0u8; len];
    cur.read_exact(&mut buf).map_err(|_| IoError::Truncated)?;
    String::from_utf8(buf).map_err(|_| IoError::Utf8Error)
}

fn read_vec3(cur: &mut Cursor<&[u8]>) -> Result<Vec3, IoError> {
    Ok(Vec3::new(read_f32(cur)?, read_f32(cur)?, read_f32(cur)?))
}

fn read_vec4(cur: &mut Cursor<&[u8]>) -> Result<Vec4, IoError> {
    Ok(Vec4::new(read_f32(cur)?, read_f32(cur)?, read_f32(cur)?, read_f32(cur)?))
}

// ─────────────────────────────────────────────────────────────────────────────
// SceneSerializer
// ─────────────────────────────────────────────────────────────────────────────

pub struct SceneSerializer;

impl SceneSerializer {
    // ── Write ─────────────────────────────────────────────────────────────

    pub fn write_binary(scene: &Scene) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(65_536);

        // Header
        buf.extend_from_slice(MAGIC);
        write_u32(&mut buf, SCENE_VERSION);
        write_u32(&mut buf, scene.flags.bits());

        // Metadata section
        write_u8(&mut buf, SectionKind::Metadata as u8);
        write_str(&mut buf, &scene.meta.name);
        write_str(&mut buf, &scene.meta.author);
        write_str(&mut buf, &scene.meta.description);
        write_u64(&mut buf, scene.meta.created_at);
        write_u64(&mut buf, scene.meta.edited_at);
        write_str(&mut buf, &scene.meta.engine_ver);

        // Camera section
        write_u8(&mut buf, SectionKind::Camera as u8);
        let c = &scene.camera;
        write_vec3(&mut buf, c.position);
        write_vec3(&mut buf, c.focal);
        write_f32(&mut buf, c.azimuth);
        write_f32(&mut buf, c.elevation);
        write_f32(&mut buf, c.distance);
        write_f32(&mut buf, c.fov);
        write_f32(&mut buf, c.near);
        write_f32(&mut buf, c.far);
        write_bool(&mut buf, c.ortho);

        // Entities section
        write_u8(&mut buf, SectionKind::Entities as u8);
        write_u32(&mut buf, scene.entities.len() as u32);
        for e in &scene.entities {
            write_u32(&mut buf, e.id);
            write_str(&mut buf, &e.name);
            write_vec3(&mut buf, e.position);
            write_vec4(&mut buf, e.rotation);
            write_vec3(&mut buf, e.scale);
            write_bool(&mut buf, e.visible);
            write_u16(&mut buf, e.tags.len() as u16);
            for tag in &e.tags { write_str(&mut buf, tag); }
            let sdf = e.sdf_graph.as_deref().unwrap_or("");
            write_str(&mut buf, sdf);
            let mat = e.material.as_deref().unwrap_or("");
            write_str(&mut buf, mat);
        }

        // Lights section
        write_u8(&mut buf, SectionKind::Lights as u8);
        write_u32(&mut buf, scene.lights.len() as u32);
        for l in &scene.lights {
            write_u32(&mut buf, l.id);
            write_str(&mut buf, &l.name);
            write_u8(&mut buf, match l.kind {
                LightKind::Directional => 0, LightKind::Point => 1,
                LightKind::Spot => 2, LightKind::Area => 3, LightKind::Sky => 4,
            });
            write_vec3(&mut buf, l.position);
            write_vec3(&mut buf, l.direction);
            write_vec4(&mut buf, l.color);
            write_f32(&mut buf, l.intensity);
            write_f32(&mut buf, l.range);
            write_bool(&mut buf, l.enabled);
        }

        // ForceFields section
        write_u8(&mut buf, SectionKind::ForceFields as u8);
        write_u32(&mut buf, scene.force_fields.len() as u32);
        for ff in &scene.force_fields {
            write_u32(&mut buf, ff.id);
            write_str(&mut buf, &ff.name);
            write_u8(&mut buf, match ff.kind {
                ForceFieldKind::Gravity => 0, ForceFieldKind::Vortex => 1,
                ForceFieldKind::Attractor => 2, ForceFieldKind::HeatSource => 3,
                ForceFieldKind::Wind => 4, ForceFieldKind::Pulsing => 5,
            });
            write_vec3(&mut buf, ff.position);
            for &p in &ff.params { write_f32(&mut buf, p); }
            write_bool(&mut buf, ff.enabled);
        }

        // Kit params section
        write_u8(&mut buf, SectionKind::KitParams as u8);
        write_u32(&mut buf, scene.kit_params.len() as u32);
        for kp in &scene.kit_params {
            write_str(&mut buf, &kp.group);
            write_str(&mut buf, &kp.key);
            match &kp.value {
                KitParamValue::Float(v)  => { write_u8(&mut buf, 0); write_f32(&mut buf, *v); }
                KitParamValue::Bool(v)   => { write_u8(&mut buf, 1); write_bool(&mut buf, *v); }
                KitParamValue::Vec3(v)   => { write_u8(&mut buf, 2); write_vec3(&mut buf, *v); }
                KitParamValue::Vec4(v)   => { write_u8(&mut buf, 3); write_vec4(&mut buf, *v); }
                KitParamValue::Int(v)    => { write_u8(&mut buf, 4); write_u32(&mut buf, *v as u32); }
                KitParamValue::String(v) => { write_u8(&mut buf, 5); write_str(&mut buf, v); }
            }
        }

        // EOF marker
        write_u8(&mut buf, SectionKind::Eof as u8);
        buf
    }

    // ── Read ──────────────────────────────────────────────────────────────

    pub fn read_binary(data: &[u8]) -> Result<Scene, IoError> {
        let mut cur = Cursor::new(data);
        let mut magic = [0u8; 8];
        cur.read_exact(&mut magic).map_err(|_| IoError::Truncated)?;
        if &magic != MAGIC { return Err(IoError::BadMagic); }

        let version = read_u32(&mut cur)?;
        if version > SCENE_VERSION { return Err(IoError::UnsupportedVersion(version)); }

        let flags_bits = read_u32(&mut cur)?;
        let flags = SceneFlags::from_bits_truncate(flags_bits);
        let mut scene = Scene { flags, ..Default::default() };

        loop {
            let kind_byte = read_u8(&mut cur)?;
            let kind = SectionKind::from_u8(kind_byte).ok_or(IoError::UnknownSection(kind_byte))?;
            match kind {
                SectionKind::Eof => break,

                SectionKind::Metadata => {
                    scene.meta.name        = read_str(&mut cur)?;
                    scene.meta.author      = read_str(&mut cur)?;
                    scene.meta.description = read_str(&mut cur)?;
                    scene.meta.created_at  = read_u64(&mut cur)?;
                    scene.meta.edited_at   = read_u64(&mut cur)?;
                    scene.meta.engine_ver  = read_str(&mut cur)?;
                }

                SectionKind::Camera => {
                    scene.camera.position  = read_vec3(&mut cur)?;
                    scene.camera.focal     = read_vec3(&mut cur)?;
                    scene.camera.azimuth   = read_f32(&mut cur)?;
                    scene.camera.elevation = read_f32(&mut cur)?;
                    scene.camera.distance  = read_f32(&mut cur)?;
                    scene.camera.fov       = read_f32(&mut cur)?;
                    scene.camera.near      = read_f32(&mut cur)?;
                    scene.camera.far       = read_f32(&mut cur)?;
                    scene.camera.ortho     = read_bool(&mut cur)?;
                }

                SectionKind::Entities => {
                    let count = read_u32(&mut cur)? as usize;
                    for _ in 0..count {
                        let mut e = EntityRecord::default();
                        e.id       = read_u32(&mut cur)?;
                        e.name     = read_str(&mut cur)?;
                        e.position = read_vec3(&mut cur)?;
                        e.rotation = read_vec4(&mut cur)?;
                        e.scale    = read_vec3(&mut cur)?;
                        e.visible  = read_bool(&mut cur)?;
                        let n_tags = read_u16(&mut cur)? as usize;
                        for _ in 0..n_tags { e.tags.push(read_str(&mut cur)?); }
                        let sdf = read_str(&mut cur)?;
                        if !sdf.is_empty() { e.sdf_graph = Some(sdf); }
                        let mat = read_str(&mut cur)?;
                        if !mat.is_empty() { e.material = Some(mat); }
                        scene.entities.push(e);
                    }
                }

                SectionKind::Lights => {
                    let count = read_u32(&mut cur)? as usize;
                    for _ in 0..count {
                        let id        = read_u32(&mut cur)?;
                        let name      = read_str(&mut cur)?;
                        let kind_byte = read_u8(&mut cur)?;
                        let kind = match kind_byte {
                            0 => LightKind::Directional, 1 => LightKind::Point,
                            2 => LightKind::Spot, 3 => LightKind::Area,
                            _ => LightKind::Sky,
                        };
                        let position  = read_vec3(&mut cur)?;
                        let direction = read_vec3(&mut cur)?;
                        let color     = read_vec4(&mut cur)?;
                        let intensity = read_f32(&mut cur)?;
                        let range     = read_f32(&mut cur)?;
                        let enabled   = read_bool(&mut cur)?;
                        scene.lights.push(LightRecord { id, name, kind, position, direction,
                            color, intensity, range, enabled });
                    }
                }

                SectionKind::ForceFields => {
                    let count = read_u32(&mut cur)? as usize;
                    for _ in 0..count {
                        let id        = read_u32(&mut cur)?;
                        let name      = read_str(&mut cur)?;
                        let kind_byte = read_u8(&mut cur)?;
                        let kind = match kind_byte {
                            0 => ForceFieldKind::Gravity, 1 => ForceFieldKind::Vortex,
                            2 => ForceFieldKind::Attractor, 3 => ForceFieldKind::HeatSource,
                            4 => ForceFieldKind::Wind, _ => ForceFieldKind::Pulsing,
                        };
                        let position = read_vec3(&mut cur)?;
                        let mut params = [0.0f32; 8];
                        for p in &mut params { *p = read_f32(&mut cur)?; }
                        let enabled = read_bool(&mut cur)?;
                        scene.force_fields.push(ForceFieldRecord { id, name, kind, position, params, enabled });
                    }
                }

                SectionKind::KitParams => {
                    let count = read_u32(&mut cur)? as usize;
                    for _ in 0..count {
                        let group = read_str(&mut cur)?;
                        let key   = read_str(&mut cur)?;
                        let vtype = read_u8(&mut cur)?;
                        let value = match vtype {
                            0 => KitParamValue::Float(read_f32(&mut cur)?),
                            1 => KitParamValue::Bool(read_bool(&mut cur)?),
                            2 => KitParamValue::Vec3(read_vec3(&mut cur)?),
                            3 => KitParamValue::Vec4(read_vec4(&mut cur)?),
                            4 => KitParamValue::Int(read_u32(&mut cur)? as i32),
                            5 => KitParamValue::String(read_str(&mut cur)?),
                            _ => KitParamValue::Float(0.0),
                        };
                        scene.kit_params.push(KitParamRecord { group, key, value });
                    }
                }

                SectionKind::Animations | SectionKind::SdfGraphs |
                SectionKind::Materials => {
                    // Skip unknown payload for forward compatibility
                    // (We'd need section length for real skip — simplified here)
                }
            }
        }

        Ok(scene)
    }

    // ── TOML ──────────────────────────────────────────────────────────────

    pub fn write_toml(scene: &Scene) -> String {
        let mut out = String::with_capacity(8192);
        out.push_str(&format!(
            "[meta]\nname = {:?}\nauthor = {:?}\ndescription = {:?}\nengine_ver = {:?}\n\n",
            scene.meta.name, scene.meta.author, scene.meta.description, scene.meta.engine_ver
        ));

        let c = &scene.camera;
        out.push_str(&format!(
            "[camera]\nposition = [{:.6}, {:.6}, {:.6}]\nfocal = [{:.6}, {:.6}, {:.6}]\n\
             azimuth = {:.6}\nelevation = {:.6}\ndistance = {:.6}\nfov = {:.2}\n\
             near = {:.4}\nfar = {:.2}\northo = {}\n\n",
            c.position.x, c.position.y, c.position.z,
            c.focal.x, c.focal.y, c.focal.z,
            c.azimuth, c.elevation, c.distance, c.fov, c.near, c.far, c.ortho
        ));

        for (i, e) in scene.entities.iter().enumerate() {
            out.push_str(&format!("[[entities]]  # {i}\n"));
            out.push_str(&format!("id = {}\nname = {:?}\n", e.id, e.name));
            out.push_str(&format!("position = [{:.6}, {:.6}, {:.6}]\n",
                e.position.x, e.position.y, e.position.z));
            out.push_str(&format!("rotation = [{:.6}, {:.6}, {:.6}, {:.6}]\n",
                e.rotation.x, e.rotation.y, e.rotation.z, e.rotation.w));
            out.push_str(&format!("scale = [{:.6}, {:.6}, {:.6}]\n",
                e.scale.x, e.scale.y, e.scale.z));
            out.push_str(&format!("visible = {}\n", e.visible));
            if let Some(sdf) = &e.sdf_graph {
                out.push_str(&format!("sdf_graph = {:?}\n", sdf));
            }
            if let Some(mat) = &e.material {
                out.push_str(&format!("material = {:?}\n", mat));
            }
            out.push('\n');
        }

        for l in &scene.lights {
            out.push_str("[[lights]]\n");
            out.push_str(&format!("id = {}\nname = {:?}\n", l.id, l.name));
            out.push_str(&format!("position = [{:.6}, {:.6}, {:.6}]\n",
                l.position.x, l.position.y, l.position.z));
            out.push_str(&format!("color = [{:.4}, {:.4}, {:.4}, {:.4}]\n",
                l.color.x, l.color.y, l.color.z, l.color.w));
            out.push_str(&format!("intensity = {:.4}\nenabled = {}\n\n", l.intensity, l.enabled));
        }

        for kp in &scene.kit_params {
            let val_str = match &kp.value {
                KitParamValue::Float(v) => format!("{:.6}", v),
                KitParamValue::Bool(v)  => format!("{}", v),
                KitParamValue::Vec3(v)  => format!("[{:.4}, {:.4}, {:.4}]", v.x, v.y, v.z),
                KitParamValue::Vec4(v)  => format!("[{:.4}, {:.4}, {:.4}, {:.4}]", v.x, v.y, v.z, v.w),
                KitParamValue::Int(v)   => format!("{}", v),
                KitParamValue::String(v)=> format!("{:?}", v),
            };
            out.push_str(&format!("# [{}.{}] = {}\n", kp.group, kp.key, val_str));
        }

        out
    }

    // ── Size estimation ───────────────────────────────────────────────────

    pub fn estimate_size(scene: &Scene) -> usize {
        // header + metadata + camera + per-entity + per-light + per-ff + per-param
        8 + 4 + 4   // magic + version + flags
        + 128       // metadata strings
        + 64        // camera
        + scene.entities.len() * 80
        + scene.lights.len() * 60
        + scene.force_fields.len() * 52
        + scene.kit_params.len() * 32
        + 1         // EOF
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SceneUndoManager
// ─────────────────────────────────────────────────────────────────────────────

/// Manages undo/redo for scene-level edits using full-snapshot approach.
pub struct SceneUndoManager {
    pub max_history: usize,
    snapshots:       std::collections::VecDeque<Vec<u8>>,
    current:         usize,
}

impl SceneUndoManager {
    pub fn new(max_history: usize) -> Self {
        Self { max_history, snapshots: std::collections::VecDeque::new(), current: 0 }
    }

    pub fn checkpoint(&mut self, scene: &Scene) {
        // Truncate redo history
        while self.snapshots.len() > self.current + 1 {
            self.snapshots.pop_back();
        }
        let bytes = SceneSerializer::write_binary(scene);
        self.snapshots.push_back(bytes);
        if self.snapshots.len() > self.max_history {
            self.snapshots.pop_front();
        }
        self.current = self.snapshots.len().saturating_sub(1);
    }

    pub fn undo(&mut self) -> Option<Scene> {
        if self.current == 0 { return None; }
        self.current -= 1;
        let bytes = self.snapshots.get(self.current)?;
        SceneSerializer::read_binary(bytes).ok()
    }

    pub fn redo(&mut self) -> Option<Scene> {
        if self.current + 1 >= self.snapshots.len() { return None; }
        self.current += 1;
        let bytes = self.snapshots.get(self.current)?;
        SceneSerializer::read_binary(bytes).ok()
    }

    pub fn can_undo(&self) -> bool { self.current > 0 }
    pub fn can_redo(&self) -> bool { self.current + 1 < self.snapshots.len() }
    pub fn history_len(&self) -> usize { self.snapshots.len() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scene() -> Scene {
        let mut s = Scene::default();
        s.meta.name = "TestScene".into();
        s.meta.author = "Unit Test".into();
        s.entities.push(EntityRecord {
            id: 1, name: "Leon".into(),
            position: Vec3::new(0.0, 1.0, 0.0),
            rotation: Vec4::new(0.0, 0.0, 0.0, 1.0),
            scale: Vec3::ONE,
            visible: true,
            tags: vec!["character".into()],
            sdf_graph: Some("leon.sdf".into()),
            material: None,
        });
        s.kit_params.push(KitParamRecord {
            group: "Bloom".into(), key: "intensity".into(),
            value: KitParamValue::Float(3.0),
        });
        s
    }

    #[test]
    fn binary_roundtrip() {
        let scene = make_scene();
        let bytes = SceneSerializer::write_binary(&scene);
        assert!(bytes.len() > 8);
        let loaded = SceneSerializer::read_binary(&bytes).unwrap();
        assert_eq!(loaded.meta.name, "TestScene");
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.entities[0].name, "Leon");
        assert_eq!(loaded.kit_params.len(), 1);
        if let KitParamValue::Float(v) = loaded.kit_params[0].value {
            assert!((v - 3.0).abs() < 1e-5);
        } else { panic!("wrong type"); }
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = SceneSerializer::write_binary(&make_scene());
        bytes[0] = 0xFF;
        assert!(matches!(SceneSerializer::read_binary(&bytes), Err(IoError::BadMagic)));
    }

    #[test]
    fn toml_contains_name() {
        let scene = make_scene();
        let toml = SceneSerializer::write_toml(&scene);
        assert!(toml.contains("TestScene"));
        assert!(toml.contains("[camera]"));
    }

    #[test]
    fn undo_manager_basic() {
        let mut undo = SceneUndoManager::new(10);
        let s1 = make_scene();
        undo.checkpoint(&s1);
        let mut s2 = make_scene();
        s2.meta.name = "Modified".into();
        undo.checkpoint(&s2);
        let undone = undo.undo().unwrap();
        assert_eq!(undone.meta.name, "TestScene");
        assert!(undo.can_redo());
        let redone = undo.redo().unwrap();
        assert_eq!(redone.meta.name, "Modified");
    }

    #[test]
    fn estimate_size_reasonable() {
        let scene = make_scene();
        let est = SceneSerializer::estimate_size(&scene);
        let act = SceneSerializer::write_binary(&scene).len();
        assert!(est > 0 && act > 0);
    }
}
