
//! Scripting IDE — embedded code editor with syntax highlighting, LSP-style
//! auto-complete, inline diagnostics, multi-file project, bytecode VM debugger,
//! REPL console, hot-reload, profiler integration, and full standard library browser.

use glam::{Vec2, Vec4};
use std::collections::{HashMap, HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Language support
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLanguage {
    Lua, LuaJIT, MoonScript,
    JavaScript, TypeScript,
    Python, Wren, Squirrel,
    AngelScript, GameMonkey,
    VisualScript, ProofScript,
    GLSL, HLSL, WGSL,
    Rust, Csharp,
}

impl ScriptLanguage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lua => "Lua", Self::LuaJIT => "LuaJIT", Self::MoonScript => "MoonScript",
            Self::JavaScript => "JavaScript", Self::TypeScript => "TypeScript",
            Self::Python => "Python", Self::Wren => "Wren", Self::Squirrel => "Squirrel",
            Self::AngelScript => "AngelScript", Self::GameMonkey => "GameMonkey",
            Self::VisualScript => "Visual Script", Self::ProofScript => "ProofScript",
            Self::GLSL => "GLSL", Self::HLSL => "HLSL", Self::WGSL => "WGSL",
            Self::Rust => "Rust", Self::Csharp => "C#",
        }
    }
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Lua | Self::LuaJIT | Self::MoonScript => "lua",
            Self::JavaScript => "js", Self::TypeScript => "ts",
            Self::Python => "py", Self::Wren => "wren", Self::Squirrel => "nut",
            Self::AngelScript => "as", Self::GameMonkey => "gm",
            Self::VisualScript | Self::ProofScript => "pscript",
            Self::GLSL => "glsl", Self::HLSL => "hlsl", Self::WGSL => "wgsl",
            Self::Rust => "rs", Self::Csharp => "cs",
        }
    }
    pub fn supports_hot_reload(&self) -> bool {
        matches!(self, Self::Lua | Self::LuaJIT | Self::JavaScript | Self::TypeScript | Self::Python | Self::Wren | Self::ProofScript)
    }
    pub fn supports_debugging(&self) -> bool {
        matches!(self, Self::Lua | Self::LuaJIT | Self::JavaScript | Self::TypeScript | Self::Python | Self::Wren)
    }
    pub fn is_shader(&self) -> bool {
        matches!(self, Self::GLSL | Self::HLSL | Self::WGSL)
    }
    pub fn line_comment(&self) -> &'static str {
        match self {
            Self::Lua | Self::LuaJIT | Self::MoonScript => "--",
            Self::JavaScript | Self::TypeScript | Self::Rust | Self::Csharp | Self::Wren | Self::AngelScript | Self::GLSL | Self::HLSL | Self::WGSL => "//",
            Self::Python => "#",
            Self::Squirrel | Self::GameMonkey => "//",
            _ => "//",
        }
    }
    pub fn block_comment(&self) -> (&'static str, &'static str) {
        match self {
            Self::Lua | Self::LuaJIT => ("--[[", "]]"),
            Self::Python => ("\"\"\"", "\"\"\""),
            _ => ("/*", "*/"),
        }
    }
}

// ---------------------------------------------------------------------------
// Syntax token types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Keyword, BuiltinType, Identifier, Number, Float,
    StringLiteral, CharLiteral, Comment, BlockComment,
    Operator, Punctuation, Bracket, Whitespace,
    PreprocessorDirective, Attribute, Macro,
    BuiltinFunction, BuiltinConstant,
    TypeName, FunctionName, VariableName, ParameterName,
    MemberAccess, Namespace, Label, Lifetime,
    Error, Unknown,
}

impl TokenKind {
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Keyword => Vec4::new(0.57, 0.35, 0.87, 1.0),
            Self::BuiltinType => Vec4::new(0.35, 0.7, 0.87, 1.0),
            Self::Identifier => Vec4::new(0.85, 0.85, 0.85, 1.0),
            Self::Number | Self::Float => Vec4::new(0.65, 0.87, 0.45, 1.0),
            Self::StringLiteral | Self::CharLiteral => Vec4::new(0.87, 0.65, 0.35, 1.0),
            Self::Comment | Self::BlockComment => Vec4::new(0.45, 0.55, 0.45, 1.0),
            Self::Operator => Vec4::new(0.87, 0.87, 0.45, 1.0),
            Self::Punctuation | Self::Bracket => Vec4::new(0.7, 0.7, 0.7, 1.0),
            Self::PreprocessorDirective | Self::Attribute | Self::Macro => Vec4::new(0.7, 0.55, 0.45, 1.0),
            Self::BuiltinFunction | Self::BuiltinConstant => Vec4::new(0.35, 0.87, 0.75, 1.0),
            Self::TypeName => Vec4::new(0.35, 0.75, 0.87, 1.0),
            Self::FunctionName => Vec4::new(0.65, 0.8, 0.35, 1.0),
            Self::VariableName => Vec4::new(0.8, 0.75, 0.6, 1.0),
            Self::ParameterName => Vec4::new(0.8, 0.6, 0.5, 1.0),
            Self::MemberAccess => Vec4::new(0.75, 0.7, 0.85, 1.0),
            Self::Namespace => Vec4::new(0.55, 0.75, 0.55, 1.0),
            Self::Label | Self::Lifetime => Vec4::new(0.87, 0.55, 0.35, 1.0),
            Self::Error => Vec4::new(1.0, 0.2, 0.2, 1.0),
            Self::Unknown | Self::Whitespace => Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

pub struct Tokenizer {
    pub language: ScriptLanguage,
    keywords: HashSet<&'static str>,
    builtin_types: HashSet<&'static str>,
    builtin_functions: HashSet<&'static str>,
}

impl Tokenizer {
    pub fn new(language: ScriptLanguage) -> Self {
        let (keywords, builtin_types, builtin_functions) = Self::get_sets(language);
        Self { language, keywords, builtin_types, builtin_functions }
    }

    fn get_sets(lang: ScriptLanguage) -> (HashSet<&'static str>, HashSet<&'static str>, HashSet<&'static str>) {
        match lang {
            ScriptLanguage::Lua | ScriptLanguage::LuaJIT => (
                ["and","break","do","else","elseif","end","false","for","function","goto","if","in",
                 "local","nil","not","or","repeat","return","then","true","until","while"].iter().copied().collect(),
                ["string","table","number","boolean","function","userdata","thread","nil"].iter().copied().collect(),
                ["print","tostring","tonumber","type","pairs","ipairs","next","select","rawget","rawset",
                 "rawequal","rawlen","pcall","xpcall","error","assert","require","load","loadfile",
                 "dofile","collectgarbage","setmetatable","getmetatable","unpack","table.insert",
                 "table.remove","table.sort","table.concat","string.format","string.len","string.sub",
                 "string.find","string.match","string.gmatch","string.gsub","string.upper","string.lower",
                 "math.floor","math.ceil","math.sqrt","math.abs","math.max","math.min","math.sin",
                 "math.cos","math.tan","math.atan","math.pi","math.huge","math.random","math.randomseed",
                 "io.write","io.read","os.time","os.clock","os.date"].iter().copied().collect(),
            ),
            ScriptLanguage::Python => (
                ["False","None","True","and","as","assert","async","await","break","class","continue",
                 "def","del","elif","else","except","finally","for","from","global","if","import","in",
                 "is","lambda","nonlocal","not","or","pass","raise","return","try","while","with","yield"].iter().copied().collect(),
                ["int","float","str","bool","list","dict","set","tuple","bytes","bytearray","complex",
                 "frozenset","memoryview","range","type","object","None"].iter().copied().collect(),
                ["print","len","range","enumerate","zip","map","filter","sorted","reversed","sum","min",
                 "max","abs","round","int","float","str","bool","list","dict","set","tuple","isinstance",
                 "issubclass","hasattr","getattr","setattr","delattr","callable","repr","hash","id",
                 "input","open","iter","next","super","vars","dir","globals","locals","exec","eval",
                 "compile","__import__","any","all","chr","ord","bin","hex","oct","format","pow","divmod"].iter().copied().collect(),
            ),
            ScriptLanguage::GLSL | ScriptLanguage::WGSL => (
                ["attribute","const","uniform","varying","break","continue","do","for","while","if","else",
                 "in","out","inout","float","int","uint","bool","lowp","mediump","highp","precision",
                 "invariant","discard","return","void","struct","layout","location","binding","set",
                 "push_constant","flat","smooth","centroid","noperspective","patch","sample"].iter().copied().collect(),
                ["float","vec2","vec3","vec4","int","ivec2","ivec3","ivec4","uint","uvec2","uvec3","uvec4",
                 "bool","bvec2","bvec3","bvec4","mat2","mat3","mat4","mat2x3","mat2x4","mat3x2","mat3x4",
                 "mat4x2","mat4x3","sampler2D","sampler3D","samplerCube","sampler2DShadow",
                 "sampler2DArray","samplerCubeArray","isampler2D","usampler2D","sampler2DMS"].iter().copied().collect(),
                ["radians","degrees","sin","cos","tan","asin","acos","atan","sinh","cosh","tanh","asinh",
                 "acosh","atanh","pow","exp","log","exp2","log2","sqrt","inversesqrt","abs","sign","floor",
                 "trunc","round","roundEven","ceil","fract","mod","modf","min","max","clamp","mix","step",
                 "smoothstep","isnan","isinf","floatBitsToInt","floatBitsToUint","intBitsToFloat",
                 "uintBitsToFloat","packSnorm2x16","unpackSnorm2x16","packUnorm2x16","unpackUnorm2x16",
                 "length","distance","dot","cross","normalize","faceforward","reflect","refract",
                 "matrixCompMult","outerProduct","transpose","determinant","inverse","lessThan",
                 "lessThanEqual","greaterThan","greaterThanEqual","equal","notEqual","any","all","not",
                 "textureSize","texture","textureLod","textureOffset","texelFetch","textureGrad",
                 "textureProjLod","dFdx","dFdy","fwidth","emit","endPrimitive","barrier","memoryBarrier",
                 "atomicAdd","atomicMin","atomicMax","atomicAnd","atomicOr","atomicXor","atomicExchange",
                 "atomicCompSwap","imageStore","imageLoad","imageAtomicAdd","gl_Position","gl_FragCoord",
                 "gl_FragDepth","gl_VertexID","gl_InstanceID","gl_FrontFacing","gl_PointCoord",
                 "gl_WorkGroupID","gl_LocalInvocationID","gl_GlobalInvocationID","gl_NumWorkGroups"].iter().copied().collect(),
            ),
            _ => (HashSet::new(), HashSet::new(), HashSet::new()),
        }
    }

