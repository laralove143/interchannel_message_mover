//! "a simple discord bot to move messages between channels"
// TODO! check docs
#![allow(
    clippy::shadow_same,
    clippy::implicit_return,
    clippy::unseparated_literal_suffix,
    clippy::pattern_type_mismatch
)]

/// interaction commands
mod commands;
/// event hander
mod events;
/// webhooks cache
mod webhooks;

use std::{env, sync::Arc};

use anyhow::Result;
use dashmap::DashMap;
use futures::StreamExt;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::id::{
    marker::{ApplicationMarker, ChannelMarker, UserMarker},
    Id,
};
use twilight_standby::Standby;
use webhooks::CachedWebhook;

/// thread safe context
type Context = Arc<ContextValue>;

/// inner of context
pub struct ContextValue {
    /// used to make http requests
    http: Client,
    /// used to wait for everyone to agree on the move
    standby: Standby,
    /// used to cache permissions and messages
    cache: InMemoryCache,
    /// webhooks cache
    webhooks: DashMap<Id<ChannelMarker>, CachedWebhook>,
    /// used for permissions cache
    user_id: Id<UserMarker>,
    /// used for interaction requests and webhooks cache
    application_id: Id<ApplicationMarker>,
}

impl ContextValue {
    /// creates a new context value:
    async fn new(resource_types: ResourceType, http: Client) -> Result<Self> {
        Ok(Self {
            standby: Standby::new(),
            cache: InMemoryCache::builder()
                .resource_types(resource_types)
                .message_cache_size(20)
                .build(),
            webhooks: DashMap::new(),
            user_id: http.current_user().exec().await?.model().await?.id,
            application_id: http
                .current_user_application()
                .exec()
                .await?
                .model()
                .await?
                .id,
            http,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let intents = Intents::GUILD_MESSAGES
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
        | EventTypeFlags::MEMBER_ADD
        | EventTypeFlags::MEMBER_UPDATE
        | EventTypeFlags::MEMBER_REMOVE
        | EventTypeFlags::MEMBER_CHUNK;
    let resource_types = ResourceType::MESSAGE
        | ResourceType::GUILD
        | ResourceType::ROLE
        | ResourceType::CHANNEL
        | ResourceType::MEMBER;

    let token = env::var("MOVER_BOT_TOKEN")?;

    let (cluster, mut events) = Cluster::builder(token.clone(), intents)
        .event_types(event_types)
        .build()
        .await?;
    cluster.up().await;

    let http = Client::new(token);

    let ctx = Arc::new(ContextValue::new(resource_types, http).await?);

    commands::create(&ctx).await?;

    while let Some((shard_id, event)) = events.next().await {
        ctx.standby.process(&event);
        ctx.cache.update(&event);
        events::request_members(&cluster, shard_id, &event).await?;
        tokio::spawn(events::handle(Arc::clone(&ctx), event));
    }

    Ok(())
}
