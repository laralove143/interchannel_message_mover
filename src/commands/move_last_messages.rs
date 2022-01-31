use anyhow::{Context as _, Result};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
    id::{marker::MessageMarker, Id},
};

use crate::{webhooks, Context};

/// `move_last_messages` command struct for `twilight_interactions`
#[derive(CreateCommand, CommandModel)]
#[command(
    name = "move_last_messages",
    desc = "move the newest messages from this channel to another channel"
)]
pub struct MoveLastMessages {
    /// the number of messages to move
    #[command(
        desc = "how many of the newest messages do you want to move?",
        min_value = 1,
        max_value = 20
    )]
    message_count: i64,
    /// the target channel
    #[command(
        desc = "where do you want to move the messages?",
        channel_types = "guild_text guild_public_thread guild_private_thread"
    )]
    channel: InteractionChannel,
}

/// run the command
pub async fn run<'a>(ctx: Context, command: ApplicationCommand) -> Result<impl Into<&'a str>> {
    let command_channel_id = command.channel_id;
    let command_member = command.member.context("command is not run in a guild")?;
    let command_member_id = command_member
        .user
        .context("the member object is attached to MESSAGE_CREATE or MESSAGE_UPDATE events")?
        .id;
    let command_member_can_manage_messages = command_member
        .permissions
        .context("the member object is not attached to an interaction")?
        .contains(Permissions::MANAGE_MESSAGES);

    let options = MoveLastMessages::from_interaction(command.data.into())?;
    let target_channel_id = options.channel.id;
    let message_count: u8 = options.message_count.try_into()?;

    let permissions_cache = ctx.cache.permissions();
    if !(permissions_cache
        .in_channel(ctx.user_id, command_channel_id)?
        .contains(Permissions::MANAGE_MESSAGES | Permissions::VIEW_CHANNEL)
        && permissions_cache
            .in_channel(ctx.user_id, target_channel_id)?
            .contains(Permissions::MANAGE_WEBHOOKS))
    {
        return Ok(
            "please make sure i have **view channels**, **manage webhooks** and **manage \
             messages** permissions >.<",
        );
    }

    let webhook = webhooks::get(&ctx, options.channel.id).await?;

    let message_ids = ctx
        .cache
        .channel_messages(command_channel_id)
        .map(|ids| {
            ids.take(message_count.into())
                .collect::<Box<[Id<MessageMarker>]>>()
        })
        .unwrap_or_default();

    if message_ids.is_empty() {
        return Ok("i can only move messages that are sent after i joined.. sorry >.<");
    };

    for message_id in message_ids.iter().rev() {
        let message = ctx
            .cache
            .message(*message_id)
            .context("message is not cached")?;
        let author_id = message.author();

        if author_id != command_member_id && !command_member_can_manage_messages {
            return Ok(
                "this message isn't yours and you don't have **manage messages** permission! i'll \
                 stop here.",
            );
        }

        let author_member = message
            .member()
            .context("cached message doesn't have a member")?;
        let author_user = ctx
            .cache
            .user(author_id)
            .context("message author user is not cached")?;

        let webhook_exec = ctx
            .http
            .execute_webhook(webhook.id, &webhook.token)
            .content(message.content())?
            .username(author_member.nick.as_ref().unwrap_or(&author_user.name));

        if let Some(ref avatar) = author_user.avatar {
            webhook_exec
                .avatar_url(&format!(
                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                    author_user.id, avatar
                ))
                .exec()
                .await?;
        } else {
            webhook_exec.exec().await?;
        }
    }

    if message_ids.len() == 1 {
        ctx.http
            .delete_message(
                command_channel_id,
                *message_ids.get(0).context("message ids is empty")?,
            )
            .exec()
            .await?;
    } else {
        ctx.http
            .delete_messages(command_channel_id, &message_ids)
            .exec()
            .await?;
    }

    Ok("done!")
}
