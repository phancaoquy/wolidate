use std::io::{Read, Seek};
use std::sync::Arc;

/// A seekable byte source.
///
/// Windows implements this with `std::fs::File`; Android resolves a
/// `content://` URI to a file descriptor first and then wraps it in
/// `std::fs::File`. The trait stays deliberately small so the core never
/// needs to know which platform opened the file.
pub trait BookSource: Read + Seek + Send + Sync {}

impl<T: Read + Seek + Send + Sync> BookSource for T {}

/// Shared, type-erased source used by the host's index cache.
pub type SharedSource = Arc<dyn BookSource>;

/// Return the size of a source without changing its final position.
pub fn source_len(source: &mut (impl Read + Seek)) -> std::io::Result<u64> {
    let current = source.stream_position()?;
    let len = source.seek(std::io::SeekFrom::End(0))?;
    source.seek(std::io::SeekFrom::Start(current))?;
    Ok(len)
}