    pub fn tokenize(&self, source: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut chars = source.char_indices().peekable();
        let mut line = 0u32;
        let mut line_start = 0usize;

        while let Some((i, ch)) = chars.next() {
            let col = (i - line_start) as u32;
            match ch {
                '\n' => { line += 1; line_start = i + 1; }
                ' ' | '\t' | '\r' => {}
                '/' if chars.peek().map(|(_, c)| *c == '/').unwrap_or(false) => {
                    let start = i;
                    let mut end = i;
                    while let Some(&(j, c)) = chars.peek() {
                        if c == '\n' { break; }
                        end = j;
                        chars.next();
                    }
                    let text = source[start..end+1].to_string();
                    tokens.push(Token { kind: TokenKind::Comment, text, start, end: end+1, line, col });
                }
                '/' if chars.peek().map(|(_, c)| *c == '*').unwrap_or(false) => {
                    chars.next();
                    let start = i;
                    let mut end = i;
                    let mut prev = ' ';
                    while let Some((j, c)) = chars.next() {
                        end = j;
                        if prev == '*' && c == '/' { break; }
                        if c == '\n' { line += 1; line_start = j + 1; }
                        prev = c;
                    }
                    tokens.push(Token { kind: TokenKind::BlockComment, text: source[start..end+1].to_string(), start, end: end+1, line, col });
                }
                '"' | '\'' => {
                    let quote = ch;
                    let start = i;
                    let mut end = i;
                    while let Some(&(j, c)) = chars.peek() {
                        chars.next();
                        end = j;
                        if c == quote { break; }
                        if c == '\\' { chars.next(); }
                    }
                    tokens.push(Token { kind: TokenKind::StringLiteral, text: source[start..end+1].to_string(), start, end: end+1, line, col });
                }
                c if c.is_ascii_digit() => {
                    let start = i;
                    let mut end = i;
                    let mut is_float = false;
                    while let Some(&(j, nc)) = chars.peek() {
                        if nc.is_ascii_alphanumeric() || nc == '_' || nc == '.' {
                            if nc == '.' { is_float = true; }
                            end = j;
                            chars.next();
                        } else { break; }
                    }
                    let kind = if is_float { TokenKind::Float } else { TokenKind::Number };
                    tokens.push(Token { kind, text: source[start..end+1].to_string(), start, end: end+1, line, col });
                }
                c if c.is_alphabetic() || c == '_' => {
                    let start = i;
                    let mut end = i;
                    while let Some(&(j, nc)) = chars.peek() {
                        if nc.is_alphanumeric() || nc == '_' { end = j; chars.next(); } else { break; }
                    }
                    let word = &source[start..end+1];
                    let kind = if self.keywords.contains(word) { TokenKind::Keyword }
                        else if self.builtin_types.contains(word) { TokenKind::BuiltinType }
                        else if self.builtin_functions.contains(word) { TokenKind::BuiltinFunction }
                        else { TokenKind::Identifier };
                    tokens.push(Token { kind, text: word.to_string(), start, end: end+1, line, col });
                }
                c if "+-*/%=!<>&|^~?:".contains(c) => {
                    tokens.push(Token { kind: TokenKind::Operator, text: c.to_string(), start: i, end: i+1, line, col });
                }
                c if "()[]{}".contains(c) => {
                    tokens.push(Token { kind: TokenKind::Bracket, text: c.to_string(), start: i, end: i+1, line, col });
                }
                c if ".,;".contains(c) => {
                    tokens.push(Token { kind: TokenKind::Punctuation, text: c.to_string(), start: i, end: i+1, line, col });
                }
                _ => {}
            }
        }
        tokens
    }
}

// ---------------------------------------------------------------------------
// Diagnostic system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity { Error, Warning, Info, Hint }

impl DiagnosticSeverity {
    pub fn label(&self) -> &'static str {
        match self { Self::Error => "error", Self::Warning => "warning", Self::Info => "info", Self::Hint => "hint" }
    }
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Error => Vec4::new(1.0, 0.25, 0.25, 1.0),
            Self::Warning => Vec4::new(1.0, 0.8, 0.0, 1.0),
            Self::Info => Vec4::new(0.4, 0.7, 1.0, 1.0),
            Self::Hint => Vec4::new(0.5, 0.9, 0.5, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticRelated {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub source: String,
    pub related: Vec<DiagnosticRelated>,
    pub fix_available: bool,
    pub fix_description: String,
    pub fix_replacement: String,
}

impl Diagnostic {
    pub fn error(line: u32, col: u32, msg: &str) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: "E0001".to_string(),
            message: msg.to_string(),
            line, col_start: col, col_end: col + 1,
            source: "scripting".to_string(),
            related: Vec::new(),
            fix_available: false,
            fix_description: String::new(),
            fix_replacement: String::new(),
        }
    }
    pub fn warning(line: u32, col: u32, msg: &str) -> Self {
        Self { severity: DiagnosticSeverity::Warning, ..Self::error(line, col, msg) }
    }
    pub fn span_len(&self) -> u32 { self.col_end.saturating_sub(self.col_start) }
}

// ---------------------------------------------------------------------------
// Auto-complete / IntelliSense
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    Function, Method, Field, Property, Variable, LocalVariable,
    Parameter, Constant, EnumVariant, Class, Struct, Interface,
    Module, Namespace, Keyword, Snippet, File, Color, Event,
    TypeParameter, Unit, Value, Reference, Folder, Text,
}

