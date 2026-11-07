use encoding_bench::args::parse_args;

fn flags(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn vary_needs_out() {
    assert!(parse_args(&flags(&["--vary"])).is_err());
}

#[test]
fn vary_with_out_is_set() {
    let a = parse_args(&flags(&["--vary", "--out", "x"])).unwrap();
    assert!(a.vary);
    assert_eq!(a.out.as_deref(), Some("x"));
}

#[test]
fn vary_is_off_by_default() {
    assert!(!parse_args(&flags(&["--out", "x"])).unwrap().vary);
    assert!(!parse_args(&[]).unwrap().vary);
}

#[test]
fn only_and_paced_are_read_and_paced_needs_out() {
    let a = parse_args(&flags(&["--only", "big", "--out", "x", "--paced"])).unwrap();
    assert_eq!(a.only.as_deref(), Some("big"));
    assert!(a.paced);
    assert!(parse_args(&flags(&["--paced"])).is_err());
    assert!(parse_args(&flags(&["--bogus"])).is_err());
    assert!(parse_args(&flags(&["--duration", "0"])).is_err());
}
