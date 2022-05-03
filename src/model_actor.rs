use std::cmp::{self, max};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use actix::clock::{interval, sleep};
use actix::{Actor, Addr, AsyncContext, Context, Handler, System, WrapFuture};
use iotics_identity::create_twin_did_with_control_delegation;
use log::{debug, error, info, warn};
use serde_json::json;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use iotics_grpc_client::common::Channel;
use iotics_grpc_client::twin::share::share_data_with_client;
use iotics_grpc_client::twin::upsert::upsert_twin_with_client;
use iotics_grpc_client::twin::{
    create_feed_api_client, create_twin_api_client, FeedApiClient, TwinApiClient,
};

use crate::config::AuthBuilder;
use crate::connector::Connector;
use crate::constants::{
    AGENT_TWIN_NAME, CONCURRENT_NEW_TWINS_LIMIT, CONCURRENT_SHARES_LIMIT, NEW_TWINS_SHARE_TICK_CAP,
    RESCHEDULE_DELAY,
};
use crate::messages::{
    ChannelsCreatedMessage, Cleanup, GetData, HeartbeatData, ShareConcurrencyReduction,
    TwinConcurrencyReduction, TwinData,
};
use crate::model::Model;
use crate::twin::Twin;
use crate::twin_actor::TwinActor;

#[derive(Debug, Clone)]
pub struct TwinActorInfo {
    addr: Addr<TwinActor>,
    created: bool,
}

#[derive(Debug)]
pub struct ModelActor {
    auth_builder: Arc<AuthBuilder>,
    model: Model,
    data_getter: Arc<dyn Connector>,
    fetch_every_secs: u64,
    delete_twins: bool,
    twins: HashMap<String, TwinActorInfo>,
    twin_channel: Option<TwinApiClient<Channel>>,
    feed_channel: Option<FeedApiClient<Channel>>,
    concurrent_new_twins: usize,
    concurrent_shares: usize,
    previously_unhandled_twins: usize,
}

impl ModelActor {
    pub fn new(
        model: Model,
        fetch_every_secs: u64,
        data_getter: Arc<dyn Connector>,
        delete_twins: bool,
    ) -> Self {
        let auth_builder = AuthBuilder::new();

        Self {
            auth_builder,
            model,
            fetch_every_secs,
            data_getter,
            delete_twins,
            twins: HashMap::new(),
            twin_channel: None,
            feed_channel: None,
            concurrent_new_twins: 0,
            concurrent_shares: 0,
            previously_unhandled_twins: 0,
        }
    }
}

impl Actor for ModelActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(32768);
        let model_label = self.model.get_label();
        info!("[{}] Model actor started", &model_label);

        let auth_builder = self.auth_builder.clone();
        let addr = ctx.address();

        // create the channels
        let fut = async move {
            let twin_channel = create_twin_api_client(auth_builder.clone())
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "[{}] failed to create the twin channel: {:?}",
                        &model_label, e
                    )
                });
            let feed_channel = create_feed_api_client(auth_builder)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "[{}] failed to create the feed channel: {:?}",
                        &model_label, e
                    )
                });

            addr.send(ChannelsCreatedMessage {
                twin_channel,
                feed_channel,
            })
            .await
            .expect("failed to send message");
        }
        .into_actor(self);

        ctx.spawn(fut);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        error!("[{}] Model actor stopped", &self.model.get_label());

        // currently stopping the system if the upsert of the model fails
        // TODO: add some retry mechanism
        System::current().stop_with_code(1);
    }
}

impl Handler<ChannelsCreatedMessage> for ModelActor {
    type Result = ();