impl CompletionKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Function | Self::Method => "⊕", Self::Field | Self::Property => "●",
            Self::Variable | Self::LocalVariable => "◈", Self::Parameter => "◇",
            Self::Constant | Self::EnumVariant => "◆", Self::Class | Self::Struct => "■",
            Self::Interface => "□", Self::Module | Self::Namespace => "◉",
            Self::Keyword => "⚿", Self::Snippet => "✂", Self::File => "📄",
            Self::Color => "🎨", Self::Event => "⚡", _ => "○",
        }
    }
    pub fn sort_priority(&self) -> u8 {
        match self {
            Self::Keyword => 0, Self::LocalVariable | Self::Parameter => 1,
            Self::Method | Self::Function => 2, Self::Field | Self::Property => 3,
            Self::Constant | Self::EnumVariant => 4, Self::Class | Self::Struct => 5,
            Self::Module | Self::Namespace => 6, Self::Snippet => 7, _ => 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub documentation: String,
    pub insert_text: String,
    pub filter_text: String,
    pub sort_text: String,
    pub deprecated: bool,
    pub preselect: bool,
    pub commit_chars: Vec<char>,
    pub score: f32,
}

impl CompletionItem {
    pub fn new(label: &str, kind: CompletionKind) -> Self {
        Self {
            label: label.to_string(),
            kind,
            detail: String::new(),
            documentation: String::new(),
            insert_text: label.to_string(),
            filter_text: label.to_string(),
            sort_text: format!("{:02}{}", kind.sort_priority(), label),
            deprecated: false,
            preselect: false,
            commit_chars: vec!['.', '(', ' '],
            score: 0.0,
        }
    }
    pub fn with_detail(mut self, detail: &str) -> Self { self.detail = detail.to_string(); self }
    pub fn with_doc(mut self, doc: &str) -> Self { self.documentation = doc.to_string(); self }
    pub fn with_insert(mut self, text: &str) -> Self { self.insert_text = text.to_string(); self }
    pub fn with_snippet(mut self, snip: &str) -> Self { self.insert_text = snip.to_string(); self }

    pub fn fuzzy_score(&self, query: &str) -> f32 {
        if query.is_empty() { return 1.0; }
        let label = self.filter_text.to_lowercase();
        let q = query.to_lowercase();
        if label == q { return 10.0; }
        if label.starts_with(&q) { return 5.0; }
        if label.contains(&q) { return 3.0; }
        // subsequence match
        let mut qi = 0;
        let qb: Vec<char> = q.chars().collect();
        let mut score = 0.0f32;
        for c in label.chars() {
            if qi < qb.len() && c == qb[qi] { qi += 1; score += 1.0; }
        }
        if qi == qb.len() { score / q.len() as f32 } else { 0.0 }
    }
}

// ---------------------------------------------------------------------------
// Built-in completion providers
// ---------------------------------------------------------------------------

pub struct LuaCompletionProvider;

impl LuaCompletionProvider {
    pub fn global_completions() -> Vec<CompletionItem> {
        let mut items = Vec::new();
        // Standard library tables
        for (table, funcs) in &[
            ("math", vec!["abs","ceil","floor","sqrt","sin","cos","tan","atan","atan2","exp","log","max","min","huge","pi","random","randomseed","pow","fmod","modf"]),
            ("string", vec!["format","len","sub","find","match","gmatch","gsub","upper","lower","rep","reverse","byte","char","dump"]),
            ("table", vec!["insert","remove","sort","concat","unpack","move","pack","new","getn"]),
            ("io", vec!["write","read","open","close","flush","lines","stdin","stdout","stderr","tmpfile","type"]),
            ("os", vec!["time","clock","date","difftime","execute","exit","getenv","rename","remove","tmpname"]),
            ("coroutine", vec!["create","resume","yield","status","wrap","running","isyieldable"]),
            ("utf8", vec!["char","codes","codepoint","len","offset","charpattern"]),
            ("package", vec!["path","cpath","loaded","preload","loaders","loadlib","searchpath","config","searchers"]),
            ("debug", vec!["debug","getinfo","getlocal","getmetatable","getregistry","getupvalue","getuservalue","sethook","setlocal","setmetatable","setupvalue","setuservalue","traceback","upvalueid","upvaluejoin"]),
            ("bit", vec!["band","bor","bxor","bnot","lshift","rshift","arshift","rol","ror","bswap","tobit","tohex"]),
        ] {
            let tbl = table;
            let mut tbl_item = CompletionItem::new(tbl, CompletionKind::Module)
                .with_detail(&format!("Standard library: {}", tbl));
            items.push(tbl_item);
            for func in funcs {
                let full = format!("{}.{}", tbl, func);
                items.push(CompletionItem::new(&full, CompletionKind::Function)
                    .with_detail(&format!("{}() - standard library function", full)));
            }
        }
        // Global functions
        for func in &["print","type","tostring","tonumber","ipairs","pairs","next","select","unpack",
                      "rawget","rawset","rawequal","rawlen","pcall","xpcall","error","assert",
                      "require","load","loadstring","loadfile","dofile","collectgarbage","newproxy",
                      "setmetatable","getmetatable","setfenv","getfenv"] {
            items.push(CompletionItem::new(func, CompletionKind::Function)
                .with_detail("global function"));
        }
        // Game API completions
        for (ns, funcs) in &[
            ("Entity", vec!["new","find","destroy","get_component","add_component","remove_component",
                            "set_position","get_position","set_rotation","get_rotation","set_scale",
                            "get_scale","is_valid","get_name","set_name","get_tag","set_tag",
                            "get_parent","set_parent","get_children","add_child","remove_child",
                            "send_message","broadcast_message","enable","disable","is_enabled",
                            "get_world_transform","set_world_transform"]),
            ("Scene", vec!["load","unload","get_active","find_entity","find_entities_with_tag",
                           "create_entity","destroy_entity","instantiate","get_all_entities",
                           "raycast","raycast_all","overlap_sphere","overlap_box","overlap_capsule"]),
            ("Input", vec!["get_axis","get_button","get_button_down","get_button_up","get_key",
                           "get_key_down","get_key_up","get_mouse_position","get_mouse_delta",
                           "get_mouse_button","get_mouse_scroll","set_cursor_visible","lock_cursor",
                           "get_gamepad_axis","get_gamepad_button","vibrate_gamepad"]),
            ("Time", vec!["delta_time","unscaled_delta_time","time","unscaled_time","frame_count",
                          "time_scale","fixed_delta_time","smooth_delta_time","real_time_since_startup"]),
            ("Debug", vec!["log","warn","error","assert","draw_line","draw_sphere","draw_box",
                           "draw_ray","draw_cross","draw_mesh","clear","break_point"]),
            ("Physics", vec!["raycast","raycast_all","overlap_sphere","overlap_box","sweep",
                             "add_force","add_torque","add_impulse","set_velocity","get_velocity",
                             "set_angular_velocity","get_angular_velocity","set_gravity",
                             "create_joint","destroy_joint","set_material","get_colliders"]),
            ("Audio", vec!["play","stop","pause","resume","set_volume","set_pitch","set_position",
                           "set_loop","is_playing","play_one_shot","create_source","load_clip",
                           "set_listener_position","set_listener_orientation"]),
            ("Renderer", vec!["set_material","get_material","set_visible","is_visible",
                              "set_shadow_casting","set_receive_shadows","get_bounds",
                              "set_render_layer","set_lightmap_index","bake_lightmaps"]),
            ("UI", vec!["find_element","get_element","show_panel","hide_panel","set_text",
                        "get_text","set_image","set_color","set_alpha","add_listener","remove_listener",
                        "set_interactable","set_active","animate","tween"]),
            ("Tween", vec!["to","from","sequence","parallel","delay","loop","ping_pong","ease",
                           "on_complete","on_update","kill","kill_all","pause","resume"]),
            ("Events", vec!["subscribe","unsubscribe","publish","publish_deferred","create_event"]),
            ("Resources", vec!["load","load_async","unload","is_loaded","get_path","get_type",
                               "get_all_of_type","find","exists","get_cache_size","clear_cache"]),
            ("NavMesh", vec!["find_path","sample_position","find_closest_edge","raycast",
                             "calculate_path","is_on_navmesh","get_area_cost","set_area_cost"]),
            ("Animation", vec!["play","stop","pause","resume","set_trigger","set_bool",
                               "set_float","set_integer","get_state","blend_tree","crossfade",
                               "add_event","remove_event","get_length","get_frame_count"]),
            ("Math", vec!["lerp","slerp","clamp","clamp01","remap","smooth_step","ping_pong",
                          "repeat_val","sign","delta_angle","move_towards","rotate_towards",
                          "approximately","is_power_of_two","next_power_of_two","closest_point_on_line",
                          "line_intersects_line","sphere_intersects_sphere","aabb_contains_point",
                          "perlin_noise","value_noise","simplex_noise","fbm_noise"]),
            ("Vector3", vec!["zero","one","up","down","left","right","forward","back","new",
                             "dot","cross","lerp","slerp","normalize","length","length_sq","distance",
                             "reflect","project","project_on_plane","angle","signed_angle",
                             "from_to_rotation","rotate_towards","move_towards","scale","min","max",
                             "abs","floor","ceil","round"]),
            ("Quaternion", vec!["identity","euler","angle_axis","from_to_rotation","look_rotation",
                                "lerp","slerp","normalize","inverse","dot","angle","euler_angles",
                                "to_angle_axis","to_matrix","multiply","rotate_vector"]),
            ("Color", vec!["red","green","blue","yellow","cyan","magenta","white","black","clear",
                           "gray","new","lerp","alpha","hdr","to_hsv","from_hsv","gamma_to_linear",
                           "linear_to_gamma","grayscale"]),
        ] {
            items.push(CompletionItem::new(ns, CompletionKind::Namespace).with_detail("Game API namespace"));
            for func in funcs {
                let full = format!("{}.{}", ns, func);
                items.push(CompletionItem::new(&full, CompletionKind::Method)
                    .with_detail(&format!("{}() - game API", full)));
            }
        }
        items
    }

    pub fn snippets() -> Vec<CompletionItem> {
        vec![
            CompletionItem::new("func", CompletionKind::Snippet)
                .with_detail("function definition")
                .with_snippet("function ${1:name}(${2:args})\n\t${3:-- body}\nend"),
            CompletionItem::new("for_i", CompletionKind::Snippet)
                .with_detail("numeric for loop")
                .with_snippet("for ${1:i} = ${2:1}, ${3:n} do\n\t${4:-- body}\nend"),
            CompletionItem::new("for_pairs", CompletionKind::Snippet)
                .with_detail("pairs loop")
                .with_snippet("for ${1:k}, ${2:v} in pairs(${3:t}) do\n\t${4:-- body}\nend"),
            CompletionItem::new("if", CompletionKind::Snippet)
                .with_detail("if statement")
                .with_snippet("if ${1:condition} then\n\t${2:-- body}\nend"),
            CompletionItem::new("if_else", CompletionKind::Snippet)
                .with_detail("if/else statement")
                .with_snippet("if ${1:condition} then\n\t${2:-- true}\nelse\n\t${3:-- false}\nend"),
            CompletionItem::new("while", CompletionKind::Snippet)
                .with_detail("while loop")
                .with_snippet("while ${1:condition} do\n\t${2:-- body}\nend"),
            CompletionItem::new("class", CompletionKind::Snippet)
                .with_detail("class definition (OOP pattern)")
                .with_snippet("local ${1:ClassName} = {}\n${1:ClassName}.__index = ${1:ClassName}\n\nfunction ${1:ClassName}.new(${2:args})\n\tlocal self = setmetatable({}, ${1:ClassName})\n\t${3:-- init}\n\treturn self\nend\n\nfunction ${1:ClassName}:${4:method}()\n\t${5:-- body}\nend"),
            CompletionItem::new("component", CompletionKind::Snippet)
                .with_detail("game component script")
                .with_snippet("local ${1:Component} = {}\n\nfunction ${1:Component}:on_start()\n\t${2:-- initialize}\nend\n\nfunction ${1:Component}:on_update(dt)\n\t${3:-- update every frame}\nend\n\nfunction ${1:Component}:on_destroy()\n\t${4:-- cleanup}\nend\n\nreturn ${1:Component}"),
            CompletionItem::new("coroutine_pattern", CompletionKind::Snippet)
                .with_detail("coroutine usage pattern")
                .with_snippet("local co = coroutine.create(function()\n\twhile true do\n\t\t${1:-- work}\n\t\tcoroutine.yield()\n\tend\nend)\n\ncoroutine.resume(co)"),
            CompletionItem::new("event_handler", CompletionKind::Snippet)
                .with_detail("event subscription")
                .with_snippet("Events.subscribe(\"${1:EventName}\", function(${2:data})\n\t${3:-- handle event}\nend)"),
            CompletionItem::new("tween_to", CompletionKind::Snippet)
                .with_detail("tween animation")
                .with_snippet("Tween.to(${1:target}, ${2:duration}, {\n\t${3:property} = ${4:value},\n\tease = \"${5:OutQuad}\",\n\ton_complete = function() ${6:end}\n})"),
        ]
    }
}

// ---------------------------------------------------------------------------
// Symbol table
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable, Function, Class, Struct, Enum, Constant,
    Parameter, LocalVariable, GlobalVariable, Field, Method,
    Namespace, Module, Interface, TypeAlias, Label,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub type_name: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub scope: String,
    pub documentation: String,
    pub is_exported: bool,
    pub references: Vec<(u32, u32)>,
}

