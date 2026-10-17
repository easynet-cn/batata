//! Streaming snapshot format.
//!
//! Layout, all integers little endian:
//!
//! ```text
//! "BSNA"                magic
//! u32                   format version (2)
//! record*               see below
//! u64                   SipHash checksum of every byte above
//! ```
//!
//! A record is one of:
//!
//! ```text
//! TAG_CF     u32 name_len  name                  start a column family
//! TAG_ENTRY  u32 key_len   key  u32 val_len val  one key/value pair
//! TAG_END                                        end of records
//! ```
//!
//! The layout is deliberately record oriented so both ends can stream: the
//! writer emits entries while iterating RocksDB, and the reader applies them
//! one at a time into a `WriteBatch`. The previous format serialized a whole
//! `HashMap<String, Vec<(Vec<u8>, Vec<u8>)>>`, which forced every snapshot to
//! be materialized in memory twice (once for the map, once for the bytes).

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io;
use std::path::Path;

use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// Magic header identifying a Batata snapshot.
pub const SNAPSHOT_MAGIC: &[u8] = b"BSNA";
/// Version of the streaming record layout.
pub const SNAPSHOT_VERSION: u32 = 2;
/// Size of the trailing checksum in bytes.
pub const CHECKSUM_SIZE: usize = std::mem::size_of::<u64>();

/// Tag introducing a column family section.
const TAG_CF: u8 = 1;
/// Tag introducing a single key/value entry.
const TAG_ENTRY: u8 = 2;
/// Tag marking the end of the record stream.
const TAG_END: u8 = 3;

/// Buffer size used when streaming a snapshot in or out.
pub const IO_BUFFER_SIZE: usize = 64 * 1024;

/// Smallest possible snapshot: magic, version, end tag and checksum.
const MIN_SNAPSHOT_SIZE: u64 = (SNAPSHOT_MAGIC.len() + 4 + 1 + CHECKSUM_SIZE) as u64;

/// A record decoded from a snapshot stream.
#[derive(Debug, PartialEq, Eq)]
pub enum RecordKind {
    /// A new column family starts; the name is written into the caller's buffer.
    ColumnFamily,
    /// A key/value pair belonging to the current column family.
    Entry,
    /// End of the record stream.
    End,
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Writer for the streaming snapshot format.
pub struct SnapshotWriter<W> {
    inner: W,
    hasher: DefaultHasher,
}

impl<W: AsyncWrite + Unpin> SnapshotWriter<W> {
    /// Wrap `inner`, starting with an empty checksum accumulator.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: DefaultHasher::new(),
        }
    }

    /// Feed `buf` to both the checksum and the underlying writer.
    async fn write_hashed(&mut self, buf: &[u8]) -> io::Result<()> {
        self.hasher.write(buf);
        self.inner.write_all(buf).await
    }

    /// Write the magic header and format version.
    pub async fn write_header(&mut self) -> io::Result<()> {
        self.write_hashed(SNAPSHOT_MAGIC).await?;
        self.write_hashed(&SNAPSHOT_VERSION.to_le_bytes()).await
    }

    /// Start a new column family section.
    pub async fn start_column_family(&mut self, name: &str) -> io::Result<()> {
        self.write_hashed(&[TAG_CF]).await?;
        self.write_hashed(&(name.len() as u32).to_le_bytes())
            .await?;
        self.write_hashed(name.as_bytes()).await
    }

    /// Write one key/value pair into the current column family.
    pub async fn write_entry(&mut self, key: &[u8], value: &[u8]) -> io::Result<()> {
        self.write_hashed(&[TAG_ENTRY]).await?;
        self.write_hashed(&(key.len() as u32).to_le_bytes()).await?;
        self.write_hashed(key).await?;
        self.write_hashed(&(value.len() as u32).to_le_bytes())
            .await?;
        self.write_hashed(value).await
    }

    /// Write the end tag followed by the checksum, then flush.
    pub async fn finish(&mut self) -> io::Result<()> {
        self.write_hashed(&[TAG_END]).await?;
        let checksum = self.hasher.finish();
        // The checksum itself is not part of what it covers.
        self.inner.write_all(&checksum.to_le_bytes()).await?;
        self.inner.flush().await
    }

    /// Hand the underlying writer back so the caller can fsync the file.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

/// Reader for the streaming snapshot format.
pub struct SnapshotReader<R> {
    inner: R,
    hasher: DefaultHasher,
}

