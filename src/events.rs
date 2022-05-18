use std::sync::Arc;

use anyhow::{IntoResult, Result};
use twilight_gateway::{Cluster, Event};
use twilight_http::Client;
use twilight_model::gateway::payload::outgoing::RequestGuildMembers;

use crate::{commands, webhooks, Context};

/// handles the event, prints the returned error to stderr and tells the owner
#[allow(clippy::print_stderr)]
pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = _handle(Arc::clone(&ctx), event).await {
        if let Err(inform_error) = inform_owner(&ctx.http).await {
            eprintln!("informing the owner also failed: {}", inform_error);
        }
        eprintln!("{err}");
    }
}

/// tell the owner there was an error
async fn inform_owner(http: &Client) -> Result<()> {
    http.create_message(
        http.create_private_channel(
            http.current_user_application()
                .exec()
                .await?
                .model()
                .await?
                .owner
                .ok()?
                .id,
        )
        .exec()
        .await?
        .model()
        .await?
        .id,
    )
    .content("an error occurred :( check the stderr")?
    .exec()
    .await?;

    Ok(())
}

/// handles the event, passing on the returned error
#[allow(clippy::wildcard_enum_match_arm)]
async fn _handle(ctx: Context, event: Event) -> Result<()> {
    match event {
        Event::InteractionCreate(interaction) => commands::handle(ctx, interaction.0).await?,
        Event::WebhooksUpdate(update) => webhooks::update(ctx, update.channel_id).await?,
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
            eprintln!("{err}");
        }
    };
}
