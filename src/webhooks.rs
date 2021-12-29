use anyhow::Result;
use twilight_model::{
    channel::webhook::Webhook,
    id::{ChannelId, WebhookId},
};

use crate::Context;

#[derive(Debug)]
pub struct CachedWebhook {
    pub id: WebhookId,
    pub token: String,
}

impl From<Webhook> for CachedWebhook {
    fn from(webhook: Webhook) -> Self {
        Self {
            id: webhook.id,
            token: webhook.token.unwrap(),
        }
    }
}

pub async fn get(ctx: &Context, channel_id: ChannelId) -> Result<&CachedWebhook> {
    if let Some(pair) = ctx.webhooks.get(&channel_id) {
        Ok(pair.value())
    } else {
        let http_application_id = ctx.http.application_id();
        let webhook = if let Some(webhook) = ctx
            .http
            .channel_webhooks(channel_id)
            .exec()
            .await?
            .models()
            .await?
            .into_iter()
            .find(|webhook| webhook.application_id == http_application_id)
        {
            webhook
        } else {
            ctx.http
                .create_webhook(channel_id, "message highway")
                .exec()
                .await?
                .model()
                .await?
        };

        ctx.webhooks.insert(channel_id, webhook.into());
        Ok(ctx.webhooks.get(&channel_id).unwrap().value())
    }
}

pub async fn update(ctx: Context, channel_id: ChannelId) -> Result<()> {
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
