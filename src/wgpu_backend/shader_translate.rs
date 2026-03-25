//! Shader translation between GLSL, WGSL, SPIRV (text), HLSL, and MSL.
//! Includes reflection (extracting bindings, inputs, outputs) and validation.

use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Severity of a shader diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// A located shader error / warning.
#[derive(Debug, Clone)]
pub struct ShaderError {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: Severity,
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error   => "error",
            Severity::Warning => "warning",
            Severity::Info    => "info",
        };
        write!(f, "{}:{}:{}: {}", sev, self.line, self.col, self.message)
    }
}

/// Translation error.
#[derive(Debug, Clone)]
pub struct TranslateError {
    pub message: String,
    pub errors: Vec<ShaderError>,
}

impl fmt::Display for TranslateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TranslateError: {}", self.message)?;
        for e in &self.errors {
            write!(f, "\n  {}", e)?;
        }
        Ok(())
    }
}

impl std::error::Error for TranslateError {}

impl TranslateError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), errors: Vec::new() }
    }
    pub fn with_error(mut self, err: ShaderError) -> Self {
        self.errors.push(err);
        self
    }
}

// ---------------------------------------------------------------------------
// Shader language enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderLanguage {
    GLSL,
    WGSL,
    SPIRV,
    HLSL,
    MSL,
}

impl fmt::Display for ShaderLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GLSL  => write!(f, "GLSL"),
            Self::WGSL  => write!(f, "WGSL"),
            Self::SPIRV => write!(f, "SPIR-V"),
            Self::HLSL  => write!(f, "HLSL"),
            Self::MSL   => write!(f, "MSL"),
        }
    }
}

// ---------------------------------------------------------------------------
// Token types (for the mini-lexer)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum GlslToken {
    Version(u32),
    In { location: u32, ty: String, name: String },
    Out { location: u32, ty: String, name: String },
    Uniform { ty: String, name: String },
    UniformBlock { name: String, binding: Option<u32> },
    Sampler { name: String },
    MainBegin,
    MainEnd,
    Line(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum WgslToken {
    Struct { name: String, fields: Vec<(String, String)> },
    Binding { group: u32, binding: u32, name: String, ty: String },
    VertexOutput { name: String },
    FragmentOutput { name: String },
    EntryPoint { stage: String, name: String },
    Line(String),
}

// ---------------------------------------------------------------------------
// GLSL parser helpers
// ---------------------------------------------------------------------------

/// Parse a GLSL source and extract tokens for the translator.
fn parse_glsl_tokens(source: &str) -> Vec<GlslToken> {
    let mut tokens = Vec::new();
    let mut in_main = false;

    for line in source.lines() {
        let trimmed = line.trim();

        // #version
        if trimmed.starts_with("#version") {
            if let Some(ver_str) = trimmed.strip_prefix("#version") {
                let ver_str = ver_str.trim().split_whitespace().next().unwrap_or("330");
                if let Ok(v) = ver_str.parse::<u32>() {
                    tokens.push(GlslToken::Version(v));
                }
            }
            continue;
        }

        // layout(location = N) in TYPE NAME;
        if let Some(rest) = try_parse_layout_in_out(trimmed) {
            tokens.push(rest);
            continue;
        }

        // uniform TYPE NAME;
        if trimmed.starts_with("uniform ") && !trimmed.contains('{') {
            let parts: Vec<&str> = trimmed.trim_end_matches(';')
                .split_whitespace().collect();
            if parts.len() >= 3 {
                let ty = parts[1].to_string();
                let name = parts[2].trim_end_matches(';').to_string();
                if ty.starts_with("sampler") {
                    tokens.push(GlslToken::Sampler { name });
                } else {
                    tokens.push(GlslToken::Uniform { ty, name });
                }
            }
            continue;
        }

        // void main()
        if trimmed.contains("void main") && trimmed.contains('(') {
            tokens.push(GlslToken::MainBegin);
            in_main = true;
            continue;
        }

        // closing brace of main (simplistic)
        if in_main && trimmed == "}" {
            tokens.push(GlslToken::MainEnd);
            in_main = false;
            continue;
        }

        tokens.push(GlslToken::Line(line.to_string()));
    }
    tokens
}

/// Try to parse `layout(location = N) in/out TYPE NAME;`
fn try_parse_layout_in_out(line: &str) -> Option<GlslToken> {
    if !line.starts_with("layout") { return None; }
    let loc = extract_location(line)?;
    let after_paren = line.find(')')? ;
    let rest = &line[after_paren + 1..].trim();

    if rest.starts_with("in ") {
        let parts: Vec<&str> = rest[3..].trim_end_matches(';').split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(GlslToken::In {
                location: loc,
                ty: parts[0].to_string(),
                name: parts[1].trim_end_matches(';').to_string(),
            });
        }
    } else if rest.starts_with("out ") {
        let parts: Vec<&str> = rest[4..].trim_end_matches(';').split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(GlslToken::Out {
                location: loc,
                ty: parts[0].to_string(),
                name: parts[1].trim_end_matches(';').to_string(),
            });
        }
    }
    None
}

