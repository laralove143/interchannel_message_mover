use anyhow::{IntoResult, Result};
use dashmap::mapref::one::Ref;
use twilight_model::{
    channel::webhook::Webhook,
    id::{
        marker::{ChannelMarker, WebhookMarker},
        Id,
    },
};

use crate::Context;

/// a cached webhook
#[derive(Debug)]
pub struct CachedWebhook {
    /// id of the webhook
    pub id: Id<WebhookMarker>,
    /// token of the webhook
    pub token: String,
}

impl TryFrom<Webhook> for CachedWebhook {
    type Error = anyhow::Error;

    fn try_from(webhook: Webhook) -> Result<Self, Self::Error> {
        Ok(Self {
            id: webhook.id,
            token: webhook.token.ok()?,
        })
    }
}

/// get a webhook from the cache, falling back to the http api
pub async fn get(
    ctx: &Context,
    channel_id: Id<ChannelMarker>,
) -> Result<Ref<'_, twilight_model::id::Id<ChannelMarker>, CachedWebhook>> {
    if let Some(pair) = ctx.webhooks.get(&channel_id) {
        Ok(pair)
    } else {
        let http_application_id = ctx.application_id;
        let webhook = if let Some(webhook) = ctx
            .http
            .channel_webhooks(channel_id)
            .exec()
            .await?
            .models()
            .await?
            .into_iter()
            .find(|webhook| webhook.application_id == Some(http_application_id))
        {
            webhook
        } else {
            ctx.http
                .create_webhook(channel_id, "message highway")?
                .exec()
                .await?
                .model()
                .await?
        };

        ctx.webhooks.insert(channel_id, webhook.try_into()?);
        Ok(ctx.webhooks.get(&channel_id).ok()?)
    }
}

/// remove deleted webhooks from the cache
pub async fn update(ctx: Context, channel_id: Id<ChannelMarker>) -> Result<()> {
    let cached_webhook_id = if let Some(webhook) = ctx.webhooks.get(&channel_id) {
        webhook.id
    } else {
        return Ok(());
    };

    if !ctx
        .http
        .channel_webhooks(channel_id)
        .exec()
        .await?
        .models()
        .await?
        .iter()
        .any(|webhook| webhook.id == cached_webhook_id)
    {
        ctx.webhooks.remove(&channel_id);
    }

    Ok(())
}
