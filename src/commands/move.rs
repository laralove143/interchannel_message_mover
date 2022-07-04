use anyhow::{IntoResult, Result};
use twilight_cache_inmemory::Reference;
use twilight_http::client::InteractionClient;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
        interaction::{ApplicationCommand, MessageComponentInteraction},
    },
    channel::{message::MessageType, Channel, ChannelType, Message},
    guild::Permissions,
    id::{marker::ChannelMarker, Id},
};
use twilight_util::builder::command::CommandBuilder;
use twilight_webhook::util::{MinimalMember, MinimalWebhook};

use crate::Context;

/// run the command, responding with the returned reply
pub async fn run<'client>(
    ctx: Context,
    client: &'client InteractionClient<'client>,
    token: &str,
    command: ApplicationCommand,
) -> Result<()> {
    let reply = _run(ctx, client, token, command).await?;

    client
        .update_response(token)
        .content(Some(reply))?
        .components(Some(&[]))?
        .exec()
        .await?;

    Ok(())
}

/// run the command, returning the reply to respond with
async fn _run<'client>(
    ctx: Context,
    client: &'client InteractionClient<'client>,
    token: &'client str,
    command: ApplicationCommand,
) -> Result<&'static str> {
    let message = command
        .data
        .resolved
        .as_ref()
        .ok()?
        .messages
        .values()
        .next()
        .ok()?;
    let member = command.member.as_ref().ok()?;
    let user = member.user.as_ref().ok()?;

    if message.author.id != user.id
        && !member
            .permissions
            .ok()?
            .contains(Permissions::MANAGE_MESSAGES)
    {
        return Ok("you need `manage messages` permission to move messages that aren't yours..");
    }
    if message_is_weird(message) {
        return Ok("this message is weird, it has something i cant recreate like an embed.. sorry");
    }
    if message.content.chars().count() > 2000 {
        return Ok(
            "this message is too long, someone with nitro sent it but bots dont have nitro sadly",
        );
    }

    let permissions_cache = ctx.cache.permissions();
    if !permissions_cache
        .in_channel(ctx.user_id, command.channel_id)?
        .contains(Permissions::MANAGE_MESSAGES)
    {
        return Ok("i need `manage messages` permission in this channel..");
    }

    let target_channel = select_target_channel(&ctx, client, token, &command).await?;
    if target_channel.kind.is_thread() {
        return Ok("i can only work in normal text channels for now.. sorry!");
    }

    if !permissions_cache
        .in_channel(ctx.user_id, target_channel.id)?
        .contains(Permissions::MANAGE_WEBHOOKS)
    {
        return Ok("i need `manage webhooks` permission in that channel..");
    }

    MinimalWebhook::try_from(
        &*ctx
            .webhooks
            .get_infallible(&ctx.http, target_channel.id, "message highway")
            .await?,
    )?
    .execute_as_member(
        &ctx.http,
        None,
        &MinimalMember::from_cached_member(
            &*ctx
                .cache
                .member(command.guild_id.ok()?, message.author.id)
                .ok()?,
            &message.author,
        ),
    )?
    .content(&message.content)?
    .exec()
    .await?;

    ctx.http
        .delete_message(message.channel_id, message.id)
        .exec()
        .await?;

    Ok("done!")
}

/// updates the response with a select menu for the target channel and returns
/// the selected channel
async fn select_target_channel<'client>(
    ctx: &'client Context,
    client: &InteractionClient<'client>,
    token: &str,
    command: &ApplicationCommand,
) -> Result<Reference<'client, Id<ChannelMarker>, Channel>> {
    let mut channels_option = Vec::new();
    for id in &*ctx.cache.guild_channels(command.guild_id.ok()?).ok()? {
        let channel = ctx.cache.channel(*id).ok()?;
        if channel.kind == ChannelType::GuildText {
            channels_option.push(SelectMenuOption {
                label: channel.name.clone().ok()?,
                value: channel.id.to_string(),
                default: false,
                description: None,
                emoji: None,
            });
        }
    }
    let menu_message_id = client
        .update_response(token)
        .content(Some("where do you want to move the messages?"))?
        .components(Some(&[Component::ActionRow(ActionRow {
            components: vec![Component::SelectMenu(SelectMenu {
                custom_id: "target_channel".to_owned(),
                options: channels_option,
                disabled: false,
                max_values: None,
                min_values: None,
                placeholder: None,
            })],
        })]))?
        .exec()
        .await?
        .model()
        .await?
        .id;

    let component = ctx
        .standby
        .wait_for_component(
            menu_message_id,
            |component: &MessageComponentInteraction| component.data.custom_id == "target_channel",
        )
        .await?;
    ctx.cache
        .channel(component.data.values.first().ok()?.parse()?)
        .ok()
}

/// get the `Command`
pub fn build() -> Command {
    CommandBuilder::new("move".to_owned(), "".to_owned(), CommandType::Message).build()
}

/// returns `true` if the message can't be recreated by the bot
fn message_is_weird(message: &Message) -> bool {
    message.activity.is_some()
        || message.application.is_some()
        || message.application_id.is_some()
        || !message.components.is_empty()
        || !message.embeds.is_empty()
        || !matches!(message.kind, MessageType::Regular | MessageType::Reply)
        || message.pinned
        || message.webhook_id.is_some()
}
