mod move_last_messages;

use std::mem;

use anyhow::{anyhow, bail, Result};
use twilight_http::Client;
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{callback::InteractionResponse, interaction::Interaction},
    channel::message::MessageFlags,
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

    ctx.http
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

    let result = match command.data.name.as_ref() {
        "move_last_messages" => move_last_messages::run(ctx.clone(), command).await,
        _ => Err(anyhow!("unexpected command name: {}", command.data.name)),
    }
    .map(|reply| reply.into());

    ctx.http
        .create_followup_message(&token)?
        .content(result.as_ref().unwrap_or(
            &"sorry.. there was an error >.< i'll let my developer know, hopefully she'll fix it \
              soon!",
        ))
        .exec()
        .await?;

    result.map(|_| ())
}

pub async fn create(http: &Client) -> Result<()> {
    http.set_global_commands(&[MoveLastMessages::create_command().into()])?
        .exec()
        .await?;

    Ok(())
}
