//! Proof Editor — Visual staging and scene authoring environment.

#[allow(unused)] mod scene;
#[allow(unused)] mod tools;
#[allow(unused)] mod commands;
#[allow(unused)] mod hotkeys;
#[allow(unused)] mod clipboard;
#[allow(unused)] mod preferences;
#[allow(unused)] mod viewport;
#[allow(unused)] mod layout;
#[allow(unused)] mod behavior_tree;
#[allow(unused)] mod dialogue_graph;
#[allow(unused)] mod particle_editor;
#[allow(unused)] mod material_system;
#[allow(unused)] mod spline_editor;
#[allow(unused)] mod quest_system;
#[allow(unused)] mod audio_mixer;
#[allow(unused)] mod physics_editor;
#[allow(unused)] mod inventory_system;
#[allow(unused)] mod world_gen;
mod editor_panels;

use proof_engine::prelude::*;
use proof_engine::input::Key;
use scene::{SceneDocument, NodeKind, FieldType};
use tools::{ToolKind, CHAR_PALETTES, COLOR_PALETTES};
use editor_panels::EditorState;
use std::sync::Arc;
use std::f32::consts::TAU;

fn main() {
    env_logger::init();
    let config = EngineConfig {
        window_title: "Proof Editor".to_string(),
        window_width: 1600, window_height: 1000,
        render: proof_engine::config::RenderConfig { bloom_enabled: true, bloom_intensity: 1.2, chromatic_aberration: 0.001, ..Default::default() },
        ..Default::default()
    };
    let mut engine = ProofEngine::new(config);
    let mut state = EditorState::new();
    let mut egui_init = false;
    let mut egui_ctx = egui::Context::default();
    let mut egui_painter: Option<egui_glow::Painter> = None;
    let mut fps_t = 0.0f32; let mut fps_n = 0u32;
    let mut spawn_ctr = 0u32;
    let mut dragging = false; let mut drag_start = Vec3::ZERO;
    let mut box_sel = false; let mut box_start = (0.0f32, 0.0f32);
    let mut clipboard: Vec<scene::SceneNode> = Vec::new();

    spawn_grid(&mut engine);

    engine.run_with_overlay(move |engine, dt, gl| {
        fps_n += 1; fps_t += dt;
        if fps_t >= 0.5 { state.fps = fps_n as f32 / fps_t; fps_n = 0; fps_t = 0.0; }
        state.status_timer = (state.status_timer - dt).max(0.0);

        if !egui_init {
            let gl_arc: Arc<glow::Context> = unsafe { let p = gl as *const glow::Context; Arc::increment_strong_count(p); Arc::from_raw(p) };
            egui_painter = Some(egui_glow::Painter::new(gl_arc, "", None, false).unwrap());
            egui_init = true;
        }

        let input = engine.input.clone();
        // Shortcuts
        if input.just_pressed(Key::F1) { state.show_help = !state.show_help; }
        if input.just_pressed(Key::Space) { engine.add_trauma(0.3); }
        if input.just_pressed(Key::Escape) { state.document.selection.clear(); dragging = false; box_sel = false; }
        if input.just_pressed(Key::V) && !input.ctrl() { state.tool = ToolKind::Select; }
        if input.just_pressed(Key::G) && !input.ctrl() { state.tool = ToolKind::Move; }
        if input.just_pressed(Key::P) && !input.ctrl() { state.tool = ToolKind::Place; }
        if input.just_pressed(Key::F) && !input.ctrl() { state.tool = ToolKind::Field; }
        if input.just_pressed(Key::E) && !input.ctrl() { state.tool = ToolKind::Entity; }
        if input.just_pressed(Key::X) && !input.ctrl() { state.tool = ToolKind::Particle; }
        if input.just_pressed(Key::Delete) { state.push_undo("Delete"); let s=state.document.selection.clone(); for i in s{state.document.remove_node(i);} state.document.selection.clear(); state.needs_rebuild=true; }
        if input.ctrl()&&input.just_pressed(Key::Z) { state.undo(); }
        if input.ctrl()&&input.just_pressed(Key::Y) { state.redo(); }
        if input.ctrl()&&input.just_pressed(Key::S) { match state.document.save("scene.json"){Ok(_)=>state.set_status("Saved"),Err(e)=>state.set_status(&format!("Err:{}",e))} }
        if input.ctrl()&&input.just_pressed(Key::O) { state.push_undo("Load"); match SceneDocument::load("scene.json"){Ok(d)=>{state.document=d;state.needs_rebuild=true;state.set_status("Loaded");}Err(e)=>state.set_status(&format!("Err:{}",e))} }
        if input.ctrl()&&input.just_pressed(Key::N) { state.push_undo("New"); state.document=SceneDocument::new(); state.needs_rebuild=true; state.set_status("New"); }
        if input.ctrl()&&input.just_pressed(Key::A) { state.document.select_all(); }
        if input.ctrl()&&input.just_pressed(Key::D) { state.push_undo("Dup"); let s=state.document.selection.clone(); let mut n=Vec::new(); for i in s{if let Some(x)=state.document.duplicate_node(i){n.push(x);}} state.document.selection=n; state.needs_rebuild=true; }
        if input.ctrl()&&input.just_pressed(Key::C) { clipboard.clear(); for &i in &state.document.selection{if let Some(n)=state.document.get_node(i){clipboard.push(n.clone());}} if !clipboard.is_empty(){state.set_status(&format!("Copied {}",clipboard.len()));} }
        if input.ctrl()&&input.just_pressed(Key::V)&&!clipboard.is_empty() { state.push_undo("Paste"); let mut n=Vec::new(); for nd in &clipboard{let mut c=nd.clone();c.position+=Vec3::new(1.0,-1.0,0.0);c.id=state.document.next_id;state.document.next_id+=1;c.name=format!("{}_copy",c.name);state.document.nodes.push(c);n.push(state.document.next_id-1);} state.document.selection=n;state.needs_rebuild=true; }

        // Camera
        let sp = 12.0*dt;
        if input.is_pressed(Key::Up)||(input.is_pressed(Key::W)&&!input.ctrl()){state.cam_y+=sp;}
        if input.is_pressed(Key::Down)||(input.is_pressed(Key::S)&&!input.ctrl()){state.cam_y-=sp;}
        if input.is_pressed(Key::Left)||(input.is_pressed(Key::A)&&!input.ctrl()){state.cam_x-=sp;}
        if input.is_pressed(Key::Right)||(input.is_pressed(Key::D)&&!input.ctrl()){state.cam_x+=sp;}
        engine.camera.position.x.target=state.cam_x; engine.camera.position.y.target=state.cam_y;
        engine.camera.position.x.position=state.cam_x; engine.camera.position.y.position=state.cam_y;

        if state.model_3d_mode {
            let az = state.model_cam_azimuth.to_radians();
            let el = state.model_cam_elevation.to_radians();
            let cam_x = state.model_cam_distance * el.cos() * az.sin();
            let cam_y = state.model_cam_distance * el.sin();
            engine.camera.position.x.target = cam_x;
            engine.camera.position.x.position = cam_x;
            engine.camera.position.y.target = cam_y;
            engine.camera.position.y.position = cam_y;
        }

        if state.needs_rebuild { rebuild_scene(engine, &state.document); state.needs_rebuild = false; }

        if let Some(painter) = egui_painter.as_mut() {
            let (ww, wh) = engine.window_size();
            let mut ri = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(ww as f32, wh as f32))), ..Default::default() };
            ri.events.push(egui::Event::PointerMoved(egui::pos2(input.mouse_x, input.mouse_y)));
            if input.mouse_left_just_pressed { ri.events.push(egui::Event::PointerButton{pos:egui::pos2(input.mouse_x,input.mouse_y),button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}); }
            if input.mouse_left_just_released { ri.events.push(egui::Event::PointerButton{pos:egui::pos2(input.mouse_x,input.mouse_y),button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}); }
            if input.scroll_delta!=0.0 { ri.events.push(egui::Event::MouseWheel{unit:egui::MouseWheelUnit::Line,delta:egui::vec2(0.0,input.scroll_delta),modifiers:Default::default()}); }

            let fo = egui_ctx.run(ri, |ctx| {
                // ── Dark theme with polished accent colors ──
                let mut visuals = egui::Visuals::dark();
                visuals.window_corner_radius = egui::CornerRadius::same(6);
                visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(28, 30, 36);
                visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 42, 50);
                visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 58, 70);
                visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 130, 200);
                visuals.window_fill = egui::Color32::from_rgb(22, 24, 30);
                visuals.panel_fill = egui::Color32::from_rgb(18, 20, 26);
                ctx.set_visuals(visuals);
                editor_panels::menu_bar(ctx, &mut state, engine);
                editor_panels::hierarchy_panel(ctx, &mut state);
                editor_panels::inspector_panel(ctx, &mut state);
                editor_panels::toolbar_panel(ctx, &mut state, engine);
                editor_panels::force_field_panel(ctx, &mut state);
                editor_panels::postfx_panel(ctx, &mut state, engine);
                editor_panels::asset_browser(ctx, &mut state, engine);
                editor_panels::console_panel(ctx, &mut state);
                editor_panels::help_window(ctx, &mut state);
                editor_panels::world_editor_panel(ctx, &mut state, engine);
                editor_panels::ai_behavior_panel(ctx, &mut state);
                editor_panels::physics_panel(ctx, &mut state);
                editor_panels::render_graph_panel(ctx, &mut state);
                editor_panels::dialogue_panel(ctx, &mut state);
                editor_panels::quest_panel(ctx, &mut state);
                editor_panels::spline_panel(ctx, &mut state);
                editor_panels::cinematic_panel(ctx, &mut state);
                editor_panels::inventory_panel(ctx, &mut state);
                editor_panels::ability_panel(ctx, &mut state);
                editor_panels::level_streaming_panel(ctx, &mut state);
                editor_panels::audio_mixer_panel(ctx, &mut state);
                editor_panels::modeling_panel(ctx, &mut state, engine);
                editor_panels::behavior_tree_panel(ctx, &mut state);
                editor_panels::dialogue_graph_panel(ctx, &mut state);
                editor_panels::particle_editor_panel(ctx, &mut state, dt);
                editor_panels::material_system_panel(ctx, &mut state);
                editor_panels::spline_editor_panel(ctx, &mut state, dt);
                editor_panels::quest_system_panel(ctx, &mut state);
                editor_panels::audio_mixer_full_panel(ctx, &mut state, dt);
                editor_panels::physics_editor_panel(ctx, &mut state, dt);
                editor_panels::inventory_system_panel(ctx, &mut state);
                editor_panels::world_gen_panel(ctx, &mut state, dt);
            });
            let ewp = egui_ctx.is_pointer_over_area();
            let pr = egui_ctx.tessellate(fo.shapes, egui_ctx.pixels_per_point());
            painter.paint_and_update_textures([ww,wh], egui_ctx.pixels_per_point(), &pr, &fo.textures_delta);

            // Viewport click
            if input.mouse_left_just_pressed && !ewp {
                let mx=(input.mouse_x/ww as f32-0.5)*18.0+state.cam_x;
                let my=-((input.mouse_y/wh as f32-0.5)*11.0)+state.cam_y;
                let wp=Vec3::new(mx,my,0.0);
                match state.tool {
                    ToolKind::Place=>{state.push_undo("Place");let(_,ch)=CHAR_PALETTES[state.char_palette_idx];let(_,co)=COLOR_PALETTES[state.color_palette_idx];let c=5+(spawn_ctr%4)as usize;for i in 0..c{let a=(i as f32/c as f32)*TAU;let r=0.3+(i as f32*0.17).sin().abs()*0.5;let chr=ch[(spawn_ctr as usize+i)%ch.len()];let(cr,cg,cb)=co[i%co.len()];let b=0.6+(i as f32*0.2).sin().abs()*0.4;let col=Vec4::new(cr*b,cg*b,cb*b,0.9);let p=wp+Vec3::new(a.cos()*r,a.sin()*r,0.0);let n=state.document.add_glyph_node(p,chr,col,state.emission,state.glow_radius);spawn_glyph_from_doc(engine,&state.document,n);}spawn_ctr+=1;state.set_status(&format!("Placed {}",c));}
                    ToolKind::Field=>{state.push_undo("Field");let ft=FieldType::all()[state.field_type_idx];state.document.add_field_node(wp,ft);engine.add_field(ft.to_force_field(wp));engine.spawn_glyph(Glyph{character:'~',position:wp,color:Vec4::new(1.0,0.7,0.2,0.8),emission:0.5,glow_color:Vec3::new(1.0,0.5,0.1),glow_radius:2.0,layer:RenderLayer::Entity,..Default::default()});state.set_status(&format!("{}",ft.label()));}
                    ToolKind::Entity=>{state.push_undo("Entity");let(_,co)=COLOR_PALETTES[state.color_palette_idx];let(cr,cg,cb)=co[0];let col=Vec4::new(cr,cg,cb,0.9);state.document.add_entity_node(wp);let mut e=AmorphousEntity::new("Entity",wp);e.entity_mass=3.0;e.cohesion=0.7;e.pulse_rate=0.5;e.pulse_depth=0.15;e.hp=100.0;e.max_hp=100.0;let ch=['@','#','*','+','o','x','X','O','.',':','~','='];for i in 0..12{let a=(i as f32/12.0)*TAU;e.formation.push(Vec3::new(a.cos()*0.8,a.sin()*0.8,0.0));e.formation_chars.push(ch[i%ch.len()]);e.formation_colors.push(col);}engine.spawn_entity(e);state.set_status("Entity");}
                    ToolKind::Particle=>{let(_,co)=COLOR_PALETTES[state.color_palette_idx];let(r,g,b)=co[0];engine.emit_particles(proof_engine::particle::EmitterPreset::DeathExplosion{color:Vec4::new(r,g,b,1.0)},wp);state.set_status("Burst!");}
                    ToolKind::Select=>{if let Some(id)=state.document.pick_at(wp,1.5){if input.shift(){state.document.toggle_selection(id);}else{state.document.selection=vec![id];}}else{box_sel=true;box_start=(input.mouse_x,input.mouse_y);if!input.shift(){state.document.selection.clear();}}}
                    ToolKind::Move=>{if let Some(id)=state.document.pick_at(wp,1.5){if!state.document.selection.contains(&id){if input.shift(){state.document.toggle_selection(id);}else{state.document.selection=vec![id];}}dragging=true;drag_start=wp;}}_=>{}
                }
            }
            // Drag
            if dragging&&!ewp&&input.mouse_left_just_released{let dp=screen_to_world(input.mouse_x,input.mouse_y,ww,wh,state.cam_x,state.cam_y);let d=dp-drag_start;if d.length()>0.05{state.push_undo("Move");let s=state.document.selection.clone();for i in s{state.document.translate_node(i,d);}state.needs_rebuild=true;}dragging=false;}
            // Box select
            if box_sel&&!ewp&&input.mouse_left_just_released{let(sx,sy)=box_start;let(ex,ey)=(input.mouse_x,input.mouse_y);let c0=screen_to_world(sx.min(ex),sy.min(ey),ww,wh,state.cam_x,state.cam_y);let c1=screen_to_world(sx.max(ex),sy.max(ey),ww,wh,state.cam_x,state.cam_y);let x0=c0.x.min(c1.x);let y0=c0.y.min(c1.y);let x1=c0.x.max(c1.x);let y1=c0.y.max(c1.y);let mut s=if input.shift(){state.document.selection.clone()}else{Vec::new()};for n in state.document.nodes(){let p=n.position;if p.x>=x0&&p.x<=x1&&p.y>=y0&&p.y<=y1&&!s.contains(&n.id){s.push(n.id);}}state.document.selection=s;box_sel=false;}
            // Gizmos
            for &id in &state.document.selection{if let Some(n)=state.document.get_node(id){let p=n.position;for i in 0..8{let a=(i as f32/8.0)*TAU;engine.spawn_glyph(Glyph{character:'.',position:Vec3::new(p.x+a.cos()*1.2,p.y+a.sin()*1.2,0.1),color:Vec4::new(1.0,0.9,0.2,0.5),emission:0.3,layer:RenderLayer::Overlay,lifetime:0.02,..Default::default()});}engine.spawn_glyph(Glyph{character:'+',position:Vec3::new(p.x,p.y,0.1),color:Vec4::new(1.0,1.0,0.3,0.6),emission:0.4,layer:RenderLayer::Overlay,lifetime:0.02,..Default::default()});if state.tool==ToolKind::Move{engine.spawn_glyph(Glyph{character:'>',position:Vec3::new(p.x+1.5,p.y,0.1),color:Vec4::new(1.0,0.2,0.2,0.8),emission:0.5,layer:RenderLayer::Overlay,lifetime:0.02,..Default::default()});engine.spawn_glyph(Glyph{character:'^',position:Vec3::new(p.x,p.y+1.5,0.1),color:Vec4::new(0.2,1.0,0.2,0.8),emission:0.5,layer:RenderLayer::Overlay,lifetime:0.02,..Default::default()});}}}
        }
    });
}

