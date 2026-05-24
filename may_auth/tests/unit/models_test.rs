

#[test]
fn test_admin_seed_hash() {
    let hash = "$2b$12$q/phIyk4Tw0VsbiBWgo29ea2iUQ031y7gpJNhri4P/pljfEKX4YPq";
    let valid = bcrypt::verify("changeme", hash);
    assert!(matches!(valid, Ok(true)), "The bcrypt hash should match 'changeme'");
}
