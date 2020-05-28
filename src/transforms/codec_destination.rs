use crate::transforms::chain::{ChainResponse, Transform, TransformChain, Wrapper};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::config::topology::TopicHolder;
use crate::config::ConfigError;
use crate::message::{Message, QueryMessage, QueryResponse, RawMessage};
use crate::protocols::cassandra_protocol2::CassandraCodec2;
use crate::protocols::cassandra_protocol2::RawFrame;
use crate::protocols::cassandra_protocol2::RawFrame::CASSANDRA;
use crate::transforms::chain::RequestError;
use crate::transforms::{Transforms, TransformsFromConfig};
use cassandra_proto::consistency::Consistency;
use cassandra_proto::frame::Frame;
use futures::{FutureExt, SinkExt};
use slog::trace;
use slog::Logger;
use std::sync::Arc;
use tokio::stream::StreamExt;
use tokio::sync::Mutex;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct CodecConfiguration {
    #[serde(rename = "remote_address")]
    pub address: String,
}

#[async_trait]
impl TransformsFromConfig for CodecConfiguration {
    async fn get_source(
        &self,
        _: &TopicHolder,
        logger: &Logger,
    ) -> Result<Transforms, ConfigError> {
        Ok(Transforms::CodecDestination(CodecDestination::new(
            self.address.clone(),
            logger,
        )))
    }
}

#[derive(Debug)]
pub struct CodecDestination {
    name: &'static str,
    address: String,
    outbound: Arc<Mutex<Option<Framed<TcpStream, CassandraCodec2>>>>,
    logger: Logger,
}

impl Clone for CodecDestination {
    fn clone(&self) -> Self {
        CodecDestination::new(self.address.clone(), &self.logger)
    }
}

impl CodecDestination {
    pub fn new(address: String, logger: &Logger) -> CodecDestination {
        CodecDestination {
            address,
            outbound: Arc::new(Mutex::new(None)),
            name: "CodecDestination",
            logger: logger.clone(),
        }
    }
}

/*
TODO:
it may be worthwhile putting the inbound and outbound tcp streams behind a
multi-consumer, single producer threadsafe queue
*/

impl CodecDestination {
    async fn send_frame(
        &self,
        frame: Frame,
        matching_query: Option<QueryMessage>,
    ) -> ChainResponse {
        trace!(self.logger, "      C -> S {:?}", frame.opcode);
        if let Ok(mut mg) = self.outbound.try_lock() {
            match *mg {
                None => {
                    let outbound_stream = TcpStream::connect(self.address.clone()).await.unwrap();
                    let mut outbound_framed_codec =
                        Framed::new(outbound_stream, CassandraCodec2::new());
                    let _ = outbound_framed_codec.send(frame).await;
                    if let Some(o) = outbound_framed_codec.next().fuse().await {
                        if let Ok(resp) = o {
                            trace!(self.logger, "      S -> C {:?}", resp.opcode);
                            mg.replace(outbound_framed_codec);
                            drop(mg);
                            //TODO build proper response
                            // return ChainResponse::Ok(Message::Response(QueryResponse{
                            //     matching_query,
                            //     original: RawFrame::CASSANDRA(resp),
                            //     result: resp.get_body().unwrap().,
                            //     error: None
                            // }))
                            return ChainResponse::Ok(Message::Bypass(RawMessage {
                                original: RawFrame::CASSANDRA(resp),
                            }));
                        }
                    }
                    mg.replace(outbound_framed_codec);
                    drop(mg);
                }
                Some(ref mut outbound_framed_codec) => {
                    let _ = outbound_framed_codec.send(frame).await;
                    if let Some(o) = outbound_framed_codec.next().fuse().await {
                        if let Ok(resp) = o {
                            trace!(self.logger, "      S -> C {:?}", resp.opcode);
                            return ChainResponse::Ok(Message::Bypass(RawMessage {
                                original: RawFrame::CASSANDRA(resp),
                            }));
                        }
                    }
                }
            }
        }
        return ChainResponse::Err(RequestError {});
    }
}

#[async_trait]
impl Transform for CodecDestination {
    async fn transform(&self, mut qd: Wrapper, _: &TransformChain) -> ChainResponse {
        // let return_query = qd.message.clone();
        match qd.message {
            Message::Bypass(rm) => {
                if let CASSANDRA(frame) = rm.original {
                    return self.send_frame(frame, None).await;
                }
            }
            Message::Query(qm) => {
                let return_query = qm.clone();
                if qd.modified {
                    return self
                        .send_frame(
                            CassandraCodec2::build_cassandra_query_frame(
                                qm,
                                Consistency::LocalQuorum,
                            ),
                            Some(return_query),
                        )
                        .await;
                }
                if let CASSANDRA(frame) = qm.original {
                    return self.send_frame(frame, Some(return_query)).await;
                }
            }
            Message::Response(_) => {}
        }
        return ChainResponse::Err(RequestError {});
    }

    fn get_name(&self) -> &'static str {
        self.name
    }
}
