
# Fix world_gen.rs: rename conflicting structs in appended section (after line 6500)
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

split_line = 6500
content_before = ''.join(lines[:split_line])
content_after = ''.join(lines[split_line:])

# Rename TectonicPlate -> TectonicPlateSim, TectonicSimConfig -> TectonicSimCfg
content_after = content_after.replace('pub struct TectonicPlate {', 'pub struct TectonicPlateSim {')
content_after = content_after.replace('impl TectonicPlate {', 'impl TectonicPlateSim {')
content_after = content_after.replace('TectonicPlate::new(', 'TectonicPlateSim::new(')
content_after = content_after.replace(': TectonicPlate', ': TectonicPlateSim')
content_after = content_after.replace('Vec<TectonicPlate>', 'Vec<TectonicPlateSim>')
content_after = content_after.replace('plates: Vec<TectonicPlateSim>', 'plates_sim: Vec<TectonicPlateSim>')
content_after = content_after.replace('mut plates:', 'mut plates_sim:')
content_after = content_after.replace('for (i, p) in plates.iter', 'for (i, p) in plates_sim.iter')
content_after = content_after.replace('for (ri, region) in regions', 'for (ri2, region2) in regions')
content_after = content_after.replace('for (i, p) in plates_sim.iter().enumerate()', 'for (i, p) in plates_sim.iter().enumerate()')
content_after = content_after.replace('let mut plates:', 'let mut plates_sim:')
content_after = content_after.replace('plates.push(', 'plates_sim.push(')
content_after = content_after.replace('plates[best_plate', 'plates_sim[best_plate')
content_after = content_after.replace('plates[pid_a]', 'plates_sim[pid_a]')
content_after = content_after.replace('plates[pid_b]', 'plates_sim[pid_b]')

# Also rename RiverNode if it conflicts
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    all_lines = f.readlines()
river_node_lines = [i for i, l in enumerate(all_lines) if 'pub struct RiverNode' in l]
river_network_lines = [i for i, l in enumerate(all_lines) if 'pub struct RiverNetwork' in l]
river_segment_lines = [i for i, l in enumerate(all_lines) if 'pub struct RiverSegment' in l]
print(f"RiverNode at: {[x+1 for x in river_node_lines]}")
print(f"RiverNetwork at: {[x+1 for x in river_network_lines]}")
print(f"RiverSegment at: {[x+1 for x in river_segment_lines]}")

# Check if there are conflicts before line 6500
conflicts_before_split = [x for x in river_node_lines if x < split_line]
print(f"RiverNode conflicts before split: {conflicts_before_split}")

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'w', encoding='utf-8') as f:
    f.write(content_before + content_after)
print("Wrote world_gen.rs")
