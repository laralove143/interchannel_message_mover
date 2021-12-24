use anyhow::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
};

use crate::Context;

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

pub async fn run(ctx: Context, command: ApplicationCommand) -> Result<impl Into<String>> {
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

    let messages = match ctx.cache.get_messages(channel_id) {
        Some(messages) => messages,
        None => {
            return Ok(
                "looks like i couldn't read anything here :( make sure i have **view channels** \
                 permission",
            )
        }
    };

    let webhook = match ctx.cache.get_webhook(options.channel.id) {
        Some(webhook) => webhook,
        None => {
            let webhook = ctx
                .http
                .create_webhook(options.channel.id, "message transit")
                .exec()
                .await?
                .model()
                .await?;
            ctx.cache.add_webhook(webhook);
            ctx.cache.get_webhook(options.channel.id).unwrap()
        }
    };

    let mut message_ids = Vec::new();

    for message in messages.iter().take(options.message_count as usize) {
        match &message.avatar {
            Some(avatar) => {
                let mut avatar_url = "https://cdn.discordapp.com/avatars/".to_string();
                avatar_url.push_str(&avatar.0.to_string());
                avatar_url.push('/');
                avatar_url.push_str(&avatar.1);
                avatar_url.push_str(".png");

                ctx.http
                    .execute_webhook(webhook.id, &webhook.token)
                    .content(&message.content)
                    .username(&message.username)
                    .avatar_url(&avatar_url)
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