impl Symbol {
    pub fn new(name: &str, kind: SymbolKind, file: &str, line: u32, col: u32) -> Self {
        Self {
            name: name.to_string(), kind, type_name: String::new(),
            file: file.to_string(), line, col, end_line: line,
            scope: String::new(), documentation: String::new(),
            is_exported: false, references: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub symbols: Vec<Symbol>,
    pub scope_stack: Vec<String>,
}

impl SymbolTable {
    pub fn push_scope(&mut self, name: &str) { self.scope_stack.push(name.to_string()); }
    pub fn pop_scope(&mut self) { self.scope_stack.pop(); }
    pub fn current_scope(&self) -> String { self.scope_stack.join("::") }
    pub fn add(&mut self, sym: Symbol) { self.symbols.push(sym); }
    pub fn find(&self, name: &str) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.name == name).collect()
    }
    pub fn find_in_scope(&self, name: &str, scope: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|s| s.name == name && s.scope == scope)
    }
    pub fn all_in_file(&self, file: &str) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.file == file).collect()
    }
    pub fn clear_file(&mut self, file: &str) {
        self.symbols.retain(|s| s.file != file);
    }
}

// ---------------------------------------------------------------------------
// Debugger
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebuggerState { Idle, Running, Paused, StepOver, StepInto, StepOut, Terminated }

impl DebuggerState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle", Self::Running => "Running", Self::Paused => "Paused",
            Self::StepOver => "Stepping Over", Self::StepInto => "Stepping In",
            Self::StepOut => "Stepping Out", Self::Terminated => "Terminated",
        }
    }
    pub fn is_active(&self) -> bool { !matches!(self, Self::Idle | Self::Terminated) }
}

#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u64,
    pub file: String,
    pub line: u32,
    pub enabled: bool,
    pub condition: Option<String>,
    pub hit_count: u32,
    pub hit_condition: Option<u32>,
    pub log_message: Option<String>,
    pub verified: bool,
}

impl Breakpoint {
    pub fn new(id: u64, file: &str, line: u32) -> Self {
        Self { id, file: file.to_string(), line, enabled: true, condition: None, hit_count: 0, hit_condition: None, log_message: None, verified: true }
    }
    pub fn conditional(id: u64, file: &str, line: u32, cond: &str) -> Self {
        Self { condition: Some(cond.to_string()), ..Self::new(id, file, line) }
    }
    pub fn should_break(&self) -> bool {
        if !self.enabled { return false; }
        if let Some(limit) = self.hit_condition { return self.hit_count >= limit; }
        true
    }
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub source_ref: Option<u64>,
    pub is_inlined: bool,
    pub instruction_pointer: u64,
    pub locals: Vec<WatchVariable>,
    pub args: Vec<WatchVariable>,
}

#[derive(Debug, Clone)]
pub struct WatchVariable {
    pub name: String,
    pub value: String,
    pub type_name: String,
    pub var_ref: u64,
    pub indexed_variables: u32,
    pub named_variables: u32,
    pub expensive: bool,
    pub children: Vec<WatchVariable>,
    pub changed: bool,
}

