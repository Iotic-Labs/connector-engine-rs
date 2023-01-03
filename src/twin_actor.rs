use actix::{Actor, ActorContext, Addr, AsyncContext, Context, Handler, WrapFuture};
use iotics_grpc_client::twin::crud::{delete_twin_with_channel, update_twin_with_channel};
use iotics_grpc_client::twin::share::share_data_with_channel;
use iotics_grpc_client::twin::upsert::upsert_twin_with_channel;
use log::{debug, error, warn};
use std::sync::Arc;
use std::time::SystemTime;

use iotics_grpc_client::{Channel, PropertyUpdate};
use iotics_identity::create_twin_did_with_control_delegation;

use crate::config::AuthBuilder;
use crate::messages::{Cleanup, TwinCreationFailure, TwinCreationSuccess, TwinData, TwinDeleted};
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
    twin_channel: Channel,
    feed_channel: Channel,
    twin_did: Option<String>,
    last_data_received_at: SystemTime,
    creation_in_flight: bool,
    shares_in_flight: usize,
}

impl TwinActor {
    pub fn new(
        model_addr: Addr<ModelActor>,
        auth_builder: Arc<AuthBuilder>,
        twin: Twin,
        model: Model,
        twin_channel: Channel,
        feed_channel: Channel,
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
            creation_in_flight: false,
            shares_in_flight: 0,
        }
    }
}

impl Actor for TwinActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        debug!("Twin {} actor started", &self.twin.label);

        self.creation_in_flight = true;

        let addr = ctx.address();

        let auth_builder = self.auth_builder.clone();
        let twin = self.twin.clone();
        let model = self.model.clone();
        let twin_channel = self.twin_channel.clone();

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

                upsert_twin_with_channel(
                    auth_builder,
                    twin_channel,
                    &twin_did,
                    properties,
                    model.get_feeds(false),
                    Vec::new(),
                    twin.location,
                )
                .await?;

                Ok::<String, anyhow::Error>(twin_did)
            }
            .await;

            match result {
                Ok(twin_did) => {
                    addr.try_send(TwinCreationSuccess { twin_did })
                        .expect("failed to send TwinCreationSuccess message to self");
                }
                Err(error) => {
                    addr.try_send(TwinCreationFailure {
                        twin_label: twin.label.clone(),
                        error,
                    })
                    .expect("failed to send TwinCreationFailure message to self");
                }
            }
        }
        .into_actor(self);

        ctx.spawn(fut);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> actix::Running {
        debug!("Twin {} actor stopping", self.twin.label);

        if self.creation_in_flight {
            warn!("Twin {} stopped while creating", self.twin.label);

            // Send the TwinConcurrencyReduction message to the model actor
            self.model_addr
                .try_send(TwinConcurrencyReduction {
                    twin_seed: Some(self.twin.seed.clone()),
                })
                .expect("failed to send TwinConcurrencyReduction message");
        }

        if self.shares_in_flight > 0 {
            warn!("Twin {} stopped while sharing", self.twin.label);

            // Send the ShareConcurrencyReduction message to the model actor
            self.model_addr
                .try_send(ShareConcurrencyReduction {
                    shares_count: self.shares_in_flight,
                })
                .expect("failed to send ShareConcurrencyReduction message");
        }

        actix::Running::Stop
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        debug!("Twin {} actor stopped", self.twin.label);
    }
}

impl Handler<TwinCreationSuccess> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinCreationSuccess, _: &mut Context<Self>) -> Self::Result {
        debug!("Twin {} actor got message {:?}", self.twin.label, message);
        self.twin_did.replace(message.twin_did);

        // Send the TwinConcurrencyReduction message to the model actor
        self.model_addr
            .try_send(TwinConcurrencyReduction {
                twin_seed: Some(self.twin.seed.clone()),
            })
            .expect("failed to send TwinConcurrencyReduction message");

        self.creation_in_flight = false;
    }
}

impl Handler<TwinCreationFailure> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinCreationFailure, ctx: &mut Context<Self>) -> Self::Result {
        debug!("Twin {} actor got message {:?}", self.twin.label, message);

        // Send the TwinConcurrencyReduction message to the model actor
        self.model_addr
            .try_send(TwinConcurrencyReduction {
                twin_seed: Some(self.twin.seed.clone()),
            })
            .expect("failed to send TwinConcurrencyReduction message");

        self.creation_in_flight = false;

        // Stop the actor
        ctx.stop();
    }
}

impl Handler<TwinData> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: TwinData, ctx: &mut Context<Self>) -> Self::Result {
        // twin_did should never be None
        // because we're not sharing data until the twin is created
        let twin_did = self.twin_did.as_ref().expect("this should not happen");

        let shares_count = message.data.feeds.len();
        self.shares_in_flight += shares_count;

        let addr = ctx.address();
        self.last_data_received_at = SystemTime::now();

        let auth_builder = self.auth_builder.clone();
        let label = self.twin.label.clone();
        let twin_did = twin_did.clone();

        let twin_channel = self.twin_channel.clone();
        let feed_channel = self.feed_channel.clone();

        let fut = async move {
            for (feed_id, feed_data) in &message.data.feeds {
                let data = feed_data.to_string().as_bytes().to_vec();

                let result = share_data_with_channel(
                    auth_builder.clone(),
                    feed_channel.clone(),
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

                let result = update_twin_with_channel(
                    auth_builder,
                    twin_channel,
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

            // Send the ShareConcurrencyReduction to self
            addr.try_send(ShareConcurrencyReduction { shares_count })
                .expect("failed to send ShareConcurrencyReduction message to self");
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<ShareConcurrencyReduction> for TwinActor {
    type Result = ();

    fn handle(
        &mut self,
        message: ShareConcurrencyReduction,
        _: &mut Context<Self>,
    ) -> Self::Result {
        // Send the ShareConcurrencyReduction message to the model actor
        self.model_addr
            .try_send(message.clone())
            .expect("failed to send ShareConcurrencyReduction message");

        self.shares_in_flight -= message.shares_count;
    }
}

impl Handler<TwinDeleted> for TwinActor {
    type Result = ();

    fn handle(&mut self, _: TwinDeleted, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop();
    }
}

impl Handler<Cleanup> for TwinActor {
    type Result = ();

    fn handle(&mut self, message: Cleanup, ctx: &mut Context<Self>) -> Self::Result {
        let expire_at = self
            .last_data_received_at
            .checked_add(message.cleanup_every_secs)
            .expect("this should not happen");

        if SystemTime::now() > expire_at {
            if message.delete_twins {
                let twin_did = self.twin_did.clone().expect("This should not happen");
                let auth_builder = self.auth_builder.clone();
                let twin_channel = self.twin_channel.clone();
                let addr = ctx.address();

                let fut = async move {
                    let result =
                        delete_twin_with_channel(auth_builder.clone(), twin_channel, &twin_did)
                            .await;
                    if let Err(e) = result {
                        error!("Failed to delete twin {} {:?}.", &twin_did, e);
                    } else {
                        debug!("Twin {} deleted", &twin_did);
                        addr.try_send(TwinDeleted)
                            .expect("Failed to send TwinDeleted message");
                    }
                }
                .into_actor(self);

                ctx.spawn(fut);
            } else {
                ctx.stop();
            }
        }
    }
}
