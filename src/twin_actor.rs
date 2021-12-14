use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, WrapFuture};
use log::{debug, error};
use std::time::{Duration, SystemTime};

use iotics_grpc_client::common::{Channel, LangLiteral};
use iotics_grpc_client::twin::upsert::upsert_twin;
use iotics_grpc_client::twin::{share_data_with_client, update_twin, FeedApiClient, TwinApiClient};
use iotics_identity::create_twin_did_with_control_delegation;

use crate::messages::TwinProperties;
use crate::{
    config::ApiConfig,
    constants::AGENT_TWIN_NAME,
    messages::{ShareConcurrencyReduction, TwinConcurrencyReduction},
    model::Model,
    twin::Twin,
};

use super::{
    messages::{Cleanup, TwinCreated, TwinData},
    model_actor::ModelActor,
};

#[derive(Debug)]
pub struct TwinActor {
    model_addr: Addr<ModelActor>,
    api_config: ApiConfig,
    token: String,
    twin: Twin,
    model: Model,
    twin_channel: TwinApiClient<Channel>,
    feed_channel: FeedApiClient<Channel>,
    twin_did: Option<String>,
    last_data_received_at: SystemTime,
}

impl TwinActor {
    pub fn new(
        model_addr: Addr<ModelActor>,
        api_config: ApiConfig,
        token: String,
        twin: Twin,
        model: Model,
        twin_channel: TwinApiClient<Channel>,
        feed_channel: FeedApiClient<Channel>,
    ) -> Self {
        Self {
            model_addr,
            api_config,
            token,
            twin,
            model,
            twin_channel,
            feed_channel,
            twin_did: None,
            last_data_received_at: SystemTime::now(),
        }
    }
}

impl Actor for TwinActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Twin {} actor started", &self.twin.label);

        let addr = ctx.address();

        let api_config = self.api_config.clone();
        let token = self.token.clone();
        let twin = self.twin.clone();
        let twin_label = twin.label.clone();
        let model = self.model.clone();
        let model_addr = self.model_addr.clone();
        let mut twin_channel = self.twin_channel.clone();

        let fut = async move {
            let properties = model.get_twin_properties(&twin.model_did);
            let result = async {
                let twin_did = create_twin_did_with_control_delegation(
                    &api_config.identity_config,
                    &twin.seed,
                    AGENT_TWIN_NAME,
                )?;

                let labels = vec![LangLiteral {
                    lang: "en".to_string(),
                    value: twin.label.to_string(),
                }];

                upsert_twin(
                    &mut twin_channel,
                    &token,
                    &twin_did,
                    labels,
                    properties,
                    model.get_feeds(false),
                    twin.location,
                    model.visibility as i32,
                )
                .await?;

                Ok::<String, anyhow::Error>(twin_did)
            };

            match result.await {
                Ok(twin_did) => {
                    addr.try_send(TwinCreated { twin_did })
                        .expect("failed to send TwinCreated message");
                }
                Err(e) => {
                    // Send the TwinConcurrencyReduction message to the model actor
                    model_addr
                        .try_send(TwinConcurrencyReduction { twin_seed: None })
                        .expect("failed to send TwinConcurrencyReduction message");

                    panic!("failed to create twin {}, {}", twin_label, e);
                }
            }
        }
        .into_actor(self);

        ctx.spawn(fut);
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        debug!("Twin {} actor stopped", self.twin.label);
    }
}

impl Handler<TwinCreated> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinCreated, _: &mut Context<Self>) -> Self::Result {
        debug!("Twin {} actor got message {:?}", self.twin.label, message);
        self.twin_did.replace(message.twin_did);

        // Send the TwinConcurrencyReduction message to the model actor
        self.model_addr
            .try_send(TwinConcurrencyReduction {
                twin_seed: Some(self.twin.seed.clone()),
            })
            .expect("failed to send TwinConcurrencyReduction message");
    }
}

impl Handler<TwinData> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinData, ctx: &mut Context<Self>) -> Self::Result {
        // twin_did should never be None
        // because we're not sharing data until the twin is created
        let twin_did = self.twin_did.as_ref().expect("this should not happen");

        let model_addr = self.model_addr.clone();
        self.last_data_received_at = SystemTime::now();

        let token = self.token.clone();
        let label = self.twin.label.clone();
        let twin_did = twin_did.clone();
        let mut feed_channel = self.feed_channel.clone();

        let fut = async move {
            for (feed_id, feed_data) in &message.data.feeds {
                let data = feed_data.to_string().as_bytes().to_vec();

                let result = share_data_with_client(
                    &mut feed_channel,
                    &token,
                    &twin_did,
                    feed_id,
                    data,
                    true,
                )
                .await;

                if let Err(e) = result {
                    error!("failed to share data to twin {} {:?}", &twin_did, e);
                } else {
                    debug!("Twin {} shared {} feed data", &label, &feed_id);
                }
            }

            // Send the ShareConcurrencyReduction message to the model actor
            model_addr
                .try_send(ShareConcurrencyReduction {
                    amount: message.data.feeds.len(),
                })
                .expect("failed to send ShareConcurrencyReduction message");
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<TwinProperties> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinProperties, ctx: &mut Context<Self>) -> Self::Result {
        // twin_did should never be None
        // because we're not sharing data until the twin is created
        let twin_did = self.twin_did.as_ref().expect("this should not happen");

        let token = self.token.clone();
        let twin_did = twin_did.clone();
        let mut twin_channel = self.twin_channel.clone();

        let fut = async move {
            let result = update_twin(
                &mut twin_channel,
                &token,
                &twin_did,
                message.properties.clone(),
                false,
            )
            .await;

            if let Err(e) = result {
                error!(
                    "failed to update properties of twin {} {:?}. Properties: {:?}",
                    &twin_did, e, message.properties
                );
            } else {
                debug!("Twin {} properties updated", &twin_did);
            }
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<Cleanup> for TwinActor {
    type Result = ();

    fn handle(&mut self, _message: Cleanup, ctx: &mut Context<Self>) -> Self::Result {
        let expire_at = self
            .last_data_received_at
            .checked_add(Duration::from_secs(60 * 15))
            .expect("this should not happen");

        if SystemTime::now() > expire_at {
            ctx.stop();
        }
    }
}
