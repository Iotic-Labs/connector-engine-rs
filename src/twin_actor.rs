use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, WrapFuture};
use log::{debug, error};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use iotics_grpc_client::common::{Channel, PropertyUpdate};
use iotics_grpc_client::twin::crud::update_twin_with_client;
use iotics_grpc_client::twin::share::share_data_with_client;
use iotics_grpc_client::twin::upsert::upsert_twin_with_client;
use iotics_grpc_client::twin::{FeedApiClient, TwinApiClient};
use iotics_identity::create_twin_did_with_control_delegation;

use crate::config::AuthBuilder;
use crate::messages::{Cleanup, TwinCreated, TwinData};
use crate::model_actor::ModelActor;
use crate::{
    constants::AGENT_TWIN_NAME,
    messages::{ShareConcurrencyReduction, TwinConcurrencyReduction},
    model::Model,
    twin::Twin,
};

#[derive(Debug)]
pub struct TwinActor {
    model_addr: Addr<ModelActor>,
    auth_builder: Arc<AuthBuilder>,
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
        auth_builder: Arc<AuthBuilder>,
        twin: Twin,
        model: Model,
        twin_channel: TwinApiClient<Channel>,
        feed_channel: FeedApiClient<Channel>,
    ) -> Self {
        Self {
            model_addr,
            auth_builder,
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

        let auth_builder = self.auth_builder.clone();
        let twin = self.twin.clone();
        let model = self.model.clone();
        let model_addr = self.model_addr.clone();
        let mut twin_channel = self.twin_channel.clone();

        let fut = async move {
            let properties = model.build_twin_properties(&twin.model_did, &twin.label);

            let result = async {
                let identity_config = auth_builder
                    .get_identity_config()
                    .expect("failed to get the identity config");
                let twin_did = create_twin_did_with_control_delegation(
                    &identity_config,
                    &twin.seed,
                    AGENT_TWIN_NAME,
                )?;

                upsert_twin_with_client(
                    auth_builder,
                    &mut twin_channel,
                    &twin_did,
                    properties,
                    model.get_feeds(false),
                    twin.location,
                    model.get_visibility() as i32,
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

                    panic!("failed to create twin {}, {}", &twin.label, e);
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

        let auth_builder = self.auth_builder.clone();
        let label = self.twin.label.clone();
        let twin_did = twin_did.clone();

        let mut twin_channel = self.twin_channel.clone();
        let mut feed_channel = self.feed_channel.clone();

        let fut = async move {
            for (feed_id, feed_data) in &message.data.feeds {
                let data = feed_data.to_string().as_bytes().to_vec();

                let result = share_data_with_client(
                    auth_builder.clone(),
                    &mut feed_channel,
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

            if !message.data.properties.is_empty() {
                let deleted_by_key = message
                    .data
                    .properties
                    .clone()
                    .into_iter()
                    .map(|p| p.key)
                    .collect();

                let result = update_twin_with_client(
                    auth_builder,
                    &mut twin_channel,
                    &twin_did,
                    PropertyUpdate {
                        cleared_all: false,
                        added: message.data.properties.clone(),
                        deleted_by_key,
                        ..Default::default()
                    },
                )
                .await;

                if let Err(e) = result {
                    error!(
                        "failed to update properties of twin {} {:?}. Properties: {:?}",
                        &twin_did, e, message.data.properties
                    );
                } else {
                    debug!("Twin {} properties updated", &twin_did);
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