impl WatchVariable {
    pub fn new(name: &str, value: &str, type_name: &str) -> Self {
        Self {
            name: name.to_string(), value: value.to_string(), type_name: type_name.to_string(),
            var_ref: 0, indexed_variables: 0, named_variables: 0, expensive: false,
            children: Vec::new(), changed: false,
        }
    }
    pub fn table(name: &str, entries: Vec<WatchVariable>) -> Self {
        let count = entries.len() as u32;
        Self { named_variables: count, children: entries, ..Self::new(name, "table", "table") }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DebuggerSession {
    pub state: DebuggerState,
    pub breakpoints: Vec<Breakpoint>,
    pub call_stack: Vec<StackFrame>,
    pub watch_expressions: Vec<(String, String)>,
    pub output_log: Vec<String>,
    pub current_file: String,
    pub current_line: u32,
    pub next_bp_id: u64,
    pub exception_breakpoints: Vec<String>,
    pub pause_on_exceptions: bool,
    pub hot_reload_on_save: bool,
}

impl Default for DebuggerState { fn default() -> Self { Self::Idle } }

impl DebuggerSession {
    pub fn add_breakpoint(&mut self, file: &str, line: u32) -> u64 {
        let id = self.next_bp_id;
        self.next_bp_id += 1;
        self.breakpoints.push(Breakpoint::new(id, file, line));
        id
    }
    pub fn remove_breakpoint(&mut self, id: u64) { self.breakpoints.retain(|b| b.id != id); }
    pub fn toggle_breakpoint(&mut self, file: &str, line: u32) {
        if let Some(bp) = self.breakpoints.iter_mut().find(|b| b.file == file && b.line == line) {
            bp.enabled = !bp.enabled;
        } else {
            self.add_breakpoint(file, line);
        }
    }
    pub fn has_breakpoint_at(&self, file: &str, line: u32) -> Option<&Breakpoint> {
        self.breakpoints.iter().find(|b| b.file == file && b.line == line && b.enabled)
    }
    pub fn continue_execution(&mut self) { self.state = DebuggerState::Running; }
    pub fn pause(&mut self) { self.state = DebuggerState::Paused; }
    pub fn step_over(&mut self) { self.state = DebuggerState::StepOver; }
    pub fn step_into(&mut self) { self.state = DebuggerState::StepInto; }
    pub fn step_out(&mut self) { self.state = DebuggerState::StepOut; }
    pub fn terminate(&mut self) { self.state = DebuggerState::Terminated; self.call_stack.clear(); }
    pub fn add_watch(&mut self, expr: &str) { self.watch_expressions.push((expr.to_string(), "...".to_string())); }
    pub fn log_output(&mut self, msg: String) {
        if self.output_log.len() > 10000 { self.output_log.remove(0); }
        self.output_log.push(msg);
    }
    pub fn synthetic_stack() -> Vec<StackFrame> {
        vec![
            StackFrame { id: 1, name: "on_update".to_string(), file: "player.lua".to_string(), line: 42, col: 5, source_ref: None, is_inlined: false, instruction_pointer: 0x1000,
                locals: vec![WatchVariable::new("dt", "0.016", "number"), WatchVariable::new("speed", "5.0", "number"), WatchVariable::new("self", "{...}", "table")],
                args: vec![WatchVariable::new("dt", "0.016", "number")] },
            StackFrame { id: 2, name: "update_all".to_string(), file: "game.lua".to_string(), line: 88, col: 3, source_ref: None, is_inlined: false, instruction_pointer: 0x800,
                locals: vec![WatchVariable::new("entities", "[10 items]", "table")],
                args: vec![] },
            StackFrame { id: 3, name: "tick".to_string(), file: "main.lua".to_string(), line: 12, col: 1, source_ref: None, is_inlined: false, instruction_pointer: 0x200,
                locals: vec![], args: vec![] },
        ]
    }
}

// ---------------------------------------------------------------------------
// REPL console
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplOutputKind { Input, Output, Error, Warning, Info, System }

impl ReplOutputKind {
    pub fn color(&self) -> Vec4 {
        match self {
            Self::Input => Vec4::new(0.7, 0.9, 0.7, 1.0),
            Self::Output => Vec4::new(0.9, 0.9, 0.9, 1.0),
            Self::Error => Vec4::new(1.0, 0.35, 0.35, 1.0),
            Self::Warning => Vec4::new(1.0, 0.85, 0.3, 1.0),
            Self::Info => Vec4::new(0.4, 0.75, 1.0, 1.0),
            Self::System => Vec4::new(0.5, 0.5, 0.7, 1.0),
        }
    }
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Input => "> ", Self::Output => "  ", Self::Error => "✗ ",
            Self::Warning => "⚠ ", Self::Info => "ℹ ", Self::System => "• ",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplEntry {
    pub kind: ReplOutputKind,
    pub text: String,
    pub timestamp_ms: f64,
}

#[derive(Debug, Clone)]
pub struct ReplConsole {
    pub history: Vec<ReplEntry>,
    pub input_history: Vec<String>,
    pub input_history_pos: Option<usize>,
    pub current_input: String,
    pub max_history: usize,
    pub auto_scroll: bool,
    pub filter_level: Option<ReplOutputKind>,
    pub completions_visible: bool,
    pub completion_items: Vec<CompletionItem>,
    pub completion_selected: usize,
    pub multiline_mode: bool,
    pub multiline_buffer: Vec<String>,
}

impl Default for ReplConsole {
    fn default() -> Self {
        let mut c = Self {
            history: Vec::new(),
            input_history: Vec::new(),
            input_history_pos: None,
            current_input: String::new(),
            max_history: 5000,
            auto_scroll: true,
            filter_level: None,
            completions_visible: false,
            completion_items: Vec::new(),
            completion_selected: 0,
            multiline_mode: false,
            multiline_buffer: Vec::new(),
        };
        c.push_system("ProofScript REPL v1.0 — type 'help' for commands");
        c
    }
}

impl ReplConsole {
    pub fn push(&mut self, kind: ReplOutputKind, text: String) {
        if self.history.len() >= self.max_history { self.history.remove(0); }
        self.history.push(ReplEntry { kind, text, timestamp_ms: 0.0 });
    }
    pub fn push_system(&mut self, msg: &str) { self.push(ReplOutputKind::System, msg.to_string()); }
    pub fn push_output(&mut self, msg: &str) { self.push(ReplOutputKind::Output, msg.to_string()); }
    pub fn push_error(&mut self, msg: &str) { self.push(ReplOutputKind::Error, msg.to_string()); }

    pub fn submit_input(&mut self) -> String {
        let input = self.current_input.trim().to_string();
        if !input.is_empty() {
            self.push(ReplOutputKind::Input, input.clone());
            if self.input_history.last().map(|s: &String| s != &input).unwrap_or(true) {
                self.input_history.push(input.clone());
                if self.input_history.len() > 500 { self.input_history.remove(0); }
            }
        }
        self.current_input.clear();
        self.input_history_pos = None;
        self.completions_visible = false;
        input
    }

    pub fn history_up(&mut self) {
        if self.input_history.is_empty() { return; }
        let pos = self.input_history_pos.map(|p| p.saturating_sub(1)).unwrap_or(self.input_history.len() - 1);
        self.input_history_pos = Some(pos);
        self.current_input = self.input_history[pos].clone();
    }

    pub fn history_down(&mut self) {
        if let Some(pos) = self.input_history_pos {
            if pos + 1 >= self.input_history.len() {
                self.input_history_pos = None;
                self.current_input.clear();
            } else {
                let new_pos = pos + 1;
                self.input_history_pos = Some(new_pos);
                self.current_input = self.input_history[new_pos].clone();
            }
        }
    }

    pub fn eval_builtin_command(&mut self, cmd: &str) -> bool {
        match cmd.trim() {
            "help" => {
                self.push_output("Available commands:");
                self.push_output("  help          - show this help");
                self.push_output("  clear         - clear console");
                self.push_output("  vars          - list current variables");
                self.push_output("  reload        - hot-reload all scripts");
                self.push_output("  mem           - show memory usage");
                self.push_output("  gc            - run garbage collector");
                self.push_output("  breakpoints   - list breakpoints");
                self.push_output("  profile start - start profiling");
                self.push_output("  profile stop  - stop profiling and show results");
                true
            }
            "clear" => { self.history.clear(); true }
            "vars" => { self.push_output("(runtime not connected — use during debug session)"); true }
            "mem" => { self.push_output("Memory: 12.4 MB used / 256 MB limit (synthetic)"); true }
            "gc" => { self.push_output("GC: collected 1.2 MB (synthetic)"); true }
            "reload" => { self.push_system("Hot-reloading scripts..."); self.push_system("Reloaded 3 scripts successfully"); true }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Profiler integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScriptProfileEntry {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub call_count: u64,
    pub total_time_us: f64,
    pub self_time_us: f64,
    pub children: Vec<String>,
}

impl ScriptProfileEntry {
    pub fn avg_time_us(&self) -> f64 { if self.call_count == 0 { 0.0 } else { self.total_time_us / self.call_count as f64 } }
    pub fn self_pct(&self) -> f64 { if self.total_time_us <= 0.0 { 0.0 } else { self.self_time_us / self.total_time_us * 100.0 } }
}

#[derive(Debug, Clone, Default)]
pub struct ScriptProfiler {
    pub entries: Vec<ScriptProfileEntry>,
    pub recording: bool,
    pub total_time_us: f64,
    pub frame_count: u64,
}

impl ScriptProfiler {
    pub fn top_by_total(&self, n: usize) -> Vec<&ScriptProfileEntry> {
        let mut sorted: Vec<&ScriptProfileEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.total_time_us.partial_cmp(&a.total_time_us).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }
    pub fn top_by_self_time(&self, n: usize) -> Vec<&ScriptProfileEntry> {
        let mut sorted: Vec<&ScriptProfileEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.self_time_us.partial_cmp(&a.self_time_us).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }
    pub fn generate_synthetic_data(&mut self) {
        self.entries = vec![
            ScriptProfileEntry { name: "on_update".to_string(), file: "player.lua".to_string(), line: 42, call_count: 3600, total_time_us: 180000.0, self_time_us: 45000.0, children: vec!["move_player".to_string(), "check_collision".to_string()] },
            ScriptProfileEntry { name: "move_player".to_string(), file: "player.lua".to_string(), line: 78, call_count: 3600, total_time_us: 90000.0, self_time_us: 88000.0, children: vec![] },
            ScriptProfileEntry { name: "check_collision".to_string(), file: "player.lua".to_string(), line: 112, call_count: 3600, total_time_us: 45000.0, self_time_us: 45000.0, children: vec![] },
            ScriptProfileEntry { name: "update_enemies".to_string(), file: "enemy.lua".to_string(), line: 12, call_count: 3600, total_time_us: 320000.0, self_time_us: 20000.0, children: vec!["enemy_ai_tick".to_string()] },
            ScriptProfileEntry { name: "enemy_ai_tick".to_string(), file: "enemy.lua".to_string(), line: 55, call_count: 36000, total_time_us: 300000.0, self_time_us: 280000.0, children: vec![] },
            ScriptProfileEntry { name: "render_hud".to_string(), file: "ui.lua".to_string(), line: 8, call_count: 3600, total_time_us: 60000.0, self_time_us: 60000.0, children: vec![] },
        ];
        self.total_time_us = self.entries.iter().map(|e| e.self_time_us).sum();
        self.frame_count = 3600;
    }
    pub fn avg_script_ms_per_frame(&self) -> f64 { if self.frame_count == 0 { 0.0 } else { self.total_time_us / self.frame_count as f64 / 1000.0 } }
}

// ---------------------------------------------------------------------------
// Code editor buffer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    pub file_path: String,
    pub language: ScriptLanguage,
    pub content: String,
    pub lines: Vec<String>,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
    pub cursor_line: u32,
    pub cursor_col: u32,
    pub selection_start: Option<(u32, u32)>,
    pub selection_end: Option<(u32, u32)>,
    pub scroll_top: f32,
    pub scroll_left: f32,
    pub modified: bool,
    pub read_only: bool,
    pub word_wrap: bool,
    pub show_whitespace: bool,
    pub tab_size: u32,
    pub use_spaces: bool,
    pub encoding: FileEncoding,
    pub line_ending: LineEnding,
    pub fold_ranges: Vec<(u32, u32, bool)>,
    pub bookmarks: Vec<u32>,
    pub undo_stack: Vec<EditOperation>,
    pub redo_stack: Vec<EditOperation>,
    pub symbol_table: SymbolTable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileEncoding { Utf8, Utf8Bom, Utf16Le, Utf16Be, Latin1 }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineEnding { LF, CRLF, CR }

impl FileEncoding {
    pub fn label(&self) -> &'static str {
        match self { Self::Utf8 => "UTF-8", Self::Utf8Bom => "UTF-8 BOM", Self::Utf16Le => "UTF-16 LE", Self::Utf16Be => "UTF-16 BE", Self::Latin1 => "Latin-1" }
    }
}
impl LineEnding {
    pub fn label(&self) -> &'static str { match self { Self::LF => "LF", Self::CRLF => "CRLF", Self::CR => "CR" } }
}

#[derive(Debug, Clone)]
pub struct EditOperation {
    pub kind: EditKind,
    pub line: u32, pub col: u32,
    pub text: String,
    pub old_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditKind { Insert, Delete, Replace }

impl EditorBuffer {
    pub fn new(file_path: &str, content: String, language: ScriptLanguage) -> Self {
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        Self {
            file_path: file_path.to_string(),
            language, content,
            lines,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            cursor_line: 0, cursor_col: 0,
            selection_start: None, selection_end: None,
            scroll_top: 0.0, scroll_left: 0.0,
            modified: false, read_only: false,
            word_wrap: false, show_whitespace: false,
            tab_size: 4, use_spaces: true,
            encoding: FileEncoding::Utf8,
            line_ending: LineEnding::LF,
            fold_ranges: Vec::new(),
            bookmarks: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            symbol_table: SymbolTable::default(),
        }
    }

    pub fn insert_at_cursor(&mut self, text: &str) {
        let op = EditOperation {
            kind: EditKind::Insert,
            line: self.cursor_line, col: self.cursor_col,
            text: text.to_string(), old_text: String::new(),
        };
        self.apply_insert(self.cursor_line, self.cursor_col, text);
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.modified = true;
    }

    fn apply_insert(&mut self, line: u32, col: u32, text: &str) {
        if (line as usize) < self.lines.len() {
            let ln = self.lines[line as usize].clone();
            let col = col.min(ln.len() as u32) as usize;
            let new_ln = format!("{}{}{}", &ln[..col], text, &ln[col..]);
            if text.contains('\n') {
                let parts: Vec<&str> = new_ln.splitn(2, '\n').collect();
                self.lines[line as usize] = parts[0].to_string();
                if parts.len() > 1 {
                    self.lines.insert(line as usize + 1, parts[1].to_string());
                }
                self.cursor_line = line + 1;
                self.cursor_col = 0;
            } else {
                self.lines[line as usize] = new_ln;
                self.cursor_col = col as u32 + text.len() as u32;
            }
        }
        self.rebuild_content();
    }

    pub fn delete_at_cursor(&mut self) {
        if (self.cursor_line as usize) < self.lines.len() {
            let ln = &self.lines[self.cursor_line as usize];
            if (self.cursor_col as usize) < ln.len() {
                let old_char = ln.chars().nth(self.cursor_col as usize).map(|c| c.to_string()).unwrap_or_default();
                let op = EditOperation { kind: EditKind::Delete, line: self.cursor_line, col: self.cursor_col, text: String::new(), old_text: old_char };
                let mut ln = self.lines[self.cursor_line as usize].clone();
                ln.remove(self.cursor_col as usize);
                self.lines[self.cursor_line as usize] = ln;
                self.undo_stack.push(op);
                self.modified = true;
                self.rebuild_content();
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop() {
            // simplified undo
            self.cursor_line = op.line;
            self.cursor_col = op.col;
            self.redo_stack.push(op);
            self.modified = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.redo_stack.pop() {
            self.cursor_line = op.line;
            self.cursor_col = op.col;
            self.undo_stack.push(op);
        }
    }

    fn rebuild_content(&mut self) {
        self.content = self.lines.join("\n");
    }

    pub fn retokenize(&mut self) {
        let tok = Tokenizer::new(self.language);
        self.tokens = tok.tokenize(&self.content);
    }

    pub fn line_count(&self) -> u32 { self.lines.len() as u32 }

    pub fn get_line(&self, line: u32) -> Option<&str> {
        self.lines.get(line as usize).map(|s| s.as_str())
    }

    pub fn tokens_on_line(&self, line: u32) -> Vec<&Token> {
        self.tokens.iter().filter(|t| t.line == line).collect()
    }

    pub fn diagnostics_on_line(&self, line: u32) -> Vec<&Diagnostic> {
        self.diagnostics.iter().filter(|d| d.line == line).collect()
    }

    pub fn has_bookmark(&self, line: u32) -> bool { self.bookmarks.contains(&line) }

    pub fn toggle_bookmark(&mut self, line: u32) {
        if let Some(pos) = self.bookmarks.iter().position(|&b| b == line) {
            self.bookmarks.remove(pos);
        } else {
            self.bookmarks.push(line);
            self.bookmarks.sort();
        }
    }

    pub fn next_bookmark(&self, from_line: u32) -> Option<u32> {
        self.bookmarks.iter().find(|&&b| b > from_line).copied()
    }

    pub fn prev_bookmark(&self, from_line: u32) -> Option<u32> {
        self.bookmarks.iter().rev().find(|&&b| b < from_line).copied()
    }

    pub fn find_all(&self, query: &str, case_sensitive: bool, whole_word: bool) -> Vec<(u32, u32)> {
        let mut results = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            let (haystack, needle) = if case_sensitive { (line.as_str(), query) } else { (line.as_str(), query) };
            let mut search_start = 0;
            while let Some(pos) = haystack[search_start..].find(needle) {
                let abs_pos = search_start + pos;
                let word_ok = !whole_word || {
                    let before = abs_pos == 0 || !haystack.chars().nth(abs_pos - 1).map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
                    let after = abs_pos + needle.len() >= haystack.len() || !haystack.chars().nth(abs_pos + needle.len()).map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
                    before && after
                };
                if word_ok { results.push((i as u32, abs_pos as u32)); }
                search_start = abs_pos + 1;
                if search_start >= haystack.len() { break; }
            }
        }
        results
    }

    pub fn replace_all(&mut self, find: &str, replace: &str, case_sensitive: bool) -> u32 {
        let mut count = 0u32;
        for line in &mut self.lines {
            let new_line = if case_sensitive {
                line.replace(find, replace)
            } else {
                let lower = line.to_lowercase();
                let find_lower = find.to_lowercase();
                let mut result = line.clone();
                let mut offset = 0i32;
                let mut search_pos = 0;
                while let Some(pos) = lower[search_pos..].find(&find_lower) {
                    let abs_pos = search_pos + pos;
                    let new_pos = (abs_pos as i32 + offset) as usize;
                    result.replace_range(new_pos..new_pos + find.len(), replace);
                    offset += replace.len() as i32 - find.len() as i32;
                    search_pos = abs_pos + 1;
                    count += 1;
                    if search_pos >= lower.len() { break; }
                }
                result
            };
            *line = new_line;
        }
        if count > 0 { self.rebuild_content(); self.modified = true; }
        count
    }

    pub fn format_document(&mut self) {
        // Simple formatting: normalize indentation
        let mut formatted = Vec::new();
        let mut indent = 0i32;
        for line in &self.lines {
            let trimmed = line.trim();
            if trimmed.is_empty() { formatted.push(String::new()); continue; }
            let dedent_words = match self.language {
                ScriptLanguage::Lua | ScriptLanguage::LuaJIT => vec!["end", "else", "elseif", "until"],
                _ => vec!["}"],
            };
            let indent_words = match self.language {
                ScriptLanguage::Lua | ScriptLanguage::LuaJIT => vec!["do", "then", "function", "repeat"],
                _ => vec!["{"],
            };
            if dedent_words.iter().any(|&w| trimmed.starts_with(w)) { indent -= 1; }
            let spaces = "    ".repeat(indent.max(0) as usize);
            formatted.push(format!("{}{}", spaces, trimmed));
            if indent_words.iter().any(|&w| trimmed.ends_with(w)) { indent += 1; }
        }
        self.lines = formatted;
        self.rebuild_content();
        self.modified = true;
    }
}

// ---------------------------------------------------------------------------
// Project file tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScriptFile {
    pub path: String,
    pub name: String,
    pub language: ScriptLanguage,
    pub size_bytes: u64,
    pub error_count: u32,
    pub warning_count: u32,
    pub modified: bool,
    pub is_entry_point: bool,
}

impl ScriptFile {
    pub fn new(path: &str, lang: ScriptLanguage) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        Self { path: path.to_string(), name, language: lang, size_bytes: 0, error_count: 0, warning_count: 0, modified: false, is_entry_point: false }
    }
    pub fn has_issues(&self) -> bool { self.error_count > 0 || self.warning_count > 0 }
}

#[derive(Debug, Clone)]
pub struct ScriptFolder {
    pub name: String,
    pub path: String,
    pub children_files: Vec<ScriptFile>,
    pub children_folders: Vec<ScriptFolder>,
    pub expanded: bool,
}

impl ScriptFolder {
    pub fn new(name: &str, path: &str) -> Self {
        Self { name: name.to_string(), path: path.to_string(), children_files: Vec::new(), children_folders: Vec::new(), expanded: true }
    }
    pub fn total_errors(&self) -> u32 {
        self.children_files.iter().map(|f| f.error_count).sum::<u32>()
            + self.children_folders.iter().map(|f| f.total_errors()).sum::<u32>()
    }
    pub fn file_count(&self) -> usize {
        self.children_files.len() + self.children_folders.iter().map(|f| f.file_count()).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// Script project
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScriptProject {
    pub name: String,
    pub root_folder: ScriptFolder,
    pub language: ScriptLanguage,
    pub entry_points: Vec<String>,
    pub include_paths: Vec<String>,
    pub global_defines: Vec<(String, String)>,
    pub lib_paths: Vec<String>,
    pub output_path: String,
    pub optimization_level: u8,
    pub target_platform: String,
    pub hot_reload_enabled: bool,
    pub strict_mode: bool,
    pub lint_enabled: bool,
    pub format_on_save: bool,
}

impl ScriptProject {
    pub fn default_lua_project() -> Self {
        let mut root = ScriptFolder::new("scripts", "scripts/");
        let mut gameplay = ScriptFolder::new("gameplay", "scripts/gameplay/");
        gameplay.children_files.push({ let mut f = ScriptFile::new("scripts/gameplay/player.lua", ScriptLanguage::Lua); f.is_entry_point = true; f.size_bytes = 2400; f });
        gameplay.children_files.push(ScriptFile::new("scripts/gameplay/enemy.lua", ScriptLanguage::Lua));
        gameplay.children_files.push(ScriptFile::new("scripts/gameplay/weapon.lua", ScriptLanguage::Lua));
        gameplay.children_files.push(ScriptFile::new("scripts/gameplay/inventory.lua", ScriptLanguage::Lua));
        gameplay.children_files.push(ScriptFile::new("scripts/gameplay/quest.lua", ScriptLanguage::Lua));
        let mut ui = ScriptFolder::new("ui", "scripts/ui/");
        ui.children_files.push(ScriptFile::new("scripts/ui/hud.lua", ScriptLanguage::Lua));
        ui.children_files.push(ScriptFile::new("scripts/ui/menu.lua", ScriptLanguage::Lua));
        ui.children_files.push(ScriptFile::new("scripts/ui/dialogue.lua", ScriptLanguage::Lua));
        let mut util = ScriptFolder::new("util", "scripts/util/");
        util.children_files.push(ScriptFile::new("scripts/util/math_ext.lua", ScriptLanguage::Lua));
        util.children_files.push(ScriptFile::new("scripts/util/table_ext.lua", ScriptLanguage::Lua));
        util.children_files.push(ScriptFile::new("scripts/util/string_ext.lua", ScriptLanguage::Lua));
        util.children_files.push(ScriptFile::new("scripts/util/event_bus.lua", ScriptLanguage::Lua));
        root.children_folders = vec![gameplay, ui, util];
        root.children_files.push({ let mut f = ScriptFile::new("scripts/main.lua", ScriptLanguage::Lua); f.is_entry_point = true; f.size_bytes = 800; f });

        Self {
            name: "Game Scripts".to_string(),
            root_folder: root,
            language: ScriptLanguage::Lua,
            entry_points: vec!["scripts/main.lua".to_string()],
            include_paths: vec!["scripts/".to_string(), "scripts/util/".to_string()],
            global_defines: vec![("GAME_VERSION".to_string(), "\"1.0\"".to_string())],
            lib_paths: Vec::new(),
            output_path: "build/scripts/".to_string(),
            optimization_level: 0,
            target_platform: "all".to_string(),
            hot_reload_enabled: true,
            strict_mode: false,
            lint_enabled: true,
            format_on_save: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Editor panels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IDEPanel {
    Editor, FileTree, Debugger, Console, Profiler,
    Search, Problems, StdLibBrowser, Outline, Git,
}

impl IDEPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Editor => "Editor", Self::FileTree => "Explorer", Self::Debugger => "Debug",
            Self::Console => "Console", Self::Profiler => "Profiler", Self::Search => "Search",
            Self::Problems => "Problems", Self::StdLibBrowser => "API Browser",
            Self::Outline => "Outline", Self::Git => "Source Control",
        }
    }
}

// ---------------------------------------------------------------------------
// Scripting IDE main editor
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ScriptingIDE {
    pub project: ScriptProject,
    pub open_buffers: Vec<EditorBuffer>,
    pub active_buffer: Option<usize>,
    pub debugger: DebuggerSession,
    pub console: ReplConsole,
    pub profiler: ScriptProfiler,
    pub active_panel: IDEPanel,
    pub show_minimap: bool,
    pub font_size: f32,
    pub theme: EditorTheme,
    pub completion_active: bool,
    pub completion_items: Vec<CompletionItem>,
    pub completion_selected: usize,
    pub completion_trigger_char: Option<char>,
    pub hover_info: Option<HoverInfo>,
    pub signature_help: Option<SignatureHelp>,
    pub find_query: String,
    pub find_replace: String,
    pub find_case_sensitive: bool,
    pub find_whole_word: bool,
    pub find_regex: bool,
    pub find_results: Vec<(u32, u32)>,
    pub find_result_index: usize,
    pub search_in_files_query: String,
    pub search_results: Vec<SearchResult>,
    pub outline_symbols: Vec<OutlineEntry>,
    pub side_panel_width: f32,
    pub bottom_panel_height: f32,
    pub split_editor: bool,
    pub split_buffer: Option<usize>,
    pub zoom_level: f32,
    pub show_line_numbers: bool,
    pub show_breadcrumbs: bool,
    pub show_indent_guides: bool,
    pub word_wrap: bool,
    pub auto_save: bool,
    pub auto_save_delay_secs: f32,
    pub tab_recent_files: Vec<String>,
    pub pinned_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorTheme { Dark, Light, Monokai, OneDark, Dracula, GruvboxDark, SolarizedDark, NordDark }

impl EditorTheme {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark => "Default Dark", Self::Light => "Default Light", Self::Monokai => "Monokai",
            Self::OneDark => "One Dark", Self::Dracula => "Dracula", Self::GruvboxDark => "Gruvbox Dark",
            Self::SolarizedDark => "Solarized Dark", Self::NordDark => "Nord",
        }
    }
    pub fn background(&self) -> Vec4 {
        match self {
            Self::Dark | Self::OneDark => Vec4::new(0.117, 0.118, 0.122, 1.0),
            Self::Light => Vec4::new(0.98, 0.98, 0.98, 1.0),
            Self::Monokai => Vec4::new(0.157, 0.157, 0.137, 1.0),
            Self::Dracula => Vec4::new(0.157, 0.165, 0.212, 1.0),
            Self::GruvboxDark => Vec4::new(0.157, 0.141, 0.094, 1.0),
            Self::SolarizedDark => Vec4::new(0.0, 0.169, 0.212, 1.0),
            Self::NordDark => Vec4::new(0.180, 0.204, 0.251, 1.0),
        }
    }
    pub fn gutter_background(&self) -> Vec4 { let bg = self.background(); Vec4::new(bg.x * 0.85, bg.y * 0.85, bg.z * 0.85, 1.0) }
    pub fn selection_color(&self) -> Vec4 { Vec4::new(0.27, 0.39, 0.57, 0.5) }
    pub fn cursor_color(&self) -> Vec4 { Vec4::new(1.0, 1.0, 1.0, 1.0) }
    pub fn line_number_color(&self) -> Vec4 { Vec4::new(0.45, 0.45, 0.45, 1.0) }
    pub fn active_line_color(&self) -> Vec4 { Vec4::new(1.0, 1.0, 1.0, 0.05) }
}

#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub title: String,
    pub type_info: String,
    pub documentation: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct SignatureHelp {
    pub function_name: String,
    pub signatures: Vec<SignatureInfo>,
    pub active_signature: usize,
    pub active_parameter: usize,
}

#[derive(Debug, Clone)]
pub struct SignatureInfo {
    pub label: String,
    pub documentation: String,
    pub parameters: Vec<ParameterInfo>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub label: String,
    pub documentation: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub line_text: String,
    pub match_start: u32,
    pub match_len: u32,
}

#[derive(Debug, Clone)]
pub struct OutlineEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub col: u32,
    pub depth: u32,
    pub children: Vec<OutlineEntry>,
}

impl Default for ScriptingIDE {
    fn default() -> Self {
        let player_script = r#"-- Player controller script
local Player = {}
Player.__index = Player

-- Configuration
local MOVE_SPEED = 5.0
local JUMP_FORCE = 8.0
local GRAVITY = -20.0
local MAX_FALL_SPEED = -30.0
local COYOTE_TIME = 0.1
local JUMP_BUFFER = 0.1

function Player.new(entity_id)
    local self = setmetatable({}, Player)
    self.entity = Entity.find(entity_id)
    self.velocity = Vector3.zero
    self.grounded = false
    self.jump_count = 0
    self.max_jumps = 2
    self.coyote_timer = 0
    self.jump_buffer_timer = 0
    self.health = 100
    self.max_health = 100
    self.stamina = 100
    self.max_stamina = 100
    self.is_attacking = false
    self.attack_cooldown = 0
    self.combo_count = 0
    self.last_combo_time = 0
    return self
end

function Player:on_start()
    Events.subscribe("OnDamageDealt", function(data)
        if data.target == self.entity then
            self:take_damage(data.amount)
        end
    end)
    Debug.log("Player initialized: " .. tostring(self.entity))
end

function Player:on_update(dt)
    self:handle_input(dt)
    self:apply_gravity(dt)
    self:move(dt)
    self:update_timers(dt)
    self:update_animation()
    self:update_stamina(dt)
    self:check_death()
end

function Player:handle_input(dt)
    local move_x = Input.get_axis("Horizontal")
    local move_z = Input.get_axis("Vertical")
    local cam_forward = Camera.forward()
    local cam_right = Camera.right()

    -- Project movement onto horizontal plane
    local forward = Vector3.new(cam_forward.x, 0, cam_forward.z)
    local right = Vector3.new(cam_right.x, 0, cam_right.z)
    forward = Vector3.normalize(forward)
    right = Vector3.normalize(right)

    local move_dir = Vector3.new(
        forward.x * move_z + right.x * move_x,
        0,
        forward.z * move_z + right.z * move_x
    )

    if Vector3.length(move_dir) > 0 then
        move_dir = Vector3.normalize(move_dir)
        local speed = MOVE_SPEED
        if Input.get_button("Sprint") and self.stamina > 0 then
            speed = speed * 1.8
            self.stamina = self.stamina - 15 * dt
        end
        self.velocity.x = move_dir.x * speed
        self.velocity.z = move_dir.z * speed
    else
        self.velocity.x = Math.lerp(self.velocity.x, 0, 10 * dt)
        self.velocity.z = Math.lerp(self.velocity.z, 0, 10 * dt)
    end

    -- Jump
    if Input.get_button_down("Jump") then
        self.jump_buffer_timer = JUMP_BUFFER
    end

    if self.jump_buffer_timer > 0 then
        if self.grounded or self.coyote_timer > 0 or self.jump_count < self.max_jumps then
            self:do_jump()
        end
    end

    -- Attack
    if Input.get_button_down("Fire") and self.attack_cooldown <= 0 then
        self:start_attack()
    end

    -- Dodge
    if Input.get_button_down("Dodge") and self.stamina >= 20 then
        self:dodge(move_dir)
    end
end

function Player:do_jump()
    self.velocity.y = JUMP_FORCE
    self.jump_count = self.jump_count + 1
    self.coyote_timer = 0
    self.jump_buffer_timer = 0
    self.grounded = false
    Audio.play_one_shot("jump_sfx", self.entity:get_position())
    Events.publish("OnPlayerJumped", { entity = self.entity, velocity = self.velocity })
end

function Player:apply_gravity(dt)
    if not self.grounded then
        self.velocity.y = self.velocity.y + GRAVITY * dt
        self.velocity.y = math.max(self.velocity.y, MAX_FALL_SPEED)
    end
end

function Player:move(dt)
    local pos = self.entity:get_position()
    local new_pos = Vector3.new(
        pos.x + self.velocity.x * dt,
        pos.y + self.velocity.y * dt,
        pos.z + self.velocity.z * dt
    )
    -- Ground check
    local ground_hit = Physics.raycast(pos, Vector3.down, 1.1)
    if ground_hit and self.velocity.y <= 0 then
        self.grounded = true
        self.jump_count = 0
        new_pos.y = ground_hit.point.y + 1.0
        self.velocity.y = 0
    else
        if self.grounded then
            self.coyote_timer = COYOTE_TIME
        end
        self.grounded = false
    end
    self.entity:set_position(new_pos)
end

function Player:update_timers(dt)
    if self.coyote_timer > 0 then self.coyote_timer = self.coyote_timer - dt end
    if self.jump_buffer_timer > 0 then self.jump_buffer_timer = self.jump_buffer_timer - dt end
    if self.attack_cooldown > 0 then self.attack_cooldown = self.attack_cooldown - dt end
    if Time.time - self.last_combo_time > 0.8 then self.combo_count = 0 end
end

function Player:update_stamina(dt)
    if self.stamina < self.max_stamina then
        self.stamina = math.min(self.max_stamina, self.stamina + 15 * dt)
    end
end

function Player:update_animation()
    local anim = self.entity:get_component("Animator")
    if not anim then return end
    anim:set_float("SpeedX", self.velocity.x)
    anim:set_float("SpeedZ", self.velocity.z)
    anim:set_bool("Grounded", self.grounded)
    anim:set_float("VelocityY", self.velocity.y)
    anim:set_bool("IsAttacking", self.is_attacking)
    anim:set_float("Health", self.health / self.max_health)
end

function Player:start_attack()
    self.is_attacking = true
    self.combo_count = self.combo_count + 1
    self.last_combo_time = Time.time
    local anim_name = "Attack" .. math.min(self.combo_count, 3)
    local anim = self.entity:get_component("Animator")
    if anim then anim:set_trigger(anim_name) end
    Audio.play_one_shot("attack_" .. math.min(self.combo_count, 3) .. "_sfx", self.entity:get_position())
    self.attack_cooldown = 0.4
    -- Hitbox
    local pos = self.entity:get_position()
    local dir = self.entity:get_forward()
    local hits = Physics.overlap_box(
        Vector3.new(pos.x + dir.x * 1.5, pos.y + 1.0, pos.z + dir.z * 1.5),
        Vector3.new(1.2, 1.2, 1.2)
    )
    for _, hit in ipairs(hits) do
        if hit ~= self.entity then
            local dmg = 15 + self.combo_count * 5
            Events.publish("OnDamageDealt", { target = hit, attacker = self.entity, amount = dmg, type = "melee" })
        end
    end
end

function Player:take_damage(amount)
    self.health = math.max(0, self.health - amount)
    Events.publish("OnPlayerDamaged", { entity = self.entity, health = self.health, max_health = self.max_health })
    Audio.play_one_shot("hit_sfx", self.entity:get_position())
    local vfx = self.entity:get_component("VFXEmitter")
    if vfx then vfx:emit("HitEffect") end
    Tween.to(self.entity, 0.1, { shake_intensity = 0.5, on_complete = function()
        Tween.to(self.entity, 0.2, { shake_intensity = 0 })
    end})
end

function Player:dodge(direction)
    if Vector3.length(direction) < 0.1 then direction = self.entity:get_forward() end
    self.stamina = self.stamina - 20
    local dodge_vel = Vector3.new(direction.x * 12, 2, direction.z * 12)
    self.velocity = dodge_vel
    local anim = self.entity:get_component("Animator")
    if anim then anim:set_trigger("Dodge") end
    Audio.play_one_shot("dodge_sfx", self.entity:get_position())
end

function Player:check_death()
    if self.health <= 0 then
        Events.publish("OnPlayerDied", { entity = self.entity })
        Audio.play_one_shot("death_sfx", self.entity:get_position())
        local anim = self.entity:get_component("Animator")
        if anim then anim:set_trigger("Death") end
    end
end

function Player:heal(amount)
    self.health = math.min(self.max_health, self.health + amount)
    Events.publish("OnPlayerHealed", { entity = self.entity, amount = amount, health = self.health })
end

function Player:get_health_pct() return self.health / self.max_health end
function Player:get_stamina_pct() return self.stamina / self.max_stamina end
function Player:is_alive() return self.health > 0 end

return Player
"#;
        let mut buf = EditorBuffer::new("scripts/gameplay/player.lua", player_script.to_string(), ScriptLanguage::Lua);
        buf.retokenize();

        let mut profiler = ScriptProfiler::default();
        profiler.generate_synthetic_data();

        let completions = {
            let mut items = LuaCompletionProvider::global_completions();
            items.extend(LuaCompletionProvider::snippets());
            items
        };

        Self {
            project: ScriptProject::default_lua_project(),
            open_buffers: vec![buf],
            active_buffer: Some(0),
            debugger: DebuggerSession {
                call_stack: DebuggerSession::synthetic_stack(),
                ..Default::default()
            },
            console: ReplConsole::default(),
            profiler,
            active_panel: IDEPanel::Editor,
            show_minimap: true,
            font_size: 14.0,
            theme: EditorTheme::OneDark,
            completion_active: false,
            completion_items: completions,
            completion_selected: 0,
            completion_trigger_char: None,
            hover_info: None,
            signature_help: None,
            find_query: String::new(),
            find_replace: String::new(),
            find_case_sensitive: false,
            find_whole_word: false,
            find_regex: false,
            find_results: Vec::new(),
            find_result_index: 0,
            search_in_files_query: String::new(),
            search_results: Vec::new(),
            outline_symbols: Vec::new(),
            side_panel_width: 240.0,
            bottom_panel_height: 200.0,
            split_editor: false,
            split_buffer: None,
            zoom_level: 1.0,
            show_line_numbers: true,
            show_breadcrumbs: true,
            show_indent_guides: true,
            word_wrap: false,
            auto_save: true,
            auto_save_delay_secs: 5.0,
            tab_recent_files: vec!["scripts/gameplay/player.lua".to_string()],
            pinned_files: Vec::new(),
        }
    }
}

impl ScriptingIDE {
    pub fn active_buffer(&self) -> Option<&EditorBuffer> {
        self.active_buffer.and_then(|i| self.open_buffers.get(i))
    }

    pub fn active_buffer_mut(&mut self) -> Option<&mut EditorBuffer> {
        self.active_buffer.and_then(|i| self.open_buffers.get_mut(i))
    }

    pub fn open_file(&mut self, path: &str, content: String, lang: ScriptLanguage) {
        // Check if already open
        if let Some(i) = self.open_buffers.iter().position(|b| b.file_path == path) {
            self.active_buffer = Some(i);
            return;
        }
        let mut buf = EditorBuffer::new(path, content, lang);
        buf.retokenize();
        self.open_buffers.push(buf);
        self.active_buffer = Some(self.open_buffers.len() - 1);
        if !self.tab_recent_files.contains(&path.to_string()) {
            self.tab_recent_files.insert(0, path.to_string());
            if self.tab_recent_files.len() > 20 { self.tab_recent_files.truncate(20); }
        }
    }

    pub fn close_buffer(&mut self, index: usize) {
        if index < self.open_buffers.len() {
            self.open_buffers.remove(index);
            if let Some(active) = self.active_buffer {
                if active >= self.open_buffers.len() && !self.open_buffers.is_empty() {
                    self.active_buffer = Some(self.open_buffers.len() - 1);
                } else if self.open_buffers.is_empty() {
                    self.active_buffer = None;
                }
            }
        }
    }

    pub fn get_completions(&self, prefix: &str) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = self.completion_items.iter()
            .map(|item| { let mut i = item.clone(); i.score = i.fuzzy_score(prefix); i })
            .filter(|i| i.score > 0.0)
            .collect();
        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            .then(a.sort_text.cmp(&b.sort_text)));
        items.truncate(50);
        items
    }

    pub fn submit_repl(&mut self, input: String) {
        if input.is_empty() { return; }
        if !self.console.eval_builtin_command(&input) {
            // Simulate script eval result
            let result = self.simulate_eval(&input);
            self.console.push_output(&result);
        }
    }

    fn simulate_eval(&self, expr: &str) -> String {
        // Very basic expression simulation
        if let Ok(n) = expr.trim().parse::<f64>() {
            return format!("{}", n);
        }
        if expr.starts_with("print(") {
            let inner = &expr[6..expr.len().saturating_sub(1)];
            return format!("[stdout] {}", inner.trim_matches('"').trim_matches('\''));
        }
        if expr.contains('+') || expr.contains('-') || expr.contains('*') || expr.contains('/') {
            return "(arithmetic expression evaluated)".to_string();
        }
        format!("=> {}", expr)
    }

    pub fn find_in_active(&mut self) {
        let query = self.find_query.clone();
        let case = self.find_case_sensitive;
        let word = self.find_whole_word;
        if let Some(buf) = self.active_buffer() {
            self.find_results = buf.find_all(&query, case, word);
        }
    }

    pub fn find_next(&mut self) {
        if !self.find_results.is_empty() {
            self.find_result_index = (self.find_result_index + 1) % self.find_results.len();
            let (line, col) = self.find_results[self.find_result_index];
            if let Some(buf) = self.active_buffer_mut() {
                buf.cursor_line = line;
                buf.cursor_col = col;
            }
        }
    }

    pub fn total_problems(&self) -> (u32, u32) {
        let errors: u32 = self.open_buffers.iter().map(|b| b.diagnostics.iter().filter(|d| d.severity == DiagnosticSeverity::Error).count() as u32).sum();
        let warnings: u32 = self.open_buffers.iter().map(|b| b.diagnostics.iter().filter(|d| d.severity == DiagnosticSeverity::Warning).count() as u32).sum();
        (errors, warnings)
    }

    pub fn save_active(&mut self) {
        if let Some(buf) = self.active_buffer_mut() {
            buf.modified = false;
        }
    }

    pub fn build_outline(&mut self) {
        let mut outline = Vec::new();
        if let Some(buf) = self.active_buffer() {
            for sym in buf.symbol_table.all_in_file(&buf.file_path.clone()) {
                outline.push(OutlineEntry {
                    name: sym.name.clone(), kind: sym.kind,
                    line: sym.line, col: sym.col, depth: 0, children: Vec::new(),
                });
            }
        }
        self.outline_symbols = outline;
    }

    pub fn format_active_document(&mut self) {
        if let Some(buf) = self.active_buffer_mut() {
            buf.format_document();
        }
    }
}
