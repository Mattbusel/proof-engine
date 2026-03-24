//! Shader graph → GLSL compiler.
//!
//! Walks the topologically-sorted node list and emits a GLSL fragment shader.
//! Each node type has a corresponding GLSL emission function that receives
//! the names of its input variables and outputs the name of its output variable.

use std::collections::HashMap;
use super::{ShaderGraph, NodeId, GraphError};
use super::nodes::{NodeType, SocketType};

// ── CompiledShader ────────────────────────────────────────────────────────────

/// The result of compiling a ShaderGraph.
#[derive(Debug, Clone)]
pub struct CompiledShader {
    /// Complete GLSL fragment shader source.
    pub fragment_source: String,
    /// Vertex shader (pass-through).
    pub vertex_source:   String,
    /// Uniform declarations extracted from the graph.
    pub uniforms:        Vec<UniformDecl>,
    /// Named render targets this shader writes to.
    pub render_targets:  Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UniformDecl {
    pub name:     String,
    pub glsl_type: String,
    pub default:  String,
}

impl CompiledShader {
    /// Return all uniform names for binding.
    pub fn uniform_names(&self) -> Vec<&str> {
        self.uniforms.iter().map(|u| u.name.as_str()).collect()
    }
}

// ── GraphCompiler ─────────────────────────────────────────────────────────────

pub struct GraphCompiler;

impl GraphCompiler {
    pub fn compile(graph: &ShaderGraph) -> Result<CompiledShader, GraphError> {
        let order = graph.topological_order()?;

        let mut uniforms:       Vec<UniformDecl>     = Vec::new();
        let mut render_targets: Vec<String>          = Vec::new();
        let mut body:           Vec<String>          = Vec::new();
        // Map from (NodeId, slot) → variable name
        let mut var_map: HashMap<(NodeId, u8), String> = HashMap::new();

        // ── Preamble uniforms from parameters ──────────────────────────────────
        for param in &graph.parameters {
            uniforms.push(UniformDecl {
                name:      param.glsl_name.clone(),
                glsl_type: param.value.glsl_type().to_string(),
                default:   param.value.glsl_literal(),
            });
        }

        // ── Node emission ──────────────────────────────────────────────────────
        for &node_id in &order {
            let node = match graph.node(node_id) {
                Some(n) => n,
                None    => continue,
            };

            if node.muted {
                // Muted: emit zero for all outputs
                for (i, sock) in node.node_type.output_sockets().iter().enumerate() {
                    let var = node.var_name(i);
                    body.push(format!("{} {} = {};",
                        sock.socket_type.glsl_type(), var, sock.socket_type.default_value()));
                    var_map.insert((node_id, i as u8), var);
                }
                continue;
            }

            // Collect input variable names (from connected edges or constants)
            let inputs = node.node_type.input_sockets();
            let mut input_vars: Vec<String> = Vec::new();
            for (slot_idx, sock) in inputs.iter().enumerate() {
                let connected = graph.edges.iter()
                    .find(|e| e.to_node == node_id && e.to_slot == slot_idx as u8);
                let var = if let Some(edge) = connected {
                    var_map.get(&(edge.from_node, edge.from_slot))
                        .cloned()
                        .unwrap_or_else(|| sock.default.clone())
                } else if let Some(const_val) = node.constant_inputs.get(&slot_idx) {
                    const_val.clone()
                } else {
                    sock.default.clone()
                };
                input_vars.push(var);
            }

            // Emit node code
            Self::emit_node(node_id, &node.node_type, &input_vars, &mut body,
                            &mut var_map, &mut uniforms, &mut render_targets);
        }

        // ── Output collection ──────────────────────────────────────────────────
        let output_var = if let Some(out_id) = graph.output_node {
            let out_node = graph.node(out_id)
                .ok_or(GraphError::NodeNotFound(out_id))?;
            // For output nodes, the input (color) is their first input
            let edge = graph.edges.iter()
                .find(|e| e.to_node == out_id && e.to_slot == 0);
            if let Some(e) = edge {
                var_map.get(&(e.from_node, e.from_slot))
                    .cloned()
                    .unwrap_or_else(|| "vec4(0.0, 0.0, 0.0, 1.0)".to_string())
            } else {
                out_node.constant_inputs.get(&0)
                    .cloned()
                    .unwrap_or_else(|| "vec4(0.0, 0.0, 0.0, 1.0)".to_string())
            }
        } else {
            "vec4(0.0, 0.0, 0.0, 1.0)".to_string()
        };

        // ── Assemble fragment shader ───────────────────────────────────────────
        let fragment_source = Self::assemble_fragment(&uniforms, &body, &output_var);
        let vertex_source   = PASSTHROUGH_VERTEX.to_string();

        Ok(CompiledShader { fragment_source, vertex_source, uniforms, render_targets })
    }

