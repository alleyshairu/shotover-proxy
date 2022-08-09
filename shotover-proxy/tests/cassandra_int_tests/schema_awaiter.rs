use scylla::{Session, SessionBuilder};

// Modifying the schema will take a while to propagate to all nodes.
// It seems adding a table doesnt cause any problems, maybe cassandra is just routing to a node that has the table.
// But for cases like adding a new function we hit issues where the function is not yet propagated to all nodes.
// So we make use of the scylla drivers await_schema_agreement logic to wait until all nodes are on the same schema.
pub struct SchemaAwaiter {
    session: Session,
}

impl SchemaAwaiter {
    pub async fn new(node: &str) -> Self {
        SchemaAwaiter {
            session: SessionBuilder::new()
                .known_node(node)
                .user("cassandra", "cassandra")
                .build()
                .await
                .unwrap(),
        }
    }

    pub async fn await_schema_agreement(&self) {
        self.session.await_schema_agreement().await.unwrap();
    }
}