use anyhow::Result;
use twilight_gateway::Event;

use crate::{commands, webhooks, Context};

/// handles incoming events, prints on error
#[allow(clippy::print_stderr)]
pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = _handle(ctx, event).await {
        eprintln!("{err}");
    }
}

#[allow(clippy::wildcard_enum_match_arm)]
/// handles the event, returns the inner error
async fn _handle(ctx: Context, event: Event) -> Result<()> {
    match event {
        Event::InteractionCreate(interaction) => commands::handle(ctx, interaction.0).await?,
        Event::WebhooksUpdate(update) => webhooks::update(ctx, update.channel_id).await?,
        _ => (),
    }
    Ok(())
}
