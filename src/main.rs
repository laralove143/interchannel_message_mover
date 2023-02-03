//! a simple discord bot to move messages between channels
#![warn(clippy::cargo, clippy::nursery, clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::shadow_same,
    clippy::implicit_return,
    clippy::unseparated_literal_suffix,
    clippy::pattern_type_mismatch,
    clippy::self_named_module_files,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core
)]

/// interaction commands
mod commands;
/// event handler
mod events;

use std::{any::type_name, env, sync::Arc, time::Duration};

use anyhow::{anyhow, Error, Result};
use chrono::{TimeZone, Utc};
use futures::StreamExt;
use tokio::time::interval;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::{
    gateway::{event::Event, payload::incoming::MessageDelete},
    id::{
        marker::{ApplicationMarker, UserMarker},
        Id,
    },
};
use twilight_standby::Standby;
use twilight_webhook::cache::WebhooksCache;

/// thread safe context
type Context = Arc<ContextValue>;

/// Trait implemented on types that can be converted to `anyhow::Result`
trait IntoResult<T>: Sized {
    /// Convert a type to an `anyhow::Result`
    fn ok(self) -> Result<T, Error>;
}

impl<T> IntoResult<T> for Option<T> {
    fn ok(self) -> Result<T, Error> {
        self.ok_or_else(|| anyhow!("Option<{}> is None", type_name::<T>()))
    }
}

/// inner of context
pub struct ContextValue {
    /// used to make http requests
    http: Client,
    /// used to wait for everyone to agree on the move
    standby: Standby,
    /// used to cache permissions and messages
    cache: InMemoryCache,
    /// webhooks cache
    webhooks: WebhooksCache,
    /// used for permissions cache
    user_id: Id<UserMarker>,
    /// used for interaction requests and webhooks cache
    application_id: Id<ApplicationMarker>,
}

impl ContextValue {
    /// creates a new context value:
    async fn new(resource_types: ResourceType, http: Client) -> Result<Self> {
        let application = http
            .current_user_application()
            .exec()
            .await?
            .model()
            .await?;
        Ok(Self {
            standby: Standby::new(),
            cache: InMemoryCache::builder()
                .resource_types(resource_types)
                .message_cache_size(20)
                .build(),
            webhooks: WebhooksCache::new(),
            user_id: http.current_user().exec().await?.model().await?.id,
            application_id: application.id,
            http,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let intents = Intents::GUILD_MESSAGES
        | Intents::MESSAGE_CONTENT
        | Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_WEBHOOKS;
    let event_types = EventTypeFlags::INTERACTION_CREATE
        | EventTypeFlags::MESSAGE_CREATE
        | EventTypeFlags::MESSAGE_UPDATE
        | EventTypeFlags::MESSAGE_DELETE
        | EventTypeFlags::MESSAGE_DELETE_BULK
        | EventTypeFlags::WEBHOOKS_UPDATE
        | EventTypeFlags::GUILD_CREATE
        | EventTypeFlags::GUILD_UPDATE
        | EventTypeFlags::GUILD_DELETE
        | EventTypeFlags::ROLE_CREATE
        | EventTypeFlags::ROLE_UPDATE
        | EventTypeFlags::ROLE_DELETE
        | EventTypeFlags::CHANNEL_CREATE
        | EventTypeFlags::CHANNEL_UPDATE
        | EventTypeFlags::CHANNEL_DELETE
        | EventTypeFlags::THREAD_CREATE
        | EventTypeFlags::THREAD_DELETE
        | EventTypeFlags::THREAD_UPDATE
        | EventTypeFlags::THREAD_LIST_SYNC
        | EventTypeFlags::THREAD_MEMBER_UPDATE
        | EventTypeFlags::THREAD_MEMBERS_UPDATE
        | EventTypeFlags::MEMBER_ADD
        | EventTypeFlags::MEMBER_UPDATE
        | EventTypeFlags::MEMBER_REMOVE
        | EventTypeFlags::MEMBER_CHUNK;
    let resource_types = ResourceType::MESSAGE
        | ResourceType::GUILD
        | ResourceType::ROLE
        | ResourceType::CHANNEL
        | ResourceType::MEMBER
        | ResourceType::USER;

    let token = env::var("MOVER_BOT_TOKEN")?;

    let (cluster, mut events) = Cluster::builder(token.clone(), intents)
        .event_types(event_types)
        .build()
        .await?;
    cluster.up().await;

    let http = Client::new(token);

    let ctx = Arc::new(ContextValue::new(resource_types, http).await?);

    tokio::spawn(cleanup_cache(Arc::clone(&ctx)));

    commands::create(&ctx).await?;

    while let Some((shard_id, event)) = events.next().await {
        ctx.standby.process(&event);
        ctx.cache.update(&event);
        ctx.webhooks.update(&event);
        events::request_members(&cluster, shard_id, &event).await;
        tokio::spawn(events::handle(Arc::clone(&ctx), event));
    }

    Ok(())
}

/// Delete messages older than 1 month
#[allow(clippy::arithmetic_side_effects, clippy::integer_arithmetic)]
async fn cleanup_cache(ctx: Context) {
    let month = Duration::from_secs(60 * 60 * 24 * 30);
    let month_chrono = chrono::Duration::seconds(60 * 60 * 24 * 30);

    let mut interval = interval(month);
    interval.tick().await;
    loop {
        interval.tick().await;

        let now = Utc::now();

        for message in ctx.cache.iter().messages() {
            let Some(message_time) = Utc.timestamp_opt(
                message
                    .edited_timestamp()
                    .unwrap_or_else(|| message.timestamp()).as_secs(), 0,
            ).single() else {
                continue;
            };

            if now - message_time > month_chrono {
                let delete_event = Event::MessageDelete(MessageDelete {
                    channel_id: message.channel_id(),
                    guild_id: message.guild_id(),
                    id: message.id(),
                });
                ctx.cache.update(&delete_event);
            }
        }
    }
}
