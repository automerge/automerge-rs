use automerge_backend::{Backend, Change};

// This test reproduces issue 95 (https://github.com/automerge/automerge-rs/issues/95)
// where compressed changes were losing their header during decompression such
// that when the compressed changes were written out again they were invalid.
#[test]
fn test_deflate_correctly() {
    let init_change: Vec<u8> = vec![
        133, 111, 74, 131, 252, 38, 106, 255, 2, 195, 2, 117, 143, 189, 74, 4, 49, 16, 128, 147,
        189, 61, 239, 7, 185, 83, 196, 43, 101, 26, 75, 183, 178, 179, 17, 181, 177, 17, 27, 181,
        14, 217, 120, 55, 144, 77, 150, 73, 178, 156, 87, 172, 133, 224, 3, 88, 248, 58, 98, 227,
        29, 86, 98, 167, 22, 118, 190, 133, 96, 86, 177, 176, 48, 153, 129, 249, 253, 102, 134,
        173, 124, 108, 220, 111, 221, 188, 239, 14, 239, 6, 184, 57, 111, 157, 84, 156, 127, 190,
        190, 93, 45, 13, 14, 13, 122, 20, 26, 103, 194, 163, 53, 172, 207, 219, 201, 112, 181, 179,
        54, 90, 223, 217, 238, 239, 45, 159, 246, 207, 94, 120, 217, 98, 201, 19, 103, 44, 153, 37,
        173, 180, 189, 212, 89, 240, 110, 221, 110, 177, 222, 188, 137, 177, 228, 146, 49, 254,
        171, 53, 235, 61, 112, 206, 146, 186, 35, 3, 57, 75, 174, 43, 39, 168, 115, 82, 38, 230,
        255, 179, 83, 175, 166, 158, 45, 120, 146, 250, 139, 82, 37, 252, 251, 69, 119, 218, 208,
        227, 79, 31, 57, 239, 198, 252, 168, 190, 229, 215, 252, 192, 26, 37, 161, 176, 90, 163,
        131, 137, 50, 17, 66, 232, 129, 208, 5, 151, 193, 49, 9, 229, 148, 241, 80, 41, 163, 76,
        188, 201, 65, 161, 124, 112, 32, 60, 120, 75, 81, 160, 12, 186, 66, 35, 8, 42, 65, 216,
        244, 252, 16, 43, 244, 66, 129, 37, 137, 224, 84, 14, 185, 213, 177, 150, 130, 167, 80,
        128, 8, 50, 118, 102, 112, 20, 180, 22, 5, 52, 183, 69, 164, 22, 18, 13, 10, 80, 36, 124,
        6, 251, 36, 28, 4, 237, 9, 37, 170, 56, 21, 65, 5, 240, 129, 202, 63, 107, 158, 19, 154,
        49, 70, 74, 86, 10, 18, 99, 18, 229, 36, 183, 50, 20, 113, 229, 103, 206, 190, 0,
    ];
    let change: Change = Change::from_bytes(init_change.clone()).unwrap();
    let mut backend = Backend::init();
    backend.apply_changes(vec![change.clone()]).unwrap();

    let change_back = backend.get_changes(&[]);
    assert_eq!(change_back[0].raw_bytes().to_vec(), init_change);
}
