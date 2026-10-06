use ridl_rt::trace::TraceContext;

#[test]
fn a_trace_context_is_25_bytes_and_its_option_26() {
    assert_eq!(core::mem::size_of::<TraceContext>(), 25);
    assert_eq!(core::mem::size_of::<Option<TraceContext>>(), 26);
}