impl<R: AsyncRead + Unpin> SnapshotReader<R> {
    /// Wrap `inner`, starting with an empty checksum accumulator.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: DefaultHasher::new(),
        }
    }

    /// Read a byte, feeding it to the checksum. `Ok(None)` means clean EOF.
    async fn next_byte(&mut self) -> io::Result<Option<u8>> {
        let mut byte = [0u8; 1];
        let read = self.inner.read(&mut byte).await?;
        if read == 0 {
            return Ok(None);
        }
        self.hasher.write(&byte);
        Ok(Some(byte[0]))
    }

    /// Read exactly `len` bytes into `buf`, feeding them to the checksum.
    async fn read_hashed(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf).await?;
        self.hasher.write(buf);
        Ok(())
    }

    /// Validate the magic header and format version.
    pub async fn read_header(&mut self) -> io::Result<()> {
        let mut magic = [0u8; SNAPSHOT_MAGIC.len()];
        self.read_hashed(&mut magic).await?;
        if magic != SNAPSHOT_MAGIC {
            return Err(invalid("snapshot magic header mismatch"));
        }

        let mut version = [0u8; 4];
        self.read_hashed(&mut version).await?;
        let version = u32::from_le_bytes(version);
        if version != SNAPSHOT_VERSION {
            return Err(invalid(format!(
                "unsupported snapshot format version {version}"
            )));
        }

        Ok(())
    }

    /// Decode the next record, reusing the caller's buffers so a large
    /// snapshot does not allocate per entry.
    ///
    /// Returns `Ok(None)` at end of file, which normally happens only after
    /// [`RecordKind::End`]; a truncated snapshot is caught by
    /// [`Self::verify_checksum`].
    pub async fn next_record(
        &mut self,
        cf: &mut String,
        key: &mut Vec<u8>,
        value: &mut Vec<u8>,
    ) -> io::Result<Option<RecordKind>> {
        let Some(tag) = self.next_byte().await? else {
            return Ok(None);
        };

        match tag {
            TAG_CF => {
                let mut len = [0u8; 4];
                self.read_hashed(&mut len).await?;
                let len = u32::from_le_bytes(len) as usize;
                let mut name = vec![0u8; len];
                self.read_hashed(&mut name).await?;
                *cf = String::from_utf8(name)
                    .map_err(|_| invalid("column family name is not valid UTF-8"))?;
                Ok(Some(RecordKind::ColumnFamily))
            }
            TAG_ENTRY => {
                let mut len = [0u8; 4];
                self.read_hashed(&mut len).await?;
                let len = u32::from_le_bytes(len) as usize;
                key.clear();
                key.resize(len, 0);
                self.read_hashed(key).await?;

                let mut len = [0u8; 4];
                self.read_hashed(&mut len).await?;
                let len = u32::from_le_bytes(len) as usize;
                value.clear();
                value.resize(len, 0);
                self.read_hashed(value).await?;

                Ok(Some(RecordKind::Entry))
            }
            TAG_END => Ok(Some(RecordKind::End)),
            other => Err(invalid(format!("unknown snapshot record tag {other}"))),
        }
    }

    /// Read the trailing checksum and compare it with the bytes read so far.
    pub async fn verify_checksum(&mut self) -> io::Result<u64> {
        let mut trailer = [0u8; CHECKSUM_SIZE];
        self.inner.read_exact(&mut trailer).await?;
        let expected = u64::from_le_bytes(trailer);
        let actual = self.hasher.finish();
        if expected != actual {
            return Err(invalid(format!(
                "snapshot checksum mismatch: expected {expected:#x}, got {actual:#x}"
            )));
        }
        Ok(actual)
    }
}

