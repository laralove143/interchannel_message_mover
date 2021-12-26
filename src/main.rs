mod commands;
mod events;
mod webhooks;

use std::{env, num::NonZeroU64, str::FromStr, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::id::{ChannelId, GuildId, UserId};
use webhooks::CachedWebhook;
type Context = Arc<ContextValue>;
use dashmap::DashMap;
pub struct ContextValue {
    http: Client,
    cache: InMemoryCache,
    webhooks: DashMap<ChannelId, CachedWebhook>,
    user_id: UserId,
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

    let token = env::var("TEST_BOT_TOKEN")?;
    let guild_id = GuildId(NonZeroU64::from_str(&env::var("GUILD_ID")?)?);

    let (cluster, mut events) = Cluster::builder(&token, intents)
        .event_types(event_types)
        .build()
        .await?;
    let cluster_spawn = Arc::new(cluster);
    tokio::spawn(async move { cluster_spawn.up().await });

    let http = Client::new(token);
    http.set_application_id(
        http.current_user_application()
            .exec()
            .await?
            .model()
            .await?
            .id,
    );
    commands::create(&http, guild_id).await?;

    let ctx = Arc::new(ContextValue {
        user_id: http.current_user().exec().await?.model().await?.id,
        http,
        cache: InMemoryCache::builder()
            .resource_types(resource_types)
            .message_cache_size(20)
            .build(),
        webhooks: DashMap::new(),
    });

    while let Some((_, event)) = events.next().await {
        ctx.cache.update(&event);
        tokio::spawn(events::handle(ctx.clone(), event));
    }

    Ok(())
}
