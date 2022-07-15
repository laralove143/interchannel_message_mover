use std::fmt::Write;

use anyhow::{bail, IntoResult, Result};
use futures::StreamExt;
use twilight_cache_inmemory::{model::CachedMessage, Reference};
use twilight_http::{
    client::InteractionClient,
    response::{marker::EmptyBody, ResponseFuture},
};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_mention::Mention;
use twilight_model::{
    application::{
        component::{button::ButtonStyle, ActionRow, Button, Component},
        interaction::{
            application_command::InteractionChannel, ApplicationCommand,
            MessageComponentInteraction,
        },
    },
    channel::{ChannelType, Webhook},
    guild::{PartialMember, Permissions},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, MessageMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::Context;

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

/// run the command, responding with the returned reply
pub async fn run<'client>(
    ctx: Context,
    client: &'client InteractionClient<'client>,
    token: &str,
    command: ApplicationCommand,
) -> Result<()> {
    let reply = _run(ctx, client, token, command).await?;

    if !reply.is_empty() {
        client
            .update_response(token)
            .content(Some(reply))?
            .exec()
            .await?;
    }

    Ok(())
}

/// run the command, returning the reply to respond with
async fn _run<'client>(
    ctx: Context,
    client: &'client InteractionClient<'client>,
    token: &'client str,
    command: ApplicationCommand,
) -> Result<&'static str> {
    let app_permissions = command.app_permissions.ok()?;
    let options = MoveLastMessages::from_interaction(command.data.into())?;
    let message_count: usize = options.message_count.try_into()?;

    if options.channel.kind != ChannelType::GuildText
        || ctx.cache.channel(command.channel_id).ok()?.kind != ChannelType::GuildText
    {
        return Ok("i can only work in normal text channels in servers for now.. sorry!");
    }

    if !has_perms(&ctx, app_permissions, options.channel.id)? {
        return Ok(
            "**please make sure i have these permissions:**\n\nview channels\nmanage \
             webhooks\nsend messages\nmanage messages",
        );
    };

    let webhook = ctx
        .webhooks
        .get_infallible(&ctx.http, options.channel.id, "message highway")
        .await?;

    let mut message_ids = match get_message_ids(&ctx, command.channel_id, message_count) {
        Some(ids) => ids,
        None => return Ok("i can only move messages that are sent after i joined"),
    };
    let messages = get_messages(&ctx, &message_ids)?;

    let webhooks = make_webhooks(&ctx, &webhook, &messages)?;

    let (should_continue, agree_message_id) = should_continue(
        &ctx,
        client,
        token,
        command.member.ok()?,
        command.channel_id,
        &messages,
    )
    .await?;

    match agree_message_id {
        Some(id) => {
            if should_continue {
                for webhook_exec in webhooks {
                    webhook_exec.await?;
                }
                message_ids.push(id);
                delete_messages(&ctx, &message_ids, command.channel_id).await?;
            } else {
                ctx.http
                    .delete_message(command.channel_id, id)
                    .exec()
                    .await?;
            }
            Ok("")
        }
        None => {
            if should_continue {
                for webhook_exec in webhooks {
                    webhook_exec.await?;
                }
                delete_messages(&ctx, &message_ids, command.channel_id).await?;
                Ok("done! i moved the messages")
            } else {
                Ok("")
            }
        }
    }
}

/// returns whether the bot has the needed permissions
fn has_perms(
    ctx: &Context,
    app_permissions: Permissions,
    target_channel_id: Id<ChannelMarker>,
) -> Result<bool> {
    Ok(app_permissions.contains(
        Permissions::MANAGE_MESSAGES | Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
    ) && ctx
        .cache
        .permissions()
        .in_channel(ctx.user_id, target_channel_id)?
        .contains(Permissions::MANAGE_WEBHOOKS))
}

/// return the `ExecuteWebhook`s to be executed if the move should continue
fn make_webhooks<'ctx>(
    ctx: &'ctx Context,
    webhook: &'ctx Webhook,
    messages: &[Reference<'ctx, Id<MessageMarker>, CachedMessage>],
) -> Result<Vec<ResponseFuture<EmptyBody>>> {
    let mut webhooks = Vec::with_capacity(messages.len());

    for message in messages.iter().rev() {
        if message.webhook_id().is_some() {
            continue;
        }

        let content = message.content();
        if content.is_empty() {
            continue;
        }

        let author_member = message.member().ok()?;
        let author_user = ctx.cache.user(message.author()).ok()?;

        let webhook_exec = ctx
            .http
            .execute_webhook(webhook.id, webhook.token.as_ref().ok()?)
            .content(content)?
            .username(author_member.nick.as_ref().unwrap_or(&author_user.name))?;
        if let Some(avatar) = &author_member.avatar.or(author_user.avatar) {
            webhooks.push(
                webhook_exec
                    .avatar_url(&format!(
                        "https://cdn.discordapp.com/avatars/{}/{}.png",
                        author_user.id, avatar
                    ))
                    .exec(),
            );
        } else {
            webhooks.push(webhook_exec.exec());
        }
    }

    Ok(webhooks)
}

