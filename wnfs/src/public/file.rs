//! Public fs file node.

use super::{PublicFileSerializable, PublicNodeSerializable};
use crate::{error::FsError, is_readable_wnfs_version, traits::Id, WNFS_VERSION};
use anyhow::{bail, Result};
use async_once_cell::OnceCell;
use chrono::{DateTime, Utc};
use libipld_core::cid::Cid;
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, sync::Arc};
use wnfs_common::{BlockStore, Metadata, RemembersCid};

/// A file in the WNFS public file system.
///
/// # Examples
///
/// ```
/// use wnfs::public::PublicFile;
/// use chrono::Utc;
/// use libipld_core::cid::Cid;
///
/// let file = PublicFile::new(Utc::now(), Cid::default());
///
/// println!("File: {:?}", file);
/// ```
#[derive(Debug)]
pub struct PublicFile {
    persisted_as: OnceCell<Cid>,
    pub metadata: Metadata,
    pub userland: Cid,
    pub previous: BTreeSet<Cid>,
}

//--------------------------------------------------------------------------------------------------
// Implementations
//--------------------------------------------------------------------------------------------------

impl PublicFile {
    /// Creates a new file with provided content CID.
    ///
    /// # Examples
    ///
    /// ```
    /// use wnfs::public::PublicFile;
    /// use chrono::Utc;
    /// use libipld_core::cid::Cid;
    ///
    /// let file = PublicFile::new(Utc::now(), Cid::default());
    ///
    /// println!("File: {:?}", file);
    /// ```
    pub fn new(time: DateTime<Utc>, content_cid: Cid) -> Self {
        Self {
            persisted_as: OnceCell::new(),
            metadata: Metadata::new(time),
            userland: content_cid,
            previous: BTreeSet::new(),
        }
    }

    /// Creates an `Arc` wrapped file.
    ///
    /// # Examples
    ///
    /// ```
    /// use wnfs::public::PublicFile;
    /// use chrono::Utc;
    /// use libipld_core::cid::Cid;
    ///
    /// let file = PublicFile::new(Utc::now(), Cid::default());
    ///
    /// println!("File: {:?}", file);
    /// ```
    pub fn new_rc(time: DateTime<Utc>, content_cid: Cid) -> Arc<Self> {
        Arc::new(Self::new(time, content_cid))
    }

    /// Takes care of creating previous links, in case the current
    /// directory was previously `.store()`ed.
    /// In any case it'll try to give you ownership of the directory if possible,
    /// otherwise it clones.
    pub(crate) fn prepare_next_revision<'a>(self: &'a mut Arc<Self>) -> &'a mut Self {
        let Some(previous_cid) = self.persisted_as.get().cloned() else {
            return Arc::make_mut(self);
        };

        let cloned = Arc::make_mut(self);
        cloned.persisted_as = OnceCell::new();
        cloned.previous = [previous_cid].into_iter().collect();

        cloned
    }

    /// Writes a new content cid to the file.
    /// This will create a new revision of the file.
    pub(crate) fn write(self: &mut Arc<Self>, time: DateTime<Utc>, content_cid: Cid) {
        let file = self.prepare_next_revision();
        file.userland = content_cid;
        file.metadata.upsert_mtime(time);
    }

    /// Gets the previous value of the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use wnfs::{public::PublicDirectory, traits::Id};
    /// use chrono::Utc;
    ///
    /// let dir = PublicDirectory::new(Utc::now());
    ///
    /// println!("id = {}", dir.get_id());
    /// ```
    pub fn get_previous(&self) -> &BTreeSet<Cid> {
        &self.previous
    }

    /// Gets the metadata of the file
    pub fn get_metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Returns a mutable reference to metadata for this file.
    pub fn get_metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    /// Returns a mutable reference to this file's metadata and ratchets forward the history, if necessary.
    pub fn get_metadata_mut_rc<'a>(self: &'a mut Arc<Self>) -> &'a mut Metadata {
        self.prepare_next_revision().get_metadata_mut()
    }

    /// Gets the content cid of a file
    pub fn get_content_cid(&self) -> &Cid {
        &self.userland
    }

    /// Stores file in provided block store.
    ///
    /// # Examples
    ///
    /// ```
    /// use wnfs::{
    ///     public::PublicFile,
    ///     traits::Id,
    ///     common::MemoryBlockStore
    /// };
    /// use chrono::Utc;
    /// use libipld_core::cid::Cid;
    ///
    /// #[async_std::main]
    /// async fn main() {
    ///     let mut store = MemoryBlockStore::default();
    ///     let file = PublicFile::new(Utc::now(), Cid::default());
    ///
    ///     file.store(&mut store).await.unwrap();
    /// }
    /// ```
    pub async fn store(&self, store: &impl BlockStore) -> Result<Cid> {
        Ok(*self
            .persisted_as
            .get_or_try_init(store.put_serializable(self))
            .await?)
    }

    /// Creates a new file from a serializable.
    pub(crate) fn from_serializable(serializable: PublicFileSerializable) -> Result<Self> {
        if !is_readable_wnfs_version(&serializable.version) {
            bail!(FsError::UnexpectedVersion(serializable.version))
        }

        Ok(Self {
            persisted_as: OnceCell::new(),
            metadata: serializable.metadata,
            userland: serializable.userland,
            previous: serializable.previous.iter().cloned().collect(),
        })
    }
}