fn extract_location(line: &str) -> Option<u32> {
    let start = line.find("location")? + "location".len();
    let rest = &line[start..];
    let eq = rest.find('=')?;
    let after_eq = &rest[eq + 1..];
    let end = after_eq.find(')')?;
    after_eq[..end].trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Type translation helpers
// ---------------------------------------------------------------------------

fn glsl_type_to_wgsl(ty: &str) -> String {
    match ty {
        "float" => "f32".into(),
        "int"   => "i32".into(),
        "uint"  => "u32".into(),
        "bool"  => "bool".into(),
        "vec2"  => "vec2<f32>".into(),
        "vec3"  => "vec3<f32>".into(),
        "vec4"  => "vec4<f32>".into(),
        "ivec2" => "vec2<i32>".into(),
        "ivec3" => "vec3<i32>".into(),
        "ivec4" => "vec4<i32>".into(),
        "uvec2" => "vec2<u32>".into(),
        "uvec3" => "vec3<u32>".into(),
        "uvec4" => "vec4<u32>".into(),
        "mat2"  => "mat2x2<f32>".into(),
        "mat3"  => "mat3x3<f32>".into(),
        "mat4"  => "mat4x4<f32>".into(),
        "sampler2D" => "texture_2d<f32>".into(),
        other => other.to_string(),
    }
}

fn wgsl_type_to_glsl(ty: &str) -> String {
    match ty {
        "f32"             => "float".into(),
        "i32"             => "int".into(),
        "u32"             => "uint".into(),
        "vec2<f32>"       => "vec2".into(),
        "vec3<f32>"       => "vec3".into(),
        "vec4<f32>"       => "vec4".into(),
        "vec2<i32>"       => "ivec2".into(),
        "vec3<i32>"       => "ivec3".into(),
        "vec4<i32>"       => "ivec4".into(),
        "vec2<u32>"       => "uvec2".into(),
        "vec3<u32>"       => "uvec3".into(),
        "vec4<u32>"       => "uvec4".into(),
        "mat2x2<f32>"     => "mat2".into(),
        "mat3x3<f32>"     => "mat3".into(),
        "mat4x4<f32>"     => "mat4".into(),
        "texture_2d<f32>" => "sampler2D".into(),
        other => other.to_string(),
    }
}

/// Translate GLSL built-in function calls to WGSL equivalents in a line.
fn translate_glsl_builtins_to_wgsl(line: &str) -> String {
    let mut out = line.to_string();
    // texture2D(sampler, uv) -> textureSample(sampler, sampler_sampler, uv)
    // We do a simple string replacement for common patterns.
    if out.contains("texture2D(") {
        out = out.replace("texture2D(", "textureSample(");
    }
    if out.contains("texture(") {
        out = out.replace("texture(", "textureSample(");
    }
    // gl_Position -> output.position
    out = out.replace("gl_Position", "output.position");
    // gl_FragColor -> output.color (simplified)
    out = out.replace("gl_FragColor", "output.color");
    // GLSL smoothstep, mix, clamp have the same names in WGSL
    out
}

/// Translate WGSL built-in patterns back to GLSL.
fn translate_wgsl_builtins_to_glsl(line: &str) -> String {
    let mut out = line.to_string();
    out = out.replace("textureSample(", "texture(");
    out = out.replace("output.position", "gl_Position");
    out = out.replace("output.color", "gl_FragColor");
    out
}

// ---------------------------------------------------------------------------
// GLSL -> WGSL
// ---------------------------------------------------------------------------

/// Translate a GLSL shader to WGSL.
pub fn glsl_to_wgsl(glsl_source: &str) -> Result<String, TranslateError> {
    let tokens = parse_glsl_tokens(glsl_source);
    let mut wgsl = String::new();
    let mut inputs: Vec<(u32, String, String)> = Vec::new();
    let mut outputs: Vec<(u32, String, String)> = Vec::new();
    let mut uniforms: Vec<(String, String)> = Vec::new();
    let mut samplers: Vec<String> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();
    let mut in_body = false;
    let mut binding_counter = 0u32;

    for token in &tokens {
        match token {
            GlslToken::Version(_) => {}
            GlslToken::In { location, ty, name } => {
                inputs.push((*location, ty.clone(), name.clone()));
            }
            GlslToken::Out { location, ty, name } => {
                outputs.push((*location, ty.clone(), name.clone()));
            }
            GlslToken::Uniform { ty, name } => {
                uniforms.push((ty.clone(), name.clone()));
            }
            GlslToken::Sampler { name } => {
                samplers.push(name.clone());
            }
            GlslToken::UniformBlock { .. } => {}
            GlslToken::MainBegin => {
                in_body = true;
            }
            GlslToken::MainEnd => {
                in_body = false;
            }
            GlslToken::Line(l) => {
                if in_body {
                    body_lines.push(l.clone());
                }
            }
        }
    }

    // Emit input struct
    if !inputs.is_empty() {
        wgsl.push_str("struct VertexInput {\n");
        for (loc, ty, name) in &inputs {
            wgsl.push_str(&format!(
                "    @location({}) {}: {},\n",
                loc, name, glsl_type_to_wgsl(ty)
            ));
        }
        wgsl.push_str("};\n\n");
    }

    // Emit output struct
    if !outputs.is_empty() {
        wgsl.push_str("struct VertexOutput {\n");
        wgsl.push_str("    @builtin(position) position: vec4<f32>,\n");
        for (loc, ty, name) in &outputs {
            wgsl.push_str(&format!(
                "    @location({}) {}: {},\n",
                loc, name, glsl_type_to_wgsl(ty)
            ));
        }
        wgsl.push_str("};\n\n");
    }

    // Emit uniforms as @group(0) @binding(N)
    for (ty, name) in &uniforms {
        wgsl.push_str(&format!(
            "@group(0) @binding({}) var<uniform> {}: {};\n",
            binding_counter, name, glsl_type_to_wgsl(ty)
        ));
        binding_counter += 1;
    }

    // Emit samplers
    for name in &samplers {
        wgsl.push_str(&format!(
            "@group(0) @binding({}) var {}: texture_2d<f32>;\n",
            binding_counter, name,
        ));
        binding_counter += 1;
        wgsl.push_str(&format!(
            "@group(0) @binding({}) var {}_sampler: sampler;\n",
            binding_counter, name,
        ));
        binding_counter += 1;
    }

    if !uniforms.is_empty() || !samplers.is_empty() {
        wgsl.push('\n');
    }

    // Emit entry point
    wgsl.push_str("@vertex\n");
    wgsl.push_str("fn vs_main(input: VertexInput) -> VertexOutput {\n");
    wgsl.push_str("    var output: VertexOutput;\n");
    for line in &body_lines {
        let translated = translate_glsl_builtins_to_wgsl(line);
        let translated = translated.trim();
        if !translated.is_empty() {
            wgsl.push_str(&format!("    {}\n", translated));
        }
    }
    wgsl.push_str("    return output;\n");
    wgsl.push_str("}\n");

    Ok(wgsl)
}

// ---------------------------------------------------------------------------
// GLSL -> SPIRV text representation
// ---------------------------------------------------------------------------

/// Produce a SPIR-V text representation from GLSL source.
/// This is a simplified textual output, not actual binary SPIR-V.
pub fn glsl_to_spirv_text(glsl_source: &str) -> Result<String, TranslateError> {
    let tokens = parse_glsl_tokens(glsl_source);
    let mut spirv = String::new();
    spirv.push_str("; SPIR-V text representation (generated by proof-engine shader_translate)\n");
    spirv.push_str("; Magic:     0x07230203\n");
    spirv.push_str("; Version:   1.0\n");
    spirv.push_str("; Generator: proof-engine\n\n");

    let mut id_counter = 1u32;
    let mut next_id = || { let id = id_counter; id_counter += 1; id };

    // Capabilities
    spirv.push_str("               OpCapability Shader\n");
    let ext_id = next_id();
    spirv.push_str(&format!("          %{}  = OpExtInstImport \"GLSL.std.450\"\n", ext_id));
    spirv.push_str("               OpMemoryModel Logical GLSL450\n");

    // Entry point
    let main_id = next_id();
    let mut interface_ids = Vec::new();

    for token in &tokens {
        match token {
            GlslToken::In { location, ty, name } => {
                let var_id = next_id();
                interface_ids.push(var_id);
                spirv.push_str(&format!(
                    "               OpDecorate %{} Location {}\n",
                    var_id, location
                ));
                let type_id = next_id();
                spirv.push_str(&format!(
                    "       %{}  = OpTypePointer Input %{} ; {}: {}\n",
                    var_id, type_id, name, ty
                ));
            }
            GlslToken::Out { location, ty, name } => {
                let var_id = next_id();
                interface_ids.push(var_id);
                spirv.push_str(&format!(
                    "               OpDecorate %{} Location {}\n",
                    var_id, location
                ));
                let type_id = next_id();
                spirv.push_str(&format!(
                    "       %{}  = OpTypePointer Output %{} ; {}: {}\n",
                    var_id, type_id, name, ty
                ));
            }
            GlslToken::Uniform { ty, name } => {
                let var_id = next_id();
                let type_id = next_id();
                spirv.push_str(&format!(
                    "       %{}  = OpTypePointer Uniform %{} ; uniform {}: {}\n",
                    var_id, type_id, name, ty
                ));
            }
            _ => {}
        }
    }

    let iface_str: String = interface_ids.iter().map(|id| format!("%{}", id)).collect::<Vec<_>>().join(" ");
    spirv.push_str(&format!(
        "               OpEntryPoint Vertex %{} \"main\" {}\n",
        main_id, iface_str
    ));

    // Main function
    let void_id = next_id();
    let func_type_id = next_id();
    spirv.push_str(&format!("       %{}  = OpTypeVoid\n", void_id));
    spirv.push_str(&format!("       %{}  = OpTypeFunction %{}\n", func_type_id, void_id));
    spirv.push_str(&format!("       %{}  = OpFunction %{} None %{}\n", main_id, void_id, func_type_id));
    let label_id = next_id();
    spirv.push_str(&format!("       %{}  = OpLabel\n", label_id));
    spirv.push_str("               OpReturn\n");
    spirv.push_str("               OpFunctionEnd\n");

    Ok(spirv)
}

// ---------------------------------------------------------------------------
// WGSL -> GLSL
// ---------------------------------------------------------------------------

/// Translate a simple WGSL shader back to GLSL 330 core.
pub fn wgsl_to_glsl(wgsl_source: &str) -> Result<String, TranslateError> {
    let mut glsl = String::from("#version 330 core\n\n");
    let mut in_struct = false;
    let mut current_struct_name = String::new();
    let mut in_fn = false;
    let mut fn_body_lines: Vec<String> = Vec::new();

    for line in wgsl_source.lines() {
        let trimmed = line.trim();

        // Parse @group(G) @binding(B) var<uniform> NAME: TYPE;
        if trimmed.starts_with("@group") && trimmed.contains("var<uniform>") {
            if let Some(rest) = trimmed.split("var<uniform>").nth(1) {
                let rest = rest.trim().trim_end_matches(';');
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let name = parts[0].trim();
                    let ty = parts[1].trim();
                    glsl.push_str(&format!("uniform {} {};\n", wgsl_type_to_glsl(ty), name));
                }
            }
            continue;
        }

        // Parse @group(G) @binding(B) var NAME: texture_2d<f32>;
        if trimmed.starts_with("@group") && trimmed.contains("var ") && trimmed.contains("texture") {
            // Skip texture bindings (handled as sampler2D in GLSL)
            continue;
        }

        // Parse @group(G) @binding(B) var NAME: sampler;
        if trimmed.starts_with("@group") && trimmed.contains("sampler") && !trimmed.contains("texture") {
            // The corresponding texture was already skipped; emit a sampler2D
            if let Some(rest) = trimmed.split("var ").nth(1) {
                let name = rest.split(':').next().unwrap_or("").trim();
                // Strip _sampler suffix
                let base = name.strip_suffix("_sampler").unwrap_or(name);
                glsl.push_str(&format!("uniform sampler2D {};\n", base));
            }
            continue;
        }

        // Struct
        if trimmed.starts_with("struct ") {
            in_struct = true;
            current_struct_name = trimmed
                .strip_prefix("struct ")
                .unwrap_or("")
                .trim_end_matches('{')
                .trim()
                .to_string();
            continue;
        }

        if in_struct {
            if trimmed.starts_with('}') {
                in_struct = false;
                continue;
            }
            // @location(N) name: type,  or  @builtin(position) ...
            if trimmed.contains("@location") {
                if let Some(loc) = extract_wgsl_location(trimmed) {
                    let rest = trimmed.split(')').last().unwrap_or("").trim();
                    let parts: Vec<&str> = rest.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let name = parts[0].trim();
                        let ty = parts[1].trim().trim_end_matches(',');
                        let is_input = current_struct_name.contains("Input");
                        let qualifier = if is_input { "in" } else { "out" };
                        glsl.push_str(&format!(
                            "layout(location = {}) {} {} {};\n",
                            loc, qualifier, wgsl_type_to_glsl(ty), name
                        ));
                    }
                }
            }
            continue;
        }

        // @vertex fn ...
        if trimmed.starts_with("@vertex") || trimmed.starts_with("@fragment") || trimmed.starts_with("@compute") {
            in_fn = true;
            fn_body_lines.clear();
            continue;
        }

        if trimmed.starts_with("fn ") && in_fn {
            // Skip the fn signature line
            continue;
        }

        if in_fn {
            if trimmed == "}" {
                // Emit main function
                glsl.push_str("\nvoid main() {\n");
                for bl in &fn_body_lines {
                    let translated = translate_wgsl_builtins_to_glsl(bl);
                    let translated = translated.trim();
                    if !translated.is_empty()
                        && !translated.starts_with("var output")
                        && !translated.starts_with("return")
                    {
                        glsl.push_str(&format!("    {}\n", translated));
                    }
                }
                glsl.push_str("}\n");
                in_fn = false;
                continue;
            }
            fn_body_lines.push(line.to_string());
        }
    }

    Ok(glsl)
}

