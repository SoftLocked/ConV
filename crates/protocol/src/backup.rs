//! Versioned full-history backups. Loading always allocates a fresh writer session.
use crate::replication::{Mutation, Replica, ReplicationError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Backup {
    version: u8,
    room: u128,
    mutations: Vec<Mutation>,
}

#[derive(Debug)]
pub enum BackupError {
    Invalid,
    WrongRoom,
    ReusedWriter,
    Replica(ReplicationError),
}

impl Replica {
    pub fn backup(&self, room: u128) -> Result<Vec<u8>, BackupError> {
        let payload = postcard::to_allocvec(&Backup {
            version: 1,
            room,
            mutations: self.history_values().cloned().collect(),
        })
        .map_err(|_| BackupError::Invalid)?;
        if payload.len() > 64 * 1024 * 1024 - 10 {
            return Err(BackupError::Invalid);
        }
        let mut bytes = b"CVBK01".to_vec();
        bytes.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        bytes.extend_from_slice(&payload);
        Ok(bytes)
    }

    pub fn merge_backup(&mut self, room: u128, bytes: &[u8]) -> Result<(), BackupError> {
        if bytes.len() < 10 || bytes.len() > 64 * 1024 * 1024 || &bytes[..6] != b"CVBK01" {
            return Err(BackupError::Invalid);
        }
        if crc32fast::hash(&bytes[10..]) != u32::from_le_bytes(bytes[6..10].try_into().unwrap()) {
            return Err(BackupError::Invalid);
        }
        let (backup, rest): (Backup, _) =
            postcard::take_from_bytes(&bytes[10..]).map_err(|_| BackupError::Invalid)?;
        if backup.version != 1 || !rest.is_empty() {
            return Err(BackupError::Invalid);
        }
        if backup.room != room {
            return Err(BackupError::WrongRoom);
        }
        let mut staged = self.clone();
        for mutation in backup.mutations {
            staged.receive(&mutation).map_err(BackupError::Replica)?;
        }
        *self = staged;
        Ok(())
    }

    pub fn restore(writer: u128, room: u128, bytes: &[u8]) -> Result<Self, BackupError> {
        if bytes.len() < 10 || bytes.len() > 64 * 1024 * 1024 || &bytes[..6] != b"CVBK01" {
            return Err(BackupError::Invalid);
        }
        let checksum = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        if crc32fast::hash(&bytes[10..]) != checksum {
            return Err(BackupError::Invalid);
        }
        let (backup, rest): (Backup, _) =
            postcard::take_from_bytes(&bytes[10..]).map_err(|_| BackupError::Invalid)?;
        if backup.version != 1 || !rest.is_empty() {
            return Err(BackupError::Invalid);
        }
        if backup.room != room {
            return Err(BackupError::WrongRoom);
        }
        let mut replica = Self::new(writer);
        for mutation in backup.mutations {
            if mutation.replica == writer {
                return Err(BackupError::ReusedWriter);
            }
            replica.receive(&mutation).map_err(BackupError::Replica)?;
        }
        Ok(replica)
    }
}
