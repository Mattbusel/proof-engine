
// ============================================================
// SECTION: SHADER REFLECTION UTILITIES
// ============================================================

pub struct UniformBlockLayout {
    pub name: String,
    pub binding: u32,
    pub set: u32,
    pub members: Vec<UniformMember>,
    pub total_size: usize,
}

#[derive(Clone, Debug)]
pub struct UniformMember {
    pub name: String,
    pub offset: usize,
    pub size: usize,
    pub type_name: String,
    pub array_count: u32,
}

impl UniformBlockLayout {
    pub fn new(name: impl Into<String>, binding: u32, set: u32) -> Self {
        Self { name: name.into(), binding, set, members: Vec::new(), total_size: 0 }
    }

    pub fn add_member(&mut self, name: impl Into<String>, type_name: &str, array_count: u32) {
        let size = Self::type_size(type_name) * array_count as usize;
        self.members.push(UniformMember {
            name: name.into(),
            offset: self.total_size,
            size,
            type_name: type_name.into(),
            array_count,
        });
        // std140 alignment
        let align = Self::type_align(type_name);
        self.total_size += size;
        let rem = self.total_size % align;
        if rem != 0 { self.total_size += align - rem; }
    }

    fn type_size(t: &str) -> usize {
        match t {
            "float" => 4, "int" | "uint" => 4, "bool" => 4,
            "vec2" => 8, "vec3" => 12, "vec4" => 16,
            "mat3" => 48, "mat4" => 64,
            _ => 16,
        }
    }

    fn type_align(t: &str) -> usize {
        match t {
            "float" | "int" | "uint" | "bool" => 4,
            "vec2" => 8,
            "vec3" | "vec4" => 16,
            "mat3" | "mat4" => 16,
            _ => 16,
        }
    }

    pub fn generate_glsl_block(&self) -> String {
        let mut s = format!("layout(std140, binding = {}) uniform {} {{\n", self.binding, self.name);
        for m in &self.members {
            if m.array_count > 1 {
                s += &format!("    {} {}[{}];\n", m.type_name, m.name, m.array_count);
            } else {
                s += &format!("    {} {};\n", m.type_name, m.name);
            }
        }
        s += &format!("}}; // total {} bytes\n", self.total_size);
        s
    }
}

// ============================================================
// SECTION: SHADER PERMUTATION BAKER
// ============================================================

#[derive(Clone, Debug)]
pub struct ShaderPermutation {
    pub key: u64,
    pub defines: Vec<(String, String)>,
    pub vertex_src: String,
    pub fragment_src: String,
    pub compile_time_ms: u64,
}

pub struct PermutationBaker {
    pub base_vertex: String,
    pub base_fragment: String,
    pub feature_flags: Vec<(&'static str, &'static str)>,
    pub baked: Vec<ShaderPermutation>,
}

impl PermutationBaker {
    pub fn new(vertex: impl Into<String>, fragment: impl Into<String>) -> Self {
        Self { base_vertex: vertex.into(), base_fragment: fragment.into(), feature_flags: Vec::new(), baked: Vec::new() }
    }

    pub fn add_feature(&mut self, define: &'static str, value: &'static str) {
        self.feature_flags.push((define, value));
    }

    pub fn bake_all(&mut self) {
        let n = self.feature_flags.len();
        let total = 1u32 << n;
        for mask in 0..total {
            let mut defines = Vec::new();
            let mut key: u64 = 0;
            for (i, &(def, val)) in self.feature_flags.iter().enumerate() {
                if (mask >> i) & 1 == 1 {
                    defines.push((def.to_string(), val.to_string()));
                    key |= 1 << i;
                }
            }
            let prefix: String = defines.iter().map(|(d, v)| format!("#define {} {}\n", d, v)).collect();
            self.baked.push(ShaderPermutation {
                key,
                defines,
                vertex_src: prefix.clone() + &self.base_vertex,
                fragment_src: prefix + &self.base_fragment,
                compile_time_ms: 0,
            });
        }
    }

    pub fn find_permutation(&self, key: u64) -> Option<&ShaderPermutation> {
        self.baked.iter().find(|p| p.key == key)
    }

    pub fn total_permutations(&self) -> usize { self.baked.len() }
}

// ============================================================
// SECTION: SHADER CACHE WITH LRU EVICTION
// ============================================================

pub struct ShaderCacheEntry {
    pub source_hash: u64,
    pub spirv: Vec<u32>,
    pub last_used: u64,
    pub hit_count: u32,
}

pub struct ShaderLruCache {
    pub entries: std::collections::HashMap<u64, ShaderCacheEntry>,
    pub max_entries: usize,
    pub frame_counter: u64,
    pub hit_total: u64,
    pub miss_total: u64,
}