fn extract_wgsl_location(line: &str) -> Option<u32> {
    let start = line.find("@location(")? + "@location(".len();
    let rest = &line[start..];
    let end = rest.find(')')?;
    rest[..end].trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Shader reflection
// ---------------------------------------------------------------------------

/// Reflected information about a shader module.
#[derive(Debug, Clone, Default)]
pub struct ShaderReflection {
    pub inputs: Vec<ReflectedBinding>,
    pub outputs: Vec<ReflectedBinding>,
    pub uniforms: Vec<ReflectedBinding>,
    pub storage_buffers: Vec<ReflectedBinding>,
    pub textures: Vec<ReflectedBinding>,
    pub samplers: Vec<ReflectedBinding>,
    pub workgroup_size: Option<[u32; 3]>,
}

/// A reflected binding.
#[derive(Debug, Clone)]
pub struct ReflectedBinding {
    pub name: String,
    pub ty: String,
    pub location_or_binding: u32,
    pub group: Option<u32>,
}

/// Reflect a GLSL shader.
pub fn reflect_glsl(source: &str) -> ShaderReflection {
    let tokens = parse_glsl_tokens(source);
    let mut refl = ShaderReflection::default();

    for token in &tokens {
        match token {
            GlslToken::In { location, ty, name } => {
                refl.inputs.push(ReflectedBinding {
                    name: name.clone(),
                    ty: ty.clone(),
                    location_or_binding: *location,
                    group: None,
                });
            }
            GlslToken::Out { location, ty, name } => {
                refl.outputs.push(ReflectedBinding {
                    name: name.clone(),
                    ty: ty.clone(),
                    location_or_binding: *location,
                    group: None,
                });
            }
            GlslToken::Uniform { ty, name } => {
                refl.uniforms.push(ReflectedBinding {
                    name: name.clone(),
                    ty: ty.clone(),
                    location_or_binding: 0,
                    group: None,
                });
            }
            GlslToken::Sampler { name } => {
                refl.samplers.push(ReflectedBinding {
                    name: name.clone(),
                    ty: "sampler2D".into(),
                    location_or_binding: 0,
                    group: None,
                });
            }
            _ => {}
        }
    }

    // Check for compute workgroup size: layout(local_size_x=X, ...)
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("local_size_x") {
            let mut ws = [1u32, 1, 1];
            for dim in ["local_size_x", "local_size_y", "local_size_z"].iter().enumerate() {
                if let Some(pos) = trimmed.find(dim.1) {
                    let rest = &trimmed[pos + dim.1.len()..];
                    if let Some(eq) = rest.find('=') {
                        let after = &rest[eq + 1..];
                        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(n) = num_str.parse::<u32>() {
                            ws[dim.0] = n;
                        }
                    }
                }
            }
            refl.workgroup_size = Some(ws);
        }
    }

    refl
}

