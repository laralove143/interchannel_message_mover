use anyhow::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
    id::MessageId,
};

use crate::{webhooks, Context};

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
    let command_channel_id = command.channel_id;
    let command_member = command.member.unwrap();
    let command_member_id = command_member.user.unwrap().id;
    let command_member_can_manage_messages = command_member
        .permissions
        .unwrap()
        .contains(Permissions::MANAGE_MESSAGES);

    let options = MoveLastMessages::from_interaction(command.data.into())?;
    let target_channel_id = options.channel.id;
    let message_count = options.message_count;

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

    let message_ids = if let Some(message_ids) = ctx.cache.channel_messages(command_channel_id) {
        message_ids
            .take(message_count as usize)
            .collect::<Box<[MessageId]>>()
    } else {
        return Ok("i can only move messages that are sent after i joined.. sorry >.<");
    };

    for message_id in message_ids.iter().rev() {
        let message = ctx.cache.message(*message_id).unwrap();
        let author_id = message.author();

        if author_id != command_member_id && !command_member_can_manage_messages {
            return Ok(
                "this message isn't yours and you don't have **manage messages** permission! i'll \
                 stop here.",
            );
        }

        let author_member = message.member().unwrap();
        let author_user = ctx.cache.user(author_id).unwrap();

        let webhook_exec = ctx
            .http
            .execute_webhook(webhook.id, &webhook.token)
            .content(message.content())
            .username(author_member.nick.as_ref().unwrap_or(&author_user.name));

        if let Some(avatar) = &author_user.avatar {
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
            .delete_message(command_channel_id, message_ids[0])
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
