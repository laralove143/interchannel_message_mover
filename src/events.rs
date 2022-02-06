use anyhow::Result;
use twilight_gateway::{Cluster, Event};
use twilight_model::gateway::payload::outgoing::RequestGuildMembers;

use crate::{commands, webhooks, Context};

/// handles the event, prints the returned error to stderr
#[allow(clippy::print_stderr)]
pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = _handle(ctx, event).await {
        eprintln!("{err}");
    }
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
/// the members and prints a message
#[allow(clippy::print_stdout)]
pub async fn request_members(cluster: &Cluster, shard_id: u64, event: &Event) -> Result<()> {
    if let Event::GuildCreate(guild) = event {
        cluster
            .command(
                shard_id,
                &RequestGuildMembers::builder(guild.id).query("", None),
            )
            .await?;
        println!("requested members");
    };
    Ok(())
}