/// Convert screen pixel coordinates to world-space position on the Z=0 plane.
/// Camera: position (cam_x, cam_y, 10), FOV 60°, looking at -Z.
fn screen_to_world(mouse_x: f32, mouse_y: f32, win_w: u32, win_h: u32, cam_x: f32, cam_y: f32) -> Vec3 {
    let fov_rad = 60.0_f32.to_radians();
    let cam_z = 10.0_f32;
    let aspect = win_w as f32 / win_h.max(1) as f32;

    // NDC: mouse (0,0)=top-left, (w,h)=bottom-right → NDC (-1,1)=top-left, (1,-1)=bottom-right
    let ndc_x = (mouse_x / win_w as f32) * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_y / win_h as f32) * 2.0; // flip Y: screen Y down → world Y up

    // Half-extents of the visible area at Z=0 (distance = cam_z)
    let half_h = (fov_rad * 0.5).tan() * cam_z;
    let half_w = half_h * aspect;

    let world_x = cam_x + ndc_x * half_w;
    let world_y = cam_y + ndc_y * half_h;

    Vec3::new(world_x, world_y, 0.0)
}

fn spawn_glyph_from_doc(engine: &mut ProofEngine, doc: &SceneDocument, nid: u32) {
    if let Some(n) = doc.get_node(nid) { engine.spawn_glyph(Glyph { character: n.character.unwrap_or('@'), position: n.position, color: n.color, emission: n.emission, glow_color: Vec3::new(n.color.x,n.color.y,n.color.z), glow_radius: n.glow_radius, mass: 0.1, layer: RenderLayer::Entity, blend_mode: BlendMode::Additive, life_function: Some(MathFunction::Breathing{rate:0.3,depth:0.08}), ..Default::default() }); }
}

