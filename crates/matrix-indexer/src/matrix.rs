use std::path::Path;

use crate::indradb_utils::{BulkInserter, MessagesMap, UUIDEventMapType, UUIDRoomMapType};
use color_eyre::Result;
use futures::StreamExt;
use matrix_sdk::{
    config::SyncSettings,
    ruma::{
        events::{
            room::message::MessageType, AnySyncMessageLikeEvent, AnySyncTimelineEvent,
            SyncMessageLikeEvent,
        },
        OwnedUserId,
    },
    Client, Session,
};
use tracing::{error, info};

struct Identifiers {
    room_type: utils::indradb::Identifier,
    room_id_type: utils::indradb::Identifier,
    room_name_type: utils::indradb::Identifier,
    room_topic_type: utils::indradb::Identifier,
    text_message_event_type: utils::indradb::Identifier,
    notice_message_event_type: utils::indradb::Identifier,
    event_id_type: utils::indradb::Identifier,
    event_in_room_type: utils::indradb::Identifier,
}

pub struct IndexerBot {
    client: Client,
    indexer_client: utils::indradb_proto::Client,
    message_map: MessagesMap,
    identifiers: Identifiers,
}

impl IndexerBot {
    pub fn access_token(&self) -> Option<String> {
        self.client.access_token()
    }

    pub fn device_id(&self) -> Option<String> {
        self.client.device_id().map(ToString::to_string)
    }
    async fn get_client(homeserver_url: String) -> Result<Client> {
        let mut client_builder = Client::builder().homeserver_url(homeserver_url);
        client_builder = client_builder.sled_store(Path::new("./matrix_data"), None)?;

        Ok(client_builder.build().await?)
    }

    async fn get_indexer_client(
        endpoint: String,
    ) -> Result<(utils::indradb_proto::Client, Identifiers)> {
        info!("Trying to connect to indradb");
        let mut indexer_client = utils::get_client_retrying(endpoint).await?;
        indexer_client.ping().await?;
        let room_type = utils::indradb::Identifier::new("matrix_room")?;
        let room_id_type = utils::indradb::Identifier::new("room_id")?;
        let room_name_type = utils::indradb::Identifier::new("room_name")?;
        let room_topic_type = utils::indradb::Identifier::new("room_topic")?;
        let text_message_event_type = utils::indradb::Identifier::new("text_message_event")?;
        let notice_message_event_type = utils::indradb::Identifier::new("notice_message_event")?;
        let event_id_type = utils::indradb::Identifier::new("event_id")?;
        let event_in_room_type = utils::indradb::Identifier::new("event_in_room")?;
        indexer_client.index_property(room_name_type).await?;
        indexer_client.index_property(room_topic_type).await?;
        indexer_client.index_property(event_id_type).await?;
        indexer_client
            .index_property(utils::indradb::Identifier::new("text_message_body")?)
            .await?;
        indexer_client
            .index_property(utils::indradb::Identifier::new("text_message_format")?)
            .await?;
        indexer_client
            .index_property(utils::indradb::Identifier::new(
                "text_message_formatted_body",
            )?)
            .await?;
        info!("Connected to indradb");

        Ok((
            indexer_client,
            Identifiers {
                room_type,
                room_id_type,
                room_name_type,
                room_topic_type,
                text_message_event_type,
                notice_message_event_type,
                event_id_type,
                event_in_room_type,
            },
        ))
    }

    pub async fn new(
        homeserver_url: String,
        user_id: String,
        password: String,
        indra_endpoint: String,
    ) -> Result<Self> {
        let client = IndexerBot::get_client(homeserver_url).await?;
        client
            .login_username(&user_id, &password)
            .initial_device_display_name("Knowledge Indexer bot")
            .send()
            .await?;

        let (indexer_client, identifiers) = IndexerBot::get_indexer_client(indra_endpoint).await?;

        let client_clone = client.clone();
        tokio::spawn(async move {
            let settings = SyncSettings::default();
            client_clone
                .sync(settings)
                .await
                .expect("Failed to start matrix sync");
        });

        Ok(IndexerBot {
            client,
            indexer_client,
            message_map: MessagesMap::default(),
            identifiers,
        })
    }

    pub async fn relogin(
        homeserver_url: String,
        user_id: String,
        access_token: String,
        device_id: String,
        indra_endpoint: String,
    ) -> Result<Self> {
        let client = IndexerBot::get_client(homeserver_url).await?;
        client
            .restore_login(Session {
                access_token,
                device_id: device_id.into(),
                refresh_token: None,
                user_id: OwnedUserId::try_from(user_id)?,
            })
            .await?;

        let (indexer_client, identifiers) = IndexerBot::get_indexer_client(indra_endpoint).await?;

        let client_clone = client.clone();
        tokio::spawn(async move {
            let settings = SyncSettings::default();
            client_clone
                .sync(settings)
                .await
                .expect("Failed to start matrix sync");
        });

        Ok(IndexerBot {
            client,
            indexer_client,
            message_map: MessagesMap::default(),
            identifiers,
        })
    }

