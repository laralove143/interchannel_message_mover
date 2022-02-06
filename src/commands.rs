/// `move_last_messages` command
mod move_last_messages;

use std::{convert::Into, mem, sync::Arc};

use anyhow::{anyhow, bail, Result};
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{callback::InteractionResponse, interaction::Interaction},
    channel::message::MessageFlags,
};
use twilight_util::builder::CallbackDataBuilder;

use self::move_last_messages::MoveLastMessages;
use crate::Context;

/// handle an interaction passing the returned error
#[allow(clippy::wildcard_enum_match_arm)]
pub async fn handle(ctx: Context, interaction: Interaction) -> Result<()> {
    let mut command = match interaction {
        Interaction::ApplicationCommand(command) => *command,
        Interaction::MessageComponent(_) => return Ok(()),
        _ => {
            bail!(
                "interaction is not an application command: {:?}",
                interaction
            );
        }
    };

    let interaction_id = command.id;
    let token = mem::take(&mut command.token);

    let client = ctx.http.interaction(ctx.application_id);

    client
        .interaction_callback(
            interaction_id,
            &token,
            &InteractionResponse::DeferredChannelMessageWithSource(
                CallbackDataBuilder::new()
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
        )
        .exec()
        .await?;

    if let Err(err) = match command.data.name.as_ref() {
        "move_last_messages" => {
            move_last_messages::run(Arc::clone(&ctx), &client, &token, command).await
        }
        _ => Err(anyhow!("unexpected command name: {}", command.data.name)),
    } {
        client
            .update_interaction_original(&token)
            .content(Some(
                "sorry.. there was an error >.< i'll let my developer know, hopefully she'll fix \
                 it soon!",
            ))?
            .exec()
            .await?;
        bail!(err);
    };

    Ok(())
}

/// create the commands globally
pub async fn create(ctx: &Context) -> Result<()> {
    ctx.http
        .interaction(ctx.application_id)
        .set_global_commands(&[MoveLastMessages::create_command().into()])
        .exec()
        .await?;

    Ok(())
}