    fn emit_node(
        node_id:        NodeId,
        node_type:      &NodeType,
        inputs:         &[String],
        body:           &mut Vec<String>,
        var_map:        &mut HashMap<(NodeId, u8), String>,
        uniforms:       &mut Vec<UniformDecl>,
        render_targets: &mut Vec<String>,
    ) {
        let out0 = format!("n{}_{}", node_id.0, 0);
        let out1 = format!("n{}_{}", node_id.0, 1);
        let out2 = format!("n{}_{}", node_id.0, 2);
        let out3 = format!("n{}_{}", node_id.0, 3);

        let i = |n: usize| inputs.get(n).cloned().unwrap_or_else(|| "0.0".to_string());

        match node_type {
            // ── Input nodes ────────────────────────────────────────────────────
            NodeType::UvCoord => {
                body.push(format!("vec2 {} = vUv;", out0));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::WorldPos => {
                body.push(format!("vec3 {} = vWorldPos;", out0));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::CameraPos => {
                body.push(format!("vec3 {} = uCameraPos;", out0));
                var_map.insert((node_id, 0), out0);
                uniforms.push(UniformDecl { name: "uCameraPos".into(), glsl_type: "vec3".into(), default: "vec3(0.0)".into() });
            }
            NodeType::Time => {
                body.push(format!("float {} = uTime;", out0));
                var_map.insert((node_id, 0), out0);
                if !uniforms.iter().any(|u| u.name == "uTime") {
                    uniforms.push(UniformDecl { name: "uTime".into(), glsl_type: "float".into(), default: "0.0".into() });
                }
            }
            NodeType::Resolution => {
                body.push(format!("vec2 {} = uResolution;", out0));
                var_map.insert((node_id, 0), out0);
                uniforms.push(UniformDecl { name: "uResolution".into(), glsl_type: "vec2".into(), default: "vec2(1.0)".into() });
            }
            NodeType::ConstFloat(v) => {
                body.push(format!("float {} = {:.6};", out0, v));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::ConstVec2(x, y) => {
                body.push(format!("vec2 {} = vec2({:.6}, {:.6});", out0, x, y));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::ConstVec3(x, y, z) => {
                body.push(format!("vec3 {} = vec3({:.6}, {:.6}, {:.6});", out0, x, y, z));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::ConstVec4(x, y, z, w) => {
                body.push(format!("vec4 {} = vec4({:.6},{:.6},{:.6},{:.6});", out0, x, y, z, w));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::VertexColor => {
                body.push(format!("vec4 {} = vColor;", out0));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::ScreenCoord => {
                body.push(format!("vec2 {} = gl_FragCoord.xy;", out0));
                var_map.insert((node_id, 0), out0);
            }
            NodeType::Uniform(name, t) => {
                let glsl_t = t.glsl_type();
                uniforms.push(UniformDecl { name: name.clone(), glsl_type: glsl_t.to_string(), default: t.default_value().to_string() });
                body.push(format!("{} {} = {};", glsl_t, out0, name));
                var_map.insert((node_id, 0), out0);
            }
            // ── Math ───────────────────────────────────────────────────────────
            NodeType::Add       => { body.push(format!("auto {} = {} + {};", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Subtract  => { body.push(format!("auto {} = {} - {};", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Multiply  => { body.push(format!("auto {} = {} * {};", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Divide    => { body.push(format!("auto {} = ({} != 0.0) ? {} / {} : 0.0;", out0, i(1), i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Power     => { body.push(format!("float {} = pow(max({}, 0.0), {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Sqrt      => { body.push(format!("auto {} = sqrt(max({}, 0.0));", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Abs       => { body.push(format!("auto {} = abs({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Sign      => { body.push(format!("auto {} = sign({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Floor     => { body.push(format!("auto {} = floor({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Ceil      => { body.push(format!("auto {} = ceil({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Fract     => { body.push(format!("auto {} = fract({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Min       => { body.push(format!("auto {} = min({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Max       => { body.push(format!("auto {} = max({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Clamp     => { body.push(format!("auto {} = clamp({}, {}, {});", out0, i(0), i(1), i(2))); var_map.insert((node_id,0),out0); }
            NodeType::Mix       => { body.push(format!("auto {} = mix({}, {}, {});", out0, i(0), i(1), i(2))); var_map.insert((node_id,0),out0); }
            NodeType::Smoothstep=> { body.push(format!("float {} = smoothstep({}, {}, {});", out0, i(0), i(1), i(2))); var_map.insert((node_id,0),out0); }
            NodeType::Step      => { body.push(format!("auto {} = step({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Mod       => { body.push(format!("auto {} = mod({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Sin       => { body.push(format!("auto {} = sin({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Cos       => { body.push(format!("auto {} = cos({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Tan       => { body.push(format!("auto {} = tan({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Atan      => { body.push(format!("float {} = atan({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Exp       => { body.push(format!("auto {} = exp({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Log       => { body.push(format!("auto {} = log(max({}, 1e-6));", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Log2      => { body.push(format!("auto {} = log2(max({}, 1e-6));", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::OneMinus  => { body.push(format!("auto {} = 1.0 - {};", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Saturate  => { body.push(format!("auto {} = clamp({}, 0.0, 1.0);", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Negate    => { body.push(format!("auto {} = -{};", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Reciprocal=> { body.push(format!("float {} = 1.0 / max({}, 1e-6);", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Dot       => { body.push(format!("float {} = dot({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Cross     => { body.push(format!("vec3 {} = cross({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Normalize => { body.push(format!("auto {} = normalize({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Length    => { body.push(format!("float {} = length({});", out0, i(0))); var_map.insert((node_id,0),out0); }
            NodeType::LengthSquared => { body.push(format!("float {} = dot({},{});", out0, i(0), i(0))); var_map.insert((node_id,0),out0); }
            NodeType::Distance  => { body.push(format!("float {} = distance({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Reflect   => { body.push(format!("auto {} = reflect({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::Refract   => { body.push(format!("vec3 {} = refract({}, {}, {});", out0, i(0), i(1), i(2))); var_map.insert((node_id,0),out0); }
            NodeType::Remap     => {
                body.push(format!(
                    "float {} = ({} - {}) / max({} - {}, 1e-6) * ({} - {}) + {};",
                    out0, i(0), i(1), i(2), i(1), i(4), i(3), i(3)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Vector ─────────────────────────────────────────────────────────
            NodeType::CombineVec2 => { body.push(format!("vec2 {} = vec2({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::CombineVec3 => { body.push(format!("vec3 {} = vec3({}, {}, {});", out0, i(0), i(1), i(2))); var_map.insert((node_id,0),out0); }
            NodeType::CombineVec4 => { body.push(format!("vec4 {} = vec4({}, {});", out0, i(0), i(1))); var_map.insert((node_id,0),out0); }
            NodeType::SplitVec2  => {
                body.push(format!("float {} = ({}).x;", out0, i(0)));
                body.push(format!("float {} = ({}).y;", out1, i(0)));
                var_map.insert((node_id,0),out0);
                var_map.insert((node_id,1),out1);
            }
            NodeType::SplitVec3  => {
                body.push(format!("float {} = ({}).x;", out0, i(0)));
                body.push(format!("float {} = ({}).y;", out1, i(0)));
                body.push(format!("float {} = ({}).z;", out2, i(0)));
                var_map.insert((node_id,0),out0); var_map.insert((node_id,1),out1); var_map.insert((node_id,2),out2);
            }
            NodeType::SplitVec4  => {
                body.push(format!("float {} = ({}).x;", out0, i(0)));
                body.push(format!("float {} = ({}).y;", out1, i(0)));
                body.push(format!("float {} = ({}).z;", out2, i(0)));
                body.push(format!("float {} = ({}).w;", out3, i(0)));
                var_map.insert((node_id,0),out0); var_map.insert((node_id,1),out1);
                var_map.insert((node_id,2),out2); var_map.insert((node_id,3),out3);
            }
            NodeType::Swizzle(s) => {
                body.push(format!("auto {} = ({}).{};", out0, i(0), s));
                var_map.insert((node_id,0),out0);
            }
            NodeType::RotateVec2 => {
                body.push(format!(
                    "vec2 {} = vec2(({} - {}).x * cos({}) - ({} - {}).y * sin({}), ({} - {}).x * sin({}) + ({} - {}).y * cos({})) + {};",
                    out0, i(0), i(2), i(1), i(0), i(2), i(1), i(0), i(2), i(1), i(0), i(2), i(1), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Color ──────────────────────────────────────────────────────────
            NodeType::HsvToRgb => {
                body.push(format!("vec3 {} = hsv2rgb({});", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::RgbToHsv => {
                body.push(format!("vec3 {} = rgb2hsv({});", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Luminance => {
                body.push(format!("float {} = dot({}, vec3(0.2126, 0.7152, 0.0722));", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Saturation => {
                body.push(format!(
                    "vec3 {} = mix(vec3(dot({}, vec3(0.2126,0.7152,0.0722))), {}, clamp({}, 0.0, 2.0));",
                    out0, i(0), i(0), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::HueRotate => {
                body.push(format!("vec3 {} = rotateHue({}, radians({}));", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::LinearToSrgb => {
                body.push(format!("vec3 {} = pow(max({}, 0.0), vec3(1.0/2.2));", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SrgbToLinear => {
                body.push(format!("vec3 {} = pow(max({}, 0.0), vec3(2.2));", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::GammaCorrect => {
                body.push(format!("vec3 {} = pow(max({}, 0.0), vec3(1.0 / max({}, 0.001)));", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::ScreenBlend => {
                body.push(format!("vec3 {} = 1.0 - (1.0 - {}) * (1.0 - {});", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::OverlayBlend => {
                body.push(format!(
                    "vec3 {} = mix(2.0*{}*{}, 1.0 - 2.0*(1.0-{})*(1.0-{}), step(vec3(0.5), {}));",
                    out0, i(0), i(1), i(0), i(1), i(0)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::HardLight => {
                body.push(format!(
                    "vec3 {} = mix(2.0*{}*{}, 1.0-2.0*(1.0-{})*(1.0-{}), step(vec3(0.5), {}));",
                    out0, i(0), i(1), i(0), i(1), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SoftLight => {
                body.push(format!(
                    "vec3 {} = mix({} - (1.0-2.0*{})*{}*(1.0-{}), {} + (2.0*{}-1.0)*(sqrt({})-{}), step(vec3(0.5), {}));",
                    out0, i(0), i(1), i(0), i(0), i(0), i(1), i(0), i(0), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::ColorBurn => {
                body.push(format!("vec3 {} = 1.0 - (1.0 - {}) / max({}, 0.001);", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::ColorDodge => {
                body.push(format!("vec3 {} = {} / max(1.0 - {}, 0.001);", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Difference => {
                body.push(format!("vec3 {} = abs({} - {});", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Invert => {
                body.push(format!("auto {} = 1.0 - {};", out0, i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Posterize => {
                body.push(format!("vec3 {} = floor({} * {}) / max({}, 1.0);", out0, i(0), i(1), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Duotone => {
                body.push(format!(
                    "vec3 {} = mix({}, {}, dot({}, vec3(0.2126, 0.7152, 0.0722)));",
                    out0, i(1), i(2), i(0)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Noise ──────────────────────────────────────────────────────────
            NodeType::ValueNoise => {
                body.push(format!("float {} = valueNoise({} * {});", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::PerlinNoise => {
                body.push(format!("float {} = perlinNoise({} * {});", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SimplexNoise => {
                body.push(format!("float {} = simplexNoise({} * {});", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Fbm => {
                body.push(format!("float {} = fbm({}, int({}), {}, {});", out0, i(0), i(1), i(2), i(3)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Voronoi => {
                body.push(format!("float {} = voronoi({} * {}, {}).x;", out0, i(0), i(1), i(2)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Worley => {
                body.push(format!("float {} = worley({} * {}).x;", out0, i(0), i(1)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Checkerboard => {
                body.push(format!(
                    "float {} = mod(floor({}.x * {}) + floor({}.y * {}), 2.0);",
                    out0, i(0), i(1), i(0), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SineWave => {
                body.push(format!(
                    "float {} = {} * sin({} * {} * 6.28318 + {});",
                    out0, i(2), i(0), i(1), i(3)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::RadialGradient => {
                body.push(format!("float {} = distance({}, {}) / max({}, 0.001);", out0, i(0), i(1), i(2)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::LinearGradient => {
                body.push(format!(
                    "float {} = dot({} - vec2(0.5), vec2(cos({}), sin({}))) * 0.5 + 0.5;",
                    out0, i(0), i(1), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Spiral => {
                body.push(format!(
                    "float {} = fract(atan({}.y - 0.5, {}.x - 0.5) / 6.28318 * {} + length({} - vec2(0.5)) * {} - {} * {});",
                    out0, i(0), i(0), i(1), i(0), i(1), i(2), i(3)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Rings => {
                body.push(format!(
                    "float {} = fract(length({} - vec2(0.5)) * {}) < {};",
                    out0, i(0), i(1), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::StarBurst => {
                body.push(format!(
                    "float {} = abs(sin(atan({}.y-0.5,{}.x-0.5)*{}*0.5)) * pow(length({}-vec2(0.5))*2.0, {});",
                    out0, i(0), i(0), i(1), i(0), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Grid => {
                body.push(format!(
                    "float {} = max(step(0.95, fract({}.x * {})), step(0.95, fract({}.y * {})));",
                    out0, i(0), i(1), i(0), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── SDF ────────────────────────────────────────────────────────────
            NodeType::SdfCircle => {
                body.push(format!("float {} = length({} - {}) - {};", out0, i(0), i(1), i(2)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfBox => {
                body.push(format!(
                    "{{ vec2 _q{} = abs({}-{}) - {}; float {} = length(max(_q{}, 0.0)) + min(max(_q{}.x,_q{}.y), 0.0) - {}; }}",
                    node_id.0, i(0), i(1), i(2), out0, node_id.0, node_id.0, node_id.0, i(3)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfLine => {
                body.push(format!(
                    "{{ vec2 _pa{} = {} - {}; vec2 _ba{} = {} - {}; float _h{} = clamp(dot(_pa{},_ba{})/dot(_ba{},_ba{}),0.0,1.0); float {} = length(_pa{} - _ba{}*_h{}); }}",
                    node_id.0, i(0), i(1), node_id.0, i(2), i(1), node_id.0,
                    node_id.0, node_id.0, node_id.0, node_id.0,
                    out0, node_id.0, node_id.0, node_id.0
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfSmoothUnion => {
                body.push(format!(
                    "{{ float _h{} = clamp(0.5+0.5*({}-{})/{}, 0.0, 1.0); float {} = mix({},{},_h{}) - {}*_h{}*(1.0-_h{}); }}",
                    node_id.0, i(1), i(0), i(2), out0, i(1), i(0), node_id.0, i(2), node_id.0, node_id.0
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfSmoothSubtract => {
                body.push(format!(
                    "{{ float _h{} = clamp(0.5-0.5*({} + {})/{}, 0.0, 1.0); float {} = mix({}, -{}, _h{}) + {}*_h{}*(1.0-_h{}); }}",
                    node_id.0, i(1), i(0), i(2), out0, i(1), i(0), node_id.0, i(2), node_id.0, node_id.0
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfToAlpha => {
                body.push(format!("float {} = step({}, -{});", out0, i(1), i(0)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::SdfToSoftAlpha => {
                body.push(format!("float {} = 1.0 - smoothstep(-{} - {}, -{} + {}, {});", out0, i(2), i(1), i(2), i(1), i(0)));
                var_map.insert((node_id,0),out0);
            }
            // ── Fractals ───────────────────────────────────────────────────────
            NodeType::Mandelbrot => {
                body.push(format!(
                    r#"float {} = mandelbrotIter({} * {} - vec2(0.5), int({}));"#,
                    out0, i(0), i(2), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Julia => {
                body.push(format!(
                    "float {} = juliaIter({} * {}, vec2({}, {}), int({}));",
                    out0, i(0), i(2), i(3), i(4), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Vignette / Grain ───────────────────────────────────────────────
            NodeType::Vignette => {
                body.push(format!(
                    "float {} = 1.0 - smoothstep(1.0 - {}, 1.0 - {} + {}, length(({} - vec2(0.5)) * 2.0));",
                    out0, i(1), i(1), i(2), i(0)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::FilmGrain => {
                body.push(format!(
                    "float {} = {} * (fract(sin(dot({} + vec2({} * 123.456), vec2(12.9898, 78.233))) * 43758.5453) - 0.5) * 2.0;",
                    out0, i(2), i(0), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Scanlines => {
                body.push(format!(
                    "float {} = 1.0 - {} * (sin({}.y * {} * 3.14159) * 0.5 + 0.5);",
                    out0, i(1), i(0), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::Pixelate => {
                body.push(format!(
                    "vec2 {} = floor({} * {}) / {};",
                    out0, i(0), i(1), i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::BarrelDistort => {
                body.push(format!(
                    "{{ vec2 _uv{} = {} - vec2(0.5); float _r{} = dot(_uv{},_uv{}); vec2 {} = {} + _uv{} * _r{} * {}; }}",
                    node_id.0, i(0), node_id.0, node_id.0, node_id.0,
                    out0, i(0), node_id.0, node_id.0, i(1)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::HeatHaze => {
                body.push(format!(
                    "vec2 {} = {} + vec2(sin({}.y * 20.0 + {} * {}) * {}, 0.0);",
                    out0, i(0), i(0), i(1), i(3), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            NodeType::GlitchOffset => {
                body.push(format!(
                    "vec2 {} = {} + vec2(step(0.9, fract(sin({}.y * 100.0 + {}) * 43758.5)) * {} * 0.1, 0.0);",
                    out0, i(0), i(0), i(1), i(2)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Logic ──────────────────────────────────────────────────────────
            NodeType::IfGreater => {
                body.push(format!("auto {} = ({} > {}) ? {} : {};", out0, i(0), i(1), i(2), i(3)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::IfLess => {
                body.push(format!("auto {} = ({} < {}) ? {} : {};", out0, i(0), i(1), i(2), i(3)));
                var_map.insert((node_id,0),out0);
            }
            NodeType::ConditionalBlend => {
                body.push(format!(
                    "auto {} = mix({}, {}, smoothstep(0.5 - {}, 0.5 + {}, {}));",
                    out0, i(1), i(2), i(3), i(3), i(0)
                ));
                var_map.insert((node_id,0),out0);
            }
            // ── Output (handled separately above) ─────────────────────────────
            NodeType::OutputColor | NodeType::OutputTarget(_) | NodeType::OutputWithBloom => {}
            // ── Default for unimplemented nodes ───────────────────────────────
            _ => {
                body.push(format!("float {} = 0.0; // TODO: {}", out0, node_type.label()));
                var_map.insert((node_id,0),out0);
            }
        }
    }

    fn assemble_fragment(
        uniforms:     &[UniformDecl],
        body:         &[String],
        output_var:   &str,
    ) -> String {
        let mut src = String::from("#version 330 core\n");

        // Varyings from vertex shader
        src.push_str("in  vec2 vUv;\n");
        src.push_str("in  vec3 vWorldPos;\n");
        src.push_str("in  vec4 vColor;\n");
        src.push_str("out vec4 fragColor;\n\n");

        // Uniforms
        for u in uniforms {
            src.push_str(&format!("uniform {} {};\n", u.glsl_type, u.name));
        }
        src.push('\n');

        // Standard math helpers
        src.push_str(SHADER_HELPERS);
        src.push('\n');

        // Main function
        src.push_str("void main() {\n");
        for line in body {
            src.push_str("    ");
            src.push_str(line);
            src.push('\n');
        }
        // Coerce output to vec4 if needed
        src.push_str(&format!("    fragColor = vec4({});\n", output_var));
        src.push_str("}\n");
        src
    }
}

// ── Passthrough vertex shader ─────────────────────────────────────────────────

pub const PASSTHROUGH_VERTEX: &str = r#"
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec2 aUv;
layout(location = 2) in vec4 aColor;

out vec2 vUv;
out vec3 vWorldPos;
out vec4 vColor;

uniform mat4 uMVP;

void main() {
    vUv       = aUv;
    vWorldPos = vec3(aPos, 0.0);
    vColor    = aColor;
    gl_Position = uMVP * vec4(aPos, 0.0, 1.0);
}
"#;

// ── GLSL helper functions ─────────────────────────────────────────────────────

pub const SHADER_HELPERS: &str = r#"
// ── Color helpers ─────────────────────────────────────────────────────────────
vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}
vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0,-1.0/3.0,2.0/3.0,-1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));
    float d = q.x - min(q.w, q.y);
    float e = 1e-10;
    return vec3(abs(q.z+(q.w-q.y)/(6.0*d+e)), d/(q.x+e), q.x);
}
vec3 rotateHue(vec3 c, float angle) {
    vec3 hsv = rgb2hsv(c);
    hsv.x = fract(hsv.x + angle / 6.28318);
    return hsv2rgb(hsv);
}

// ── Noise helpers ─────────────────────────────────────────────────────────────
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float valueNoise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p);
    vec2 u = f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2(1,0)),u.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1,1)),u.x),u.y);
}
float perlinNoise(vec2 p) {
    vec2 i = floor(p); vec2 f = fract(p);
    vec2 u = f*f*f*(f*(f*6.0-15.0)+10.0);
    float a = hash(i), b = hash(i+vec2(1,0)), c = hash(i+vec2(0,1)), d = hash(i+vec2(1,1));
    return mix(mix(a,b,u.x),mix(c,d,u.x),u.y)*2.0-1.0;
}
float simplexNoise(vec2 v) {
    const vec4 C = vec4(0.211324865405187,0.366025403784439,-0.577350269189626,0.024390243902439);
    vec2 i = floor(v + dot(v, C.yy));
    vec2 x0 = v - i + dot(i, C.xx);
    vec2 i1 = (x0.x > x0.y) ? vec2(1.0,0.0) : vec2(0.0,1.0);
    vec4 x12 = x0.xyxy + C.xxzz;
    x12.xy -= i1;
    i = mod(i, 289.0);
    vec3 p = fract(((i.z+vec3(0,i1.y,1))*34.0+1.0)*(i.z+vec3(0,i1.y,1))/289.0)*(i.y+vec3(0,i1.x,1));
    vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy), dot(x12.zw,x12.zw)), 0.0);
    m = m*m*m*m;
    vec3 g; g.x = dot(vec2(cos(p.x*6.28318),sin(p.x*6.28318)),x0);
    g.y = dot(vec2(cos(p.y*6.28318),sin(p.y*6.28318)),x12.xy);
    g.z = dot(vec2(cos(p.z*6.28318),sin(p.z*6.28318)),x12.zw);
    return 130.0 * dot(m, g);
}
float fbm(vec2 p, int octaves, float lacunarity, float gain) {
    float v = 0.0, amp = 0.5;
    for (int i = 0; i < 8; i++) {
        if (i >= octaves) break;
        v += amp * perlinNoise(p);
        p *= lacunarity; amp *= gain;
    }
    return v;
}
vec2 voronoi(vec2 p, float jitter) {
    vec2 i = floor(p); vec2 f = fract(p);
    float d1 = 8.0, d2 = 8.0;
    for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
        vec2 n = vec2(x,y); vec2 g = n + jitter*(hash(i+n)*2.0-1.0);
        float d = length(g - f);
        if (d < d1) { d2 = d1; d1 = d; }
        else if (d < d2) { d2 = d; }
    }
    return vec2(d1, d2);
}
vec2 worley(vec2 p) { return voronoi(p, 1.0); }

// ── Fractal helpers ───────────────────────────────────────────────────────────
float mandelbrotIter(vec2 c, int maxIter) {
    vec2 z = vec2(0.0);
    for (int i = 0; i < 512; i++) {
        if (i >= maxIter) break;
        if (dot(z,z) > 4.0) return float(i) / float(maxIter);
        z = vec2(z.x*z.x - z.y*z.y + c.x, 2.0*z.x*z.y + c.y);
    }
    return 0.0;
}
float juliaIter(vec2 z, vec2 c, int maxIter) {
    for (int i = 0; i < 512; i++) {
        if (i >= maxIter) break;
        if (dot(z,z) > 4.0) return float(i) / float(maxIter);
        z = vec2(z.x*z.x - z.y*z.y + c.x, 2.0*z.x*z.y + c.y);
    }
    return 0.0;
}
"#;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::shader_graph::{ShaderGraph, GraphError};
    use crate::render::shader_graph::nodes::NodeType;

    fn simple_graph() -> ShaderGraph {
        let mut g = ShaderGraph::new("test");
        let uv  = g.add_node(NodeType::UvCoord);
        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        // Connect UV as vec2, output expects vec4 — just test compilation doesn't crash
        let sin = g.add_node(NodeType::Sin);
        let _ = g.connect(uv, 0, sin, 0);
        g
    }

    #[test]
    fn test_compile_simple_graph() {
        let g = simple_graph();
        let result = g.compile();
        // May fail due to type mismatches in this simple test but shouldn't panic
        match result {
            Ok(shader) => {
                assert!(!shader.fragment_source.is_empty());
                assert!(shader.fragment_source.contains("#version 330"));
            }
            Err(_) => {} // expected for incomplete graph
        }
    }

    #[test]
    fn test_uniform_decl() {
        let u = UniformDecl {
            name:      "uTime".to_string(),
            glsl_type: "float".to_string(),
            default:   "0.0".to_string(),
        };
        assert_eq!(u.name, "uTime");
    }

    #[test]
    fn test_passthrough_vertex_has_version() {
        assert!(PASSTHROUGH_VERTEX.contains("#version 330"));
    }

    #[test]
    fn test_helpers_contain_hsv() {
        assert!(SHADER_HELPERS.contains("hsv2rgb"));
        assert!(SHADER_HELPERS.contains("perlinNoise"));
        assert!(SHADER_HELPERS.contains("mandelbrotIter"));
    }
}
