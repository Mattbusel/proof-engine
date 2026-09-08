
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

split = 10800
before = ''.join(lines[:split])
after = ''.join(lines[split:])

# The struct literal DungeonRoom { inside the impl DungeonRoom2 block needs to be DungeonRoom2 {
# This was missed because the struct literal uses 'DungeonRoom {' which might still be there
# Also fix SplineNode.position references in spline_editor

# Fix the struct literal in the impl block
after = after.replace('        DungeonRoom {\n', '        DungeonRoom2 {\n')
# Also DungeonRoomType2 was created but DungeonRoomType still used in some places
after = after.replace('room_type: DungeonRoom2Type', 'room_type: DungeonRoomType2')
after = after.replace('rt: DungeonRoom2Type', 'rt: DungeonRoomType2')

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed world_gen.rs DungeonRoom struct literal")

# Fix spline_editor.rs - SplineNode has no .position field, uses .pos or direct x/y
# Need to find what field it actually has
with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Find SplineNode struct definition
import re
m = re.search(r'pub struct SplineNode \{([^}]+)\}', content)
if m:
    print(f"SplineNode fields: {m.group(1)[:200]}")
else:
    # Try multi-line
    idx = content.find('pub struct SplineNode')
    if idx >= 0:
        print(f"SplineNode def: {content[idx:idx+300]}")
