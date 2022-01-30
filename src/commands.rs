/// `move_last_messages` command
mod move_last_messages;

use std::{convert::Into, mem, sync::Arc};

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

/// handle an interaction
/// 1. get the inner command if it's an application command, return an error
/// otherwise 2. copy and save the `interaction_id`
/// 3. mutate command to take and save the token
/// 4. defer the command with an empty response to avoid it being invalidated
/// while processing 5. run the command and save its result
/// 6. respond the command with result saved or the generic error message
/// 6. return the saved error, if any
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
        "move_last_messages" => move_last_messages::run(Arc::clone(&ctx), command).await,
        _ => Err(anyhow!("unexpected command name: {}", command.data.name)),
    }
    .map(Into::into);

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

/// create the commands globally
pub async fn create(http: &Client) -> Result<()> {
    http.set_global_commands(&[MoveLastMessages::create_command().into()])?
        .exec()
        .await?;

    Ok(())
}
