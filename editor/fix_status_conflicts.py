
with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

split_line = 7840
before = ''.join(lines[:split_line])
after = ''.join(lines[split_line:])

# Rename new StatusEffectType -> StatusEffectKind, StatusEffect -> CharStatusEffect
after = after.replace('pub enum StatusEffectType {', 'pub enum StatusEffectKind {')
after = after.replace('StatusEffectType::', 'StatusEffectKind::')
after = after.replace(': StatusEffectType', ': StatusEffectKind')
after = after.replace('Vec<StatusEffectType>', 'Vec<StatusEffectKind>')
after = after.replace('pub struct StatusEffect {', 'pub struct CharStatusEffect {')
after = after.replace('StatusEffect {', 'CharStatusEffect {')
after = after.replace('Vec<StatusEffect>', 'Vec<CharStatusEffect>')
after = after.replace(': StatusEffect', ': CharStatusEffect')
after = after.replace('StatusEffect::', 'CharStatusEffect::')
# Fix StatusEffectPanelState fields if referencing StatusEffect
after = after.replace('effects: Vec<CharStatusEffect>', 'effects: Vec<CharStatusEffect>')

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed status effect conflicts")
