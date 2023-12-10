use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use sparkle_convenience::{error::IntoError, reply::Reply};
use twilight_model::{application::command::{Command, CommandType}, channel::message::MessageFlags};
use twilight_util::builder::command::CommandBuilder;

use crate::{interaction::InteractionContext, message};

pub const NAME: &str = "move this message and below";

pub fn command() -> Command {
    CommandBuilder::new(NAME, "", CommandType::Message)
        .dm_permission(false)
        .build()
}

impl InteractionContext<'_> {
    pub async fn handle_move_message_and_below_command(self) -> Result<()> {
        let guild_id = self.interaction.guild_id.ok()?;

        let mut messages = vec![self.handle_message_command()?];

        let channel = self.wait_for_channel_select_interaction().await?;

        let mut channel_messages = self
            .ctx
            .bot
            .http
            .channel_messages(messages[0].channel_id)
            .after(messages[0].id)
            .await?
            .models()
            .await?;
        channel_messages.reverse();
        messages.append(&mut channel_messages);

        for message in &messages {
            message::check(message)?;
        }

        let reply_content = match messages.len() {
            0..=10 => "starting up the car :red_car:",
            11..=20 => "starting up the truck :pickup_truck:",
            21..=30 => "starting up the truck :truck:",
            31..=40 => "starting up the lorry :articulated_lorry:",
            _ => "starting up the ship :ship:",
        };
        self.handle
            .reply(
                Reply::new()
                    .ephemeral()
                    .update_last()
                    .content(reply_content),
            )
            .await?;

        for (idx, message) in messages.iter().enumerate() {
            if (idx + 1) % 10 == 0 {
                println!(
                    "moving messages in {guild_id}: {}/{}",
                    idx + 1,
                    messages.len()
                );
            }

            // Check if the message has any attachments
            if !message.attachments.is_empty() {
                let mut http_attachments = Vec::new();

                for channel_attachment in &message.attachments {
                    // Check if it has a spoiler
                    let filename = if let Some(flags) = message.flags {
                        if flags.contains(MessageFlags::EPHEMERAL) {
                            format!("SPOILER_{}", channel_attachment.filename)
                        } else {
                            channel_attachment.filename.clone()
                        }
                    } else {
                        channel_attachment.filename.clone()
                    };
                    
                    let id = channel_attachment.id.into();
                    
                    // Download the attachment content
                    let file_content = reqwest::get(&channel_attachment.url)
                        .await?
                        .bytes()
                        .await?
                        .to_vec();
                    
                    let mut http_attachment = twilight_model::http::attachment::Attachment::from_bytes(filename, file_content, id);
                    // Check if the attachment has a description (alt)
                    if let Some(description) = &channel_attachment.description {
                        http_attachment.description(description.clone());
                    }
                    http_attachments.push(http_attachment);
                }

                self.ctx
                    .execute_webhook_as_member(&message, &channel, &http_attachments)
                    .await?;
            } else {
                // Send the message content
                self.ctx
                    .execute_webhook_as_member(&message, &channel, &[])
                    .await?;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        if (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
            - u64::try_from(messages[0].timestamp.as_secs())?)
            > 2 * 7 * 24 * 60 * 60
            || messages.len() == 1
        {
            for (idx, message) in messages.iter().enumerate() {
                if (idx + 1) % 10 == 0 {
                    println!(
                        "deleting messages in {guild_id}: {}/{}",
                        idx + 1,
                        messages.len()
                    );
                }

                self.ctx
                    .bot
                    .http
                    .delete_message(message.channel_id, message.id)
                    .await?;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        } else {
            self.ctx
                .bot
                .http
                .delete_messages(
                    messages[0].channel_id,
                    &messages
                        .iter()
                        .map(|message| message.id)
                        .collect::<Vec<_>>(),
                )?
                .await?;
        }

        self.handle
            .reply(
                Reply::new()
                    .ephemeral()
                    .update_last()
                    .content("done :incoming_envelope:"),
            )
            .await?;

        println!("{guild_id} done");

        Ok(())
    }
}