    fn handle(&mut self, message: ChannelsCreatedMessage, ctx: &mut Context<Self>) -> Self::Result {
        let mut twin_channel = message.twin_channel;
        let feed_channel = message.feed_channel;

        self.twin_channel.replace(twin_channel.clone());
        self.feed_channel.replace(feed_channel);

        let addr = ctx.address();

        let auth_builder = self.auth_builder.clone();
        let identity_config = auth_builder
            .get_identity_config()
            .expect("failed to get identity config");
        let model = self.model.clone();
        let fetch_every_secs = self.fetch_every_secs;

        // upsert the model and start the update interval
        let fut = async move {
            let result = async {
                let model_did = create_twin_did_with_control_delegation(
                    &identity_config,
                    &model.get_seed(),
                    AGENT_TWIN_NAME,
                )?;

                upsert_twin_with_client(
                    auth_builder,
                    &mut twin_channel,
                    &model_did,
                    model.get_model_properties().clone(),
                    model.get_feeds(true),
                    None,
                    model.get_visibility() as i32,
                )
                .await?;

                Ok::<String, anyhow::Error>(model_did)
            };

            let model_label = model.get_label();

            match result.await {
                Ok(model_did) => {
                    debug!("[{}] model did {}", &model_label, &model_did);

                    loop {
                        // start the fetch data timer
                        let now = SystemTime::now();

                        addr.send(GetData {
                            model_did: model_did.clone(),
                        })
                        .await
                        .unwrap_or_else(|_| panic!("[{}] failed to send message", &model_label));

                        let elapsed = now.elapsed().expect("this should not happen").as_secs();
                        let sleep_amount = max(0, fetch_every_secs - elapsed);

                        sleep(Duration::from_secs(sleep_amount)).await;
                    }
                }
                Err(e) => {
                    panic!("[{}] failed to create model: {}", &model_label, e);
                }
            }
        }
        .into_actor(self);

        ctx.spawn(fut);

        // start the cleanup timer
        let addr = ctx.address();
        let model_label = self.model.get_label();
        let delete_twins = self.delete_twins;

        let fut = async move {
            // set the cleanup interval to be 3.5 bigger than the fetch interval
            let cleanup_every_secs = (fetch_every_secs as f64 * 3.5) as u64;
            let mut interval = interval(Duration::from_secs(cleanup_every_secs));

            loop {
                interval.tick().await;
                addr.try_send(Cleanup { delete_twins })
                    .unwrap_or_else(|_| panic!("[{}] failed to send twins cleanup", &model_label));
            }
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<GetData> for ModelActor {
    type Result = ();

    fn handle(&mut self, message: GetData, ctx: &mut Context<Self>) -> Self::Result {
        let model_label = self.model.get_label();

        let addr = ctx.address();
        let model_did = message.model_did;
        let twins = self.twins.clone();
        let data_getter = self.data_getter.clone();
        let fetch_every_secs = self.fetch_every_secs;
        let concurrent_new_twins = self.concurrent_new_twins;
        let concurrent_shares = self.concurrent_shares;
        let previously_unhandled_twins = self.previously_unhandled_twins;

        // reset previously_unhandled_twins
        self.previously_unhandled_twins = 0;

        let fut = async move {
            info!("[{}] Requesting data", &model_label);
            let results = data_getter.get_data().await;

            // Allow creating new Twin Actors and sharing data to existing Twin Actors
            // only for a specific time period for better host performance
            let expire_after_secs =
                (fetch_every_secs as f64 * NEW_TWINS_SHARE_TICK_CAP) as u64;
            let expire_time = SystemTime::now()
                .checked_add(Duration::from_secs(expire_after_secs)).unwrap_or_else(|| {
                    panic!("[{}] this should not happen", &model_label)
                });

            match results {
                Ok(results) => {
                    info!("[{}] Got data for {} twins", &model_label, results.len());
                    info!(
                        "[{}] There are {} twins currently running, {} unhandled twins in the last run", &model_label,
                        twins.len(),
                        previously_unhandled_twins,
                    );

                    if concurrent_new_twins > 0 || concurrent_shares > 0 {
                        warn!(
                            "[{}] There are {} concurrent new twins and {} concurrent shares. Both should be 0.", &model_label,
                            concurrent_new_twins,
                            concurrent_shares,
                        );
                    }

                    let mut shares = 0;

                    for data in results {
                        addr.try_send(TwinData {
                            model_did: model_did.clone(),
                            data,
                            expire_time,
                        }).unwrap_or_else(|_| {
                            panic!("[{}] failed to send received data", &model_label)
                        });

                        shares += 1;
                    }

                    addr.try_send(HeartbeatData {
                        model_did: model_did.clone(),
                        shares,
                    }).unwrap_or_else(|_| {
                        panic!("[{}] failed to send received data", &model_label)
                    });
                }
                Err(e) => {
                    error!("[{}] failed to receive data {}", &model_label, e);
                }
            };
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<TwinData> for ModelActor {
    type Result = ();

    fn handle(&mut self, message: TwinData, ctx: &mut Context<Self>) -> Self::Result {
        if SystemTime::now() > message.expire_time {
            // the message is expired - drop it
            self.previously_unhandled_twins += 1;
            return;
        }

        let model = self.model.clone();
        let twin_seed = model.get_twin_seed(&message.data.id);
        let twin_addr = self.twins.get(&twin_seed);
        let twin_channel = self
            .twin_channel
            .as_ref()
            .expect("this should not happen")
            .clone();
        let feed_channel = self
            .feed_channel
            .as_ref()
            .expect("this should not happen")
            .clone();

        let start_twin = match twin_addr {
            Some(twin) => !twin.addr.connected(),
            None => true,
        };

        if start_twin {
            // Throttle the creation of new twin actors for better host performance
            if self.concurrent_new_twins > CONCURRENT_NEW_TWINS_LIMIT {
                // re-schedule the message
                ctx.notify_later(message, RESCHEDULE_DELAY);
                return;
            }

            let twin_label = model.get_twin_label(&message.data.label);

            let twin_actor = TwinActor::new(
                ctx.address(),
                self.auth_builder.clone(),
                Twin::new(
                    message.model_did.clone(),
                    twin_seed.clone(),
                    twin_label,
                    message.data.location.clone(),
                ),
                model,
                twin_channel,
                feed_channel,
            );

            let addr = twin_actor.start();

            self.twins.insert(
                twin_seed.clone(),
                TwinActorInfo {
                    addr,
                    created: false,
                },
            );

            self.concurrent_new_twins += 1;
        };

        let twin_actor = self
            .twins
            .get(&twin_seed)
            .unwrap_or_else(|| panic!("[{}] this should not happen", &self.model.get_label()));

        // Throttle the sharing of data for better host performance
        if !twin_actor.created
            || self.concurrent_shares + message.data.feeds.len() > CONCURRENT_SHARES_LIMIT
        {
            // re-schedule the message
            ctx.notify_later(message, RESCHEDULE_DELAY);
            return;
        }

        self.concurrent_shares += message.data.feeds.len();
        // Share/update twin data & properties
        twin_actor.addr.do_send(message);
    }
}

impl Handler<TwinConcurrencyReduction> for ModelActor {
    type Result = ();

    fn handle(&mut self, message: TwinConcurrencyReduction, _: &mut Context<Self>) -> Self::Result {
        if let Some(twin_seed) = message.twin_seed {
            if let Some(twin_actor) = self.twins.get_mut(&twin_seed) {
                twin_actor.created = true;
            }
        }

        self.concurrent_new_twins = cmp::max(self.concurrent_new_twins - 1, 0);
    }
}

impl Handler<ShareConcurrencyReduction> for ModelActor {
    type Result = ();

    fn handle(
        &mut self,
        message: ShareConcurrencyReduction,
        _: &mut Context<Self>,
    ) -> Self::Result {
        self.concurrent_shares = cmp::max(self.concurrent_shares - message.amount, 0);
    }
}

impl Handler<HeartbeatData> for ModelActor {
    type Result = ();

    fn handle(&mut self, message: HeartbeatData, ctx: &mut Context<Self>) -> Self::Result {
        let mut feed_channel = self
            .feed_channel
            .as_ref()
            .expect("this should not happen")
            .clone();
        let model_label = self.model.get_label();

        let model_did = message.model_did.clone();
        let auth_builder = self.auth_builder.clone();

        let data = json!({
            "timestamp": OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("this should not happen"),
            "shares": message.shares,
        })
        .to_string()
        .as_bytes()
        .to_vec();

        let fut = async move {
            let result = share_data_with_client(
                auth_builder,
                &mut feed_channel,
                &model_did,
                "heartbeat",
                data,
                true,
            )
            .await;

            if let Err(e) = result {
                error!(
                    "[{}] failed to share model heartbeat data {:?}",
                    &model_label, e
                );
            } else {
                debug!("[{}] Model shared model heartbeat data", &model_label);
            }
        }
        .into_actor(self);

        ctx.spawn(fut);
    }
}

impl Handler<Cleanup> for ModelActor {
    type Result = ();

    fn handle(&mut self, _message: Cleanup, _ctx: &mut Context<Self>) -> Self::Result {
        let model_label = self.model.get_label();
        info!("[{}] Twin cleanup", &model_label);

        let mut to_remove = Vec::new();
        let delete_twins = self.delete_twins;

        for (twin_did, twin_actor) in &self.twins {
            if !twin_actor.addr.connected() {
                to_remove.push(twin_did.clone());
            } else {
                twin_actor
                    .addr
                    .try_send(Cleanup { delete_twins })
                    .unwrap_or_else(|_| panic!("[{}] failed to send twins cleanup", &model_label));
            }
        }

        for twin_did in to_remove.iter() {
            self.twins.remove(twin_did);
        }
    }
}
