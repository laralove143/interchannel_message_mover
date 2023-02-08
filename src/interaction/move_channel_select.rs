use anyhow::Result;
use sparkle_convenience::{
    error::IntoError,
    interaction::{extract::InteractionDataExt, DeferVisibility},
};
use twilight_model::{
    application::interaction::Interaction,
    channel::{Channel, ChannelType},
    guild::Permissions,
    id::{marker::ChannelMarker, Id},
};
use twilight_util::permission_calculator::PermissionCalculator;

use crate::{
    interaction::{channel_select_menu::ChannelSelectMenu, InteractionContext},
    CustomError,
};

pub const CUSTOM_ID: &str = "move_channel";

impl<'a> InteractionContext<'a> {
    pub async fn wait_for_channel_select_interaction(self) -> Result<InteractionContext<'a>> {
        self.handle.defer(DeferVisibility::Ephemeral).await?;
        let channel_select_message = self
            .ctx
            .followup_with_channel_select_menu(
                &self.interaction.token,
                "where do you want to move the message?".to_owned(),
                true,
                ChannelSelectMenu::new(
                    CUSTOM_ID.to_owned(),
                    vec![
                        ChannelType::GuildText,
                        ChannelType::GuildAnnouncement,
                        ChannelType::PublicThread,
                    ],
                ),
            )
            .await?
            .model()
            .await?;

        let interaction = self
            .ctx
            .standby
            .wait_for_component(channel_select_message.id, |_interaction: &Interaction| true)
            .await?;

        Ok(InteractionContext {
            ctx: self.ctx,
            handle: self.ctx.bot.interaction_handle(&interaction),
            interaction,
        })
    }

    pub async fn handle_channel_select(&self) -> Result<Channel> {
        let channel_id = self
            .interaction
            .data
            .clone()
            .ok()?
            .component()
            .ok()?
            .values
            .into_iter()
            .next()
            .ok()?
            .parse::<Id<ChannelMarker>>()?;
        let member = self.interaction.member.as_ref().ok()?;

        let guild = self
            .ctx
            .bot
            .http
            .guild(self.interaction.guild_id.ok()?)
            .await?
            .model()
            .await?;
        let channel = self.ctx.bot.http.channel(channel_id).await?.model().await?;
        let permission_overwrites = if channel.kind.is_thread() {
            self.ctx
                .bot
                .http
                .channel(channel.parent_id.ok()?)
                .await?
                .model()
                .await?
                .permission_overwrites
                .ok()?
        } else {
            channel.permission_overwrites.clone().ok()?
        };

        let everyone_role = guild
            .roles
            .iter()
            .find_map(|role| (role.id.cast() == guild.id).then_some(role.permissions))
            .ok()?;

        let member_roles = guild
            .roles
            .iter()
            .filter_map(|role| {
                member
                    .roles
                    .contains(&role.id)
                    .then_some((role.id, role.permissions))
            })
            .collect::<Vec<_>>();

        let permissions = PermissionCalculator::new(
            guild.id,
            member.user.as_ref().ok()?.id,
            everyone_role,
            &member_roles,
        )
        .in_channel(channel.kind, &permission_overwrites);

        let required_permissions = if channel.kind.is_thread() {
            Permissions::SEND_MESSAGES_IN_THREADS
        } else {
            Permissions::SEND_MESSAGES
        };
        if !permissions.contains(required_permissions) {
            return Err(CustomError::SendMessagesPermissionMissing.into());
        }

        Ok(channel)
    }
}
