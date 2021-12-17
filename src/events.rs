use anyhow::anyhow;
use twilight_gateway::Event;

use crate::{commands, Context};

pub async fn handle(ctx: Context, event: Event) {
    if let Err(err) = match event {
        Event::InteractionCreate(interaction) => commands::handle(ctx, interaction.0).await,
        _ => Err(anyhow!("unexpected event: {:?}", event)),
    } {
        eprintln!("{}", err);
    }
}
