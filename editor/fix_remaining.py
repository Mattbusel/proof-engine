
# Fix inventory_system.rs
with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Fix simulate_roll type annotation
content = content.replace('let optional: Vec<&LootEntry> =', 'let optional: Vec<&LootEntryDef> =')
# Fix init in LootTableEditorState::new()
content = content.replace('s.tables = vec![LootTableDef::goblin_loot(), LootTableDef::dragon_hoard()];',
                           's.loot_table_defs = vec![LootTableDef::goblin_loot(), LootTableDef::dragon_hoard()];')

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'w', encoding='utf-8') as f:
    f.write(content)
print("Fixed inventory_system.rs")

# Fix world_gen.rs - add Default impl for RiverNetwork
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Add Default to RiverNetwork derive
content = content.replace(
    '#[derive(Clone, Debug, Serialize, Deserialize)]\npub struct RiverNetwork {',
    '#[derive(Clone, Debug, Serialize, Deserialize, Default)]\npub struct RiverNetwork {'
)

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'w', encoding='utf-8') as f:
    f.write(content)
print("Fixed world_gen.rs")
