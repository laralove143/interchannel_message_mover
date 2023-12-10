use anyhow::Result;
use sparkle_convenience::reply::Reply;
use twilight_model::{application::command::{Command, CommandType}, channel::message::MessageFlags};
use twilight_util::builder::command::CommandBuilder;

use crate::{interaction::InteractionContext, message};

pub const NAME: &str = "move message";

pub fn command() -> Command {
    CommandBuilder::new(NAME, "", CommandType::Message)
        .dm_permission(false)
        .build()
}

impl InteractionContext<'_> {
    pub async fn handle_move_message_command(self) -> Result<()> {
        let message = self.handle_message_command()?;
        message::check(&message)?;

        let message_id = message.id;
        let message_channel_id = message.channel_id;

        let channel = self.wait_for_channel_select_interaction().await?;

        self.handle
            .reply(
                Reply::new()
                    .ephemeral()
                    .update_last()
                    .content("starting up the bike :motor_scooter:"),
            )
            .await?;

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

        self.ctx
            .bot
            .http
            .delete_message(message_channel_id, message_id)
            .await?;

        self.handle
            .reply(
                Reply::new()
                    .ephemeral()
                    .update_last()
                    .content("done :incoming_envelope:"),
            )
            .await?;

        Ok(())
    }
}
