#!/usr/bin/env python3
filepath = r'C:\proof-engine\src\editor\terrain_road_tool.rs'
with open(filepath, 'r', encoding='utf-8') as f:
    lines = f.readlines()

print(f"Total lines: {len(lines)}")

# Keep only up to line 7015 (0-indexed 7014), discarding everything after
lines = lines[:7015]
if lines and not lines[-1].endswith('\n'):
    lines[-1] += '\n'

with open(filepath, 'w', encoding='utf-8') as f:
    f.writelines(lines)

print(f"File now has {len(lines)} lines")
