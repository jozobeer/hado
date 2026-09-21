//! §58: the pure parts of the LSP server. The transport and the position
//! encoding are functions, so they are tested directly instead of through a
//! subprocess: the conversions must be right for `あ` and `😀`, the framing
//! must count bytes, and `initialize` must advertise one encoding.

use hado::lsp::{
    decode_position, encode_position, frame_message, initialize_result, library_root,
    normalize_path, read_message, scalar_to_byte, scalar_to_utf16, units_before, uri_to_path,
    PositionEncoding,
};
use std::io::Cursor;
use std::path::PathBuf;

const BMP: &str = "あ";
const NON_BMP: &str = "😀";

#[test]
fn units_count_scalars_in_each_encoding() {
    // `あ` is 3 UTF-8 bytes and 1 UTF-16 unit; `😀` is 4 bytes and a surrogate
    // pair, so UTF-16 counts it twice.
    let line = format!("{BMP}{NON_BMP}x");
    assert_eq!(units_before(&line, 0, PositionEncoding::Utf8), 0);
    assert_eq!(units_before(&line, 1, PositionEncoding::Utf8), 3);
    assert_eq!(units_before(&line, 2, PositionEncoding::Utf8), 7);
    assert_eq!(units_before(&line, 3, PositionEncoding::Utf8), 8);
    assert_eq!(scalar_to_byte(&line, 2), 7);

    assert_eq!(units_before(&line, 0, PositionEncoding::Utf16), 0);
    assert_eq!(units_before(&line, 1, PositionEncoding::Utf16), 1);
    assert_eq!(units_before(&line, 2, PositionEncoding::Utf16), 3);
    assert_eq!(units_before(&line, 3, PositionEncoding::Utf16), 4);
    assert_eq!(scalar_to_utf16(&line, 2), 3);

    assert_eq!(units_before(&line, 2, PositionEncoding::Utf32), 2);
    assert_eq!(units_before(&line, 3, PositionEncoding::Utf32), 3);

    // A count past the end of the line stops at its end.
    assert_eq!(units_before(&line, 99, PositionEncoding::Utf8), 8);
}

#[test]
fn positions_round_trip_through_each_encoding() {
    let source = format!("{BMP}{NON_BMP}x\ndef target = 1");
    for encoding in [
        PositionEncoding::Utf8,
        PositionEncoding::Utf16,
        PositionEncoding::Utf32,
    ] {
        for col in 1..=4 {
            let (line, character) = encode_position(encoding, &source, 1, col);
            assert_eq!(line, 0);
            assert_eq!(
                decode_position(encoding, &source, line, character),
                (1, col)
            );
        }
    }
    // The same scalar position is a different character per encoding.
    assert_eq!(
        encode_position(PositionEncoding::Utf16, &source, 1, 3),
        (0, 3)
    );
    assert_eq!(
        encode_position(PositionEncoding::Utf8, &source, 1, 3),
        (0, 7)
    );
    assert_eq!(
        encode_position(PositionEncoding::Utf32, &source, 1, 3),
        (0, 2)
    );
    assert_eq!(
        encode_position(PositionEncoding::Utf16, &source, 2, 5),
        (1, 4)
    );
}

#[test]
fn negotiate_position_encoding() {
    let all = vec![
        "utf-8".to_string(),
        "utf-16".to_string(),
        "utf-32".to_string(),
    ];
    assert_eq!(
        PositionEncoding::negotiate(Some(all.as_slice())),
        PositionEncoding::Utf8
    );
    let utf32_first = vec!["utf-32".to_string(), "utf-8".to_string()];
    assert_eq!(
        PositionEncoding::negotiate(Some(utf32_first.as_slice())),
        PositionEncoding::Utf32
    );
    let utf16_first = vec!["utf-16".to_string(), "utf-8".to_string()];
    assert_eq!(
        PositionEncoding::negotiate(Some(utf16_first.as_slice())),
        PositionEncoding::Utf16
    );
    // Absent, or a list with none this server knows, defaults to utf-16.
    assert_eq!(PositionEncoding::negotiate(None), PositionEncoding::Utf16);
    let unknown = vec!["ascii".to_string()];
    assert_eq!(
        PositionEncoding::negotiate(Some(unknown.as_slice())),
        PositionEncoding::Utf16
    );
}

