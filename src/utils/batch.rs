/// Batch processing utilities for handling large message selections

/// Default batch size for IMAP operations
pub const DEFAULT_BATCH_SIZE: usize = 100;

/// Split UIDs into chunks for batch processing
pub fn chunk_uids(uids: &[u32], batch_size: usize) -> Vec<Vec<u32>> {
    uids.chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Process UIDs in batches, collecting results
pub async fn process_in_batches<F, Fut, T, E>(
    uids: &[u32],
    batch_size: usize,
    mut processor: F,
) -> Result<Vec<T>, E>
where
    F: FnMut(Vec<u32>) -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let chunks = chunk_uids(uids, batch_size);
    let mut results = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        let result = processor(chunk).await?;
        results.push(result);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_uids_empty() {
        let uids: Vec<u32> = vec![];
        let chunks = chunk_uids(&uids, 10);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_uids_smaller_than_batch() {
        let uids: Vec<u32> = vec![1, 2, 3];
        let chunks = chunk_uids(&uids, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], vec![1, 2, 3]);
    }

    #[test]
    fn test_chunk_uids_exact_batch() {
        let uids: Vec<u32> = vec![1, 2, 3, 4, 5];
        let chunks = chunk_uids(&uids, 5);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_chunk_uids_multiple_batches() {
        let uids: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7];
        let chunks = chunk_uids(&uids, 3);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], vec![1, 2, 3]);
        assert_eq!(chunks[1], vec![4, 5, 6]);
        assert_eq!(chunks[2], vec![7]);
    }
}
