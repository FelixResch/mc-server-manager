//! Module to track IO operations during installation and updates to perform roll-backs.

use std::path::PathBuf;

/// Stores changes made to the file system. New changes are stored at the back and on roll-back
/// the newest changes are undone first.
///
/// File transactions should be concluded once a safe-point is reached. In future versions the transaction
/// will store the changes in a file to roll-back failed installations that have panicked.
#[allow(dead_code)]
pub struct FileTransaction {
    /// The changes made during a transaction
    changes: Vec<FileChange>,
}

/// Different types of file changes with their roll-back strategy
#[allow(dead_code)]
enum FileChange {
    /// Add a new file/dir at PathBuf.
    ///
    /// Rollback: Delete file
    AddFile(PathBuf),
}

#[allow(dead_code)]
impl FileTransaction {
    /// Create a new file transaction
    fn new() -> Self {
        Self { changes: vec![] }
    }

    /// Add a new file to the recorded file system changes
    fn add_file(&mut self, path: PathBuf) {
        self.changes.push(FileChange::AddFile(path));
    }
}
