use anyhow::Result;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::{application_command::InteractionChannel, ApplicationCommand},
    guild::Permissions,
};

use crate::{cache::MessageContent, Context};

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

    if !ctx
        .twilight_cache
        .permissions()
        .in_channel(ctx.cache.user_id, channel_id)?
        .contains(Permissions::MANAGE_MESSAGES)
    {
        return Ok("give me the manage messages permission first please");
    }

    let options = MoveLastMessages::from_interaction(command.data.into())?;
    let messages = match ctx.cache.get_messages(channel_id) {
        Some(messages) => messages,
        None => {
            return Ok("looks like i couldn't read anything here :( check my permissions please")
        }
    };

    let (content, embeds, message_ids) = messages
        .iter()
        .rev()
        .take(options.message_count as usize)
        .rfold(
            (String::new(), Vec::new(), Vec::new()),
            |(mut message_content, mut message_embeds, mut message_ids), message| {
                if let MessageContent::Valid { content, embeds } = &message.content {
                    message_content.push_str(&content);
                    message_embeds.extend(embeds);
                    message_content.push('\n');
                    message_ids.push(message.id);
                }
                (message_content, message_embeds, message_ids)
            },
        );

    ctx.http
        .delete_messages(channel_id, &message_ids)
        .exec()
        .await?;

    Ok("done!")
}
