#[allow(dead_code)]
mod toolchain_support;
use ridl_cli::toolchain::install;
use ridl_core::release::parse_release;
use std::{
    fs,
    io::{BufRead, BufReader},
    net::TcpListener,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use toolchain_support as support;

#[test]
#[ignore = "requires a local TCP listener; run by just install-check outside the sandbox"]
fn toolchain_ureq_path_verifies_and_installs() {
    let assets = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let dir = support::fixture(assets.path(), &support::regular_files());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let files = assets.path().to_path_buf();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut paths = Vec::new();
        while paths.len() < 4 {
            assert!(Instant::now() < deadline, "HTTP fixture timed out");
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap().to_string();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            support::respond(
                &mut stream,
                &fs::read(files.join(path.trim_start_matches('/'))).unwrap(),
            );
            paths.push(path);
        }
        paths
    });
    let p = install(
        root.path(),
        parse_release("0.23.0").unwrap(),
        support::TARGET,
        &base,
    )
    .unwrap();
    support::assert_complete(&p);
    support::assert_no_stage(root.path());
    let bad = TempDir::new().unwrap();
    fs::write(
        dir.join(format!("{}.sha256", support::archive_name())),
        format!("{}  {}\n", "0".repeat(64), support::archive_name()),
    )
    .unwrap();
    assert!(
        install(
            bad.path(),
            parse_release("0.23.0").unwrap(),
            support::TARGET,
            &base
        )
        .is_err()
    );
    assert!(!bad.path().join("versions/0.23.0").exists());
    support::assert_no_stage(bad.path());
    let paths = server.join().unwrap();
    let archive = format!("/editor-v0.23.0/{}", support::archive_name());
    assert_eq!(
        paths,
        [
            archive.clone(),
            format!("{archive}.sha256"),
            archive.clone(),
            format!("{archive}.sha256")
        ]
    );
}
