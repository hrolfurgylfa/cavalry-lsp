#![feature(let_chains)]

use std::pin::Pin;
use std::time;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tower_lsp::jsonrpc;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod vfs;
use vfs::VFS;

mod fmt;
use fmt::{format_in_python, format_to_text_edits};

#[derive(Debug)]
struct Backend {
    client: Client,
    vfs: VFS,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        if let Some(client_info) = params.client_info {
            println!(
                "Hello {} v{}",
                client_info.name,
                client_info.version.unwrap_or_else(|| "?".to_owned())
            );
        } else {
            println!("Hello anonymous editor");
        }

        pyo3::prepare_freethreaded_python();

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                code_lens_provider: None,
                document_formatting_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..Default::default()
                    },
                )),
                position_encoding: Some(PositionEncodingKind::UTF8),
                ..Default::default()
            },
            server_info: Some(ServerInfo::default()),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        println!("Formatting document: {:?}", params.text_document);
        let start_timer = time::Instant::now();

        let text = self.vfs.get_doc(params.text_document).unwrap().text;
        let formatted = format_in_python(text.clone());
        let changes = format_to_text_edits(&text, &formatted);

        println!("Format run in in: {}ms", start_timer.elapsed().as_millis());
        Ok(Some(changes))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        println!("Did open");
        self.vfs.add_doc(params.text_document);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        println!("Did change");
        self.vfs
            .apply_changes(params.text_document, params.content_changes);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        println!("Did close");
        self.vfs.close_doc(params.text_document);
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        println!("Goodbye");
        Ok(())
    }
}

enum RunType {
    Std,
    Tcp,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let debug = true;
    let io_type = match debug {
        true => RunType::Tcp,
        false => RunType::Std,
    };

    let (reader, writer): (Pin<Box<dyn AsyncRead>>, Pin<Box<dyn AsyncWrite>>) = match io_type {
        RunType::Tcp => {
            let listener = TcpListener::bind("127.0.0.1:7325").await?;
            let (stream, _) = listener.accept().await?;
            let (input, output) = stream.into_split().into();
            (Box::pin(input), Box::pin(output))
        }
        RunType::Std => {
            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();
            (Box::pin(stdin), Box::pin(stdout))
        }
    };

    let (service, socket) = LspService::new(|client| Backend {
        client,
        vfs: VFS::default(),
    });
    Server::new(reader, writer, socket).serve(service).await;
    Ok(())
}
