//! a simple discord bot to move messages between channels
#![warn(clippy::cargo, clippy::nursery, clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::shadow_same,
    clippy::implicit_return,
    clippy::unseparated_literal_suffix,
    clippy::pattern_type_mismatch
)]

/// interaction commands
mod commands;
/// event handler
mod events;

use std::{env, path::PathBuf, sync::Arc};

use anyhow::{IntoResult, Result};
use futures::StreamExt;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_error::ErrorHandler;
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::id::{
    marker::{ApplicationMarker, UserMarker},
    Id,
};
use twilight_standby::Standby;
use twilight_webhook::cache::Cache as WebhooksCache;

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
    webhooks: WebhooksCache,
    /// error handler
    error_handler: ErrorHandler,
    /// used for permissions cache
    user_id: Id<UserMarker>,
    /// used for interaction requests and webhooks cache
    application_id: Id<ApplicationMarker>,
}

impl ContextValue {
    /// creates a new context value:
    async fn new(resource_types: ResourceType, http: Client) -> Result<Self> {
        let mut error_handler = ErrorHandler::new();
        let application = http
            .current_user_application()
            .exec()
            .await?
            .model()
            .await?;
        error_handler.channel(
            http.create_private_channel(application.owner.ok()?.id)
                .exec()
                .await?
                .model()
                .await?
                .id,
        );
        error_handler.file(PathBuf::from("message_mover_bot_errors.txt".to_owned()));
        Ok(Self {
            standby: Standby::new(),
            cache: InMemoryCache::builder()
                .resource_types(resource_types)
                .message_cache_size(20)
                .build(),
            webhooks: WebhooksCache::new(),
            error_handler,
            user_id: http.current_user().exec().await?.model().await?.id,
            application_id: application.id,
            http,
        })
    }
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
        ctx.webhooks.update(&event);
        events::request_members(&ctx, &cluster, shard_id, &event).await;
        tokio::spawn(events::handle(Arc::clone(&ctx), event));
    }

    Ok(())
}
