import re

# Fix inventory_system.rs: rename LootEntry -> LootEntryDef, LootTable -> LootTableDef
# (only in the appended section, after line 7300)
with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find the appended section boundary - the second struct LootEntry
loot_entry_lines = [i for i, l in enumerate(lines) if 'pub struct LootEntry' in l or 'pub struct LootTable {' in l and 'loot_table' not in l.lower()]
print(f"LootEntry/LootTable definitions at lines: {[x+1 for x in loot_entry_lines]}")

# The first occurrences are the originals (lines ~600, 615)
# The new ones start around line 7300+
# We need to rename the new ones: LootEntry -> LootEntryDef, LootTable -> LootTableDef
# Find the split point (after the first two definitions)
# We rename everything after line 7000

content = ''.join(lines)
# Split at line 7000
split_idx = sum(len(l) for l in lines[:7000])
before = content[:split_idx]
after = content[split_idx:]

# In the after section, rename the types
after = after.replace('pub struct LootEntry {', 'pub struct LootEntryDef {')
after = after.replace('pub struct LootTable {', 'pub struct LootTableDef {')
after = after.replace('impl LootEntry {', 'impl LootEntryDef {')
after = after.replace('impl LootTable {', 'impl LootTableDef {')
# Replace usages
after = after.replace('Vec<LootEntry>', 'Vec<LootEntryDef>')
after = after.replace('LootEntry::new(', 'LootEntryDef::new(')
after = after.replace('LootEntry::guaranteed(', 'LootEntryDef::guaranteed(')
after = after.replace(': LootEntry', ': LootEntryDef')
after = after.replace('LootTable::new(', 'LootTableDef::new(')
after = after.replace('LootTable::goblin_loot()', 'LootTableDef::goblin_loot()')
after = after.replace('LootTable::dragon_hoard()', 'LootTableDef::dragon_hoard()')
after = after.replace('Vec<LootTable>', 'Vec<LootTableDef>')
after = after.replace(': LootTable', ': LootTableDef')
after = after.replace('&LootTable', '&LootTableDef')
after = after.replace('pub tables: Vec<LootTableDef>', 'pub loot_table_defs: Vec<LootTableDef>')
after = after.replace('state.tables', 'state.loot_table_defs')

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed inventory_system.rs")

# Fix world_gen.rs: rename SettlementType, Settlement, NoiseType in appended section
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Find second occurrences
settlement_type_lines = [i for i, l in enumerate(lines) if 'pub enum SettlementType' in l]
settlement_lines = [i for i, l in enumerate(lines) if 'pub struct Settlement {' in l]
noise_type_lines = [i for i, l in enumerate(lines) if 'pub enum NoiseType' in l]
print(f"SettlementType at lines: {[x+1 for x in settlement_type_lines]}")
print(f"Settlement at lines: {[x+1 for x in settlement_lines]}")
print(f"NoiseType at lines: {[x+1 for x in noise_type_lines]}")

# Split at line 6500 (before the new additions)
content = ''.join(lines)
split_idx = sum(len(l) for l in lines[:6500])
before = content[:split_idx]
after = content[split_idx:]

# Rename the new enums/structs
after = after.replace('pub enum SettlementType', 'pub enum SettlementKind')
after = after.replace('SettlementType::', 'SettlementKind::')
after = after.replace(': SettlementType', ': SettlementKind')
after = after.replace('settlement_type: SettlementType', 'settlement_type: SettlementKind')
after = after.replace('settlement_type: SettlementKind', 'settlement_kind: SettlementKind')
after = after.replace('s.settlement_type =', 's.settlement_kind =')
after = after.replace('.settlement_type.name()', '.settlement_kind.name()')
after = after.replace('matches!(s.settlement_type', 'matches!(s.settlement_kind')
after = after.replace('let st = if', 'let sk = if')
after = after.replace('let mut s = Settlement::new(&name, fx, fy, st);', 'let mut s = NewSettlement::new(&name, fx, fy, sk);')

# Rename Settlement struct to NewSettlement
after = after.replace('pub struct Settlement {', 'pub struct NewSettlement {')
after = after.replace('impl Settlement {', 'impl NewSettlement {')
after = after.replace('Vec<Settlement>', 'Vec<NewSettlement>')
after = after.replace(': Settlement', ': NewSettlement')
after = after.replace('Settlement::new(', 'NewSettlement::new(')
after = after.replace('&[Settlement]', '&[NewSettlement]')
after = after.replace('settlements: Vec<NewSettlement>', 'new_settlements: Vec<NewSettlement>')
after = after.replace('state.settlements', 'state.new_settlements')
after = after.replace('pub settlements:', 'pub new_settlements:')

# Rename NoiseType to HeightmapNoiseType
after = after.replace('pub enum NoiseType', 'pub enum HeightmapNoiseType')
after = after.replace('NoiseType::', 'HeightmapNoiseType::')
after = after.replace(': NoiseType', ': HeightmapNoiseType')
after = after.replace('noise_type: NoiseType', 'noise_type: HeightmapNoiseType')
after = after.replace('noise_type: HeightmapNoiseType', 'noise_kind: HeightmapNoiseType')
after = after.replace('cfg.noise_type', 'cfg.noise_kind')
after = after.replace('HeightmapNoiseType::FBM => fbm', 'HeightmapNoiseType::Fbm => fbm')
after = after.replace('HeightmapNoiseType::FBM', 'HeightmapNoiseType::Fbm')
after = after.replace('cfg.noise_kind = nt', 'cfg.noise_kind = nt.clone()')
after = after.replace('NoiseHeightmapConfig', 'HeightmapNoiseCfg')
after = after.replace('pub struct HeightmapNoiseCfg', 'pub struct HeightmapNoiseCfg')

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed world_gen.rs")