#[test]
fn initialize_advertises_one_position_encoding() {
    let result = initialize_result(PositionEncoding::Utf16);
    let capabilities = &result["capabilities"];
    assert_eq!(capabilities["textDocumentSync"].as_u64(), Some(1));
    assert_eq!(capabilities["hoverProvider"].as_bool(), Some(true));
    assert_eq!(capabilities["codeActionProvider"].as_bool(), Some(true));
    // One string, not an array, and not under `general`.
    assert_eq!(capabilities["positionEncoding"].as_str(), Some("utf-16"));
    assert!(capabilities.get("positionEncodings").is_none());
    assert!(capabilities.get("general").is_none());
    assert_eq!(
        initialize_result(PositionEncoding::Utf8)["capabilities"]["positionEncoding"].as_str(),
        Some("utf-8")
    );
}

#[test]
fn framing_round_trips_a_multibyte_body() {
    let payload =
        r#"{"jsonrpc":"2.0","id":1,"method":"textDocument/hover","params":{"text":"あ😀"}}"#;
    let framed = frame_message(payload);
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    assert!(framed.starts_with(header.as_bytes()));
    // The header counts bytes, so it is larger than the character count.
    assert!(payload.chars().count() < payload.len());
    let mut cursor = Cursor::new(framed);
    assert_eq!(read_message(&mut cursor).unwrap().as_deref(), Some(payload));
    // Reading past the end is end of input, not an error.
    assert!(read_message(&mut cursor).unwrap().is_none());
}

#[test]
fn framing_accepts_other_headers() {
    let framed =
        b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\nContent-Length: 2\r\n\r\n{}";
    assert_eq!(
        read_message(&mut Cursor::new(framed)).unwrap().as_deref(),
        Some("{}")
    );
}

#[test]
fn uri_to_path_decodes_file_urls() {
    assert_eq!(
        uri_to_path("file:///tmp/a%20b.hd"),
        Some(PathBuf::from("/tmp/a b.hd"))
    );
    assert_eq!(
        uri_to_path("file:///tmp/あ.hd"),
        Some(PathBuf::from("/tmp/あ.hd"))
    );
    assert_eq!(
        uri_to_path("file://localhost/tmp/x.hd"),
        Some(PathBuf::from("/tmp/x.hd"))
    );
    assert_eq!(uri_to_path("untitled:foo"), None);
}

#[test]
fn normalize_path_removes_dot_components() {
    assert_eq!(
        normalize_path(std::path::Path::new("/tmp/./a/../b.hd")),
        PathBuf::from("/tmp/b.hd")
    );
}

#[test]
fn library_root_walks_up_to_the_directory_declaring_library() {
    // §58.5: with no workspace folder and no `rootUri`, the entry candidates for
    // a `.hdi` opened on its own come from the outermost ANCESTOR that declares
    // `library`. A nested contract must not be rooted at its own directory, or
    // no `.hd` candidate is found and the contract gets no diagnostics.
    let base = std::env::temp_dir().join(format!("hado-lsp-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("api")).unwrap();
    std::fs::write(base.join("root.hd"), "library\n").unwrap();
    std::fs::write(base.join("main.hd"), "def use (x: Int) => x\n").unwrap();
    std::fs::write(
        base.join("api/store.hdi"),
        "module M\ndef f (x: Int): Int\n",
    )
    .unwrap();

    let root = library_root(&base.join("api/store.hdi"));
    assert_eq!(
        root, base,
        "the root must be the ancestor declaring `library`, not the contract's own directory"
    );

    // With no `library` anywhere above it, the contract's own directory is used.
    std::fs::remove_file(base.join("root.hd")).unwrap();
    let root = library_root(&base.join("api/store.hdi"));
    assert_eq!(root, base.join("api"));

    let _ = std::fs::remove_dir_all(&base);
}
