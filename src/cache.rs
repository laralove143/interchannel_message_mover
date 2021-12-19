use dashmap::DashMap;
use twilight_model::{
    channel::{embed::Embed, Message},
    id::{ChannelId, WebhookId},
};

pub struct Cache {
    messages: DashMap<ChannelId, Vec<CachedMessage>>,
    webhooks: DashMap<ChannelId, CachedWebhook>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            messages: DashMap::new(),
            webhooks: DashMap::new(),
        }
    }

    pub fn add_message(&self, message: Message) {
        let channel_id = message.channel_id;
        self.messages
            .get_mut(&channel_id)
            .unwrap_or_else(|| {
                self.messages.insert(channel_id, Vec::new());
                self.messages.get_mut(&channel_id).unwrap()
            })
            .value_mut()
            .push(message.into());
    }
}

enum CachedMessage {
    Valid { content: String, embeds: Vec<Embed> },
    HasAttachmentsOrComponents,
}

impl From<Message> for CachedMessage {
    fn from(message: Message) -> Self {
        if message.attachments.is_empty() && message.components.is_empty() {
            Self::Valid {
                content: message.content,
                embeds: message.embeds,
            }
        } else {
            Self::HasAttachmentsOrComponents
        }
    }
}

struct CachedWebhook {
    id: WebhookId,
    token: String,
}
