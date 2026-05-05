//! Backend-specific cache value types.

pub trait BackendTypes: 'static {
    type Picture: Clone;
    type Image: Clone;
    type GlyphPath: Clone;
    type GlyphImage: Clone;
}

#[cfg(test)]
mod tests {
    use super::BackendTypes;

    #[test]
    fn associated_types_resolve_for_marker_struct() {
        struct MockBackend;
        impl BackendTypes for MockBackend {
            type Picture = Vec<u8>;
            type Image = Vec<u8>;
            type GlyphPath = String;
            type GlyphImage = Vec<u8>;
        }

        fn make_picture<B: BackendTypes<Picture = Vec<u8>>>() -> B::Picture {
            vec![1, 2, 3]
        }
        let picture = make_picture::<MockBackend>();
        assert_eq!(picture, vec![1, 2, 3]);
    }
}
