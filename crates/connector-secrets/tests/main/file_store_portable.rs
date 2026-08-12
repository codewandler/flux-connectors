//! The durable store is part of the public surface on every supported platform.

use connector_secrets::FileStore;

#[test]
fn file_store_is_unconditionally_public() {
    assert!(std::mem::size_of::<FileStore>() > 0);
}
