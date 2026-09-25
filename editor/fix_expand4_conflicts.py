
# Fix world_gen.rs conflicts after line 10800
with open(r'C:\proof-engine\editor\src\world_gen.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

split = 10800
before = ''.join(lines[:split])
after = ''.join(lines[split:])

# Rename new DungeonRoom -> DungeonRoom2, DungeonTheme -> DungeonTheme2, etc.
after = after.replace('pub struct DungeonRoom {', 'pub struct DungeonRoom2 {')
after = after.replace('impl DungeonRoom {', 'impl DungeonRoom2 {')
after = after.replace(': DungeonRoom', ': DungeonRoom2')
after = after.replace('Vec<DungeonRoom>', 'Vec<DungeonRoom2>')
after = after.replace('DungeonRoom::new(', 'DungeonRoom2::new(')
after = after.replace('DungeonRoomType::', 'DungeonRoomType2::')
after = after.replace('pub enum DungeonRoomType {', 'pub enum DungeonRoomType2 {')
after = after.replace('impl DungeonRoomType {', 'impl DungeonRoomType2 {')
after = after.replace(': DungeonRoomType', ': DungeonRoomType2')

after = after.replace('pub enum DungeonTheme {', 'pub enum DungeonTheme2 {')
after = after.replace('impl DungeonTheme {', 'impl DungeonTheme2 {')
after = after.replace(': DungeonTheme', ': DungeonTheme2')
after = after.replace('DungeonTheme::', 'DungeonTheme2::')

after = after.replace('pub struct DungeonEditorState {', 'pub struct DungeonEditorState2 {')
after = after.replace('impl Default for DungeonEditorState {', 'impl Default for DungeonEditorState2 {')
after = after.replace('impl DungeonEditorState {', 'impl DungeonEditorState2 {')
after = after.replace('state: &mut DungeonEditorState)', 'state: &mut DungeonEditorState2)')
after = after.replace('DungeonEditorState {', 'DungeonEditorState2 {')
after = after.replace(': DungeonEditorState', ': DungeonEditorState2')
after = after.replace('DungeonEditorState::default()', 'DungeonEditorState2::default()')

after = after.replace('pub struct DungeonLevel {', 'pub struct DungeonLevel2 {')
after = after.replace('impl DungeonLevel {', 'impl DungeonLevel2 {')
after = after.replace(': DungeonLevel', ': DungeonLevel2')
after = after.replace('Vec<DungeonLevel>', 'Vec<DungeonLevel2>')
after = after.replace('DungeonLevel::new(', 'DungeonLevel2::new(')

after = after.replace('pub struct DungeonCorridor {', 'pub struct DungeonCorridor2 {')
after = after.replace('impl DungeonCorridor {', 'impl DungeonCorridor2 {')
after = after.replace(': DungeonCorridor', ': DungeonCorridor2')
after = after.replace('Vec<DungeonCorridor>', 'Vec<DungeonCorridor2>')
after = after.replace('DungeonCorridor::new(', 'DungeonCorridor2::new(')

after = after.replace('pub struct DungeonGenConfig {', 'pub struct DungeonGenConfig2 {')
after = after.replace('impl Default for DungeonGenConfig {', 'impl Default for DungeonGenConfig2 {')
after = after.replace('DungeonGenConfig {', 'DungeonGenConfig2 {')
after = after.replace('DungeonGenConfig::default()', 'DungeonGenConfig2::default()')
after = after.replace(': DungeonGenConfig', ': DungeonGenConfig2')
after = after.replace('cfg: &DungeonGenConfig', 'cfg: &DungeonGenConfig2')

after = after.replace('pub fn generate_dungeon(', 'pub fn generate_dungeon2(')
after = after.replace('generate_dungeon(&cfg)', 'generate_dungeon2(&cfg)')

after = after.replace('pub enum SculptMode {', 'pub enum SculptMode2 {')
after = after.replace('impl SculptMode {', 'impl SculptMode2 {')
after = after.replace(': SculptMode', ': SculptMode2')
after = after.replace('SculptMode::', 'SculptMode2::')

after = after.replace('pub struct WorldExportState {', 'pub struct WorldExportState2 {')
after = after.replace('impl Default for WorldExportState {', 'impl Default for WorldExportState2 {')
after = after.replace('impl WorldExportState {', 'impl WorldExportState2 {')
after = after.replace('state: &mut WorldExportState)', 'state: &mut WorldExportState2)')
after = after.replace('WorldExportState {', 'WorldExportState2 {')
after = after.replace(': WorldExportState', ': WorldExportState2')
after = after.replace('WorldExportState::default()', 'WorldExportState2::default()')

after = after.replace('pub struct WorldExportOptions {', 'pub struct WorldExportOptions2 {')
after = after.replace('impl Default for WorldExportOptions {', 'impl Default for WorldExportOptions2 {')
after = after.replace('WorldExportOptions {', 'WorldExportOptions2 {')
after = after.replace('WorldExportOptions::default()', 'WorldExportOptions2::default()')
after = after.replace(': WorldExportOptions', ': WorldExportOptions2')

with open(r'C:\proof-engine\editor\src\world_gen.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed world_gen.rs expansion4 conflicts")

# Fix inventory_system.rs - EquipSlot already exists at line 289
with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'r', encoding='utf-8') as f:
    lines = f.readlines()

split = 8200
before = ''.join(lines[:split])
after = ''.join(lines[split:])

after = after.replace('pub enum EquipSlot {', 'pub enum EquipSlotFull {')
after = after.replace('impl EquipSlot {', 'impl EquipSlotFull {')
after = after.replace(': EquipSlot', ': EquipSlotFull')
after = after.replace('Vec<EquipSlot>', 'Vec<EquipSlotFull>')
after = after.replace('EquipSlot::', 'EquipSlotFull::')
after = after.replace('for slot in EquipSlot::all()', 'for slot in EquipSlotFull::all()')
after = after.replace('Option<EquipSlot>', 'Option<EquipSlotFull>')

with open(r'C:\proof-engine\editor\src\inventory_system.rs', 'w', encoding='utf-8') as f:
    f.write(before + after)
print("Fixed inventory_system.rs expansion4 conflicts")
