use std::{
    collections::{BTreeMap, VecDeque},
    mem::replace,
};

use color_eyre::Result;
use lazy_static::lazy_static;
use matrix_sdk::ruma::{OwnedEventId, OwnedRoomId};
use tokio::task::JoinHandle;
use utils::{
    indradb::{self, BulkInsertItem},
    indradb_proto as proto,
};
use uuid::{Context, Uuid};

const REQUEST_BUFFER_SIZE: usize = 10_000;

lazy_static! {
    static ref CONTEXT: Context = Context::new(0);
}

pub struct BulkInserter {
    requests: async_channel::Sender<Vec<indradb::BulkInsertItem>>,
    workers: Vec<JoinHandle<Result<()>>>,
    buf: Vec<indradb::BulkInsertItem>,
    client: proto::Client,
}

impl BulkInserter {
    pub fn new(client: proto::Client) -> Self {
        let (tx, rx) = async_channel::bounded::<Vec<indradb::BulkInsertItem>>(10);
        let mut workers = Vec::default();

        for _ in 0..10 {
            let rx = rx.clone();
            let mut client = client.clone();
            workers.push(tokio::spawn(async move {
                while let Ok(buf) = rx.recv().await {
                    client.bulk_insert(buf).await?;
                }
                Ok(())
            }));
        }

        Self {
            client,
            requests: tx,
            workers,
            buf: Vec::with_capacity(REQUEST_BUFFER_SIZE),
        }
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.client.sync().await?;
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<()> {
        if !self.buf.is_empty() {
            self.requests.send(self.buf.clone()).await?;
        }
        //self.requests.close();
        // for worker in &self.workers {
        //     worker.await??;
        // }
        self.sync().await?;
        Ok(())
    }

    pub async fn push(&mut self, item: indradb::BulkInsertItem) -> Result<()> {
        self.buf.push(item);
        if self.buf.len() >= REQUEST_BUFFER_SIZE {
            let buf = replace(&mut self.buf, Vec::with_capacity(REQUEST_BUFFER_SIZE));
            self.requests.send(buf).await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct UUIDEventMapType {
    pub event_id: OwnedEventId,
    pub uuid: EventUuid,
    pub event_type: utils::indradb::Identifier,
    pub event_properties: EventProperties,
}

#[derive(Clone)]
pub struct UUIDRoomMapType {
    pub room_id: OwnedRoomId,
    pub uuid: RoomUuid,
    pub room_properties: RoomProperties,
}

#[derive(Clone)]
pub enum EventProperties {
    TextMessage(String, Option<String>, Option<String>),
}

#[derive(Clone)]
pub struct RoomProperties {
    pub name: Option<String>,
    pub topic: Option<String>,
}

pub type RoomUuid = Uuid;
pub type EventUuid = Uuid;

// TODO: Track all the properties!
#[derive(Default)]
pub struct MessagesMap {
    event_uuids: BTreeMap<OwnedEventId, EventUuid>,
    pub message_list: VecDeque<UUIDEventMapType>,
    room_uuids: BTreeMap<OwnedRoomId, RoomUuid>,
    pub room_list: VecDeque<UUIDRoomMapType>,
    pub room_event_links: BTreeMap<EventUuid, RoomUuid>,
}

impl EventProperties {
    pub fn as_vec(&self, uuid: Uuid) -> Result<Vec<BulkInsertItem>> {
        match self {
            EventProperties::TextMessage(body, format, formatted_body) => {
                let mut vector = Vec::with_capacity(3);
                vector.push(utils::indradb::BulkInsertItem::VertexProperty(
                    uuid,
                    utils::indradb::Identifier::new("text_message_body")?,
                    serde_json::Value::String(body.to_string()).into(),
                ));
                if let Some(format) = format {
                    vector.push(utils::indradb::BulkInsertItem::VertexProperty(
                        uuid,
                        utils::indradb::Identifier::new("text_message_format")?,
                        serde_json::Value::String(format.to_string()).into(),
                    ));
                }
                if let Some(formatted_body) = formatted_body {
                    vector.push(utils::indradb::BulkInsertItem::VertexProperty(
                        uuid,
                        utils::indradb::Identifier::new("text_message_formatted_body")?,
                        serde_json::Value::String(formatted_body.to_string()).into(),
                    ));
                }

                Ok(vector)
            }
        }
    }
}

impl MessagesMap {
    pub fn insert_event(
        &mut self,
        event_id: OwnedEventId,
        room_uuid: RoomUuid,
        event_type: utils::indradb::Identifier,
        event_properties: EventProperties,
    ) -> EventUuid {
        // FIXME: We need to actually look them up here to get the uuid since we index live.
        // This means the uuid list might not have all events when we are indexing reactions.
        if let Some(&uuid) = self.event_uuids.get(&event_id) {
            return uuid;
        }

        let uuid = Uuid::new_v4();
        let map_thingy = UUIDEventMapType {
            event_id: event_id.clone(),
            uuid,
            event_type,
            event_properties,
        };
        self.event_uuids.insert(event_id, uuid);
        self.message_list.push_back(map_thingy);
        self.room_event_links.insert(uuid, room_uuid);
        uuid
    }

    pub fn insert_room(
        &mut self,
        room_id: OwnedRoomId,
        room_properties: RoomProperties,
    ) -> RoomUuid {
        // FIXME: We need to actually look them up here to get the uuid since we index live.
        // This means the uuid list might not have all events when we are indexing reactions.
        // FIXME: we probably need to also make updates for roomnames and topic here
        if let Some(&uuid) = self.room_uuids.get(&room_id) {
            return uuid;
        }

        let uuid = Uuid::new_v4();
        let map_thingy = UUIDRoomMapType {
            room_id: room_id.clone(),
            uuid,
            room_properties,
        };
        self.room_uuids.insert(room_id, uuid);
        self.room_list.push_back(map_thingy);
        uuid
    }
}
