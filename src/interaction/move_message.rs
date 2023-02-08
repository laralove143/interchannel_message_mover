use anyhow::Result;
use sparkle_convenience::{
    error::IntoError, interaction::extract::InteractionDataExt, reply::Reply,
};
use twilight_model::{
    application::command::{Command, CommandType},
    channel::Message,
    guild::Permissions,
};
use twilight_util::builder::command::CommandBuilder;

use crate::{err_reply, interaction::InteractionContext, CustomError, REQUIRED_PERMISSIONS};

pub const NAME: &str = "move message";

pub fn command() -> Command {
    CommandBuilder::new(NAME, "", CommandType::Message)
        .dm_permission(false)
        .build()
}

impl InteractionContext<'_> {
    pub async fn handle_move_message_command(self) -> Result<()> {
        self.handle.check_permissions(REQUIRED_PERMISSIONS)?;

        let message = self
            .interaction
            .data
            .clone()
            .ok()?
            .command()
            .ok()?
            .resolved
            .ok()?
            .messages
            .into_iter()
            .next()
            .ok()?
            .1;
        let member = self.interaction.member.as_ref().ok()?;
        let user = member.user.as_ref().ok()?;

        if message.author.id != user.id
            && !member
                .permissions
                .ok()?
                .contains(Permissions::MANAGE_MESSAGES)
        {
            return Err(CustomError::ManageMessagesPermissionsMissing.into());
        }

        let ctx = self.wait_for_channel_select_interaction().await?;
        let handle = ctx.handle.clone();
        if let Err(err) = ctx.handle_move_message_channel_select(message).await {
            handle
                .handle_error::<CustomError>(err_reply(&err), err)
                .await;
        }

        Ok(())
    }

    async fn handle_move_message_channel_select(self, message: Message) -> Result<()> {
        let channel = self.handle_channel_select().await?;

        self.handle.defer_update_message().await?;
        self.ctx.execute_webhook_as_member(message, channel).await?;
        self.handle
            .update_message(Reply::new().ephemeral().content("done!".to_owned()))
            .await?;

        Ok(())
    }
}
