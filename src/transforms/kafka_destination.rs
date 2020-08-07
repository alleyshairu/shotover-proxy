use crate::transforms::chain::{Transform, TransformChain, Wrapper};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};

use crate::config::topology::TopicHolder;
use crate::message::{Message, QueryResponse};
use crate::transforms::{Transforms, TransformsFromConfig};
use async_trait::async_trait;
use std::collections::HashMap;
use crate::error::{ChainResponse};
use anyhow::{Result};
use rdkafka::util::Timeout;

#[derive(Clone)]
pub struct KafkaDestination {
    producer: FutureProducer,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct KafkaConfig {
    #[serde(rename = "config_values")]
    pub keys: HashMap<String, String>,
}

#[async_trait]
impl TransformsFromConfig for KafkaConfig {
    async fn get_source(
        &self,
        _topics: &TopicHolder,
    ) -> Result<Transforms> {
        Ok(Transforms::KafkaDestination(
            KafkaDestination::new_from_config(&self.keys),
        ))
    }
}

impl KafkaDestination {
    pub fn new_from_config(
        config_map: &HashMap<String, String>,
    ) -> KafkaDestination {
        let mut config = ClientConfig::new();
        for (k, v) in config_map.iter() {
            config.set(k.as_str(), v.as_str());
        }
        return KafkaDestination {
            producer: config.create().expect("Producer creation error"),
        };
    }

    pub fn new() -> KafkaDestination {
        KafkaDestination {
            producer: ClientConfig::new()
                .set("bootstrap.servers", "127.0.0.1:9092")
                .set("message.timeout.ms", "5000")
                .create()
                .expect("Producer creation error"),
        }
    }
}

#[async_trait]
impl Transform for KafkaDestination {
    async fn transform(&self, qd: Wrapper, _: &TransformChain) -> ChainResponse {
        match qd.message {
            Message::Bypass(_) => {},
            Message::Query(qm) => {
                if let Some(ref key) = qm.get_namespaced_primary_key() {
                    if let Some(values) = qm.query_values {
                        let message = serde_json::to_string(&values)?;
                        let a = FutureRecord::to("test_topic").payload(&message).key(&key);
                        self.producer.send(a, Timeout::Never).await;
                    }
                }
            },
            _ => {},
        }
        return ChainResponse::Ok(Message::Response(QueryResponse::empty()));
    }

    fn get_name(&self) -> &'static str {
        "Kafka"
    }
}