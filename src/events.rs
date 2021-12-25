use anyhow::Result;
use twilight_gateway::Event;

use crate::{commands, Cache, Context};

pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = _handle(ctx, event).await {
        eprintln!("{}", err);
    }
}

async fn _handle(ctx: Context, event: Event) -> Result<()> {
    match event {
        Event::InteractionCreate(interaction) => commands::handle(ctx, interaction.0).await?,
        Event::MessageCreate(message) => ctx.cache.add_message(message.0),
        Event::MessageUpdate(message) => ctx.cache.update_message(*message),
        Event::MessageDelete(message) => ctx.cache.delete_message(message),
        Event::MessageDeleteBulk(messages) => ctx.cache.delete_messages(messages),
        Event::WebhooksUpdate(update) => Cache::update_webhooks(ctx, update.channel_id).await?,
        _ => (),
    }
    Ok(())
}
