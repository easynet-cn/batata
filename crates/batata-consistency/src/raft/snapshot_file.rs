//! Snapshot body backed by a file on disk.
//!
//! openraft moves the snapshot body around as `TypeConfig::SnapshotData`, which
//! must be `AsyncRead + AsyncWrite + AsyncSeek`. Keeping that body in a file
//! instead of a `Vec<u8>` is what makes snapshot transfer streaming: the leader
//! reads chunks straight off disk, and the follower writes the incoming chunks
//! straight onto disk. At no point does the whole snapshot have to fit in
//! memory, so a multi-gigabyte state machine can be replicated on a node with
//! a modest heap.

use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::fs::{File, OpenOptions};
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, ReadBuf, SeekFrom,
};

/// Buffer size used when copying a snapshot body between files.
const COPY_BUFFER_SIZE: usize = 64 * 1024;

/// A snapshot body stored as a file, satisfying openraft's `SnapshotData`.
#[derive(Debug)]
pub struct SnapshotFile {
    file: File,
    path: PathBuf,
}

impl SnapshotFile {
    /// Create `path` (truncating it if it already exists) for writing.
    ///
    /// The file is opened read-write because openraft's `SnapshotData` bound
    /// requires `AsyncRead` as well: the same handle is written to and then
    /// read back once the transfer finished.
    pub async fn create(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .await?;
        Ok(Self { file, path })
    }

    /// Open an existing file for reading, positioned at the first byte.
    ///
    /// The handle is opened read-write because openraft's `SnapshotData` bound
    /// requires `AsyncWrite` in addition to `AsyncRead`.
    pub async fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .await?;
        file.seek(SeekFrom::Start(0)).await?;
        Ok(Self { file, path })
    }

    /// Write `bytes` to `path` and return a handle positioned at the start.
    ///
    /// Convenience for callers (and tests) that already hold the encoded body.
    pub async fn write_bytes(path: impl AsRef<Path>, bytes: &[u8]) -> io::Result<Self> {
        let mut file = Self::create(path).await?;
        file.write_all(bytes).await?;
        file.flush().await?;
        file.sync_all().await?;
        file.rewind().await?;
        Ok(file)
    }

    /// Path of the backing file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Size of the snapshot body in bytes.
    pub async fn len(&self) -> io::Result<u64> {
        Ok(self.file.metadata().await?.len())
    }

    /// Whether the snapshot body is empty.
    pub async fn is_empty(&self) -> io::Result<bool> {
        self.len().await.map(|len| len == 0)
    }

    /// Block until the body is durable on disk.
    pub async fn sync_all(&self) -> io::Result<()> {
        self.file.sync_all().await
    }

    /// Move the read/write cursor back to the first byte.
    pub async fn rewind(&mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(0)).await?;
        Ok(())
    }

    /// Copy the whole body into `dst`, streaming through a fixed-size buffer.
    ///
    /// Used when a snapshot is published into the node's own snapshot
    /// directory. The source is left untouched, so the caller keeps ownership
    /// of it (and deletes it when it is a staging file).
    pub async fn copy_to_path(&mut self, dst: &Path) -> io::Result<u64> {
        self.rewind().await?;

        let mut out = File::create(dst).await?;
        let mut buf = vec![0u8; COPY_BUFFER_SIZE];
        let mut copied = 0u64;

        loop {
            let read = self.file.read(&mut buf).await?;
            if read == 0 {
                break;
            }
            out.write_all(&buf[..read]).await?;
            copied += read as u64;
        }

        out.flush().await?;
        out.sync_all().await?;
        self.rewind().await?;
        Ok(copied)
    }
}

impl AsyncRead for SnapshotFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_read(cx, buf)
    }
}

impl AsyncWrite for SnapshotFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.file).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.file).poll_shutdown(cx)
    }
}

impl AsyncSeek for SnapshotFile {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        Pin::new(&mut self.file).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        Pin::new(&mut self.file).poll_complete(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn create_then_read_back_streams_the_same_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body");

        let mut file = SnapshotFile::create(&path).await.unwrap();
        file.write_all(b"hello snapshot").await.unwrap();
        file.flush().await.unwrap();
        drop(file);

        let reopened = SnapshotFile::open(&path).await.unwrap();
        assert_eq!(reopened.len().await.unwrap(), 14);
        assert!(!reopened.is_empty().await.unwrap());

        let mut reader = BufReader::new(reopened);
        let mut contents = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut contents)
            .await
            .unwrap();
        assert_eq!(contents, "hello snapshot");
    }

    #[tokio::test]
    async fn write_bytes_positions_the_handle_at_the_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body");

        let mut file = SnapshotFile::write_bytes(&path, b"abcd").await.unwrap();
        let mut first = [0u8; 4];
        file.read_exact(&mut first).await.unwrap();
        assert_eq!(&first, b"abcd");
    }

    #[tokio::test]
    async fn copy_to_path_leaves_the_source_intact() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        let mut file = SnapshotFile::write_bytes(&src, b"payload").await.unwrap();
        let copied = file.copy_to_path(&dst).await.unwrap();

        assert_eq!(copied, 7);
        assert_eq!(tokio::fs::read(&dst).await.unwrap(), b"payload");
        // The source is still readable: copying must not consume it.
        assert_eq!(tokio::fs::read(&src).await.unwrap(), b"payload");
    }

    #[tokio::test]
    async fn seek_rewinds_for_a_second_pass() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body");
        let mut file = SnapshotFile::write_bytes(&path, b"0123456789")
            .await
            .unwrap();

        let mut head = [0u8; 3];
        file.read_exact(&mut head).await.unwrap();
        assert_eq!(&head, b"012");

        file.rewind().await.unwrap();
        let mut head_again = [0u8; 3];
        file.read_exact(&mut head_again).await.unwrap();
        assert_eq!(&head_again, b"012");
    }
}
