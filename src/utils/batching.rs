pub fn chunk_vec<T: Clone>(items: Vec<T>, chunk_size: usize) -> Vec<Vec<T>> {
    if chunk_size == 0 {
        return Vec::new();
    }

    items
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}
