use std::collections::VecDeque;

use anyhow::Result;
use dashmap::DashMap;
use twilight_http::client::Client;
use twilight_model::{
    channel::{webhook::Webhook, Message},
    gateway::payload::incoming::{MessageDelete, MessageDeleteBulk, MessageUpdate},
    id::{ChannelId, MessageId, UserId, WebhookId},
};

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

    pub fn get_webhook(&self, channel_id: ChannelId) -> Option<&CachedWebhook> {
        Some(self.webhooks.get(&channel_id)?.value())
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

    pub fn add_webhook(&self, webhook: Webhook) {
        self.webhooks.insert(webhook.channel_id, webhook.into());
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

#[derive(Debug)]
pub struct CachedMessage {
    pub id: MessageId,
    pub content: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

impl From<Message> for CachedMessage {
    fn from(message: Message) -> Self {
        Self {
            id: message.id,
            content: message.content,
            username: message.author.name,
            avatar_url: message.author.avatar.map(|avatar| {
                let mut avatar_url = "https://cdn.discordapp.com/avatars/".to_string();
                avatar_url.push_str(&message.author.id.get().to_string());
                avatar_url.push('/');
                avatar_url.push_str(&avatar);
                // TODO: test with gif avatars
                if avatar.starts_with("a_") {
                    avatar_url.push_str(".gif");
                } else {
                    avatar_url.push_str(".png");
                }
                avatar_url
            }),
        }
    }
}

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
