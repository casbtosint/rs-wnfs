use crate::private::{
    forest::{hamt::HamtForest, traits::PrivateForest},
    FileContent, PrivateDirectory, PrivateForestContent, PrivateNode, PrivateRef,
};
use anyhow::Result;
use libipld_core::ipld::Ipld;
use rand_core::CryptoRngCore;
use std::sync::Arc;
use wnfs_common::{decode, libipld::cbor::DagCborCodec, utils::SnapshotBlockStore};
use wnfs_nameaccumulator::Name;

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

pub(crate) async fn walk_dir(
    store: &mut SnapshotBlockStore,
    forest: &mut Arc<HamtForest>,
    root_dir: &Arc<PrivateDirectory>,
    rng: &mut impl CryptoRngCore,
) -> Result<()> {
    let mut stack = vec![root_dir.clone()];
    while let Some(dir) = stack.pop() {
        let private_ref: PrivateRef = dir.store(forest, store, rng).await?;
        let temporal_key = private_ref.temporal_key;
        let snapshot_key = temporal_key.derive_snapshot_key();
        store.add_block_handler(
            private_ref.content_cid,
            Box::new(move |bytes| decode(&snapshot_key.decrypt(bytes.as_ref())?, DagCborCodec)),
        );
        store.add_block_handler(
            dir.header.store(store, forest).await?,
            Box::new(move |bytes| {
                decode(
                    &temporal_key.key_wrap_decrypt(bytes.as_ref())?,
                    DagCborCodec,
                )
            }),
        );

        let entries = dir.ls(&[], true, forest, store).await?;
        for (name, _) in entries.iter() {
            let node = dir.lookup_node(name, true, forest, store).await?;
            match node.as_ref() {
                Some(PrivateNode::Dir(dir)) => {
                    stack.push(dir.clone());
                }
                Some(PrivateNode::File(file)) => {
                    let private_ref: PrivateRef = file.store(forest, store, rng).await?;
                    let temporal_key = private_ref.temporal_key;
                    let snapshot_key = temporal_key.derive_snapshot_key();
                    store.add_block_handler(
                        private_ref.content_cid,
                        Box::new(move |bytes| {
                            decode(&snapshot_key.decrypt(bytes.as_ref())?, DagCborCodec)
                        }),
                    );
                    store.add_block_handler(
                        file.header.store(store, forest).await?,
                        Box::new(move |bytes| {
                            decode(
                                &temporal_key.key_wrap_decrypt(bytes.as_ref())?,
                                DagCborCodec,
                            )
                        }),
                    );
                    if let FileContent::External(PrivateForestContent {
                        key,
                        block_count,
                        base_name,
                        ..
                    }) = &file.content.content
                    {
                        for name in PrivateForestContent::generate_shard_labels(
                            key,
                            0,
                            *block_count,
                            &Name::new(base_name.clone(), []),
                        ) {
                            match forest.get_encrypted(&name, store).await? {
                                Some(cids) => {
                                    let key = key.clone();
                                    store.add_block_handler(
                                        *cids.first().unwrap(),
                                        Box::new(move |bytes| {
                                            Ok(Ipld::Bytes(key.decrypt(bytes.as_ref())?))
                                        }),
                                    )
                                }
                                None => unreachable!(),
                            };
                        }
                    }
                }
                None => unreachable!(),
            }
        }
    }

    Ok(())
}
