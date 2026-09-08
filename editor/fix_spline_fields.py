with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Fix .points -> .nodes (only in appended section, after the boundary)
# These only appear in appended code so safe to replace globally
content = content.replace('spline.points.is_empty()', 'spline.nodes.is_empty()')
content = content.replace('editor.zoom)', 'editor.canvas_zoom)')
content = content.replace('editor.zoom;', 'editor.canvas_zoom;')

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'w', encoding='utf-8') as f:
    f.write(content)
print("Done")
