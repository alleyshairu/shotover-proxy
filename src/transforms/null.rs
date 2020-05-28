use crate::transforms::chain::{ChainResponse, Transform, TransformChain, Wrapper};

use crate::message::{Message, QueryResponse};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Null {
    name: &'static str,
}

impl Null {
    pub fn new() -> Null {
        Null { name: "Null" }
    }
}

#[async_trait]
impl Transform for Null {
    async fn transform(&self, mut qd: Wrapper, t: &TransformChain) -> ChainResponse {
        if let Message::Query(qm) = qd.message {
            return ChainResponse::Ok(Message::Response(QueryResponse::emptyWithOriginal(qm)));
        }
        return ChainResponse::Ok(Message::Response(QueryResponse::empty()));
    }

    fn get_name(&self) -> &'static str {
        self.name
    }
}
