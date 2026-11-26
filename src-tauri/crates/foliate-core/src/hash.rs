use crate::Result;
use std::io::{Read, Seek, SeekFrom};

/// Number of bytes read from the head and tail of a book.
pub const HASH_CHUNK: u64 = 65_536;

/// Deterministic, cheap book identity.
///
/// The hash is `BLAKE3(first 64 KiB || last 64 KiB || u64_le(file_size))`.
/// It never includes the path or URI, so the same bytes get the same id on
/// Windows and Android. Two files that differ only in their middle section
/// collide by design; v0.1 accepts that trade-off for sub-millisecond hashing.
pub fn book_hash(source: &mut (impl Read + Seek), file_size: u64) -> Result<String> {
    let mut hasher = blake3::Hasher::new();

    let head_len = file_size.min(HASH_CHUNK);
    let mut head = vec![0_u8; head_len as usize];
    source.seek(SeekFrom::Start(0))?;
    source.read_exact(&mut head)?;
    hasher.update(&head);

    let tail_start = file_size.saturating_sub(HASH_CHUNK);
    let tail_len = file_size.min(HASH_CHUNK);
    let mut tail = vec![0_u8; tail_len as usize];
    source.seek(SeekFrom::Start(tail_start))?;
    source.read_exact(&mut tail)?;
    hasher.update(&tail);

    hasher.update(&file_size.to_le_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn hash_bytes(bytes: &[u8]) -> String {
        let mut cursor = Cursor::new(bytes.to_vec());
        book_hash(&mut cursor, bytes.len() as u64).unwrap()
    }

    #[test]
    fn hash_is_64_lowercase_hex() {
        let hash = hash_bytes(b"hello");
        assert_eq!(hash.len(), 64);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn hash_is_stable_for_the_same_bytes() {
        assert_eq!(hash_bytes(b"same bytes"), hash_bytes(b"same bytes"));
    }

    #[test]
    fn hash_changes_with_size_and_tail() {
        assert_ne!(hash_bytes(b"abc"), hash_bytes(b"abcd"));
        assert_ne!(hash_bytes(b"abcX"), hash_bytes(b"abcY"));
    }

    #[test]
    fn empty_file_is_hashable() {
        let hash = hash_bytes(b"");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn overlaps_head_and_tail_for_short_files() {
        let bytes = b"a short book";
        let mut cursor = Cursor::new(bytes.to_vec());
        let hash = book_hash(&mut cursor, bytes.len() as u64).unwrap();
        let mut expected = blake3::Hasher::new();
        expected.update(bytes);
        expected.update(bytes);
        expected.update(&(bytes.len() as u64).to_le_bytes());
        assert_eq!(hash, expected.finalize().to_hex().to_string());
    }

    #[test]
    fn large_files_only_read_the_edges() {
        let mut bytes = vec![0_u8; 500_000];
        bytes[0] = 1;
        bytes[499_999] = 2;
        let mut cursor = Cursor::new(bytes);
        let hash = book_hash(&mut cursor, 500_000).unwrap();
        assert_eq!(hash.len(), 64);
    }
}
