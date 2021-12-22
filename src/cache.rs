use std::collections::VecDeque;

use anyhow::Result;
use dashmap::DashMap;
use twilight_http::client::Client;
use twilight_model::{
    channel::{embed::Embed, Message},
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
        let mut messages = self.messages.get_mut(&message.channel_id)?;

        for cached_message in messages.value_mut() {
            if cached_message.id == message.id {
                if !message.attachments.map_or(true, |v| v.is_empty()) {
                    cached_message.content = MessageContent::AttachmentsOrComponents;
                } else if let MessageContent::Valid { content, embeds } =
                    &mut cached_message.content
                {
                    if let Some(updated_content) = message.content {
                        *content = updated_content;
                    }
                    if let Some(updated_embeds) = message.embeds {
                        *embeds = updated_embeds;
                    }
                }
                return Some(());
            }
        }

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

        Some(())
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
    pub content: MessageContent,
}

#[derive(Debug)]
pub enum MessageContent {
    Valid { content: String, embeds: Vec<Embed> },
    AttachmentsOrComponents,
}

impl From<Message> for CachedMessage {
    fn from(message: Message) -> Self {
        Self {
            id: message.id,
            content: if message.attachments.is_empty() && message.components.is_empty() {
                MessageContent::Valid {
                    content: message.content,
                    embeds: message.embeds,
                }
            } else {
                MessageContent::AttachmentsOrComponents
            },
        }
    }
}

struct CachedWebhook {
    id: WebhookId,
    token: String,
}