impl Serialize for PublicFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PublicNodeSerializable::File(PublicFileSerializable {
            version: WNFS_VERSION,
            metadata: self.metadata.clone(),
            userland: self.userland,
            previous: self.previous.iter().cloned().collect(),
        })
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PublicFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match PublicNodeSerializable::deserialize(deserializer)? {
            PublicNodeSerializable::File(file) => {
                PublicFile::from_serializable(file).map_err(DeError::custom)
            }
            _ => Err(DeError::custom(FsError::InvalidDeserialization(
                "Expected directory".into(),
            ))),
        }
    }
}

impl Id for PublicFile {
    fn get_id(&self) -> String {
        format!("{:p}", &self.metadata)
    }
}

impl PartialEq for PublicFile {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.userland == other.userland
            && self.previous == other.previous
    }
}

impl Clone for PublicFile {
    fn clone(&self) -> Self {
        Self {
            persisted_as: self
                .persisted_as
                .get()
                .cloned()
                .map(OnceCell::new_with)
                .unwrap_or_default(),
            metadata: self.metadata.clone(),
            userland: self.userland,
            previous: self.previous.clone(),
        }
    }
}

impl RemembersCid for PublicFile {
    fn persisted_as(&self) -> &OnceCell<Cid> {
        &self.persisted_as
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use wnfs_common::{MemoryBlockStore, CODEC_RAW};

    #[async_std::test]
    async fn previous_links_get_set() {
        let time = Utc::now();
        let store = &MemoryBlockStore::default();

        let content_cid = store
            .put_block(b"Hello World".to_vec(), CODEC_RAW)
            .await
            .unwrap();

        let file = &mut PublicFile::new_rc(time, content_cid);
        let previous_cid = &file.store(store).await.unwrap();
        let next_file = file.prepare_next_revision();

        assert_eq!(
            next_file.previous.iter().collect::<Vec<_>>(),
            vec![previous_cid]
        );
    }

    #[async_std::test]
    async fn prepare_next_revision_shortcuts_if_possible() {
        let time = Utc::now();
        let store = &MemoryBlockStore::default();
        let content_cid = store
            .put_block(b"Hello World".to_vec(), CODEC_RAW)
            .await
            .unwrap();

        let file = &mut PublicFile::new_rc(time, content_cid);
        let previous_cid = &file.store(store).await.unwrap();
        let next_file = file.prepare_next_revision();
        let next_file_clone = &mut Arc::new(next_file.clone());
        let yet_another_file = next_file_clone.prepare_next_revision();

        assert_eq!(
            yet_another_file.previous.iter().collect::<Vec<_>>(),
            vec![previous_cid]
        );
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use chrono::TimeZone;
    use wnfs_common::utils::SnapshotBlockStore;

    #[async_std::test]
    async fn test_simple_file() {
        let store = &SnapshotBlockStore::default();
        let time = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();

        let file = &mut PublicFile::new_rc(time, Cid::default());
        let cid = file.store(store).await.unwrap();

        let file = store.get_block_snapshot(&cid).await.unwrap();

        insta::assert_json_snapshot!(file);
    }

    #[async_std::test]
    async fn test_file_with_previous_links() {
        let store = &SnapshotBlockStore::default();
        let time = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();

        let file = &mut PublicFile::new_rc(time, Cid::default());
        let _ = file.store(store).await.unwrap();

        file.write(time, Cid::default());
        let cid = file.store(store).await.unwrap();

        let file = store.get_block_snapshot(&cid).await.unwrap();

        insta::assert_json_snapshot!(file);
    }
}
