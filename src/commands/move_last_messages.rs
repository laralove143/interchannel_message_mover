use anyhow::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
};

use crate::{Cache, Context};

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

pub async fn run<'a>(ctx: Context, command: ApplicationCommand) -> Result<impl Into<&'a str>> {
    let channel_id = command.channel_id;

    if !command
        .member
        .unwrap()
        .permissions
        .unwrap()
        .contains(Permissions::MANAGE_MESSAGES)
    {
        return Ok("you need **manage messages** permission for that!");
    }

    let options = MoveLastMessages::from_interaction(command.data.into())?;

    let permissions_cache = ctx.twilight_cache.permissions();

    if !(permissions_cache
        .in_channel(ctx.cache.user_id, channel_id)?
        .contains(Permissions::MANAGE_MESSAGES)
        || permissions_cache
            .in_channel(ctx.cache.user_id, options.channel.id)?
            .contains(Permissions::MANAGE_WEBHOOKS))
    {
        return Ok("please give me **manage messages** and **manage webhooks** permissions >.<");
    }

    let messages = if let Some(messages) = ctx.cache.get_messages(channel_id) {
        messages
    } else {
        return Ok(
            "looks like i couldn't read anything here :( make sure i have **view channels** \
             permission",
        );
    };

    let webhook = Cache::get_webhook(&ctx, options.channel.id).await?;

    let mut message_ids = Vec::new();

    for message in messages.iter().take(options.message_count as usize) {
        match &message.avatar {
            Some(avatar) => {
                ctx.http
                    .execute_webhook(webhook.id, &webhook.token)
                    .content(&message.content)
                    .username(&message.username)
                    .avatar_url(&format!(
                        "https://cdn.discordapp.com/avatars/{}/{}.png",
                        avatar.0, avatar.1
                    ))
                    .exec()
                    .await?;
            }
            None => {
                ctx.http
                    .execute_webhook(webhook.id, &webhook.token)
                    .content(&message.content)
                    .username(&message.username)
                    .exec()
                    .await?;
            }
        }

        message_ids.push(message.id);
    }

    // TODO: fix this
    if message_ids.len() == 1 {
        ctx.http
            .delete_message(channel_id, message_ids[0])
            .exec()
            .await?;
    } else {
        ctx.http
            .delete_messages(channel_id, &message_ids)
            .exec()
            .await?;
    }

    Ok("done!")
}
