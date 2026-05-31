use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Select,
    Delete,
}

#[derive(Debug, Clone)]
pub struct UndoTransaction {
    pub media_id: String,
    pub operation: OperationType,
    // Store relative paths (from project root / intake folder) or absolute paths
    // We store absolute paths of the files inside the target folder (selected/deleted)
    // and where they should go back (intake folder)
    pub moved_files: Vec<(PathBuf, PathBuf)>, // Vec<(CurrentPath, TargetPath)>
}

pub struct UndoManager {
    stack: Vec<UndoTransaction>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, transaction: UndoTransaction) {
        self.stack.push(transaction);
    }

    pub fn pop(&mut self) -> Option<UndoTransaction> {
        self.stack.pop()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Reverses the last select/delete operation, moving files back.
    pub fn undo_last(&mut self) -> Result<Option<UndoTransaction>, String> {
        let transaction = match self.pop() {
            Some(t) => t,
            None => return Ok(None),
        };

        // Move all files in this transaction back to their original paths
        for (current_path, original_path) in &transaction.moved_files {
            if !current_path.exists() {
                // If the file was somehow moved or deleted externally, return error and abort undo
                return Err(format!(
                    "Cannot undo: file not found in selected/deleted folder: {:?}",
                    current_path
                ));
            }

            // Ensure destination parent directory exists (it should be the intake folder)
            if let Some(parent) = original_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            fs::rename(current_path, original_path)
                .map_err(|e| format!("Failed to move file back during undo: {}", e))?;
        }

        Ok(Some(transaction))
    }
}
