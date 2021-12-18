mod cache;
mod commands;
mod events;

use std::{env, num::NonZeroU64, str::FromStr, sync::Arc};

use anyhow::Result;
use cache::Cache;
use futures::StreamExt;
use twilight_gateway::{Cluster, EventTypeFlags, Intents};
use twilight_http::Client;
use twilight_model::id::GuildId;

type Context = Arc<ContextValue>;

pub struct ContextValue {
    http: Client,
    cache: Cache,
}

#[tokio::main]
async fn main() -> Result<()> {
    let intents = Intents::GUILD_MESSAGES;
    let event_types = EventTypeFlags::INTERACTION_CREATE
        | EventTypeFlags::MESSAGE_CREATE
        | EventTypeFlags::MESSAGE_UPDATE
        | EventTypeFlags::MESSAGE_DELETE
        | EventTypeFlags::MESSAGE_DELETE_BULK;

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
        http,
        cache: Cache::new(),
    });

    while let Some((_, event)) = events.next().await {
        tokio::spawn(events::handle(ctx.clone(), event));
    }

    Ok(())
}
