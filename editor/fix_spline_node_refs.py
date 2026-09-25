
with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find the split point - our expansion 4 starts after the existing content
# Look for SplinePhysicsBody which we added
split = None
for i, l in enumerate(lines):
    if 'pub struct SplinePhysicsBody {' in l:
        split = i
        break

if split is None:
    print("Could not find split point")
    exit(1)

print(f"Split at line {split+1}")
before = ''.join(lines[:split])
after = ''.join(lines[split:])

# Fix SplineNode field access: node.position.x -> node.point.position[0]
# node.position.y -> node.point.position[1]
after = after.replace('.position.x', '.point.position[0]')
after = after.replace('.position.y', '.point.position[1]')

# Also fix SplineNode { position: ... } struct literal
after = after.replace('SplineNode {\n                position: egui::pos2(', 'SplineNode {\n                point: ControlPoint { position: [')

# Fix the specific test that creates SplineNode
# SplineNode { position: egui::pos2(x, y), ..SplineNode::default() }
import re
# Replace SplineNode { position: egui::pos2(X, Y), ..SplineNode::default() }
after = re.sub(
    r'SplineNode \{\s*\n\s*position: egui::pos2\(([^,]+),\s*([^)]+)\),\s*\n\s*\.\.',
    lambda m: f'SplineNode {{ point: ControlPoint {{ position: [{m.group(1)}, {m.group(2)}], ..Default::default() }}, ..',
    after
)

# Also fix the build_from_spline method which accesses node.position
after = after.replace(
    'spline.nodes.iter().map(|nd| (nd.position.x, nd.position.y)).collect()',
    'spline.nodes.iter().map(|nd| (nd.point.position[0], nd.point.position[1])).collect()'
)

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed SplineNode references in expansion4")
