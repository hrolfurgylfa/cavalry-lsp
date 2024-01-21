use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tower_lsp::lsp_types::{
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    VersionedTextDocumentIdentifier,
};

#[derive(Debug, Clone, Default)]
pub struct VFS {
    map: Arc<RwLock<HashMap<String, Document>>>,
}

impl VFS {
    pub fn add_doc(&self, doc: TextDocumentItem) {
        let uri = doc.uri.as_str().to_owned();
        let version = doc.version;
        let content = doc.text;

        let arc = self.map.clone();
        let mut vfs = arc.write().unwrap();
        let prev = vfs.insert(uri, Document::new(content, version));
        assert!(prev.is_none());
    }

    pub fn apply_changes(
        &self,
        doc: VersionedTextDocumentIdentifier,
        changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        let arc = self.map.clone();
        let mut vfs = arc.write().unwrap();
        let file = vfs.get_mut(doc.uri.as_str()).unwrap();

        for change in changes {
            assert_eq!(change.range, None);
            assert!(doc.version > file.version);
            file.version = doc.version;
            file.text = change.text;
        }
    }

    pub fn close_doc(&self, doc: TextDocumentIdentifier) {
        let arc = self.map.clone();
        let mut vfs = arc.write().unwrap();
        let removed = vfs.remove(doc.uri.as_str());
        assert!(removed.is_none());
    }

    pub fn get_doc(&self, doc: TextDocumentIdentifier) -> Option<Document> {
        let arc = self.map.clone();
        let vfs = arc.read().unwrap();
        vfs.get(doc.uri.as_str()).map(|d| d.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub version: i32,
}

impl Document {
    pub fn new(text: String, version: i32) -> Document {
        Document { text, version }
    }
}