/// if the author doesn't have permission to manage messages and at least one of
/// the messages isn't theirs, sends the agree message, waits until everyone
/// agrees, and returns whether the move should be done and the id of the agree
/// message, otherwise returns `true`
async fn should_continue<'client>(
    ctx: &Context,
    client: &'client InteractionClient<'client>,
    token: &'client str,
    author: PartialMember,
    command_channel_id: Id<ChannelMarker>,
    messages: &[Reference<'_, Id<MessageMarker>, CachedMessage>],
) -> Result<(bool, Option<Id<MessageMarker>>)> {
    let author_id = author.user.ok()?.id;

    if author
        .permissions
        .ok()?
        .contains(Permissions::MANAGE_MESSAGES)
    {
        return Ok((true, None));
    }

    let mut agree_waiting = get_agree_waiting(messages, author_id);
    if agree_waiting.is_empty() {
        return Ok((true, None));
    }

    let content = get_agree_message_content(&agree_waiting)?;
    let message_components = [Component::ActionRow(ActionRow {
        components: vec![
            Component::Button(Button {
                custom_id: Some("agree".to_owned()),
                disabled: false,
                emoji: None,
                label: Some("agree".to_owned()),
                style: ButtonStyle::Primary,
                url: None,
            }),
            Component::Button(Button {
                custom_id: Some("refuse".to_owned()),
                disabled: false,
                emoji: None,
                label: Some("refuse".to_owned()),
                style: ButtonStyle::Danger,
                url: None,
            }),
        ],
    })];

    let message_id = ctx
        .http
        .create_message(command_channel_id)
        .content(&content)?
        .components(&message_components)?
        .exec()
        .await?
        .model()
        .await?
        .id;

    client
        .update_response(token)
        .content(Some(
            "i'll wait until everyone agrees, if the message with the buttons is deleted, that \
             means someone refused...",
        ))?
        .exec()
        .await?;

    let mut components = ctx.standby.wait_for_component_stream(
        message_id,
        |component: &MessageComponentInteraction| {
            let id = component.data.custom_id.as_str();
            id == "agree" || id == "refuse"
        },
    );

    while let Some(component) = components.next().await {
        let agreed_author_id = component.author_id().ok()?;

        match component.data.custom_id.as_ref() {
            "agree" => {
                if let Some(index) = agree_waiting.iter().position(|&id| id == agreed_author_id) {
                    agree_waiting.swap_remove(index);
                }

                if agree_waiting.is_empty() {
                    return Ok((true, Some(message_id)));
                }

                client
                    .create_response(
                        component.id,
                        &component.token,
                        &InteractionResponse {
                            kind: InteractionResponseType::UpdateMessage,
                            data: Some(
                                InteractionResponseDataBuilder::new()
                                    .content(get_agree_message_content(&agree_waiting)?)
                                    .components(message_components.clone())
                                    .build(),
                            ),
                        },
                    )
                    .exec()
                    .await?;
            }
            "refuse" => {
                if agree_waiting.iter().any(|&id| id == agreed_author_id) {
                    return Ok((false, Some(message_id)));
                }
            }
            _ => bail!("component custom id is not agree or refuse"),
        }
    }

    Ok((true, Some(message_id)))
}

/// return the ids of the users that need to agree
fn get_agree_waiting(
    messages: &[Reference<Id<MessageMarker>, CachedMessage>],
    author_id: Id<UserMarker>,
) -> Vec<Id<UserMarker>> {
    let mut agree_waiting: Vec<Id<UserMarker>> = messages
        .iter()
        .map(|m| m.author())
        .filter(|&id| id != author_id)
        .collect();
    agree_waiting.sort_unstable();
    agree_waiting.dedup();
    agree_waiting
}

/// get the content of the agree message using the given ids
fn get_agree_message_content(agree_waiting: &[Id<UserMarker>]) -> Result<String> {
    let mut content = "**some of the messages aren't yours so i'll mention the people that need \
                       to agree first**\n"
        .to_owned();
    for user_id in agree_waiting {
        write!(content, "\n{}", user_id.mention())?;
    }

    Ok(content)
}

/// get the ids of the messages to move from the cache
fn get_message_ids(
    ctx: &Context,
    command_channel_id: Id<ChannelMarker>,
    message_count: usize,
) -> Option<Vec<Id<MessageMarker>>> {
    let ids: Vec<Id<MessageMarker>> = ctx
        .cache
        .channel_messages(command_channel_id)?
        .take(message_count)
        .collect();

    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

/// get the cached messages to move from the cache, using the given message ids
fn get_messages<'ctx>(
    ctx: &'ctx Context,
    message_ids: &[Id<MessageMarker>],
) -> Result<Vec<Reference<'ctx, Id<MessageMarker>, CachedMessage>>> {
    let mut messages = Vec::with_capacity(message_ids.len());

    for message_id in message_ids {
        let message = ctx.cache.message(*message_id).ok()?;
        if message.webhook_id().is_none() {
            messages.push(message);
        }
    }

    Ok(messages)
}

/// delete the given messages
async fn delete_messages(
    ctx: &Context,
    message_ids: &[Id<MessageMarker>],
    command_channel_id: Id<ChannelMarker>,
) -> Result<()> {
    if message_ids.len() == 1 {
        ctx.http
            .delete_message(command_channel_id, *message_ids.get(0).ok()?)
            .exec()
            .await?;
    } else {
        ctx.http
            .delete_messages(command_channel_id, message_ids)
            .exec()
            .await?;
    };
    Ok(())
}
