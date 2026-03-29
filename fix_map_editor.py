#!/usr/bin/env python
# Truncate map_editor.rs at the first duplicate impl and rewrite

filepath = r'C:\proof-engine\src\editor\map_editor.rs'
with open(filepath, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find the line with the appended section (empty line + blank + "impl TileAnimation {")
# The original file ends at line 6344 (0-indexed 6343)
# We want to keep up to line 6344
cutoff = None
for i, line in enumerate(lines):
    if line.strip() == '' and i > 6340:
        # Check if next non-empty is an impl TileAnimation
        for j in range(i+1, min(i+5, len(lines))):
            if lines[j].startswith('impl TileAnimation {'):
                cutoff = i
                break
        if cutoff is not None:
            break

if cutoff is None:
    print("Could not find cutoff, using line 6344")
    cutoff = 6344

print(f"Truncating at line {cutoff+1}")
lines = lines[:cutoff]

# Make sure file ends with newline
if lines and not lines[-1].endswith('\n'):
    lines[-1] += '\n'

with open(filepath, 'w', encoding='utf-8') as f:
    f.writelines(lines)

print(f"File now has {len(lines)} lines")
