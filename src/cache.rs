use std::collections::VecDeque;

use anyhow::{Context as _, Result};
use dashmap::DashMap;
use twilight_http::client::Client;
use twilight_model::{
    channel::{webhook::Webhook, Message},
    gateway::payload::incoming::{MessageDelete, MessageDeleteBulk, MessageUpdate},
    id::{ChannelId, MessageId, UserId, WebhookId},
};

use crate::Context;

pub struct Cache {
    pub user_id: UserId,
    messages: DashMap<ChannelId, VecDeque<CachedMessage>>,
    webhooks: DashMap<ChannelId, CachedWebhook>,
}

impl Cache {
    pub async fn new(http: &Client) -> Result<Self> {
        Ok(Cache {
            user_id: http.current_user().exec().await?.model().await?.id,
            messages: DashMap::new(),
            webhooks: DashMap::new(),
        })
    }

    pub fn get_messages(&self, channel_id: ChannelId) -> Option<&VecDeque<CachedMessage>> {
        Some(self.messages.get(&channel_id)?.value())
    }

    pub async fn get_webhook(ctx: &Context, channel_id: ChannelId) -> Result<&CachedWebhook> {
        if let Some(pair) = ctx.cache.webhooks.get(&channel_id) {
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

            ctx.cache.webhooks.insert(channel_id, webhook.into());
            Ok(ctx
                .cache
                .webhooks
                .get(&channel_id)
                .context("created or retrieved webhook is not cached")?
                .value())
        }
    }

    pub fn add_message(&self, message: Message) {
        let channel_id = message.channel_id;

        let mut messages = self.messages.get_mut(&channel_id).unwrap_or_else(|| {
            self.messages
                .insert(channel_id, VecDeque::with_capacity(20));
            self.messages.get_mut(&channel_id).unwrap()
        });

        if messages.len() == 20 {
            messages.pop_front();
        }
        messages.push_back(message.into());
    }

    pub fn update_message(&self, message: MessageUpdate) {
        self._update_message(message);
    }

    fn _update_message(&self, message: MessageUpdate) -> Option<()> {
        let content = message.content?;

        self.messages
            .get_mut(&message.channel_id)?
            .value_mut()
            .iter_mut()
            .find(|cached_message| cached_message.id == message.id)?
            .content = content;

        None
    }

    pub async fn update_webhooks(ctx: Context, channel_id: ChannelId) -> Result<()> {
        let cached_webhook_id = if let Some(webhook) = ctx.cache.webhooks.get(&channel_id) {
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
            ctx.cache.webhooks.remove(&channel_id);
        }

        Ok(())
    }

    pub fn delete_message(&self, message: MessageDelete) {
        self._delete_message(message);
    }

    fn _delete_message(&self, message: MessageDelete) -> Option<()> {
        let mut messages = self.messages.get_mut(&message.channel_id)?;
        let message_position = messages
            .iter_mut()
            .position(|cached_message| cached_message.id == message.id)?;

        messages.remove(message_position);

        None
    }

    pub fn delete_messages(&self, messages: MessageDeleteBulk) {
        self._delete_messages(messages);
    }

    fn _delete_messages(&self, messages: MessageDeleteBulk) -> Option<()> {
        let mut cached_messages = self.messages.get_mut(&messages.channel_id)?;

        for message_id in messages.ids {
            if let Some(index) = cached_messages
                .iter_mut()
                .position(|cached_message| cached_message.id == message_id)
            {
                cached_messages.remove(index);
            }
        }

        Some(())
    }
}

pub struct CachedMessage {
    pub id: MessageId,
    pub content: String,
    pub username: String,
    pub avatar: Option<(UserId, String)>,
}

impl From<Message> for CachedMessage {
    fn from(message: Message) -> Self {
        Self {
            id: message.id,
            content: message.content,
            username: message.author.name,
            avatar: message
                .author
                .avatar
                .map(|avatar| (message.author.id, avatar)),
        }
    }
}

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