impl ShaderLruCache {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: std::collections::HashMap::new(), max_entries, frame_counter: 0, hit_total: 0, miss_total: 0 }
    }

    pub fn tick(&mut self) { self.frame_counter += 1; }

    pub fn get(&mut self, hash: u64) -> Option<&Vec<u32>> {
        if let Some(entry) = self.entries.get_mut(&hash) {
            entry.last_used = self.frame_counter;
            entry.hit_count += 1;
            self.hit_total += 1;
            Some(&entry.spirv)
        } else {
            self.miss_total += 1;
            None
        }
    }

    pub fn insert(&mut self, hash: u64, spirv: Vec<u32>) {
        if self.entries.len() >= self.max_entries {
            // Evict LRU entry
            if let Some(&lru_key) = self.entries.iter().min_by_key(|(_, e)| e.last_used).map(|(k, _)| k) {
                self.entries.remove(&lru_key);
            }
        }
        self.entries.insert(hash, ShaderCacheEntry { source_hash: hash, spirv, last_used: self.frame_counter, hit_count: 0 });
    }

    pub fn hit_rate(&self) -> f32 {
        let total = self.hit_total + self.miss_total;
        if total == 0 { return 0.0; }
        self.hit_total as f32 / total as f32
    }

    pub fn evict_stale(&mut self, max_age_frames: u64) {
        let threshold = self.frame_counter.saturating_sub(max_age_frames);
        self.entries.retain(|_, e| e.last_used >= threshold);
    }
}

// ============================================================
// SECTION: GLSL FUNCTION SIGNATURE PARSER
// ============================================================

#[derive(Clone, Debug)]
pub struct GlslFunctionSignature {
    pub name: String,
    pub return_type: String,
    pub params: Vec<(String, String)>,  // (type, name)
    pub qualifiers: Vec<String>,
}

pub struct GlslSignatureParser;

impl GlslSignatureParser {
    pub fn parse(src: &str) -> Vec<GlslFunctionSignature> {
        let mut sigs = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = src.chars().collect();
        let n = chars.len();
        while i < n {
            // Find identifier
            while i < n && !chars[i].is_alphabetic() && chars[i] != '_' { i += 1; }
            if i >= n { break; }
            let start = i;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
            let token = chars[start..i].iter().collect::<String>();
            // Skip whitespace
            while i < n && chars[i] == ' ' { i += 1; }
            // Look for function pattern: type name(
            if i < n && chars[i] == '(' {
                sigs.push(GlslFunctionSignature {
                    name: token.clone(),
                    return_type: "unknown".into(),
                    params: Vec::new(),
                    qualifiers: Vec::new(),
                });
                // Skip to matching )
                let mut depth = 1;
                i += 1;
                while i < n && depth > 0 {
                    if chars[i] == '(' { depth += 1; }
                    if chars[i] == ')' { depth -= 1; }
                    i += 1;
                }
            }
        }
        sigs
    }
}

// ============================================================
// SECTION: SHADER COMPILER FINAL VERSION
// ============================================================

pub fn shader_compiler_full_version() -> &'static str {
    "ShaderCompiler v3.0 - Full Pipeline - GLSL/SPIRV/WGSL"
}

pub fn build_standard_uniform_blocks() -> Vec<UniformBlockLayout> {
    let mut blocks = Vec::new();
    let mut camera = UniformBlockLayout::new("CameraUniforms", 0, 0);
    camera.add_member("view_matrix", "mat4", 1);
    camera.add_member("proj_matrix", "mat4", 1);
    camera.add_member("view_proj", "mat4", 1);
    camera.add_member("camera_position", "vec3", 1);
    camera.add_member("near_far", "vec2", 1);
    camera.add_member("fov", "float", 1);
    blocks.push(camera);

    let mut light = UniformBlockLayout::new("LightUniforms", 1, 0);
    light.add_member("light_positions", "vec4", 8);
    light.add_member("light_colors", "vec4", 8);
    light.add_member("light_directions", "vec4", 8);
    light.add_member("light_count", "uint", 1);
    blocks.push(light);

    let mut material = UniformBlockLayout::new("MaterialUniforms", 2, 0);
    material.add_member("albedo_factor", "vec4", 1);
    material.add_member("metallic_roughness", "vec2", 1);
    material.add_member("emissive_factor", "vec3", 1);
    material.add_member("alpha_cutoff", "float", 1);
    material.add_member("flags", "uint", 1);
    blocks.push(material);

    blocks
}

#[test]
fn test_uniform_block_layout() {
    let blocks = build_standard_uniform_blocks();
    assert_eq!(blocks.len(), 3);
    assert!(blocks[0].total_size > 0);
    let glsl = blocks[0].generate_glsl_block();
    assert!(glsl.contains("CameraUniforms"));
}

#[test]
fn test_permutation_baker() {
    let mut baker = PermutationBaker::new("void main(){}", "void main(){}");
    baker.add_feature("USE_NORMAL_MAP", "1");
    baker.add_feature("USE_SHADOWS", "1");
    baker.bake_all();
    assert_eq!(baker.total_permutations(), 4);
}

#[test]
fn test_shader_lru_cache() {
    let mut cache = ShaderLruCache::new(4);
    cache.insert(0xABCD, vec![1, 2, 3, 4]);
    assert!(cache.get(0xABCD).is_some());
    assert!(cache.get(0xDEAD).is_none());
    assert!(cache.hit_rate() > 0.0);
}
