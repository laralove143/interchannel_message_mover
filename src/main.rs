mod commands;
mod events;

use std::{env, num::NonZeroU64, str::FromStr, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::id::GuildId;

type Context = Arc<ContextValue>;

pub struct ContextValue {
    http: Client,
    cache: InMemoryCache,
}

#[tokio::main]
async fn main() -> Result<()> {
    let intents = Intents::empty();
    let event_types = EventTypeFlags::INTERACTION_CREATE;
    let resource_types = ResourceType::MESSAGE;

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

    let cache = InMemoryCache::builder()
        .resource_types(resource_types)
        .message_cache_size(20)
        .build();

    let ctx = Arc::new(ContextValue { http, cache });

    while let Some((_, event)) = events.next().await {
        tokio::spawn(events::handle(ctx.clone(), event));
    }

    Ok(())
}
