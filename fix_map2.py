#!/usr/bin/env python
filepath = r'C:\proof-engine\src\editor\map_editor.rs'
with open(filepath, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Truncate at 6688 (0-indexed 6687)
cutoff = 6687  # keep 6687 lines (0-6686)
lines = lines[:cutoff]
if lines and not lines[-1].endswith('\n'):
    lines[-1] += '\n'

with open(filepath, 'w', encoding='utf-8') as f:
    f.writelines(lines)
print(f"File truncated to {len(lines)} lines")