/// Reflect a WGSL shader.
pub fn reflect_wgsl(source: &str) -> ShaderReflection {
    let mut refl = ShaderReflection::default();

    for line in source.lines() {
        let trimmed = line.trim();

        // @group(G) @binding(B) var<uniform> ...
        if trimmed.starts_with("@group") && trimmed.contains("@binding") {
            let group = extract_wgsl_group(trimmed);
            let binding = extract_wgsl_binding_num(trimmed);

            if trimmed.contains("var<uniform>") {
                if let Some(rest) = trimmed.split("var<uniform>").nth(1) {
                    let rest = rest.trim().trim_end_matches(';');
                    let parts: Vec<&str> = rest.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        refl.uniforms.push(ReflectedBinding {
                            name: parts[0].trim().to_string(),
                            ty: parts[1].trim().to_string(),
                            location_or_binding: binding.unwrap_or(0),
                            group,
                        });
                    }
                }
            } else if trimmed.contains("var<storage") {
                if let Some(rest) = trimmed.split('>').last() {
                    let rest = rest.trim().trim_end_matches(';');
                    let parts: Vec<&str> = rest.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        refl.storage_buffers.push(ReflectedBinding {
                            name: parts[0].trim().to_string(),
                            ty: parts[1].trim().to_string(),
                            location_or_binding: binding.unwrap_or(0),
                            group,
                        });
                    }
                }
            } else if trimmed.contains("texture") {
                if let Some(rest) = trimmed.split("var ").nth(1) {
                    let parts: Vec<&str> = rest.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        refl.textures.push(ReflectedBinding {
                            name: parts[0].trim().to_string(),
                            ty: parts[1].trim().trim_end_matches(';').to_string(),
                            location_or_binding: binding.unwrap_or(0),
                            group,
                        });
                    }
                }
            } else if trimmed.contains("sampler") {
                if let Some(rest) = trimmed.split("var ").nth(1) {
                    let name = rest.split(':').next().unwrap_or("").trim().to_string();
                    refl.samplers.push(ReflectedBinding {
                        name,
                        ty: "sampler".into(),
                        location_or_binding: binding.unwrap_or(0),
                        group,
                    });
                }
            }
        }

        // @location(N) name: type  (inside a struct)
        if trimmed.contains("@location(") && !trimmed.starts_with("@group") {
            if let Some(loc) = extract_wgsl_location(trimmed) {
                let rest = trimmed.split(')').last().unwrap_or("").trim();
                let parts: Vec<&str> = rest.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let binding = ReflectedBinding {
                        name: parts[0].trim().to_string(),
                        ty: parts[1].trim().trim_end_matches(',').to_string(),
                        location_or_binding: loc,
                        group: None,
                    };
                    // Heuristic: if we haven't seen outputs yet, treat as input
                    refl.inputs.push(binding);
                }
            }
        }

        // @workgroup_size(X, Y, Z)
        if trimmed.contains("@workgroup_size(") {
            let start = trimmed.find("@workgroup_size(").unwrap() + "@workgroup_size(".len();
            let rest = &trimmed[start..];
            if let Some(end) = rest.find(')') {
                let nums: Vec<u32> = rest[..end]
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u32>().ok())
                    .collect();
                let mut ws = [1u32, 1, 1];
                for (i, &n) in nums.iter().enumerate().take(3) {
                    ws[i] = n;
                }
                refl.workgroup_size = Some(ws);
            }
        }
    }

    refl
}

