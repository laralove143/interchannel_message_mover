mod move_last_messages;

use anyhow::{bail, Result};
use twilight_http::Client;
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{callback::InteractionResponse, interaction::Interaction},
    channel::message::MessageFlags,
    id::GuildId,
};
use twilight_util::builder::CallbackDataBuilder;

use crate::Context;

use self::move_last_messages::MoveLastMessages;

pub struct CommandResult {
    ctx: Context,
    token: String,
    reply: String,
}

impl<T: Into<String>> From<(Context, String, T)> for CommandResult {
    fn from(result: (Context, String, T)) -> Self {
        Self {
            ctx: result.0,
            token: result.1,
            reply: result.2.into(),
        }
    }
}

pub async fn handle(ctx: Context, interaction: Interaction) -> Result<()> {
    let command = if let Interaction::ApplicationCommand(command) = interaction {
        *command
    } else {
        bail!(
            "interaction is not an application command: {:?}",
            interaction
        );
    };

    let interaction_id = command.id;

    let result = match command.data.name.as_ref() {
        "move_last_messages" => move_last_messages::run(ctx, command).await?,
        _ => bail!("unexpected command name: {}", command.data.name),
    };

    result
        .ctx
        .http
        .interaction_callback(
            interaction_id,
            &result.token,
            &InteractionResponse::ChannelMessageWithSource(
                CallbackDataBuilder::new()
                    .content(result.reply)
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
        )
        .exec()
        .await?;

    Ok(())
}

pub async fn create(http: &Client, guild_id: GuildId) -> Result<()> {
    http.set_guild_commands(guild_id, &[MoveLastMessages::create_command().into()])?
        .exec()
        .await?;

    Ok(())
}
