use std::path::PathBuf;

pub struct FileTransaction {
    changes: Vec<FileChange>,
}

enum FileChange {
    AddFile(PathBuf),
}

impl FileTransaction {
    fn new() -> Self {
        Self { changes: vec![] }
    }

    fn add_file(&mut self, path: PathBuf) {
        self.changes.push(FileChange::AddFile(path));
    }
}