fn extract_wgsl_group(line: &str) -> Option<u32> {
    let start = line.find("@group(")? + "@group(".len();
    let rest = &line[start..];
    let end = rest.find(')')?;
    rest[..end].trim().parse::<u32>().ok()
}

fn extract_wgsl_binding_num(line: &str) -> Option<u32> {
    let start = line.find("@binding(")? + "@binding(".len();
    let rest = &line[start..];
    let end = rest.find(')')?;
    rest[..end].trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Shader validation
// ---------------------------------------------------------------------------

/// Validate a shader source in the given language.
pub fn validate_shader(source: &str, language: ShaderLanguage) -> Vec<ShaderError> {
    let mut errors = Vec::new();

    match language {
        ShaderLanguage::GLSL => validate_glsl(source, &mut errors),
        ShaderLanguage::WGSL => validate_wgsl(source, &mut errors),
        _ => {
            // Minimal validation for other languages
            if source.trim().is_empty() {
                errors.push(ShaderError {
                    line: 1, col: 1,
                    message: "Empty shader source".into(),
                    severity: Severity::Error,
                });
            }
        }
    }

    errors
}

fn validate_glsl(source: &str, errors: &mut Vec<ShaderError>) {
    let mut has_version = false;
    let mut has_main = false;
    let mut brace_depth: i32 = 0;

    for (i, line) in source.lines().enumerate() {
        let ln = i + 1;
        let trimmed = line.trim();

        if trimmed.starts_with("#version") {
            has_version = true;
            if ln != 1 {
                errors.push(ShaderError {
                    line: ln, col: 1,
                    message: "#version must be on the first line".into(),
                    severity: Severity::Warning,
                });
            }
        }

        if trimmed.contains("void main") {
            has_main = true;
        }

        for ch in trimmed.chars() {
            if ch == '{' { brace_depth += 1; }
            if ch == '}' { brace_depth -= 1; }
        }

        if brace_depth < 0 {
            errors.push(ShaderError {
                line: ln, col: 1,
                message: "Unmatched closing brace".into(),
                severity: Severity::Error,
            });
        }
    }

    if !has_version {
        errors.push(ShaderError {
            line: 1, col: 1,
            message: "Missing #version directive".into(),
            severity: Severity::Warning,
        });
    }

    if !has_main {
        errors.push(ShaderError {
            line: 1, col: 1,
            message: "Missing void main() entry point".into(),
            severity: Severity::Error,
        });
    }

    if brace_depth != 0 {
        errors.push(ShaderError {
            line: source.lines().count(), col: 1,
            message: format!("Unbalanced braces (depth {})", brace_depth),
            severity: Severity::Error,
        });
    }
}

fn validate_wgsl(source: &str, errors: &mut Vec<ShaderError>) {
    let mut has_entry = false;
    let mut brace_depth: i32 = 0;

    for (i, line) in source.lines().enumerate() {
        let ln = i + 1;
        let trimmed = line.trim();

        if trimmed.starts_with("@vertex") || trimmed.starts_with("@fragment") || trimmed.starts_with("@compute") {
            has_entry = true;
        }

        for ch in trimmed.chars() {
            if ch == '{' { brace_depth += 1; }
            if ch == '}' { brace_depth -= 1; }
        }

        if brace_depth < 0 {
            errors.push(ShaderError {
                line: ln, col: 1,
                message: "Unmatched closing brace".into(),
                severity: Severity::Error,
            });
        }

        // Check for GLSL-isms that are wrong in WGSL
        if trimmed.starts_with("#version") {
            errors.push(ShaderError {
                line: ln, col: 1,
                message: "#version is not valid WGSL".into(),
                severity: Severity::Error,
            });
        }

        if trimmed.contains("void main") {
            errors.push(ShaderError {
                line: ln, col: 1,
                message: "WGSL does not use 'void main()'; use @vertex/@fragment fn".into(),
                severity: Severity::Error,
            });
        }
    }

    if !has_entry {
        errors.push(ShaderError {
            line: 1, col: 1,
            message: "Missing entry point (@vertex, @fragment, or @compute)".into(),
            severity: Severity::Warning,
        });
    }

    if brace_depth != 0 {
        errors.push(ShaderError {
            line: source.lines().count(), col: 1,
            message: format!("Unbalanced braces (depth {})", brace_depth),
            severity: Severity::Error,
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_VERT_GLSL: &str = r#"#version 330 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec2 aUV;
layout(location = 0) out vec2 vUV;
uniform mat4 uMVP;
void main() {
    gl_Position = uMVP * vec4(aPos, 1.0);
    vUV = aUV;
}
"#;

    #[test]
    fn parse_glsl_tokens_simple() {
        let tokens = parse_glsl_tokens(SIMPLE_VERT_GLSL);
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::Version(330))));
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::In { location: 0, .. })));
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::Out { location: 0, .. })));
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::Uniform { .. })));
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::MainBegin)));
        assert!(tokens.iter().any(|t| matches!(t, GlslToken::MainEnd)));
    }

    #[test]
    fn glsl_to_wgsl_simple() {
        let wgsl = glsl_to_wgsl(SIMPLE_VERT_GLSL).unwrap();
        assert!(wgsl.contains("struct VertexInput"));
        assert!(wgsl.contains("@location(0) aPos: vec3<f32>"));
        assert!(wgsl.contains("@location(1) aUV: vec2<f32>"));
        assert!(wgsl.contains("struct VertexOutput"));
        assert!(wgsl.contains("@group(0) @binding(0) var<uniform> uMVP: mat4x4<f32>"));
        assert!(wgsl.contains("@vertex"));
        assert!(wgsl.contains("fn vs_main"));
    }

    #[test]
    fn glsl_to_wgsl_translates_builtins() {
        let wgsl = glsl_to_wgsl(SIMPLE_VERT_GLSL).unwrap();
        assert!(wgsl.contains("output.position"));
        assert!(!wgsl.contains("gl_Position"));
    }

    #[test]
    fn wgsl_to_glsl_roundtrip() {
        let wgsl = glsl_to_wgsl(SIMPLE_VERT_GLSL).unwrap();
        let glsl_back = wgsl_to_glsl(&wgsl).unwrap();
        // The round-tripped GLSL should have the key elements
        assert!(glsl_back.contains("#version 330 core"));
        assert!(glsl_back.contains("uniform mat4x4 uMVP") || glsl_back.contains("uniform mat4 uMVP"));
        assert!(glsl_back.contains("void main()"));
    }

    #[test]
    fn glsl_to_spirv_text_has_structure() {
        let spirv = glsl_to_spirv_text(SIMPLE_VERT_GLSL).unwrap();
        assert!(spirv.contains("OpCapability Shader"));
        assert!(spirv.contains("OpMemoryModel Logical GLSL450"));
        assert!(spirv.contains("OpEntryPoint Vertex"));
        assert!(spirv.contains("OpReturn"));
        assert!(spirv.contains("OpFunctionEnd"));
    }

    #[test]
    fn type_translation_glsl_to_wgsl() {
        assert_eq!(glsl_type_to_wgsl("float"), "f32");
        assert_eq!(glsl_type_to_wgsl("vec3"), "vec3<f32>");
        assert_eq!(glsl_type_to_wgsl("mat4"), "mat4x4<f32>");
        assert_eq!(glsl_type_to_wgsl("sampler2D"), "texture_2d<f32>");
    }

    #[test]
    fn type_translation_wgsl_to_glsl() {
        assert_eq!(wgsl_type_to_glsl("f32"), "float");
        assert_eq!(wgsl_type_to_glsl("vec3<f32>"), "vec3");
        assert_eq!(wgsl_type_to_glsl("mat4x4<f32>"), "mat4");
    }

    #[test]
    fn reflect_glsl_shader() {
        let refl = reflect_glsl(SIMPLE_VERT_GLSL);
        assert_eq!(refl.inputs.len(), 2);
        assert_eq!(refl.outputs.len(), 1);
        assert_eq!(refl.uniforms.len(), 1);
        assert_eq!(refl.uniforms[0].name, "uMVP");
        assert_eq!(refl.inputs[0].name, "aPos");
        assert_eq!(refl.inputs[0].location_or_binding, 0);
    }

    #[test]
    fn reflect_glsl_compute_workgroup() {
        let src = r#"#version 430
layout(local_size_x=64, local_size_y=1, local_size_z=1) in;
void main() {}
"#;
        let refl = reflect_glsl(src);
        assert_eq!(refl.workgroup_size, Some([64, 1, 1]));
    }

    #[test]
    fn reflect_glsl_sampler() {
        let src = r#"#version 330 core
uniform sampler2D uTexture;
void main() {}
"#;
        let refl = reflect_glsl(src);
        assert_eq!(refl.samplers.len(), 1);
        assert_eq!(refl.samplers[0].name, "uTexture");
    }

    #[test]
    fn reflect_wgsl_shader() {
        let src = r#"
@group(0) @binding(0) var<uniform> uMVP: mat4x4<f32>;
@group(0) @binding(1) var myTex: texture_2d<f32>;
@group(0) @binding(2) var mySampler: sampler;
@vertex
fn vs_main() -> vec4<f32> {
    return vec4<f32>(0.0);
}
"#;
        let refl = reflect_wgsl(src);
        assert_eq!(refl.uniforms.len(), 1);
        assert_eq!(refl.textures.len(), 1);
        assert_eq!(refl.samplers.len(), 1);
    }

    #[test]
    fn reflect_wgsl_compute_workgroup() {
        let src = r#"
@compute @workgroup_size(256, 1, 1)
fn main() {}
"#;
        let refl = reflect_wgsl(src);
        assert_eq!(refl.workgroup_size, Some([256, 1, 1]));
    }

    #[test]
    fn validate_valid_glsl() {
        let errs = validate_shader(SIMPLE_VERT_GLSL, ShaderLanguage::GLSL);
        // Should have no errors (may have warnings)
        let real_errors: Vec<_> = errs.iter().filter(|e| e.severity == Severity::Error).collect();
        assert!(real_errors.is_empty(), "Unexpected errors: {:?}", real_errors);
    }

    #[test]
    fn validate_glsl_missing_main() {
        let src = "#version 330 core\nuniform float x;\n";
        let errs = validate_shader(src, ShaderLanguage::GLSL);
        assert!(errs.iter().any(|e| e.message.contains("main")));
    }

    #[test]
    fn validate_glsl_unbalanced_braces() {
        let src = "#version 330 core\nvoid main() {\n";
        let errs = validate_shader(src, ShaderLanguage::GLSL);
        assert!(errs.iter().any(|e| e.message.contains("brace")));
    }

    #[test]
    fn validate_valid_wgsl() {
        let wgsl = glsl_to_wgsl(SIMPLE_VERT_GLSL).unwrap();
        let errs = validate_shader(&wgsl, ShaderLanguage::WGSL);
        let real_errors: Vec<_> = errs.iter().filter(|e| e.severity == Severity::Error).collect();
        assert!(real_errors.is_empty(), "Unexpected errors: {:?}", real_errors);
    }

    #[test]
    fn validate_wgsl_with_glsl_isms() {
        let src = "#version 330\nvoid main() {}\n";
        let errs = validate_shader(src, ShaderLanguage::WGSL);
        assert!(errs.iter().any(|e| e.message.contains("#version")));
    }

    #[test]
    fn validate_empty_shader() {
        let errs = validate_shader("", ShaderLanguage::HLSL);
        assert!(errs.iter().any(|e| e.message.contains("Empty")));
    }

    #[test]
    fn shader_language_display() {
        assert_eq!(format!("{}", ShaderLanguage::GLSL), "GLSL");
        assert_eq!(format!("{}", ShaderLanguage::SPIRV), "SPIR-V");
    }

    #[test]
    fn translate_error_display() {
        let err = TranslateError::new("test error")
            .with_error(ShaderError {
                line: 5, col: 10,
                message: "bad token".into(),
                severity: Severity::Error,
            });
        let s = format!("{}", err);
        assert!(s.contains("test error"));
        assert!(s.contains("bad token"));
    }

    #[test]
    fn glsl_texture_builtin_translation() {
        let line = "vec4 c = texture2D(myTex, uv);";
        let translated = translate_glsl_builtins_to_wgsl(line);
        assert!(translated.contains("textureSample("));
    }

    #[test]
    fn wgsl_texture_builtin_translation() {
        let line = "let c = textureSample(myTex, mySampler, uv);";
        let translated = translate_wgsl_builtins_to_glsl(line);
        assert!(translated.contains("texture("));
    }
}
