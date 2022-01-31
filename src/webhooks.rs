use std::fmt::{Display, Formatter};

use anyhow::Result;
use twilight_model::{
    channel::webhook::Webhook,
    id::{
        marker::{ChannelMarker, WebhookMarker},
        Id,
    },
};

use crate::Context;

/// an error occured while caching a webhook
#[derive(Debug)]
pub enum Error {
    /// the webhook is not an incoming webhook
    NotIncoming,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("the webhook to cache is not an incoming webhook")
    }
}

impl std::error::Error for Error {}

/// a cached webhook
#[derive(Debug)]
pub struct CachedWebhook {
    /// id of the webhook
    pub id: Id<WebhookMarker>,
    /// token of the webhook
    pub token: String,
}

impl TryFrom<Webhook> for CachedWebhook {
    type Error = Error;

    fn try_from(webhook: Webhook) -> Result<Self, Error> {
        Ok(Self {
            id: webhook.id,
            token: webhook.token.ok_or(Error::NotIncoming)?,
        })
    }
}

/// get a webhook from the cache, get it from the http api and cache it if not
/// found, create a new one if that's also not found
pub async fn get(ctx: &Context, channel_id: Id<ChannelMarker>) -> Result<&CachedWebhook> {
    if let Some(pair) = ctx.webhooks.get(&channel_id) {
        Ok(pair.value())
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
                .create_webhook(channel_id, "message highway")
                .exec()
                .await?
                .model()
                .await?
        };

        ctx.webhooks.insert(channel_id, webhook.try_into()?);
        Ok(ctx.webhooks.get(&channel_id).unwrap().value())
    }
}

/// get webhooks in channel using the http api and remove the cached webhook if
/// it's not found
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
