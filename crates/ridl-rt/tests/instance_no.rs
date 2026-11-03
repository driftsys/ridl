use ridl_rt::contract::InstanceNo;

#[test]
fn positive_instance_bounds() {
    const FIRST: Option<InstanceNo> = InstanceNo::new(1);
    const LAST: Option<InstanceNo> = InstanceNo::new(i32::MAX);
    const FIRST_NUMBER: i32 = match FIRST {
        Some(n) => n.get(),
        None => panic!("positive instance"),
    };
    assert_eq!(FIRST_NUMBER, 1);
    assert_eq!(LAST.unwrap().get(), i32::MAX);
    for n in [0, -1, -i32::MAX, i32::MIN] {
        assert_eq!(InstanceNo::new(n), None);
    }
}

#[cfg(feature = "debug")]
#[test]
fn debug_identity_partition() {
    const FIRST: InstanceNo = match InstanceNo::new(1) {
        Some(n) => n,
        None => panic!("positive instance"),
    };
    const TWIN: InstanceNo = match FIRST.twin() {
        Some(n) => n,
        None => panic!("twin of a positive instance"),
    };
    const TWIN_NUMBER: i32 = TWIN.get();
    const IS_TWIN: bool = TWIN.is_twin();
    const IS_LISTENER: bool = InstanceNo::LISTENER.is_listener();
    assert_eq!(TWIN_NUMBER, -1);
    assert_eq!((IS_TWIN, IS_LISTENER), (true, true));
    for (n, expected) in [(1, -1), (2, -2), (i32::MAX, -2_147_483_647)] {
        let positive = InstanceNo::new(n).unwrap();
        let twin = positive.twin().unwrap();
        assert_eq!(twin.get(), expected);
        assert!(!positive.is_twin());
        assert!(!positive.is_listener());
        assert!(twin.is_twin());
        assert!(!twin.is_listener());
        assert_eq!(twin.twin(), None);
    }
    let listener = InstanceNo::LISTENER;
    assert_eq!(listener.get(), i32::MIN);
    assert!(!listener.is_twin());
    assert!(listener.is_listener());
    assert_eq!(listener.twin(), None);
}
