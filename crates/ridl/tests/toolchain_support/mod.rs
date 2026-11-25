use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const TARGET: &str = if cfg!(windows) {
    "x86_64-pc-windows-msvc"
} else {
    "aarch64-apple-darwin"
};
pub const BINARY: &[u8] = b"fixture executable bytes\n";
pub const LICENSE: &[u8] = b"fixture license bytes\n";
pub const NOTICES: &[u8] = b"fixture third party notice bytes\n";
pub fn binary_name() -> &'static str {
    if cfg!(windows) { "ridl.exe" } else { "ridl" }
}
pub fn archive_name() -> String {
    format!("ridl-{TARGET}.tar.gz")
}
pub fn regular_files() -> Vec<(&'static str, u8, &'static [u8])> {
    vec![
        (binary_name(), b'0', BINARY),
        ("LICENSE", b'0', LICENSE),
        ("THIRD-PARTY-NOTICES.txt", b'0', NOTICES),
    ]
}
pub fn fixture(base: &Path, entries: &[(&str, u8, &[u8])]) -> PathBuf {
    let dir = base.join("editor-v0.23.0");
    fs::create_dir_all(&dir).unwrap();
    let zip = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(zip);
    for (path, kind, bytes) in entries {
        let mut h = tar::Header::new_gnu();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o644);
        h.set_entry_type(tar::EntryType::new(*kind));
        // Raw headers let negative controls represent paths the safe builder refuses.
        h.as_mut_bytes()[..100].fill(0);
        h.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        if matches!(kind, b'1' | b'2') {
            h.set_link_name("outside").unwrap();
        }
        h.set_cksum();
        tar.append(&h, *bytes).unwrap();
    }
    let bytes = tar.into_inner().unwrap().finish().unwrap();
    write_asset(&dir, &bytes);
    dir
}
pub fn write_asset(dir: &Path, bytes: &[u8]) {
    let archive = archive_name();
    fs::write(dir.join(&archive), bytes).unwrap();
    fs::write(
        dir.join(format!("{archive}.sha256")),
        format!("{:x}  {archive}\n", Sha256::digest(bytes)),
    )
    .unwrap();
}
pub fn source(base: &Path) -> String {
    format!("file://{}", base.display())
}
pub fn assert_complete(binary: &Path) {
    let dir = binary.parent().unwrap();
    assert_eq!(fs::read(binary).unwrap(), BINARY);
    assert_eq!(fs::read(dir.join("LICENSE")).unwrap(), LICENSE);
    assert_eq!(
        fs::read(dir.join("THIRD-PARTY-NOTICES.txt")).unwrap(),
        NOTICES
    );
    assert_eq!(fs::read(dir.join(".complete")).unwrap(), b"");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(binary).unwrap().permissions().mode() & 0o111,
            0o111
        );
    }
}
pub fn assert_no_stage(root: &Path) {
    let own: Vec<_> = fs::read_dir(root.join("versions"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n.to_string_lossy().starts_with(".tmp-"))
        .collect();
    assert!(own.is_empty(), "{own:?}");
}

// A bounded fixture server is used only by the ignored HTTP integration test.
pub fn respond(stream: &mut impl Write, body: &[u8]) {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
}
