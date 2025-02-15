//! The bindgen API for PrivateFile.
use super::Name;
use crate::{
    fs::{
        metadata::JsMetadata,
        utils::{self, error},
        BlockStore, ForeignBlockStore, JsResult, PrivateForest, PrivateNode, Rng,
    },
    value,
};
use chrono::{DateTime, Utc};
use js_sys::{Date, Promise, Uint8Array};
use std::sync::Arc;
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};
use wasm_bindgen_futures::future_to_promise;
use wnfs::{
    private::{PrivateFile as WnfsPrivateFile, PrivateNode as WnfsPrivateNode},
    traits::Id,
};

//--------------------------------------------------------------------------------------------------
// Type Definitions
//--------------------------------------------------------------------------------------------------

/// A file in a WNFS public file system.
#[wasm_bindgen]
pub struct PrivateFile(pub(crate) Arc<WnfsPrivateFile>);

//--------------------------------------------------------------------------------------------------
// Implementations
//--------------------------------------------------------------------------------------------------

#[wasm_bindgen]
impl PrivateFile {
    /// Creates an empty private file.
    #[wasm_bindgen(constructor)]
    pub fn new(parent_bare_name: Name, time: &Date, mut rng: Rng) -> JsResult<PrivateFile> {
        let time = DateTime::<Utc>::from(time);

        Ok(Self(Arc::new(WnfsPrivateFile::new(
            &parent_bare_name.0,
            time,
            &mut rng,
        ))))
    }

    /// Creates a file with provided content.
    #[wasm_bindgen(js_name = "withContent")]
    pub fn with_content(
        parent_bare_name: Name,
        time: &Date,
        content: Vec<u8>,
        forest: &PrivateForest,
        store: BlockStore,
        mut rng: Rng,
    ) -> JsResult<Promise> {
        let mut store = ForeignBlockStore(store);
        let time = DateTime::<Utc>::from(time);
        let mut forest = Arc::clone(&forest.0);

        Ok(future_to_promise(async move {
            let file = WnfsPrivateFile::with_content(
                &parent_bare_name.0,
                time,
                content,
                &mut forest,
                &mut store,
                &mut rng,
            )
            .await
            .map_err(error("Cannot create a file with provided content"))?;

            Ok(utils::create_private_forest_result(
                PrivateFile(Arc::new(file)).into(),
                forest,
            )?)
        }))
    }

    /// Persists the current state of this file in the BlockStore and PrivateForest.
    /// This will also force a history entry to be created, if there were changes.
    pub fn store(&self, forest: &PrivateForest, store: BlockStore, rng: Rng) -> JsResult<Promise> {
        let node = PrivateNode(WnfsPrivateNode::File(Arc::clone(&self.0)));
        node.store(forest, store, rng)
    }

    /// Gets the entire content of a file.
    #[wasm_bindgen(js_name = "getContent")]
    pub fn get_content(&self, forest: &PrivateForest, store: BlockStore) -> JsResult<Promise> {
        let file = Arc::clone(&self.0);
        let store = ForeignBlockStore(store);
        let forest = Arc::clone(&forest.0);

        Ok(future_to_promise(async move {
            let content = file
                .get_content(&forest, &store)
                .await
                .map_err(error("Cannot get content of file"))?;

            Ok(value!(Uint8Array::from(content.as_slice())))
        }))
    }

    /// Gets the metadata of this file.
    pub fn metadata(&self) -> JsResult<JsValue> {
        JsMetadata(self.0.get_metadata()).try_into()
    }

    /// Gets a unique id for node.
    #[wasm_bindgen(js_name = "getId")]
    pub fn get_id(&self) -> String {
        self.0.get_id()
    }

    /// Converts this file to a node.
    #[wasm_bindgen(js_name = "asNode")]
    pub fn as_node(&self) -> PrivateNode {
        PrivateNode(WnfsPrivateNode::File(Arc::clone(&self.0)))
    }
}
