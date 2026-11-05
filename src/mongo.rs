//! MongoDB document store (NoSQL), via the official async driver.
//!
//! MongoDB does not fit the relational ORM, so it gets its own small
//! document API: JSON documents in named collections. Available with the
//! default `mongodb` Cargo feature.

use mongodb::bson::{doc, Document};
use mongodb::options::ClientOptions;
use mongodb::Client;

/// A MongoDB database handle with JSON document operations.
#[derive(Clone)]
pub struct MongoStore {
    client: Client,
    database: String,
}

impl MongoStore {
    /// Connect to MongoDB, e.g. `mongodb://127.0.0.1:27017`, using `database`.
    pub async fn connect(url: &str, database: impl Into<String>) -> Result<Self, String> {
        let options = ClientOptions::parse(url).await.map_err(|e| e.to_string())?;
        let client = Client::with_options(options).map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            database: database.into(),
        })
    }

    fn collection(&self, name: &str) -> mongodb::Collection<Document> {
        self.client.database(&self.database).collection(name)
    }

    /// Insert one JSON document. Returns the inserted id as a string.
    /// Non-object documents are wrapped as `{ "value": ... }`.
    pub async fn insert(
        &self,
        collection: &str,
        document: serde_json::Value,
    ) -> Result<String, String> {
        let document = to_document(document);
        let result = self
            .collection(collection)
            .insert_one(document)
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.inserted_id.to_string())
    }

    /// Find documents matching a JSON filter (`{}` matches all).
    pub async fn find(
        &self,
        collection: &str,
        filter: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, String> {
        let filter = to_document(filter);
        let mut cursor = self
            .collection(collection)
            .find(filter)
            .await
            .map_err(|e| e.to_string())?;
        use futures_util::TryStreamExt;
        let mut out = Vec::new();
        while let Some(document) = cursor.try_next().await.map_err(|e| e.to_string())? {
            out.push(serde_json::to_value(document).map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Count documents matching a JSON filter.
    pub async fn count(&self, collection: &str, filter: serde_json::Value) -> Result<u64, String> {
        self.collection(collection)
            .count_documents(to_document(filter))
            .await
            .map_err(|e| e.to_string())
    }

    /// Delete documents matching a JSON filter. Returns how many were removed.
    pub async fn delete(&self, collection: &str, filter: serde_json::Value) -> Result<u64, String> {
        let result = self
            .collection(collection)
            .delete_many(to_document(filter))
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.deleted_count)
    }
}

fn to_document(value: serde_json::Value) -> Document {
    match mongodb::bson::to_document(&value) {
        Ok(document) => document,
        Err(_) => doc! { "value": mongodb::bson::to_bson(&value).unwrap_or(mongodb::bson::Bson::Null) },
    }
}
