#!/usr/bin/env python3
filepath = r'C:\proof-engine\src\editor\terrain_road_tool.rs'
with open(filepath, 'r', encoding='utf-8') as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Step 1: Remove duplicate stubs at lines 2610-2620 (0-indexed 2609-2619)
# These are: add_road_point, add_city_node, begin_road_placement, finish_road_placement,
# generate_procedural_roads, place_roundabout, run_erosion_simulation, step_traffic_simulation,
# statistics, serialize, deserialize
# Lines 2610-2620 (1-indexed), 0-indexed 2609-2619
stub_methods = [
    'pub fn add_road_point(&mut self, _pt: Vec3)',
    'pub fn add_city_node(&mut self, _pos: Vec3, _node_type: RoadNodeType, _population: u32)',
    'pub fn begin_road_placement(&mut self, _road_type: RoadType)',
    'pub fn finish_road_placement(&mut self)',
    'pub fn generate_procedural_roads(&mut self)',
    'pub fn place_roundabout(&mut self, _center: Vec3, _arms: Vec<Vec3>)',
    'pub fn run_erosion_simulation(&mut self, _steps: usize)',
    'pub fn step_traffic_simulation(&mut self, _dt: f32)',
    'pub fn statistics(&self) -> RoadNetworkStats',
    'pub fn serialize(&self) -> Vec<u8>',
    'pub fn deserialize(&mut self, _data: &[u8])',
]

# Find and remove these stub lines (only the first occurrence, around line 2610)
new_lines = []
skip_set = set()
for i, line in enumerate(lines):
    stripped = line.strip()
    if i < 2650 and i > 2605:  # Only in the first impl block region
        found = False
        for stub in stub_methods:
            if stub in stripped:
                skip_set.add(i)
                found = True
                break
        if found:
            continue
    new_lines.append(line)

print(f"Removed {len(lines) - len(new_lines)} duplicate stub lines from first impl block")
lines = new_lines

# Step 2: Truncate at line 7016 (1-indexed), i.e., keep only lines 0..7015 (0-indexed)
# Find the "// ============================================================" separator before the appended impls
cutoff = None
for i in range(len(lines)-1, 6900, -1):
    if '// ============================================================' in lines[i]:
        cutoff = i
        break

if cutoff is None:
    # fallback: find first impl HorizontalCurve after line 7000
    for i in range(7000, len(lines)):
        if lines[i].startswith('impl HorizontalCurve {'):
            cutoff = i - 1
            break

print(f"Truncating at line {cutoff+1} (0-indexed {cutoff})")
lines = lines[:cutoff]

# Ensure trailing newline
if lines and not lines[-1].endswith('\n'):
    lines[-1] += '\n'

with open(filepath, 'w', encoding='utf-8') as f:
    f.writelines(lines)

print(f"File now has {len(lines)} lines")