    // FIXME:_split into multiple functions
    #[allow(clippy::too_many_lines)]
    pub async fn start_processing(&mut self) -> Result<()> {
        let mut inserter = BulkInserter::new(self.indexer_client.clone());

        let mut sync_stream = Box::pin(self.client.sync_stream(SyncSettings::default()).await);

        while let Some(Ok(response)) = sync_stream.next().await {
            for (ref room_id, room) in response.rooms.join {
                for e in &room.timeline.events {
                    let room_uuid = if let Some(room) = self.client.get_joined_room(room_id) {
                        self.message_map.insert_room(
                            room_id.clone(),
                            crate::indradb_utils::RoomProperties {
                                name: room.name(),
                                topic: room.topic(),
                            },
                        )
                    } else {
                        self.message_map.insert_room(
                            room_id.clone(),
                            crate::indradb_utils::RoomProperties {
                                name: None,
                                topic: None,
                            },
                        )
                    };

                    match e.event.deserialize() {
                        Ok(AnySyncTimelineEvent::MessageLike(
                            AnySyncMessageLikeEvent::RoomMessage(event),
                        )) => {
                            if let SyncMessageLikeEvent::Original(message) = event {
                                match message.content.msgtype {
                                    MessageType::Text(message_content) => {
                                        self.message_map.insert_event(
                                            message.event_id,
                                            room_uuid,
                                            self.identifiers.text_message_event_type,
                                            crate::indradb_utils::EventProperties::TextMessage(
                                                message_content.body,
                                                message_content
                                                    .formatted
                                                    .clone()
                                                    .map(|x| x.format.to_string()),
                                                message_content.formatted.map(|x| x.body),
                                            ),
                                        );
                                    }
                                    MessageType::Notice(message_content) => {
                                        self.message_map.insert_event(
                                            message.event_id,
                                            room_uuid,
                                            self.identifiers.notice_message_event_type,
                                            crate::indradb_utils::EventProperties::TextMessage(
                                                message_content.body,
                                                message_content
                                                    .formatted
                                                    .clone()
                                                    .map(|x| x.format.to_string()),
                                                message_content.formatted.map(|x| x.body),
                                            ),
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // TODO: index space hierachy
                        Ok(
                            AnySyncTimelineEvent::MessageLike(_) | AnySyncTimelineEvent::State(_),
                        ) => {}
                        Err(e) => {
                            error!("Error deserializing event: {}", e);
                        }
                    }
                }
            }

            // Push to indexer after we preprocessed it
            for UUIDRoomMapType {
                room_id,
                uuid,
                room_properties,
            } in &self.message_map.room_list
            {
                inserter
                    .push(utils::indradb::BulkInsertItem::Vertex(
                        utils::indradb::Vertex::with_id(*uuid, self.identifiers.room_type),
                    ))
                    .await?;
                inserter
                    .push(utils::indradb::BulkInsertItem::VertexProperty(
                        *uuid,
                        self.identifiers.room_id_type,
                        serde_json::Value::String(room_id.to_string()).into(),
                    ))
                    .await?;
                if let Some(room_name) = &room_properties.name {
                    inserter
                        .push(utils::indradb::BulkInsertItem::VertexProperty(
                            *uuid,
                            self.identifiers.room_name_type,
                            serde_json::Value::String(room_name.to_string()).into(),
                        ))
                        .await?;
                }
                if let Some(room_topic) = &room_properties.topic {
                    inserter
                        .push(utils::indradb::BulkInsertItem::VertexProperty(
                            *uuid,
                            self.identifiers.room_topic_type,
                            serde_json::Value::String(room_topic.to_string()).into(),
                        ))
                        .await?;
                }
            }
            for UUIDEventMapType {
                event_id,
                uuid,
                event_type,
                event_properties,
            } in &self.message_map.message_list
            {
                inserter
                    .push(utils::indradb::BulkInsertItem::Vertex(
                        utils::indradb::Vertex::with_id(*uuid, *event_type),
                    ))
                    .await?;
                inserter
                    .push(utils::indradb::BulkInsertItem::VertexProperty(
                        *uuid,
                        self.identifiers.event_id_type,
                        serde_json::Value::String(event_id.to_string()).into(),
                    ))
                    .await?;
                for event_property in event_properties
                    .as_vec(*uuid)
                    .expect("Unable to convert to indradb properties")
                {
                    inserter.push(event_property).await?;
                }
            }

            for (event_uuid, room_uuid) in &self.message_map.room_event_links {
                inserter
                    .push(utils::indradb::BulkInsertItem::Edge(
                        utils::indradb::Edge::new(
                            *event_uuid,
                            self.identifiers.event_in_room_type,
                            *room_uuid,
                        ),
                    ))
                    .await?;
            }
            inserter.sync().await?;
        }
        Ok(())
    }
}
