mod move_last_messages;

use std::mem;

use anyhow::{bail, Result};
use twilight_http::Client;
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{callback::InteractionResponse, interaction::Interaction},
    channel::message::MessageFlags,
    id::GuildId,
};
use twilight_util::builder::CallbackDataBuilder;

use self::move_last_messages::MoveLastMessages;
use crate::Context;

pub async fn handle(ctx: Context, interaction: Interaction) -> Result<()> {
    let mut command = if let Interaction::ApplicationCommand(command) = interaction {
        *command
    } else {
        bail!(
            "interaction is not an application command: {:?}",
            interaction
        );
    };

    let interaction_id = command.id;
    let token = mem::take(&mut command.token);

    let reply = match command.data.name.as_ref() {
        "move_last_messages" => move_last_messages::run(ctx.clone(), command).await?,
        _ => bail!("unexpected command name: {}", command.data.name),
    };

    ctx.http
        .interaction_callback(
            interaction_id,
            &token,
            &InteractionResponse::ChannelMessageWithSource(
                CallbackDataBuilder::new()
                    .content(reply.into())
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
