//! description = "a simple discord bot to move messages between channels"
#![allow(clippy::shadow_same, clippy::implicit_return)]

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
use webhooks::CachedWebhook;

/// thread safe context
type Context = Arc<ContextValue>;

/// inner of context
pub struct ContextValue {
    /// used to make http requests
    http: Client,
    /// used to cache permissions and messages
    cache: InMemoryCache,
    /// webhooks cache
    webhooks: DashMap<Id<ChannelMarker>, CachedWebhook>,
    /// used for permissions cache
    user_id: Id<UserMarker>,
    /// used for interaction requests and webhooks cache
    application_id: Id<ApplicationMarker>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let intents = Intents::GUILD_MESSAGES | Intents::GUILDS | Intents::GUILD_WEBHOOKS;
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
        | EventTypeFlags::MEMBER_REMOVE;
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
    let cluster_spawn = Arc::new(cluster);
    tokio::spawn(async move { cluster_spawn.up().await });

    let http = Client::new(token);

    let ctx = Arc::new(ContextValue {
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
    });

    commands::create(&ctx).await?;

    while let Some((_, event)) = events.next().await {
        ctx.cache.update(&event);
        tokio::spawn(events::handle(Arc::clone(&ctx), event));
    }

    Ok(())
}
