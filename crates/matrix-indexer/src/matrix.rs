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

pub struct IndexerBot {
    client: Client,
    indexer_client: utils::indradb_proto::Client,
    message_map: MessagesMap,
    room_type: utils::indradb::Identifier,
    room_id_type: utils::indradb::Identifier,
    room_name_type: utils::indradb::Identifier,
    room_topic_type: utils::indradb::Identifier,
    text_message_event_type: utils::indradb::Identifier,
    notice_message_event_type: utils::indradb::Identifier,
    event_id_type: utils::indradb::Identifier,
}

impl IndexerBot {
    async fn get_client(homeserver_url: String) -> Result<Client> {
        let mut client_builder = Client::builder().homeserver_url(homeserver_url);
        client_builder = client_builder.sled_store(Path::new("./matrix_data"), None)?;

        Ok(client_builder.build().await?)
    }

    pub async fn new(homeserver_url: String, user_id: String, password: String) -> Result<Self> {
        let client = IndexerBot::get_client(homeserver_url).await?;
        client
            .login_username(&user_id, &password)
            .initial_device_display_name("Knowledge Indexer bot")
            .send()
            .await?;

        info!("Trying to connect to indradb");
        let mut indexer_client = utils::get_client_retrying().await?;
        indexer_client.ping().await?;
        info!("Connected to indradb");

        Ok(IndexerBot {
            client,
            indexer_client,
            message_map: MessagesMap::default(),
            room_type: utils::indradb::Identifier::new("matrix_room")?,
            room_id_type: utils::indradb::Identifier::new("room_id")?,
            room_name_type: utils::indradb::Identifier::new("room_name")?,
            room_topic_type: utils::indradb::Identifier::new("room_topic")?,
            text_message_event_type: utils::indradb::Identifier::new("text_message_event")?,
            notice_message_event_type: utils::indradb::Identifier::new("notice_message_event")?,
            event_id_type: utils::indradb::Identifier::new("event_id")?,
        })
    }

    pub async fn relogin(
        homeserver_url: String,
        user_id: String,
        access_token: String,
        device_id: String,
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

        info!("Trying to connect to indradb");
        let mut indexer_client = utils::get_client_retrying().await?;
        indexer_client.ping().await?;
        info!("Connected to indradb");

        let client_clone = client.clone();
        tokio::spawn(async move {
            let settings = SyncSettings::default();
            client_clone.sync(settings).await.unwrap();
        });

        Ok(IndexerBot {
            client,
            indexer_client,
            message_map: MessagesMap::default(),
            room_type: utils::indradb::Identifier::new("matrix_room")?,
            room_id_type: utils::indradb::Identifier::new("room_id")?,
            room_name_type: utils::indradb::Identifier::new("room_name")?,
            room_topic_type: utils::indradb::Identifier::new("room_topic")?,
            text_message_event_type: utils::indradb::Identifier::new("text_message_event")?,
            notice_message_event_type: utils::indradb::Identifier::new("notice_message_event")?,
            event_id_type: utils::indradb::Identifier::new("event_id")?,
        })
    }

    pub async fn start_processing(&mut self) {
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
                                            self.text_message_event_type,
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
                                            self.notice_message_event_type,
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
                        Ok(AnySyncTimelineEvent::MessageLike(_)) => {}
                        Ok(AnySyncTimelineEvent::State(_)) => {}
                        Err(e) => {
                            error!("Error deserializing event: {}", e)
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
                        utils::indradb::Vertex::with_id(*uuid, self.room_type),
                    ))
                    .await;
                inserter
                    .push(utils::indradb::BulkInsertItem::VertexProperty(
                        *uuid,
                        self.room_id_type,
                        serde_json::Value::String(room_id.to_string()).into(),
                    ))
                    .await;
                if let Some(room_name) = &room_properties.name {
                    inserter
                        .push(utils::indradb::BulkInsertItem::VertexProperty(
                            *uuid,
                            self.room_name_type,
                            serde_json::Value::String(room_name.to_string()).into(),
                        ))
                        .await;
                }
                if let Some(room_topic) = &room_properties.topic {
                    inserter
                        .push(utils::indradb::BulkInsertItem::VertexProperty(
                            *uuid,
                            self.room_topic_type,
                            serde_json::Value::String(room_topic.to_string()).into(),
                        ))
                        .await;
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
                    .await;
                inserter
                    .push(utils::indradb::BulkInsertItem::VertexProperty(
                        *uuid,
                        self.event_id_type,
                        serde_json::Value::String(event_id.to_string()).into(),
                    ))
                    .await;
                for event_property in event_properties
                    .as_vec(*uuid)
                    .expect("Unable to convert to indradb properties")
                {
                    inserter.push(event_property).await;
                }
            }
            inserter.sync().await.expect("Unable to sync indradb");
        }
    }
}
