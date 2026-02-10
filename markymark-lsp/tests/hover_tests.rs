//! LSP hover handler integration tests.

use markymark_core::DocumentUri;
use markymark_lsp::server::create_service;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::LanguageServer;

/// Helper: create a Backend pre-loaded with test documents.
///
/// Returns the service, socket, main URI, and other-page URI.
async fn setup_workspace() -> (
    tower_lsp_server::LspService<markymark_lsp::server::Backend>,
    tower_lsp_server::ClientSocket,
    Uri,
    Uri,
) {
    let (service, socket) = create_service();
    let backend = service.inner();

    let uri_main: Uri = "file:///workspace/main.md".parse().unwrap();
    let uri_other: Uri = "file:///workspace/other-page.md".parse().unwrap();

    let main_text = concat!(
        "# Main Document\n",
        "\n",
        "## Introduction\n",
        "\n",
        "See [[other-page]] for details.\n",
        "\n",
        "Also check [[other-page#details]] and [[#introduction]].\n",
        "\n",
        "A markdown link: [intro](#introduction)\n",
    );

    let other_text = concat!(
        "# Other Page\n",
        "\n",
        "## Details\n",
        "\n",
        "Some detailed content here.\n",
    );

    // Populate state via the Backend's state handle
    {
        let mut state = backend.state().write().await;
        let core_main = DocumentUri::new("file:///workspace/main.md").unwrap();
        let core_other = DocumentUri::new("file:///workspace/other-page.md").unwrap();
        state.open_document(core_main, main_text.to_string());
        state.open_document(core_other, other_text.to_string());
    }

    (service, socket, uri_main, uri_other)
}

#[tokio::test]
async fn test_hover_on_heading() {
    // Cursor on heading -> should return markdown with heading info
    // Line 2: "## Introduction"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(2, 5), // on "Introduction"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on heading should return hover info"
    );
    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(
                markup.value.contains("Introduction"),
                "hover content should mention the heading text"
            );
        }
        HoverContents::Scalar(MarkedString::String(s)) => {
            assert!(s.contains("Introduction"));
        }
        HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
            assert!(ls.value.contains("Introduction"));
        }
        HoverContents::Array(arr) => {
            let text: String = arr
                .iter()
                .map(|m| match m {
                    MarkedString::String(s) => s.clone(),
                    MarkedString::LanguageString(ls) => ls.value.clone(),
                })
                .collect();
            assert!(text.contains("Introduction"));
        }
    }
}

#[tokio::test]
async fn test_hover_on_wiki_link() {
    // Cursor on [[other-page]] -> should return info about target
    // Line 4: "See [[other-page]] for details."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 8), // inside "other-page"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link should return hover info about the target"
    );
}

#[tokio::test]
async fn test_hover_on_plain_text_returns_none() {
    // Cursor on plain text -> None
    // Line 4: "See [[other-page]] for details." -- on "for"
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(4, 22), // on "for" in plain text
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(result.is_none(), "hover on plain text should return None");
}

#[tokio::test]
async fn test_hover_on_wiki_link_with_heading() {
    // Cursor on [[other-page#details]] -> should return hover info about the heading target
    // Line 6: "Also check [[other-page#details]] and [[#introduction]]."
    let (service, _socket, uri_main, _) = setup_workspace().await;
    let backend = service.inner();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: uri_main.clone(),
            },
            position: Position::new(6, 20), // inside "other-page#details"
        },
        work_done_progress_params: Default::default(),
    };

    let result = backend.hover(params).await.unwrap();
    assert!(
        result.is_some(),
        "hover on wiki link with heading fragment should return info"
    );
}
