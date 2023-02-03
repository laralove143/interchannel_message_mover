use std::sync::Arc;

use anyhow::Result;
use twilight_gateway::{Cluster, Event};
use twilight_model::gateway::payload::outgoing::RequestGuildMembers;

use crate::{commands, Context};

/// handles the event and maybe the error
#[allow(clippy::print_stderr)]
pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = _handle(Arc::clone(&ctx), event).await {
        eprintln!("{err:#?}");
    }
}

/// handles the event, passing on the returned error
#[allow(clippy::wildcard_enum_match_arm)]
async fn _handle(ctx: Context, event: Event) -> Result<()> {
    match event {
        Event::InteractionCreate(interaction) => commands::handle(ctx, interaction.0).await?,
        Event::WebhooksUpdate(update) => {
            ctx.webhooks
                .validate(
                    &ctx.http,
                    update.channel_id,
                    ctx.cache
                        .permissions()
                        .in_channel(ctx.user_id, update.channel_id)?,
                )
                .await?;
        }
        _ => (),
    }
    Ok(())
}

/// if the event is a guild create event, sends the shard a command to request
/// the members, prints to stderr if it fails
#[allow(clippy::print_stderr)]
pub async fn request_members(cluster: &Cluster, shard_id: u64, event: &Event) {
    if let Event::GuildCreate(guild) = event {
        if let Err(err) = cluster
            .command(
                shard_id,
                &RequestGuildMembers::builder(guild.id).query("", None),
            )
            .await
        {
            eprintln!("{err:#?}");
        }
    };
}
