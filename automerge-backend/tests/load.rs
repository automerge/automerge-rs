use automerge_backend::Backend;

#[test]
fn test_load_index_out_of_bounds() {
    // these are just random bytes
    let bytes = vec![133, 111, 74, 131, 0, 46, 128, 0];
    let _ = Backend::load(bytes);
}
