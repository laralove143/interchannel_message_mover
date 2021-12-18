use dashmap::DashMap;
use twilight_model::{
    application::component::Component,
    channel::{embed::Embed, Attachment},
    id::{ChannelId, MessageId, WebhookId},
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
}

struct CachedMessage {
    content: String,
    embeds: Vec<Embed>,
    attachments: Vec<Attachment>,
    components: Vec<Component>,
    reference: MessageId,
}

struct CachedWebhook {
    id: WebhookId,
    token: String,
}
