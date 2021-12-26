use anyhow::Result;
use twilight_cache_inmemory::model::CachedMessage;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
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

    let permissions_cache = ctx.cache.permissions();

    if !(permissions_cache
        .in_channel(ctx.user_id, channel_id)?
        .contains(Permissions::MANAGE_MESSAGES)
        || permissions_cache
            .in_channel(ctx.user_id, options.channel.id)?
            .contains(Permissions::MANAGE_WEBHOOKS))
    {
        return Ok("please give me **manage messages** and **manage webhooks** permissions >.<");
    }

    let mut messages: Vec<&CachedMessage> = ctx
        .cache
        .iter()
        .messages()
        .filter(|message| message.channel_id() == command.channel_id)
        .map(|pair| pair.value())
        .collect();

    messages.sort_unstable_by(|message1, message2| {
        message1
            .timestamp()
            .as_micros()
            .cmp(&message2.timestamp().as_micros())
    });

    let webhook = webhooks::get(&ctx, options.channel.id).await?;

    let mut message_ids = Vec::new();

    for message in messages.iter().take(options.message_count as usize) {
        let author = message.member().unwrap();

        let exec = ctx
            .http
            .execute_webhook(webhook.id, &webhook.token)
            .content(message.content())
            .username(
                author
                    .nick
                    .as_ref()
                    .unwrap_or(&author.user.as_ref().unwrap().name),
            );

        if let Some(avatar) = &author.avatar {
            exec.avatar_url(&format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png",
                message.author(),
                avatar
            ))
            .exec()
            .await?;
        } else {
            exec.exec().await?;
        }

        message_ids.push(message.id());
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
