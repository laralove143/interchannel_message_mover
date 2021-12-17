use anyhow::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::{
    application_command::InteractionChannel, ApplicationCommand,
};

use crate::Context;

use super::CommandResult;

#[derive(CreateCommand, CommandModel)]
#[command(
    name = "move_last_messages",
    desc = "move the newest messages from this channel to another channel"
)]
pub struct MoveLastMessages {
    #[command(
        desc = "how many of the newest messages do you want to move?",
        min_value = 1,
        max_value = 20
    )]
    message_count: i64,
    #[command(
        desc = "where do you want to move the messages?",
        channel_types = "guild_text guild_public_thread guild_private_thread"
    )]
    channel: InteractionChannel,
}

pub async fn run(ctx: Context, command: ApplicationCommand) -> Result<CommandResult> {
    let token = command.token;
    let options = MoveLastMessages::from_interaction(command.data.into())?;

    Ok((ctx, token, "Done!").into())
}
