//! Undo/redo command history.

use std::collections::VecDeque;

/// A recorded command that can be undone/redone.
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub data: CommandData,
}

#[derive(Debug, Clone)]
pub enum CommandData {
    PlaceNode { node_id: u32 },
    RemoveNode { node_id: u32 },
    MoveNodes { node_ids: Vec<u32>, delta: glam::Vec3 },
    ChangeProperty { node_id: u32, property: String, old_value: String, new_value: String },
}

pub struct CommandHistory {
    undo_stack: VecDeque<Command>,
    redo_stack: VecDeque<Command>,
    max_size: usize,
}

impl CommandHistory {
    pub fn new(max_size: usize) -> Self {
        Self { undo_stack: VecDeque::new(), redo_stack: VecDeque::new(), max_size }
    }

    pub fn push(&mut self, cmd: Command) {
        self.redo_stack.clear();
        self.undo_stack.push_back(cmd);
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.pop_front();
        }
    }

    pub fn undo(&mut self) -> Option<String> {
        let cmd = self.undo_stack.pop_back()?;
        let name = cmd.name.clone();
        self.redo_stack.push_back(cmd);
        Some(name)
    }

    pub fn redo(&mut self) -> Option<String> {
        let cmd = self.redo_stack.pop_back()?;
        let name = cmd.name.clone();
        self.undo_stack.push_back(cmd);
        Some(name)
    }

    pub fn undo_count(&self) -> usize { self.undo_stack.len() }
    pub fn redo_count(&self) -> usize { self.redo_stack.len() }
    pub fn clear(&mut self) { self.undo_stack.clear(); self.redo_stack.clear(); }
}