/// Verify a snapshot file's checksum without buffering the whole body.
///
/// An empty file is accepted: it represents a state machine with nothing in it,
/// which is what a freshly initialized node installs.
pub async fn verify_file(path: &Path) -> io::Result<()> {
    let file = File::open(path).await?;
    let len = file.metadata().await?.len();
    if len == 0 {
        return Ok(());
    }
    if len < MIN_SNAPSHOT_SIZE {
        return Err(invalid(format!("snapshot is truncated: {len} bytes")));
    }

    let mut reader = BufReader::with_capacity(IO_BUFFER_SIZE, file);
    let mut hasher = DefaultHasher::new();
    let mut buf = vec![0u8; IO_BUFFER_SIZE];
    let mut remaining = len - CHECKSUM_SIZE as u64;

    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let read = reader.read(&mut buf[..want]).await?;
        if read == 0 {
            return Err(invalid("snapshot ended before its checksum"));
        }
        hasher.write(&buf[..read]);
        remaining -= read as u64;
    }

    let mut trailer = [0u8; CHECKSUM_SIZE];
    reader.read_exact(&mut trailer).await?;
    let expected = u64::from_le_bytes(trailer);
    let actual = hasher.finish();
    if expected != actual {
        return Err(invalid(format!(
            "snapshot checksum mismatch: expected {expected:#x}, got {actual:#x}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn writer_and_reader_round_trip() {
        let mut body = Vec::new();
        let mut writer = SnapshotWriter::new(&mut body);
        writer.write_header().await.unwrap();
        writer.start_column_family("cf_config").await.unwrap();
        writer.write_entry(b"k1", b"v1").await.unwrap();
        writer.write_entry(b"k2", b"").await.unwrap();
        writer.start_column_family("cf_users").await.unwrap();
        writer.write_entry(b"u1", b"blob").await.unwrap();
        writer.finish().await.unwrap();

        let mut reader = SnapshotReader::new(Cursor::new(body));
        reader.read_header().await.unwrap();

        let mut cf = String::new();
        let mut key = Vec::new();
        let mut value = Vec::new();
        let mut seen: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();
        let mut current = String::new();

        while let Some(kind) = reader
            .next_record(&mut cf, &mut key, &mut value)
            .await
            .unwrap()
        {
            match kind {
                RecordKind::ColumnFamily => {
                    current = cf.clone();
                }
                RecordKind::Entry => {
                    seen.push((current.clone(), key.clone(), value.clone()));
                }
                RecordKind::End => break,
            }
        }
        reader.verify_checksum().await.unwrap();

        assert_eq!(
            seen,
            vec![
                ("cf_config".to_string(), b"k1".to_vec(), b"v1".to_vec()),
                ("cf_config".to_string(), b"k2".to_vec(), Vec::new()),
                ("cf_users".to_string(), b"u1".to_vec(), b"blob".to_vec()),
            ]
        );
    }

    #[tokio::test]
    async fn corrupted_body_is_rejected() {
        let mut body = Vec::new();
        let mut writer = SnapshotWriter::new(&mut body);
        writer.write_header().await.unwrap();
        writer.start_column_family("cf_config").await.unwrap();
        writer.write_entry(b"k1", b"v1").await.unwrap();
        writer.finish().await.unwrap();

        // Flip the last payload byte. The trailer is `[TAG_END][checksum]`, so
        // the final payload byte sits one byte before `TAG_END`.
        let last = body.len() - CHECKSUM_SIZE - 2;
        assert_ne!(body[last], TAG_END, "guard: this must not hit the tag");
        body[last] ^= 0xFF;

        let mut reader = SnapshotReader::new(Cursor::new(body));
        reader.read_header().await.unwrap();
        let mut cf = String::new();
        let mut key = Vec::new();
        let mut value = Vec::new();
        while let Some(kind) = reader
            .next_record(&mut cf, &mut key, &mut value)
            .await
            .unwrap()
        {
            if kind == RecordKind::End {
                break;
            }
        }
        assert!(reader.verify_checksum().await.is_err());
    }

    #[tokio::test]
    async fn corrupted_record_tag_is_rejected() {
        let mut body = Vec::new();
        let mut writer = SnapshotWriter::new(&mut body);
        writer.write_header().await.unwrap();
        writer.start_column_family("cf_config").await.unwrap();
        writer.write_entry(b"k1", b"v1").await.unwrap();
        writer.finish().await.unwrap();

        // Corruption may just as well land on a tag as on a payload byte; the
        // reader must reject it either way rather than skipping past it.
        let tag = body.len() - CHECKSUM_SIZE - 1;
        assert_eq!(body[tag], TAG_END, "guard: this must hit the end tag");
        body[tag] ^= 0xFF;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot");
        tokio::fs::write(&path, &body).await.unwrap();

        assert!(verify_file(&path).await.is_err());
    }

    #[tokio::test]
    async fn verify_file_accepts_a_written_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot");

        let file = tokio::fs::File::create(&path).await.unwrap();
        let mut writer = SnapshotWriter::new(tokio::io::BufWriter::new(file));
        writer.write_header().await.unwrap();
        writer.start_column_family("cf_config").await.unwrap();
        writer.write_entry(b"k", b"v").await.unwrap();
        writer.finish().await.unwrap();

        verify_file(&path).await.unwrap();
    }

    #[tokio::test]
    async fn verify_file_rejects_a_truncated_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot");
        tokio::fs::write(&path, b"BSNA").await.unwrap();

        assert!(verify_file(&path).await.is_err());
    }

    #[tokio::test]
    async fn verify_file_accepts_an_empty_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot");
        tokio::fs::write(&path, b"").await.unwrap();

        verify_file(&path).await.unwrap();
    }
}