fn rebuild_scene(engine: &mut ProofEngine, doc: &SceneDocument) {
    engine.scene = SceneGraph::new();
    spawn_grid(engine);
    for n in doc.nodes() { match n.kind {
        NodeKind::Glyph => { engine.spawn_glyph(Glyph{character:n.character.unwrap_or('@'),position:n.position,color:n.color,emission:n.emission,glow_color:Vec3::new(n.color.x,n.color.y,n.color.z),glow_radius:n.glow_radius,mass:0.1,layer:RenderLayer::Entity,blend_mode:BlendMode::Additive,life_function:Some(MathFunction::Breathing{rate:0.3,depth:0.08}),..Default::default()}); }
        NodeKind::Field => { if let Some(ref ft)=n.field_type{engine.add_field(ft.to_force_field(n.position));engine.spawn_glyph(Glyph{character:'~',position:n.position,color:Vec4::new(1.0,0.7,0.2,0.8),emission:0.5,glow_color:Vec3::new(1.0,0.5,0.1),glow_radius:2.0,layer:RenderLayer::Entity,..Default::default()});} }
        NodeKind::Entity => { let mut e=AmorphousEntity::new(&n.name,n.position);e.entity_mass=3.0;e.cohesion=0.7;e.pulse_rate=0.5;e.pulse_depth=0.15;e.hp=100.0;e.max_hp=100.0;let ch=['@','#','*','+','o','x','X','O','.',':','~','='];for i in 0..12{let a=(i as f32/12.0)*TAU;e.formation.push(Vec3::new(a.cos()*0.8,a.sin()*0.8,0.0));e.formation_chars.push(ch[i%ch.len()]);e.formation_colors.push(n.color);}engine.spawn_entity(e); }
        _=>{}
    }}
}

fn spawn_grid(engine: &mut ProofEngine) {
    for y in -15..=15{for x in -20..=20{
        let ax=x==0||y==0;let mj=x%5==0&&y%5==0;
        let ch=if ax&&mj{'+'}else if ax{'-'}else if mj{'.'}else if(x+y)%4==0{'.'}else{continue};
        let c=if x==0&&y==0{Vec4::new(1.0,1.0,0.3,0.4)}else if ax{Vec4::new(0.2,0.3,0.5,0.25)}else{Vec4::new(0.15,0.15,0.2,0.12)};
        engine.spawn_glyph(Glyph{character:ch,position:Vec3::new(x as f32,y as f32,-2.0),color:c,emission:if ax{0.15}else{0.03},layer:RenderLayer::Background,..Default::default()});
    }}
}
