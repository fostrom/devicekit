/// Moonlight is Fostrom's Binary TCP-based protocol to communicate with Devices.
/// Devices open a connection to Luna's Moonlight port and send and receive packets.
/// Moonlight is used in Fostrom's Device Agent, which powers all of Fostrom's SDKs.
///
/// Notes:
///
/// * The protocol supports JSON and MsgPack serialization formats, with the ability to
///   support more in the future. The serialization format is denoted by a single byte in
///   the CONNECT packet. (1 for MsgPack, 2 for JSON).
///
/// * Each packet's first byte is always the `packet_type`.
///
/// * Each packet's second byte is always a `flags` byte, which for now is mostly zero
///   in most packets. However, each flag represents something different for each packet.
///
/// * That being said, **the high-watermark of the byte (the first bit)
///   is reserved for future use**. The specific use of that bit is to represent
///   a second byte of flags if needed, just like how variable-length encoding
///   works in protocols such as MQTT.
///
/// * The **second high-watermark bit** is reserved for future use,
///   to indicate if the request is for a virtual device instead of the current device.
///
/// * If a packet needs to indicate success, that should be the low-watermark flag
///   (last bit) of the flags byte. This should be 1 in case of success and 0 in
///   case of failure.
//
//
// ---------------
// --- IMPORTS ---
// ---------------
use anyhow::{Result, anyhow};
use deku::prelude::*;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use strum::{Display, EnumIter, EnumString};
use thiserror::Error;

// -----------------------------------------
// --- Base Enums for Moonlight Protocol ---
// -----------------------------------------

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
pub enum SerializationFormat {
    #[deku(id = 0x01)]
    #[strum(serialize = "msgpack")]
    MsgPack = 1,
    #[deku(id = 0x02)]
    #[strum(serialize = "json")]
    JSON = 2,
}

#[derive(Display, EnumIter, Debug, Clone, Copy, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
pub enum PulseType {
    #[deku(id = 0)]
    #[strum(serialize = "unknown")]
    Unknown = 0,
    #[deku(id = 1)]
    #[strum(serialize = "system")]
    System = 1,
    #[deku(id = 2)]
    #[strum(serialize = "datapoint")]
    Data = 2,
    #[deku(id = 3)]
    #[strum(serialize = "msg")]
    Msg = 3,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
pub enum UnauthorizedError {
    #[deku(id = 0)]
    #[error("unauthorized: Unknown Error")]
    Unknown = 0,
    #[deku(id = 1)]
    #[error(
        "unauthorized: The credentials are invalid. Please confirm the Fleet ID, Device ID, and the Device Secret are correctly entered."
    )]
    InvalidCredentials = 1,
    #[deku(id = 2)]
    #[error("unauthorized: No fleet with the provided Fleet ID exists.")]
    FleetNotFound = 2,
    #[deku(id = 3)]
    #[error("unauthorized: No device with the provided Device ID exists.")]
    DeviceNotFound = 3,
    #[deku(id = 4)]
    #[error("unauthorized: The device secret is incorrect. It may have been reset.")]
    DeviceSecretIncorrect = 4,
    #[deku(id = 5)]
    #[error(
        "unauthorized: The device is disabled. Enable it on fostrom.io to use it. Please note that after enabling, the device may not retry connecting for up to ten minutes. Restart the device if possible."
    )]
    DeviceDisabled = 5,
    #[deku(id = 6)]
    #[error(
        "unauthorized: The device has been temporarily banned. This happens when the device retries connecting too frequently and receives unauthorized responses. The ban is automatically lifted every 15 minutes."
    )]
    TemporaryBan = 6,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, Hash, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
pub enum ConnectFailedError {
    #[deku(id = 0)]
    #[error("connect_failed: Unknown Error")]
    Unknown = 0,
    #[deku(id = 1)]
    #[error("connect_failed: Fostrom servers are restarting...")]
    ServiceRestarting = 1,
    #[deku(id = 2)]
    #[error("connect_failed: Fostrom is currently unavailable.")]
    ServiceUnavailable = 2,
    #[deku(id = 3)]
    #[error("connect_failed: Fostrom is currently experiencing degraded availability.")]
    ServiceDegraded = 3,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectedReason {
    #[error("disconnect: Socket Terminated")]
    ForceCloseSocket,
    #[error("disconnect: Disconnected from Server on User Request")]
    NormalDisconnect,
    #[error(transparent)]
    Unauthorized(#[from] UnauthorizedError),
    #[error(transparent)]
    ConnectFailed(#[from] ConnectFailedError),
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
pub enum PulseErrorReason {
    #[deku(id = 0)]
    #[error("unknown_error: An unknown error occurred while sending the packet.")]
    Unknown = 0,
    #[deku(id = 1)]
    #[error(
        "deserialization_failed: The Fostrom server failed to deserialize the packet correctly."
    )]
    DeserializationFailed = 1,
    #[deku(id = 2)]
    #[error(
        "packet_schema_not_found: There is no Packet Schema associated with the name provided."
    )]
    PacketSchemaNotFound = 2,
    #[deku(id = 3)]
    #[error(
        "packet_schema_type_mismatch: The Packet Schema does exist but not of the type provided."
    )]
    PacketSchemaTypeMismatch = 3,
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(id_type = "u8", ctx = "endian: deku::ctx::Endian")]
#[strum(serialize_all = "snake_case")]
pub enum MailAckType {
    #[deku(id = 1)]
    #[strum(serialize = "acknowledge")]
    Ack = 1,
    #[deku(id = 2)]
    Reject = 2,
    #[deku(id = 3)]
    Requeue = 3,
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneralErrors {
    #[error(
        "invalid_pulse_id: The provided Pulse ID is invalid. A Pulse ID is a 64-bit unsigned integer."
    )]
    InvalidPulseID,

    #[error("mail_ack_failed: Failed to {ack_type} the mail with ID {pulse_id}.")]
    AckMailFailed {
        pulse_id: u64,
        ack_type: MailAckType,
    },

    #[error(
        "channel_write_failed: An internal error occured. A channel that should have been writable wasn't."
    )]
    ChannelWriteFailed,

    #[error(
        "channel_read_failed: An internal error occured. A channel that should have been readable wasn't."
    )]
    ChannelReadFailed,

    #[error("duplicate_request: A request with the same transaction ID has already been queued.")]
    DuplicateReq,
}

// -------------------
// --- Mail Struct ---
// -------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mail {
    pub pulse_id: u64,
    pub name: String,
    pub payload: Option<Value>,
    pub mailbox_size: u16,
}

// -------------
// --- CREDS ---
// -------------

#[derive(Error, Debug, Clone, PartialEq, Eq, Hash, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum CredErr {
    #[error(
        "Fleet ID is missing. Fleet ID is an 8-character alphanumeric string. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    FleetIDMissing,

    #[error(
        "Fleet ID is invalid. Fleet ID is an 8-character alphanumeric string. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    FleetIDInvalid,

    #[error(
        "Device ID is missing. Device ID is a 10-character alphanumeric string. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    DeviceIDMissing,

    #[error(
        "Device ID is invalid. Device ID is a 10-character alphanumeric string. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    DeviceIDInvalid,

    #[error(
        "Device Secret is missing. Device Secret is a 36-character alphanumeric string that begins with `FOS-`. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    DeviceSecretMissing,

    #[error(
        "Device Secret is invalid. Device Secret is a 36-character alphanumeric string that begins with `FOS-`. You'll find the device credentials in your Fleet's Devices tab on https://fostrom.io."
    )]
    DeviceSecretInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creds {
    pub fleet_id: String,
    pub device_id: String,
    pub device_secret: String,
    pub prod: bool,
}

impl Creds {
    pub fn new(
        fleet_id: impl ToString,
        device_id: impl ToString,
        device_secret: impl ToString,
        prod: bool,
    ) -> Result<Self, CredErr> {
        let creds = Self {
            fleet_id: fleet_id.to_string(),
            device_id: device_id.to_string(),
            device_secret: device_secret.to_string(),
            prod,
        };

        creds.validate()?;
        Ok(creds)
    }

    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.fleet_id.as_bytes());
        hasher.update(self.device_id.as_bytes());
        hasher.update(self.device_secret.as_bytes());
        hasher.update(self.prod.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn validate(&self) -> Result<(), CredErr> {
        Self::validate_fleet_id(&self.fleet_id)?;
        Self::validate_device_id(&self.device_id)?;
        Self::validate_device_secret(&self.device_secret)?;
        Ok(())
    }

    fn validate_fleet_id(fleet_id: &str) -> Result<(), CredErr> {
        if fleet_id.is_empty() {
            Err(CredErr::FleetIDMissing)
        } else if fleet_id.len() != 8 || !fleet_id.chars().all(|c| c.is_alphanumeric()) {
            Err(CredErr::FleetIDInvalid)
        } else {
            Ok(())
        }
    }

    fn validate_device_id(device_id: &str) -> Result<(), CredErr> {
        if device_id.is_empty() {
            Err(CredErr::DeviceIDMissing)
        } else if device_id.len() != 10 || !device_id.chars().all(|c| c.is_alphanumeric()) {
            Err(CredErr::DeviceIDInvalid)
        } else {
            Ok(())
        }
    }

    fn validate_device_secret(device_secret: &str) -> Result<(), CredErr> {
        if device_secret.is_empty() {
            Err(CredErr::DeviceSecretMissing)
        } else if device_secret.len() != 36
            || !device_secret.starts_with("FOS-")
            || !device_secret.chars().skip(4).all(|c| c.is_alphanumeric())
        {
            Err(CredErr::DeviceSecretInvalid)
        } else {
            Ok(())
        }
    }
}

// ------------------------------------
// --- All Moonlight Packet Structs ---
// ------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, DekuRead, DekuWrite)]
#[deku(endian = "big", id_type = "u8")]
pub enum MoonlightPacket {
    #[deku(id = "1")]
    CloseConnection {
        #[deku(bits = "1", pad_bits_before = "7")]
        server: bool,
    },

    #[deku(id = "2")]
    Connect {
        #[deku(bits = "1", pad_bits_before = "7")]
        keep_alive: bool,

        protocol_version: u8,

        serialization_format: SerializationFormat,

        #[deku(count = "8")]
        fleet_id: Vec<u8>,

        #[deku(count = "10")]
        device_id: Vec<u8>,

        #[deku(count = "36")]
        device_secret: Vec<u8>,
    },

    #[deku(id = "3")]
    Connected {
        #[deku(bits = "1", pad_bits_before = "6")]
        mail_available: bool,

        #[deku(bits = "1")]
        keep_alive: bool,
    },

    #[deku(id = "4")]
    Unauthorized {
        #[deku(pad_bytes_before = "1")]
        reason: UnauthorizedError,
    },

    #[deku(id = "5")]
    ConnectFailed {
        #[deku(pad_bytes_before = "1")]
        reason: ConnectFailedError,
    },

    #[deku(id = "8")]
    Heartbeat(u8),

    #[deku(id = "9")]
    HeartbeatAck {
        #[deku(bits = "1", pad_bits_before = "7")]
        successful: bool,
    },

    #[deku(id = "10")]
    Pulse {
        #[deku(pad_bytes_before = "1")]
        pulse_type: PulseType,

        pulse_id: u64,

        name_len: u8,
        #[deku(count = "name_len")]
        name: Vec<u8>,

        payload_len: u32,
        #[deku(count = "payload_len")]
        payload: Vec<u8>,
    },

    #[deku(id = "11")]
    PulseResp {
        #[deku(bits = "1", pad_bits_before = "7")]
        successful: bool,
        pulse_id: u64,
        #[deku(cond = "!successful")]
        error_reason: Option<PulseErrorReason>,
    },

    #[deku(id = "20")]
    NewMailEvent {
        #[deku(pad_bytes_before = "1")]
        mailbox_size: u16,
        pulse_id: u64,
    },

    #[deku(id = "21")]
    MailboxNext {
        #[deku(bits = "1", pad_bits_before = "7")]
        header_only: bool,
        txn_id: u64,
    },

    #[deku(id = "22")]
    MailboxNextResp {
        #[deku(bits = "1", pad_bits_before = "6")]
        header_only: bool,

        #[deku(bits = "1")]
        successful: bool,

        txn_id: u64,
        mailbox_size: u16,

        #[deku(cond = "*successful && *mailbox_size > 0")]
        pulse_id: Option<u64>,

        #[deku(cond = "*successful && *mailbox_size > 0")]
        name_len: Option<u8>,

        #[deku(
            cond = "*successful && *mailbox_size > 0",
            count = "name_len.unwrap_or(0)"
        )]
        name: Option<Vec<u8>>,

        #[deku(cond = "*successful && !header_only && *mailbox_size > 0")]
        payload_len: Option<u32>,
        #[deku(
            cond = "*successful && !header_only && *mailbox_size > 0",
            count = "payload_len.unwrap_or(0)"
        )]
        payload: Option<Vec<u8>>,
    },

    #[deku(id = "25")]
    AckMail {
        #[deku(pad_bytes_before = "1")]
        pulse_id: u64,
        ack_type: MailAckType,
    },

    #[deku(id = "26")]
    AckMailResp {
        #[deku(bits = "1", pad_bits_before = "7")]
        successful: bool,
        mailbox_size: u16,
        pulse_id: u64,
        ack_type: MailAckType,
    },
}

// ---------------------------
// --- PACKET CONSTRUCTORS ---
// ---------------------------

impl MoonlightPacket {
    // connect() is the only function with a different return signature.
    // It returns a Result of (Packet, Creds) or CredErr enum
    // All other functions simply return the Packet
    pub fn connect(
        fleet_id: String,
        device_id: String,
        device_secret: String,
        prod: bool,
    ) -> Result<(Self, Creds), CredErr> {
        let creds = Creds::new(&fleet_id, &device_id, &device_secret, prod)?;

        let connect_packet = Self::Connect {
            keep_alive: true,
            protocol_version: 1,
            serialization_format: SerializationFormat::JSON,
            fleet_id: fleet_id.as_bytes().to_vec(),
            device_id: device_id.as_bytes().to_vec(),
            device_secret: device_secret.as_bytes().to_vec(),
        };

        Ok((connect_packet, creds))
    }

    pub fn server_close_connection() -> Self {
        Self::CloseConnection { server: true }
    }

    pub fn client_close_connection() -> Self {
        Self::CloseConnection { server: false }
    }

    pub fn connected(mail_available: bool, keep_alive: bool) -> Self {
        Self::Connected {
            mail_available,
            keep_alive,
        }
    }

    pub fn unauthorized(reason: UnauthorizedError) -> Self {
        Self::Unauthorized { reason }
    }

    pub fn connect_failed(reason: ConnectFailedError) -> Self {
        Self::ConnectFailed { reason }
    }

    pub fn heartbeat() -> Self {
        Self::Heartbeat(0)
    }

    pub fn heartbeat_ack(successful: bool) -> Self {
        Self::HeartbeatAck { successful }
    }

    pub fn pulse(pulse_type: PulseType, pulse_id: u64, name: String, payload: String) -> Self {
        if name.len() > 255 {
            panic!("Mail name cannot be more than 255 characters");
        }

        Self::Pulse {
            pulse_type,
            pulse_id,
            name_len: name.len() as u8,
            payload_len: payload.len() as u32,
            name: name.as_bytes().to_vec(),
            payload: payload.as_bytes().to_vec(),
        }
    }

    pub fn pulse_resp_success(pulse_id: u64) -> Self {
        Self::PulseResp {
            successful: true,
            pulse_id,
            error_reason: None,
        }
    }

    pub fn pulse_resp_error(pulse_id: u64, error_reason: PulseErrorReason) -> Self {
        Self::PulseResp {
            successful: false,
            pulse_id,
            error_reason: Some(error_reason),
        }
    }

    pub fn new_mail_event(mailbox_size: u16, pulse_id: u64) -> Self {
        Self::NewMailEvent {
            mailbox_size,
            pulse_id,
        }
    }

    pub fn mailbox_next(header_only: bool, txn_id: u64) -> Self {
        Self::MailboxNext {
            header_only,
            txn_id,
        }
    }

    pub fn mailbox_next_resp_empty(txn_id: u64) -> Self {
        Self::MailboxNextResp {
            header_only: false,
            successful: true,
            txn_id,
            mailbox_size: 0,
            pulse_id: None,
            name_len: None,
            name: None,
            payload_len: None,
            payload: None,
        }
    }

    pub fn mailbox_next_resp_failed(txn_id: u64) -> Self {
        Self::MailboxNextResp {
            header_only: false,
            successful: false,
            txn_id,
            mailbox_size: 0,
            pulse_id: None,
            name_len: None,
            name: None,
            payload_len: None,
            payload: None,
        }
    }

    pub fn mailbox_next_resp_header_only(
        txn_id: u64,
        mailbox_size: u16,
        pulse_id: u64,
        name: String,
    ) -> Self {
        Self::MailboxNextResp {
            header_only: true,
            successful: true,
            txn_id,
            mailbox_size,
            pulse_id: Some(pulse_id),
            name_len: Some(name.len() as u8),
            name: Some(name.as_bytes().to_vec()),
            payload_len: None,
            payload: None,
        }
    }

    pub fn mailbox_next_resp_full(
        txn_id: u64,
        mailbox_size: u16,
        pulse_id: u64,
        name: String,
        payload: String,
    ) -> Self {
        if name.len() > 255 {
            panic!("Mail name cannot be more than 255 characters");
        }

        Self::MailboxNextResp {
            header_only: false,
            successful: true,
            txn_id,
            mailbox_size,
            pulse_id: Some(pulse_id),
            name_len: Some(name.len() as u8),
            name: Some(name.as_bytes().to_vec()),
            payload_len: Some(payload.len() as u32),
            payload: Some(payload.as_bytes().to_vec()),
        }
    }

    pub fn ack_mail(pulse_id: u64, ack_type: MailAckType) -> Self {
        Self::AckMail { pulse_id, ack_type }
    }

    pub fn ack_mail_resp(mailbox_size: u16, pulse_id: u64, ack_type: MailAckType) -> Self {
        Self::AckMailResp {
            successful: true,
            mailbox_size,
            pulse_id,
            ack_type,
        }
    }

    pub fn ack_mail_resp_failed(pulse_id: u64, ack_type: MailAckType) -> Self {
        Self::AckMailResp {
            successful: false,
            mailbox_size: 0,
            pulse_id,
            ack_type,
        }
    }
}

// -------------
// --- CODEC ---
// -------------

/// Implements the encoder and streaming decoder
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Codec {
    buffer: Vec<u8>,
}

impl Codec {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024 * 1024),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Encodes a single packet
    pub fn encode(packet: &MoonlightPacket) -> Result<Vec<u8>> {
        packet
            .to_bytes()
            .map_err(|e| anyhow!("Invalid Packet: {}", e))
    }

    /// Decodes a single packet
    fn decode(bytes: &[u8]) -> Result<Option<(MoonlightPacket, usize)>> {
        match MoonlightPacket::from_bytes((bytes, 0)) {
            Ok(((rest, _bit_offset), packet)) => {
                let consumed = bytes.len() - rest.len();
                Ok(Some((packet, consumed)))
            }
            Err(DekuError::Incomplete(_)) => Ok(None),
            Err(e) => Err(anyhow!("Failed to decode packet: {}", e)),
        }
    }

    fn process_packets(&mut self) -> Result<Vec<MoonlightPacket>> {
        let mut packets = Vec::new();

        while let Some((packet, consumed)) = Codec::decode(&self.buffer)? {
            packets.push(packet);
            self.buffer.drain(..consumed);
        }

        Ok(packets)
    }
}

// ------------------------------
// --- SERVER RESPONSE PARSER ---
// ------------------------------

use MoonlightPacket as P;

/// ServerResp is an enum of all the possible variants that
/// the client needs to handle and process that originate from the server
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerResp {
    // Unknown Packet
    ForceCloseSocket,

    // Events
    Connected(bool),
    Disconnected(DisconnectedReason),
    HeartbeatAck,
    NewMail,

    // Transactions
    PulseResp(Result<u64, (u64, PulseErrorReason)>),
    AckMailResp(Result<(u64, bool), (u64, MailAckType)>),
    MailboxNext(Result<(u64, Option<Mail>), u64>),
}

impl ServerResp {
    fn handle_packet(packet: P) -> ServerResp {
        match packet {
            P::CloseConnection { server: true } => {
                ServerResp::Disconnected(DisconnectedReason::NormalDisconnect)
            }

            P::ConnectFailed { reason } => {
                ServerResp::Disconnected(DisconnectedReason::ConnectFailed(reason))
            }

            P::Unauthorized { reason } => {
                ServerResp::Disconnected(DisconnectedReason::Unauthorized(reason))
            }

            P::Connected { mail_available, .. } => ServerResp::Connected(mail_available),

            P::HeartbeatAck { .. } => ServerResp::HeartbeatAck,

            P::PulseResp {
                successful,
                pulse_id,
                error_reason,
            } => {
                let r = if successful {
                    Ok(pulse_id)
                } else if error_reason.is_some() {
                    Err((pulse_id, error_reason.unwrap()))
                } else {
                    Err((pulse_id, PulseErrorReason::Unknown))
                };

                ServerResp::PulseResp(r)
            }

            P::AckMailResp {
                successful,
                mailbox_size,
                pulse_id,
                ack_type,
            } => ServerResp::AckMailResp(if successful {
                Ok((pulse_id, mailbox_size > 0))
            } else {
                Err((pulse_id, ack_type))
            }),

            P::MailboxNextResp {
                header_only,
                successful,
                txn_id,
                mailbox_size,
                pulse_id,
                name_len: _,
                name,
                payload_len: _,
                payload,
            } => {
                let mut mail = Mail {
                    pulse_id: 0,
                    name: "".to_string(),
                    payload: None,
                    mailbox_size,
                };

                if successful && mailbox_size != 0 {
                    let pulse_id = match pulse_id {
                        Some(pulse_id) => pulse_id,
                        None => return ServerResp::MailboxNext(Err(txn_id)),
                    };

                    let name = match name {
                        Some(name) => name,
                        None => return ServerResp::MailboxNext(Err(txn_id)),
                    };

                    let name = match String::from_utf8(name) {
                        Ok(name) => name,
                        Err(_) => return ServerResp::MailboxNext(Err(txn_id)),
                    };

                    mail.pulse_id = pulse_id;
                    mail.name = name;

                    if header_only {
                        ServerResp::MailboxNext(Ok((txn_id, Some(mail))))
                    } else {
                        mail.payload = match payload {
                            Some(pl) => match String::from_utf8(pl) {
                                Ok(pl) => serde_json::from_str(&pl).unwrap_or_default(),
                                _ => None,
                            },
                            None => None,
                        };

                        ServerResp::MailboxNext(Ok((txn_id, Some(mail))))
                    }
                } else if successful && mailbox_size == 0 {
                    ServerResp::MailboxNext(Ok((txn_id, None)))
                } else {
                    ServerResp::MailboxNext(Err(txn_id))
                }
            }

            P::NewMailEvent {
                mailbox_size: _,
                pulse_id: _,
            } => ServerResp::NewMail,

            _ => ServerResp::ForceCloseSocket,
        }
    }
}

// --------------------
// --- CLIENT LOGIC ---
// --------------------

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnChanResult {
    Ok,
    Err(String),
    Timeout,
    Mail(Option<Mail>),

    /// the bool is more-mail-available
    MailAckSuccessful(bool),
}

use ReturnChanResult as R;

/// Return Channel is a result of option<mail>.
/// When mailbox_next is the operation, None signifies mailbox is empty.
/// For any other operation, the Option is always None when successful.
type ReturnChan = Sender<ReturnChanResult>;

/// Commands that users of the client can send
#[derive(Debug, Clone)]
pub enum ClientCmd {
    /// SendPulse(PulseType, name, payload)
    SendPulse(PulseType, String, Option<Value>, ReturnChan),

    /// MailboxNext(header_only?)
    MailboxNext(bool, ReturnChan),

    /// MailOp(MailAckType, mail_id)
    MailOp(MailAckType, u64, ReturnChan),
}

/// An enum of all possible events that the client can process and receive
/// over a single mpsc channel
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// A generic event that can be sent to the ClientLogic process
    /// to conduct general cleanup, such as checking for timeouts
    /// in the pending_txns list.
    Refresh,

    /// Sent by the HeartbeatProc to signal that
    /// the client should send a heartbeat to the server
    HeartbeatTick,

    /// Sent by the Transport Stream whenever it receives bytes.
    /// To be fed into the Codec -> decode into ServerResps -> perform side-effects
    TransportRecv(Vec<u8>),

    /// Sent by the Transport Stream whenever it ends unexpectedly
    TransportClose,

    /// Sent by the user of the Moonlight client for operations
    Cmd(ClientCmd),
}

/// ClientLogic is a pure functional and stateful loop,
/// which handles all client-related logic while accepting
/// events over a channel and performing side effects.

#[derive(Debug)]
pub struct ClientLogic {
    /// Main channel to receive client events.
    /// chan.recv() is executed in a loop in start_loop()
    proc_mailbox_chan: Receiver<ClientEvent>,

    /// Transport Write Stream
    transport_write_chan: Sender<Vec<u8>>,

    /// A channel for notifications such as next_mail(mail) or connected/disconnected.
    /// The type is (EventName, EventPayload [Serialized JSON]).
    /// This can directly be pushed to Server-Sent Events streams.
    notify_chan: Sender<(String, String)>,

    /// A channel to send nothing when a HeartbeatAck packet is received from the server.
    /// The use-case is that the heartbeat loop can decide whether to force terminate the socket,
    /// or aggressively increase the number of heartbeats sent in an interval before closing.
    ping_chan: Sender<()>,

    /// Moonlight Codec struct for the streaming decoder
    codec: Codec,

    /// Pending Txns: the u64 is the pulse_id/txn_id.
    /// The Instant is tracked to check for timeouts
    next_txn_id: u64,
    pending_txns: HashMap<u64, (Instant, ReturnChan)>,

    /// Encoded Connect Packet and Creds Struct
    _creds: Creds,
    connect_packet_bytes: Vec<u8>,

    /// Authenticated
    authenticated: AtomicBool,
}

impl ClientLogic {
    pub fn new(
        fleet_id: String,
        device_id: String,
        device_secret: String,
        prod: bool,
        notify_chan: Sender<(String, String)>,
        ping_chan: Sender<()>,
        transport_write_chan: Sender<Vec<u8>>,
    ) -> Result<(Sender<ClientEvent>, Self)> {
        let (tx, rx): (Sender<ClientEvent>, Receiver<ClientEvent>) = channel();
        let (connect_packet, creds) = P::connect(fleet_id, device_id, device_secret, prod)?;
        let connect_packet_bytes = Codec::encode(&connect_packet)?;

        let codec = Codec::new();

        let client_logic = Self {
            proc_mailbox_chan: rx,
            transport_write_chan,
            notify_chan,
            ping_chan,
            codec,
            next_txn_id: 0,
            pending_txns: HashMap::with_capacity(32),
            _creds: creds,
            connect_packet_bytes,
            authenticated: AtomicBool::new(false),
        };

        Ok((tx, client_logic))
    }

    /// Use this function to convert string mail_ids into u64
    /// before sending them over the request channel.
    pub fn to_pulse_id(mail_id: impl ToString) -> Result<u64> {
        mail_id
            .to_string()
            .parse()
            .map_err(|_| anyhow!(GeneralErrors::InvalidPulseID.to_string()))
    }

    /// The primary purpose of this function is to ensure the
    /// transport stream gets authenticated before any other events happen.
    /// This function needs to be called before anything else has the opportunity
    /// to write to the proc_mailbox_chan channel, and waits to receive bytes
    /// from the transport.
    fn wait_for_authentication(&mut self) -> Result<(), DisconnectedReason> {
        let connect_packet = self.connect_packet_bytes.clone();

        match self.transport_write_chan.send(connect_packet) {
            Ok(_) => (),
            Err(_) => return Err(DisconnectedReason::ForceCloseSocket),
        }

        // Read the process mailbox until we can form a complete connect packet response.
        while let Ok(client_event) = self.proc_mailbox_chan.recv_timeout(Duration::from_secs(10)) {
            if let ClientEvent::TransportRecv(bytes) = client_event {
                self.codec.feed(&bytes);

                let mut packets = match self.codec.process_packets() {
                    Ok(packets) => packets.into_iter(),
                    Err(_) => return Err(DisconnectedReason::ForceCloseSocket),
                };

                if let Some(packet) = packets.next() {
                    match ServerResp::handle_packet(packet) {
                        ServerResp::Connected(mail_available) => {
                            self.authenticated.store(true, Ordering::SeqCst);

                            let notification = ("connected".to_string(), "".to_string());
                            let _ = self.notify_chan.send(notification);

                            if mail_available {
                                let notification = ("new_mail".to_string(), "".to_string());
                                let _ = self.notify_chan.send(notification);
                            }

                            // If there are remaining packets sent along with connect
                            // we can process them here before moving forward.
                            if packets.len() > 0 {
                                for packet in packets {
                                    let server_resp = ServerResp::handle_packet(packet);
                                    if let Some(disconnected_reason) =
                                        self.handle_server_resp(server_resp)
                                    {
                                        return Err(disconnected_reason);
                                    }
                                }
                            }

                            return Ok(());
                        }
                        ServerResp::Disconnected(disconnected_reason) => {
                            return Err(disconnected_reason);
                        }
                        _ => return Err(DisconnectedReason::ForceCloseSocket),
                    }
                } else {
                    continue;
                }
            } else {
                return Err(DisconnectedReason::ForceCloseSocket);
            }
        }

        Err(DisconnectedReason::ForceCloseSocket)
    }

    /// It blocks on chan.recv(), and can either return a DisconnectedReason
    /// or code inside this loop can panic, at which point, the thread
    /// should be cleanly restarted.
    fn start_loop(&mut self, shutdown_flag: Arc<AtomicBool>) -> DisconnectedReason {
        while !shutdown_flag.load(Ordering::SeqCst)
            && let Ok(client_event) = self.proc_mailbox_chan.recv()
        {
            if let Some(disconnected_reason) = self.process_client_event(client_event) {
                return disconnected_reason;
            }
        }

        DisconnectedReason::ForceCloseSocket
    }

    fn process_client_event(&mut self, client_event: ClientEvent) -> Option<DisconnectedReason> {
        match client_event {
            ClientEvent::Refresh => self.refresh(),
            ClientEvent::HeartbeatTick => {
                // Write heartbeat packet to transport
                let p = Codec::encode(&P::heartbeat()).unwrap();

                if self.transport_write_chan.send(p).is_err() {
                    return Some(DisconnectedReason::ForceCloseSocket);
                }
            }
            ClientEvent::TransportRecv(bytes) => {
                self.codec.feed(&bytes);
                let packets = match self.codec.process_packets() {
                    Ok(packets) => packets,
                    Err(_) => return Some(DisconnectedReason::ForceCloseSocket),
                };

                for packet in packets {
                    let server_resp = ServerResp::handle_packet(packet);
                    if let Some(disconnected_reason) = self.handle_server_resp(server_resp) {
                        return Some(disconnected_reason);
                    }
                }
            }
            ClientEvent::TransportClose => {
                // Close the loop, causing a full client restart
                return Some(DisconnectedReason::ForceCloseSocket);
            }
            ClientEvent::Cmd(cmd) => {
                if self.handle_cmd(cmd).is_err() {
                    return Some(DisconnectedReason::ForceCloseSocket);
                }
            }
        }

        None
    }

    fn next_txn_id(&mut self) -> u64 {
        let current_id = self.next_txn_id;
        self.next_txn_id = current_id.wrapping_add(1);
        current_id
    }

    /// Encode a MoonlightPacket into bytes and send it to the transport channel
    fn write_packet_to_transport(&mut self, packet: MoonlightPacket) -> Result<()> {
        let bytes = Codec::encode(&packet)?;
        self.transport_write_chan.send(bytes)?;
        Ok(())
    }

    /// Handle any generic routine cleanups
    fn refresh(&mut self) {
        // Check for any timeouts in the pending_txns list
        let now = Instant::now();
        // Timeout duration for pending transactions
        let timeout = Duration::from_secs(10);

        // Collect timed-out transaction IDs to avoid mutating the map while iterating
        let timed_out: Vec<u64> = self
            .pending_txns
            .iter()
            .filter_map(|(txn_id, (ts, _))| {
                if now.duration_since(*ts) > timeout {
                    Some(*txn_id)
                } else {
                    None
                }
            })
            .collect();

        // Remove timed-out entries and notify the waiting caller with a timeout error
        for txn_id in timed_out {
            if let Some((_ts, chan)) = self.pending_txns.remove(&txn_id) {
                let _ = chan.send(ReturnChanResult::Timeout);
            }
        }
    }

    fn push_txn(&mut self, return_chan: ReturnChan) -> Result<u64> {
        let now = Instant::now();
        let mut txn_id = self.next_txn_id();

        for _ in 0..3 {
            if self.pending_txns.contains_key(&txn_id) {
                txn_id = self.next_txn_id();
            } else {
                break;
            }
        }

        #[allow(clippy::map_entry)]
        if self.pending_txns.contains_key(&txn_id) {
            let _ = return_chan.send(R::Err("txn_failed: Transaction ID Exhaustion".to_string()));
            Err(anyhow!("txn_id_exhaustion"))
        } else {
            self.pending_txns.insert(txn_id, (now, return_chan));
            Ok(txn_id)
        }
    }

    fn handle_cmd(&mut self, cmd: ClientCmd) -> Result<()> {
        match cmd {
            ClientCmd::SendPulse(pulse_type, name, payload, return_chan) => {
                if name.len() > 255 {
                    let _ = return_chan.send(ReturnChanResult::Err(
                        "invalid_name: Pulse Name needs to be under 255 characters.".to_string(),
                    ));
                    return Ok(());
                }

                let pl = if let Some(payload) = payload {
                    serde_json::to_string(&payload).unwrap()
                } else {
                    "".to_string()
                };

                let txn_id = self.push_txn(return_chan)?;
                let p = P::pulse(pulse_type, txn_id, name, pl);
                self.write_packet_to_transport(p)
            }
            ClientCmd::MailboxNext(header_only, return_chan) => {
                let txn_id = self.push_txn(return_chan)?;
                let p = P::mailbox_next(header_only, txn_id);
                self.write_packet_to_transport(p)
            }
            ClientCmd::MailOp(ack_type, pulse_id, return_chan) => {
                // Clippy has a known inference issue here,
                // even though we aren't doing an insert in the if branch.
                #[allow(clippy::map_entry)]
                if self.pending_txns.contains_key(&pulse_id) {
                    let _ = return_chan.send(R::Err(GeneralErrors::DuplicateReq.to_string()));
                    Ok(())
                } else {
                    let p = P::ack_mail(pulse_id, ack_type);
                    self.pending_txns
                        .insert(pulse_id, (Instant::now(), return_chan));
                    self.write_packet_to_transport(p)
                }
            }
        }
    }

    fn handle_server_resp(&mut self, server_resp: ServerResp) -> Option<DisconnectedReason> {
        match server_resp {
            ServerResp::ForceCloseSocket => return Some(DisconnectedReason::ForceCloseSocket),
            ServerResp::Disconnected(disconnected_reason) => return Some(disconnected_reason),

            ServerResp::Connected(_mail_available) => {
                // This branch is unreachable because start_loop needs to be called
                // after successful authentication only.
                unreachable!();
            }

            ServerResp::HeartbeatAck => {
                let _ = self.ping_chan.send(());
            }

            ServerResp::NewMail => {
                let notification = ("new_mail".to_string(), "".to_string());
                let _ = self.notify_chan.send(notification);
            }

            ServerResp::PulseResp(pulse_result) => match pulse_result {
                Ok(txn_id) => self.resolve_txn(txn_id, R::Ok),
                Err((txn_id, pulse_error_reason)) => {
                    self.resolve_txn(txn_id, R::Err(pulse_error_reason.to_string()))
                }
            },

            ServerResp::AckMailResp(ack_result) => match ack_result {
                Ok((pulse_id, false)) => self.resolve_txn(pulse_id, R::MailAckSuccessful(false)),
                Ok((pulse_id, true)) => self.resolve_txn(pulse_id, R::MailAckSuccessful(true)),
                Err((pulse_id, mail_ack_type)) => self.resolve_txn(
                    pulse_id,
                    R::Err(
                        GeneralErrors::AckMailFailed {
                            pulse_id,
                            ack_type: mail_ack_type,
                        }
                        .to_string(),
                    ),
                ),
            },

            ServerResp::MailboxNext(mail_result) => match mail_result {
                Ok((txn_id, Some(mail))) => self.resolve_txn(txn_id, R::Mail(Some(mail))),
                Ok((txn_id, None)) => self.resolve_txn(txn_id, R::Mail(None)),
                Err(txn_id) => self.resolve_txn(
                    txn_id,
                    R::Err("failed: Failed to fetch next mail".to_string()),
                ),
            },
        }

        None
    }

    fn resolve_txn(&mut self, txn_id: u64, return_value: ReturnChanResult) {
        if let Some((_, return_chan)) = self.pending_txns.get(&txn_id) {
            let _ = return_chan.send(return_value);
            self.pending_txns.remove(&txn_id);
        }
    }
}

// ----------------------
// --- CLIENT PROCESS ---
// ----------------------

use std::sync::{Arc, Mutex};

use crate::moonlight_socket;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectMode {
    Prod,
    Local(u16),
}

// The Moonlight Client implements the functionality that covers
// managing the connection and restarting of side-effect threads
// while initializing the ClientLogic and starting its tight-loop.
#[derive(Debug, Clone)]
pub struct MoonlightClient {
    // Constants
    pub fleet_id: String,
    pub device_id: String,
    device_secret: String,
    connect_mode: ConnectMode,

    // Global
    shutdown_flag: Arc<AtomicBool>,

    // Session Dependent
    authenticated: Arc<AtomicBool>,
    disconnected_reason: Arc<Mutex<Option<DisconnectedReason>>>,
    reconnect_in: Arc<Mutex<Option<Duration>>>,
    mailbox_chan: Arc<Mutex<Option<Sender<ClientEvent>>>>,
}

impl MoonlightClient {
    pub fn new(
        fleet_id: String,
        device_id: String,
        device_secret: String,
        connect_mode: ConnectMode,
    ) -> Self {
        Self {
            fleet_id,
            device_id,
            device_secret,
            connect_mode,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            authenticated: Arc::new(AtomicBool::new(false)),
            disconnected_reason: Arc::new(Mutex::new(None)),
            reconnect_in: Arc::new(Mutex::new(None)),
            mailbox_chan: Arc::new(Mutex::new(None)),
        }
    }

    pub fn connected(&self) -> bool {
        self.authenticated.load(Ordering::SeqCst)
    }

    pub fn start(&mut self, notify_chan_tx: Sender<(String, String)>) -> Result<()> {
        while !self.shutdown_flag.load(Ordering::SeqCst) {
            let disconnect_reason = self.session_lifecycle(notify_chan_tx.clone())?;

            // Perform cleanup of session-related variables
            *self.disconnected_reason.lock().unwrap() = Some(disconnect_reason);
            *self.mailbox_chan.lock().unwrap() = None;
            self.authenticated.store(false, Ordering::SeqCst);

            // Change the backoff interval for reconnecting.
            // The sleep_time is the current sleep time,
            // while the reconnect_interval is now the next sleep time.
            let sleep_time = match disconnect_reason {
                DisconnectedReason::Unauthorized(_) => self.backoff(true),
                _ => self.backoff(false),
            };

            let notification = json!({
                "error": disconnect_reason.to_string(),
                "reconnecting_in_ms": sleep_time.as_millis() as u64
            });

            let _ = notify_chan_tx.send((
                "disconnected".to_string(),
                serde_json::to_string(&notification).unwrap(),
            ));

            // Sleep if we don't have to shutdown
            if !self.shutdown_flag.load(Ordering::SeqCst) {
                sleep(sleep_time);
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);

        let mailbox_chan = self.mailbox_chan.lock().unwrap();
        if mailbox_chan.is_some() {
            let _ = mailbox_chan
                .as_ref()
                .unwrap()
                .send(ClientEvent::TransportClose);
        }
    }

    fn session_lifecycle(
        &mut self,
        notify_chan_tx: Sender<(String, String)>,
    ) -> Result<DisconnectedReason> {
        let (ping_chan_tx, ping_chan_rx) = channel();
        let (transport_write_chan_tx, transport_write_chan_rx) = channel();

        let prod = match self.connect_mode {
            ConnectMode::Prod => true,
            ConnectMode::Local(_) => false,
        };

        let (mailbox_chan, mut logic) = ClientLogic::new(
            self.fleet_id.clone(),
            self.device_id.clone(),
            self.device_secret.clone(),
            prod,
            notify_chan_tx,
            ping_chan_tx,
            transport_write_chan_tx.clone(),
        )?;

        // Starts the transport process
        let (socket_handle, socket_close) = match moonlight_socket::connect(
            self.connect_mode.clone(),
            mailbox_chan.clone(),
            transport_write_chan_rx,
        ) {
            Err(_) => return Ok(DisconnectedReason::ForceCloseSocket),
            Ok((handle, close)) => (handle, close),
        };

        *self.mailbox_chan.lock().unwrap() = Some(mailbox_chan.clone());

        let disconnect_reason = match logic.wait_for_authentication() {
            Err(disconnected_reason) => disconnected_reason,
            Ok(()) => {
                self.authenticated.store(true, Ordering::SeqCst);
                *self.reconnect_in.lock().unwrap() = None;
                *self.disconnected_reason.lock().unwrap() = None;

                let shutdown_flag = Arc::new(AtomicBool::new(false));
                let shutdown_flag_1 = shutdown_flag.clone();
                let shutdown_flag_2 = shutdown_flag.clone();

                let mailbox_clone = mailbox_chan.clone();

                let timer_proc_handle = std::thread::spawn(move || {
                    Self::timer_proc(shutdown_flag_1, mailbox_clone, ping_chan_rx)
                });

                let disconnected_reason = logic.start_loop(shutdown_flag_2);
                shutdown_flag.store(true, Ordering::SeqCst);
                let _ = timer_proc_handle.join();
                disconnected_reason
            }
        };

        // Close the socket proc thread by
        // setting the shutdown flag
        socket_close();

        // Close the transport write channel,
        // which should shutdown the socket proc thread
        // in case it isn't already shutdown
        drop(transport_write_chan_tx);

        // Wait for the socket thread to close
        let _ = socket_handle.join();

        // At this point the cleanup is complete,
        // and the start() function will create a new session_lifecycle again.
        Ok(disconnect_reason)
    }

    fn timer_proc(
        shutdown_flag: Arc<AtomicBool>,
        mailbox: Sender<ClientEvent>,
        ping_chan: Receiver<()>,
    ) {
        let mut last_refresh_sent = Instant::now();
        let mut last_heartbeat_sent = Instant::now();
        let mut last_heartbeat_ack = Instant::now();

        while !shutdown_flag.load(Ordering::SeqCst) {
            Self::timer_logic(
                &shutdown_flag,
                &mailbox,
                &ping_chan,
                &mut last_refresh_sent,
                &mut last_heartbeat_sent,
                &mut last_heartbeat_ack,
            );

            sleep(Duration::from_millis(100));
        }
    }

    /// To make it easier to test the timer logic separately
    /// the logic is extracted into this function and called
    /// from timer_proc() above.
    fn timer_logic(
        shutdown_flag: &Arc<AtomicBool>,
        mailbox: &Sender<ClientEvent>,
        ping_chan: &Receiver<()>,
        last_refresh_sent: &mut Instant,
        last_heartbeat_sent: &mut Instant,
        last_heartbeat_ack: &mut Instant,
    ) {
        if ping_chan.try_recv() == Ok(()) {
            *last_heartbeat_ack = Instant::now();
        }

        if last_heartbeat_sent.elapsed() >= Duration::from_secs(30) {
            let _ = mailbox.send(ClientEvent::HeartbeatTick);
            *last_heartbeat_sent = Instant::now();
        }

        if last_heartbeat_sent.elapsed() >= Duration::from_secs(5)
            && last_heartbeat_sent > last_heartbeat_ack
        {
            // Missed heartbeat. Try sending again.
            let _ = mailbox.send(ClientEvent::HeartbeatTick);
            *last_heartbeat_sent = Instant::now();
        }

        if last_heartbeat_ack.elapsed() >= Duration::from_secs(90) {
            // Missed multiple heartbeats, shutdown everything.
            let _ = mailbox.send(ClientEvent::TransportClose);
            shutdown_flag.store(true, Ordering::SeqCst);
        }

        if last_refresh_sent.elapsed() >= Duration::from_millis(500) {
            let _ = mailbox.send(ClientEvent::Refresh);
            *last_refresh_sent = Instant::now();
        }
    }

    pub fn status(&self) -> Value {
        if self.authenticated.load(Ordering::SeqCst) {
            json!({"connected": true})
        } else {
            match *self.disconnected_reason.lock().unwrap() {
                None => json!({"connected": false}),
                Some(DisconnectedReason::Unauthorized(reason)) => {
                    json!({
                        "connected": false,
                        "error": "unauthorized",
                        "msg": reason.to_string(),
                    })
                }
                Some(DisconnectedReason::ConnectFailed(reason)) => {
                    let reconnect: u64 = if let Some(reconnect) = *self.reconnect_in.lock().unwrap()
                    {
                        reconnect.as_millis() as u64
                    } else {
                        0
                    };

                    json!({
                        "connected": false,
                        "error": "connect_failed",
                        "msg": reason.to_string(),
                        "reconnecting_in": reconnect,
                    })
                }
                Some(_) => {
                    json!({
                        "connected": false,
                        "error": "connect_failed",
                        "msg": ConnectFailedError::Unknown.to_string(),
                    })
                }
            }
        }
    }

    fn backoff(&mut self, unauthorized: bool) -> Duration {
        if unauthorized {
            let interval = Duration::from_secs(5 * 60);
            *self.reconnect_in.lock().unwrap() = Some(interval);
            return interval;
        }

        let reconnect_in = self
            .reconnect_in
            .lock()
            .unwrap()
            .map_or(0, |r| r.as_millis());

        let milliseconds = match reconnect_in {
            0 => 1_000,
            1_000 => 2_500,
            2_500 => 5_000,
            5_000 => 10_000,
            10_000 => 15_000,
            15_000 => 30_000,
            _ => 30_000,
        };

        let interval = Duration::from_millis(milliseconds);
        *self.reconnect_in.lock().unwrap() = Some(interval);
        Duration::from_millis(reconnect_in as u64)
    }

    pub fn send_cmd(&self, cmd: ClientCmd) {
        let mailbox_guard = self.mailbox_chan.lock();

        let sent = if let Ok(mailbox_guard) = mailbox_guard
            && mailbox_guard.is_some()
        {
            let mailbox_chan = mailbox_guard.as_ref().unwrap();
            let send_event = mailbox_chan.send(ClientEvent::Cmd(cmd.clone()));

            match send_event {
                Err(_) => None,
                Ok(_) => Some(()),
            }
        } else {
            None
        };

        // If there's a failure to send or the mailbox_chan
        // is not available, we need to immediately write an
        // error on the return channel
        if sent.is_none() {
            let chan = match cmd {
                ClientCmd::SendPulse(_, _, _, return_chan) => return_chan,
                ClientCmd::MailboxNext(_, return_chan) => return_chan,
                ClientCmd::MailOp(_, _, return_chan) => return_chan,
            };

            let _ = chan.send(ReturnChanResult::Err("mailbox write failed".to_string()));
        }
    }
}

// ------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use MoonlightPacket as P;
    use rand::{
        distr::{Alphanumeric, SampleString},
        prelude::IndexedRandom,
    };
    use std::{cmp::max, sync::mpsc::TryRecvError};
    use strum::IntoEnumIterator;

    fn gen_rand_str(length: usize) -> String {
        Alphanumeric
            .sample_string(&mut rand::rng(), length)
            .to_uppercase()
    }

    fn txn_id() -> u64 {
        rand::random()
    }

    fn pulse_id() -> u64 {
        rand::random()
    }

    fn gen_fleet_id() -> String {
        gen_rand_str(8)
    }

    fn gen_device_id() -> String {
        gen_rand_str(10)
    }

    fn gen_device_secret() -> String {
        "FOS-".to_string() + &gen_rand_str(32)
    }

    #[test]
    fn test_creds() {
        assert_eq!(
            Creds::new("", "", "", true).unwrap_err(),
            CredErr::FleetIDMissing
        );

        assert_eq!(
            Creds::new(gen_rand_str(7), "", "", true).unwrap_err(),
            CredErr::FleetIDInvalid
        );

        assert_eq!(
            Creds::new(gen_rand_str(8), "", "", true).unwrap_err(),
            CredErr::DeviceIDMissing
        );

        assert_eq!(
            Creds::new(gen_rand_str(8), gen_rand_str(9), "", true).unwrap_err(),
            CredErr::DeviceIDInvalid
        );

        assert_eq!(
            Creds::new(gen_rand_str(8), gen_rand_str(10), "", true).unwrap_err(),
            CredErr::DeviceSecretMissing
        );

        assert_eq!(
            Creds::new(gen_rand_str(8), gen_rand_str(10), gen_rand_str(33), true).unwrap_err(),
            CredErr::DeviceSecretInvalid
        );

        assert!(Creds::new(gen_rand_str(8), gen_rand_str(10), gen_device_secret(), true).is_ok());
    }

    #[test]
    fn test_creds_hashing() {
        let cred1 =
            Creds::new(gen_rand_str(8), gen_rand_str(10), gen_device_secret(), true).unwrap();

        let cred2 =
            Creds::new(gen_rand_str(8), gen_rand_str(10), gen_device_secret(), true).unwrap();

        assert_ne!(cred1.hash(), cred2.hash());
        assert_eq!(cred1.hash(), cred1.hash());
    }

    #[test]
    fn test_creds_errors() {
        assert!(CredErr::FleetIDMissing.to_string().contains("fostrom.io"));
        assert!(CredErr::DeviceIDMissing.to_string().contains("fostrom.io"));
        assert!(
            CredErr::DeviceSecretMissing
                .to_string()
                .contains("fostrom.io")
        );

        assert!(CredErr::FleetIDInvalid.to_string().contains("fostrom.io"));
        assert!(CredErr::DeviceIDInvalid.to_string().contains("fostrom.io"));
        assert!(
            CredErr::DeviceSecretInvalid
                .to_string()
                .contains("fostrom.io")
        );

        assert!(CredErr::FleetIDMissing.to_string().contains("missing"));
        assert!(CredErr::DeviceIDMissing.to_string().contains("missing"));
        assert!(CredErr::DeviceSecretMissing.to_string().contains("missing"));

        assert!(CredErr::FleetIDInvalid.to_string().contains("invalid"));
        assert!(CredErr::DeviceIDInvalid.to_string().contains("invalid"));
        assert!(CredErr::DeviceSecretInvalid.to_string().contains("invalid"));
    }

    fn cmp(packet: P, expected_bytes: &[u8]) {
        // This function tests multiple aspects of the Packet Codec

        // Deku's to_bytes() and compare bytes
        // let bytes = packet.to_bytes().unwrap();
        let bytes = Codec::encode(&packet).unwrap();
        assert_eq!(bytes, expected_bytes);

        // Deku's from_bytes() and compare packet
        let (decoded, len) = Codec::decode(&bytes).unwrap().unwrap();
        assert_eq!(len, bytes.len()); // All bytes consumed
        assert_eq!(packet, decoded);

        let mut codec = Codec::new();
        codec.feed(&bytes);
        let packets = codec.process_packets().unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0], packet);
    }

    #[test]
    fn test_close_connection_packet() {
        cmp(P::server_close_connection(), &[1, 1]);
        cmp(P::client_close_connection(), &[1, 0]);
    }

    #[test]
    fn test_connect() {
        let fleet_id = gen_fleet_id();
        let device_id = gen_device_id();
        let device_secret = gen_device_secret();

        let (fid, did, ds) = (fleet_id.clone(), device_id.clone(), device_secret.clone());

        let mut bytes: Vec<u8> = vec![
            2, // Packet Number
            1, // Flags: Keep Alive = True
            1, // Protocol Version
            2, // Serialization Format = Default JSON
        ];

        bytes.extend_from_slice(fleet_id.as_bytes());
        bytes.extend_from_slice(device_id.as_bytes());
        bytes.extend_from_slice(device_secret.as_bytes());

        let (packet, creds) = P::connect(fleet_id, device_id, device_secret, true).unwrap();

        assert_eq!(creds.fleet_id, fid);
        assert_eq!(creds.device_id, did);
        assert_eq!(creds.device_secret, ds);
        assert!(creds.prod);

        cmp(packet, &bytes);

        assert_eq!(
            P::connect(gen_rand_str(7), gen_device_id(), gen_device_secret(), true),
            Err(CredErr::FleetIDInvalid)
        );
    }

    #[test]
    fn test_connected() {
        cmp(P::connected(false, false), &[3, 0]);
        cmp(P::connected(false, true), &[3, 1]);
        cmp(P::connected(true, false), &[3, 2]);
        cmp(P::connected(true, true), &[3, 3]);
    }

    #[test]
    fn test_unauthorized() {
        use super::UnauthorizedError as UE;
        cmp(P::unauthorized(UE::Unknown), &[4, 0, 0]);
        cmp(P::unauthorized(UE::InvalidCredentials), &[4, 0, 1]);
        cmp(P::unauthorized(UE::FleetNotFound), &[4, 0, 2]);
        cmp(P::unauthorized(UE::DeviceNotFound), &[4, 0, 3]);
        cmp(P::unauthorized(UE::DeviceSecretIncorrect), &[4, 0, 4]);
        cmp(P::unauthorized(UE::DeviceDisabled), &[4, 0, 5]);
        cmp(P::unauthorized(UE::TemporaryBan), &[4, 0, 6]);
    }

    #[test]
    fn test_connect_failed() {
        use super::ConnectFailedError as CE;
        cmp(P::connect_failed(CE::Unknown), &[5, 0, 0]);
        cmp(P::connect_failed(CE::ServiceRestarting), &[5, 0, 1]);
        cmp(P::connect_failed(CE::ServiceUnavailable), &[5, 0, 2]);
        cmp(P::connect_failed(CE::ServiceDegraded), &[5, 0, 3]);
    }

    #[test]
    fn test_heartbeat_packet() {
        cmp(P::heartbeat(), &[8, 0]);
        cmp(P::heartbeat_ack(true), &[9, 1]);
        cmp(P::heartbeat_ack(false), &[9, 0]);
    }

    fn cmp_pulse() {
        let pulse_types: Vec<PulseType> = PulseType::iter().collect();
        let mut rng = rand::rng();
        let pulse_type = pulse_types.choose(&mut rng).unwrap().to_owned();
        let pulse_type_u8 = pulse_type as u8;

        let pulse_id = pulse_id();
        let name = gen_rand_str(rand::random_range(1..255));
        let payload = gen_rand_str(rand::random_range(1..500_000));
        let name_len = name.len() as u8;
        let payload_len = payload.len() as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[10, 0]);
        bytes.extend_from_slice(&pulse_type_u8.to_be_bytes());
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.extend_from_slice(&name_len.to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&payload_len.to_be_bytes());
        bytes.extend_from_slice(payload.as_bytes());
        let pulse = P::pulse(pulse_type, pulse_id, name, payload);
        cmp(pulse, &bytes)
    }

    #[test]
    #[should_panic]
    fn test_invalid_pulse_name() {
        P::pulse(
            PulseType::Unknown,
            1,
            gen_rand_str(300),
            "random_pl".to_string(),
        );
    }

    #[test]
    #[should_panic]
    fn test_invalid_mailbox_resp_name() {
        P::mailbox_next_resp_full(1, 1, 1, gen_rand_str(300), "random_pl".to_string());
    }

    #[test]
    fn test_pulse() {
        for _ in 0..10 {
            cmp_pulse();
        }
    }

    #[test]
    fn test_pulse_resp_success() {
        let pulse_id = pulse_id();
        let pulse = P::pulse_resp_success(pulse_id);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[11, 1]);
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        cmp(pulse, &bytes);
    }

    fn cmp_pulse_resp_error(reason: PulseErrorReason) {
        let pulse_id = pulse_id();
        let pulse = P::pulse_resp_error(pulse_id, reason);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[11, 0]);
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.extend_from_slice(&[reason as u8]);
        cmp(pulse, &bytes);
    }

    #[test]
    fn test_pulse_resp_error() {
        cmp_pulse_resp_error(PulseErrorReason::Unknown);
        cmp_pulse_resp_error(PulseErrorReason::DeserializationFailed);
        cmp_pulse_resp_error(PulseErrorReason::PacketSchemaNotFound);
        cmp_pulse_resp_error(PulseErrorReason::PacketSchemaTypeMismatch);
    }

    fn make_vec_with_txn_id(magic: u8, flag: u8, txn_id: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[magic, flag]);
        bytes.extend_from_slice(&txn_id.to_be_bytes());
        bytes
    }

    #[test]
    fn test_new_mail_event() {
        let mailbox_size: u16 = max(1, rand::random());
        let pulse_id = pulse_id();
        let mut bytes = vec![20, 0];
        bytes.extend_from_slice(&mailbox_size.to_be_bytes());
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        cmp(P::new_mail_event(mailbox_size, pulse_id), &bytes);
    }

    #[test]
    fn test_mailbox_next() {
        let txn_id = txn_id();
        let bytes = make_vec_with_txn_id(21, 0, txn_id);
        cmp(P::mailbox_next(false, txn_id), &bytes);

        let bytes = make_vec_with_txn_id(21, 1, txn_id);
        cmp(P::mailbox_next(true, txn_id), &bytes);
    }

    #[test]
    fn test_mailbox_next_resp_failed() {
        let txn_id = txn_id();
        let mut bytes = make_vec_with_txn_id(22, 0, txn_id);
        bytes.extend_from_slice(&[0, 0]);
        cmp(P::mailbox_next_resp_failed(txn_id), &bytes);
    }

    #[test]
    fn test_mailbox_next_resp_empty() {
        let txn_id = txn_id();
        let mut bytes = make_vec_with_txn_id(22, 1, txn_id);
        bytes.extend_from_slice(&[0, 0]);
        cmp(P::mailbox_next_resp_empty(txn_id), &bytes);
    }

    #[test]
    fn test_mailbox_next_resp_header_only() {
        let txn_id = txn_id();
        let pulse_id = pulse_id();
        let name = gen_rand_str(rand::random_range(1..10));
        let name_len = name.len() as u8;
        let mailbox_size: u16 = max(1, rand::random());
        let mut bytes = make_vec_with_txn_id(22, 3, txn_id);
        bytes.extend_from_slice(&mailbox_size.to_be_bytes());
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.extend_from_slice(&name_len.to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        let packet = P::mailbox_next_resp_header_only(txn_id, mailbox_size, pulse_id, name);
        cmp(packet, &bytes);
    }

    #[test]
    fn test_mailbox_next_resp_full() {
        let txn_id = txn_id();
        let pulse_id = pulse_id();
        let mailbox_size: u16 = max(1, rand::random());
        let name = gen_rand_str(rand::random_range(1..255));
        let payload = gen_rand_str(rand::random_range(1..500_000));
        let name_len = name.len() as u8;
        let payload_len = payload.len() as u32;
        let mut bytes = make_vec_with_txn_id(22, 1, txn_id);
        bytes.extend_from_slice(&mailbox_size.to_be_bytes());
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.extend_from_slice(&name_len.to_be_bytes());
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&payload_len.to_be_bytes());
        bytes.extend_from_slice(payload.as_bytes());
        let packet = P::mailbox_next_resp_full(txn_id, mailbox_size, pulse_id, name, payload);
        cmp(packet, &bytes);
    }

    fn cmp_ack_mail(ack_type: MailAckType) {
        let pulse_id = pulse_id();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[25, 0]);
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.push(ack_type as u8);
        cmp(P::ack_mail(pulse_id, ack_type), &bytes);
    }

    fn cmp_ack_mail_resp(successful: bool, ack_type: MailAckType) {
        let mailbox_size: u16 = if successful {
            rand::random_range(1..1000)
        } else {
            0
        };

        let pulse_id = pulse_id();
        let mut bytes = Vec::new();
        let flags_byte = if successful { 1 } else { 0 };
        bytes.extend_from_slice(&[26, flags_byte]);
        bytes.extend_from_slice(&mailbox_size.to_be_bytes());
        bytes.extend_from_slice(&pulse_id.to_be_bytes());
        bytes.push(ack_type as u8);

        if successful {
            cmp(P::ack_mail_resp(mailbox_size, pulse_id, ack_type), &bytes);
        } else {
            cmp(P::ack_mail_resp_failed(pulse_id, ack_type), &bytes);
        }
    }

    #[test]
    fn test_ack_mail() {
        use super::MailAckType as AT;
        cmp_ack_mail(AT::Ack);
        cmp_ack_mail(AT::Reject);
        cmp_ack_mail(AT::Requeue);
        cmp_ack_mail_resp(true, AT::Ack);
        cmp_ack_mail_resp(false, AT::Ack);
        cmp_ack_mail_resp(true, AT::Reject);
        cmp_ack_mail_resp(false, AT::Reject);
        cmp_ack_mail_resp(true, AT::Requeue);
        cmp_ack_mail_resp(false, AT::Requeue);
    }

    #[test]
    fn test_partial_message() {
        // let (mut client, server) = duplex(1024);
        // let mut framed = Framed::new(server, MoonlightCodec);

        let fleet_id = gen_fleet_id();
        let device_id = gen_device_id();
        let device_secret = gen_device_secret();

        // See the connect packet test for details on the first 4 bytes
        let mut bytes: Vec<u8> = vec![2, 1, 1, 2];
        bytes.extend_from_slice(fleet_id.as_bytes());
        bytes.extend_from_slice(device_id.as_bytes());
        bytes.extend_from_slice(device_secret.as_bytes());

        let (packet, _creds) = P::connect(fleet_id, device_id, device_secret, true).unwrap();
        cmp(packet.clone(), &bytes);

        let packet_bytes = packet.to_bytes().unwrap();

        let mut codec = Codec::new();
        codec.feed(&packet_bytes[..10]);

        // Ensure process_packets() succeeds but returns an empty vec
        let packets = codec.process_packets().unwrap();
        assert_eq!(packets.len(), 0);

        // Write the rest
        codec.feed(&packet_bytes[10..]);
        let packets = codec.process_packets().unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0], packet);
    }

    #[test]
    fn test_multiple_messages() {
        let (connect, _) =
            P::connect(gen_fleet_id(), gen_device_id(), gen_device_secret(), true).unwrap();
        let heartbeat = P::heartbeat();
        let pulse = P::pulse(
            PulseType::Unknown,
            pulse_id(),
            gen_rand_str(rand::random_range(1..255)),
            gen_rand_str(rand::random_range(1..500_000)),
        );

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&connect.to_bytes().unwrap());
        bytes.extend_from_slice(&heartbeat.to_bytes().unwrap());
        bytes.extend_from_slice(&pulse.to_bytes().unwrap());

        let mut codec = Codec::new();
        codec.feed(&bytes);
        let packets = codec.process_packets().unwrap();

        assert_eq!(packets[0], connect);
        assert_eq!(packets[1], heartbeat);
        assert_eq!(packets[2], pulse);
    }

    #[test]
    fn test_decode_packet_failed() {
        let e = Codec::decode(&[255, 0]).unwrap_err();
        assert!(e.to_string().contains("Failed to decode packet: "));

        let mut codec = Codec::new();
        codec.feed(&[255, 0]);
        let e = codec.process_packets().unwrap_err();
        assert!(e.to_string().contains("Failed to decode packet: "));
    }

    struct Client {
        notify_chan_rx: Receiver<(String, String)>,
        ping_chan_rx: Receiver<()>,
        transport_write_chan_rx: Receiver<Vec<u8>>,
        chan: Sender<ClientEvent>,
    }

    fn make_client_logic() -> (Client, ClientLogic) {
        let fleet_id = gen_fleet_id();
        let device_id = gen_device_id();
        let device_secret = gen_device_secret();
        let prod = rand::random_bool(0.5);

        let (notify_chan_tx, notify_chan_rx) = channel();
        let (ping_chan_tx, ping_chan_rx) = channel();
        let (transport_write_chan_tx, transport_write_chan_rx) = channel();

        let (chan, client_logic) = ClientLogic::new(
            fleet_id,
            device_id,
            device_secret,
            prod,
            notify_chan_tx,
            ping_chan_tx,
            transport_write_chan_tx,
        )
        .unwrap();

        (
            Client {
                notify_chan_rx,
                ping_chan_rx,
                transport_write_chan_rx,
                chan,
            },
            client_logic,
        )
    }

    #[test]
    fn test_client_logic_new() {
        let (client, mut logic) = make_client_logic();

        assert!(logic.pending_txns.is_empty());
        assert!(logic.pending_txns.capacity() >= 32);
        assert!(logic.next_txn_id == 0);
        assert!(logic.next_txn_id() == 0);
        assert!(logic.next_txn_id() == 1);
        assert!(logic.next_txn_id() == 2);
        assert!(logic.next_txn_id == 3);

        assert!(logic.codec.buffer.is_empty());

        let r: u64 = rand::random();
        let m = ClientLogic::to_pulse_id(r.to_string()).unwrap();
        assert_eq!(r, m);

        logic.write_packet_to_transport(P::heartbeat()).unwrap();
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        assert_eq!(bytes.len(), 2);
        assert_eq!(bytes, &[8, 0]);

        let (ret_tx, ret_rx) = channel();
        logic.pending_txns.insert(1, (Instant::now(), ret_tx));
        assert!(!logic.pending_txns.is_empty());
        logic.resolve_txn(1, R::Ok);
        assert!(logic.pending_txns.is_empty());
        let return_value = ret_rx.recv().unwrap();
        assert_eq!(return_value, R::Ok);
    }

    #[test]
    fn test_client_logic_basic_loop() {
        let (client, mut logic) = make_client_logic();

        client.chan.send(ClientEvent::TransportClose).unwrap();
        let s = Arc::new(AtomicBool::new(false));
        let r = logic.start_loop(s);
        assert_eq!(r, DisconnectedReason::ForceCloseSocket);
    }

    #[test]
    fn test_client_logic_refresh() {
        let (_client, mut logic) = make_client_logic();
        assert!(logic.pending_txns.is_empty());

        let now = Instant::now();
        let ago = now - Duration::from_secs(20);
        let (ret_tx, ret_rx) = channel();
        let (ret_tx_2, _ret_rx_2) = channel();
        logic.pending_txns.insert(1, (ago, ret_tx));
        logic.pending_txns.insert(2, (now, ret_tx_2));

        assert_eq!(logic.process_client_event(ClientEvent::Refresh), None);
        assert!(logic.pending_txns.len() == 1);

        let timeout = ret_rx.recv().unwrap();
        assert_eq!(timeout, R::Timeout);

        assert!(logic.pending_txns.contains_key(&2));
    }

    #[test]
    fn test_client_logic_heartbeat_tick() {
        let (client, mut logic) = make_client_logic();

        client.chan.send(ClientEvent::HeartbeatTick).unwrap();
        client.chan.send(ClientEvent::TransportClose).unwrap();

        let s = Arc::new(AtomicBool::new(false));
        let r = logic.start_loop(s);
        assert_eq!(r, DisconnectedReason::ForceCloseSocket);

        let b = client.transport_write_chan_rx.recv().unwrap();
        let (p, _) = Codec::decode(&b).unwrap().unwrap();
        assert_eq!(p, P::heartbeat());

        let heartbeat_ack = Codec::encode(&P::heartbeat_ack(true)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(heartbeat_ack);
        assert_eq!(logic.process_client_event(transport_recv), None);

        client.ping_chan_rx.recv().unwrap();
    }

    #[test]
    fn test_client_logic_cmd_send_pulse() {
        let (client, mut logic) = make_client_logic();
        let (ret_tx, ret_rx) = channel();

        let cmd_pulse = ClientCmd::SendPulse(
            PulseType::Data,
            "hello".to_string(),
            Some(json!({"world": true})),
            ret_tx,
        );

        assert_eq!(
            logic.process_client_event(ClientEvent::Cmd(cmd_pulse)),
            None
        );

        assert!(!logic.pending_txns.is_empty());
        assert!(logic.pending_txns.contains_key(&0));

        let b = client.transport_write_chan_rx.recv().unwrap();
        let (p, _) = Codec::decode(&b).unwrap().unwrap();
        assert!(matches!(p, P::Pulse { .. }));

        let pulse_resp = Codec::encode(&P::pulse_resp_success(0)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);

        assert!(logic.pending_txns.is_empty());
        assert!(!logic.pending_txns.contains_key(&0));
        assert_eq!(ret_rx.recv().unwrap(), R::Ok);

        let (ret_tx, ret_rx) = channel();

        let cmd_pulse = ClientCmd::SendPulse(
            PulseType::Data,
            "bad_packet".to_string(),
            Some(json!({"bad_world": true})),
            ret_tx,
        );

        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let b = client.transport_write_chan_rx.recv().unwrap();
        assert_eq!(b[0], 10);

        let err = PulseErrorReason::PacketSchemaTypeMismatch;
        let pulse_resp = Codec::encode(&P::pulse_resp_error(1, err)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);

        assert!(logic.pending_txns.is_empty());
        assert!(!logic.pending_txns.contains_key(&0));
        let return_value = ret_rx.recv().unwrap();

        assert!(
            matches!(return_value, ReturnChanResult::Err(str) if str.starts_with("packet_schema_type_mismatch"))
        );

        let (ret_tx, _ret_rx) = channel();
        let cmd_pulse = ClientCmd::SendPulse(PulseType::Msg, "empty_pl".to_string(), None, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let b = client.transport_write_chan_rx.recv().unwrap();
        assert_eq!(b[0], 10);
    }

    #[test]
    fn test_client_logic_cmd_mail_op() {
        let (client, mut logic) = make_client_logic();
        let (ret_tx, ret_rx) = channel();

        let cmd_pulse = ClientCmd::MailOp(MailAckType::Ack, 1, ret_tx);

        assert_eq!(
            logic.process_client_event(ClientEvent::Cmd(cmd_pulse)),
            None
        );

        assert!(!logic.pending_txns.is_empty());
        assert!(logic.pending_txns.contains_key(&1));

        let b = client.transport_write_chan_rx.recv().unwrap();
        let (p, _) = Codec::decode(&b).unwrap().unwrap();
        matches!(p, P::AckMail { .. });

        let pulse_resp = Codec::encode(&P::ack_mail_resp(0, 1, MailAckType::Ack)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);

        assert!(logic.pending_txns.is_empty());
        assert!(!logic.pending_txns.contains_key(&1));
        assert_eq!(ret_rx.recv().unwrap(), R::MailAckSuccessful(false));

        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailOp(MailAckType::Ack, 2, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::ack_mail_resp(1, 2, MailAckType::Ack)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        assert_eq!(ret_rx.recv().unwrap(), R::MailAckSuccessful(true));

        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailOp(MailAckType::Ack, 3, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::ack_mail_resp_failed(3, MailAckType::Ack)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        let return_value = ret_rx.recv().unwrap();

        assert!(matches!(
            return_value,
            ReturnChanResult::Err(str) if str.starts_with("mail_ack_failed") && str.contains("Failed to acknowledge the mail with ID 3"),
        ));

        let (ret_tx, _ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailOp(MailAckType::Ack, 100, ret_tx);

        let (ret_tx_2, ret_rx_2) = channel();
        let cmd_pulse_2 = ClientCmd::MailOp(MailAckType::Ack, 100, ret_tx_2);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse_2));

        let return_value = ret_rx_2.recv().unwrap();

        assert!(
            matches!(return_value, ReturnChanResult::Err(str) if str.starts_with("duplicate_request") && str.contains("already been queued"))
        );
    }

    #[test]
    fn test_client_logic_cmd_mailbox_next() {
        let (client, mut logic) = make_client_logic();

        // Test mailbox resp empty
        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailboxNext(true, ret_tx);

        assert_eq!(
            logic.process_client_event(ClientEvent::Cmd(cmd_pulse)),
            None
        );

        assert!(!logic.pending_txns.is_empty());
        assert!(logic.pending_txns.contains_key(&0));
        let b = client.transport_write_chan_rx.recv().unwrap();
        let (p, _) = Codec::decode(&b).unwrap().unwrap();
        matches!(p, P::MailboxNext { .. });
        let pulse_resp = Codec::encode(&P::mailbox_next_resp_empty(0)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        assert!(logic.pending_txns.is_empty());
        assert!(!logic.pending_txns.contains_key(&0));
        assert_eq!(ret_rx.recv().unwrap(), R::Mail(None));

        // Test mailbox resp failed
        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailboxNext(true, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::mailbox_next_resp_failed(1)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        let result = ret_rx.recv().unwrap();

        assert!(
            matches!(result, ReturnChanResult::Err(str) if str.starts_with("failed: ") && str.contains("Failed to fetch next mail"))
        );

        // Test mailbox resp header-only
        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailboxNext(true, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::mailbox_next_resp_header_only(
            2,
            3,
            500,
            "hello".to_string(),
        ))
        .unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        let result = ret_rx.recv().unwrap();

        assert!(
            matches!(result, ReturnChanResult::Mail(Some(mail)) if mail.mailbox_size == 3 && mail.pulse_id == 500 && mail.name == "hello")
        );

        // Test mailbox resp full

        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailboxNext(false, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::mailbox_next_resp_full(
            3,
            3,
            500,
            "hello".to_string(),
            "{\"world\": true}".to_string(),
        ))
        .unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        let result = ret_rx.recv().unwrap();

        let mail = match result {
            ReturnChanResult::Mail(Some(mail)) => Some(mail),
            _ => None,
        }
        .unwrap();

        assert_eq!(mail.mailbox_size, 3);
        assert_eq!(mail.pulse_id, 500);
        assert_eq!(mail.name, "hello".to_string());
        assert_eq!(mail.payload.unwrap()["world"], true);

        // Test mailbox resp full with empty payload
        let (ret_tx, ret_rx) = channel();
        let cmd_pulse = ClientCmd::MailboxNext(false, ret_tx);
        logic.process_client_event(ClientEvent::Cmd(cmd_pulse));
        let pulse_resp = Codec::encode(&P::mailbox_next_resp_full(
            4,
            3,
            500,
            "hello".to_string(),
            "".to_string(),
        ))
        .unwrap();
        let transport_recv = ClientEvent::TransportRecv(pulse_resp);
        assert_eq!(logic.process_client_event(transport_recv), None);
        let result = ret_rx.recv().unwrap();

        assert!(
            matches!(result, ReturnChanResult::Mail(Some(mail)) if mail.mailbox_size == 3 && mail.pulse_id == 500 && mail.name == "hello" && mail.payload.is_none())
        );
    }

    #[test]
    fn test_client_logic_new_mail_available() {
        let (client, mut logic) = make_client_logic();
        let bytes = Codec::encode(&P::new_mail_event(5, 1500)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        assert_eq!(logic.process_client_event(transport_recv), None);

        let notification = client.notify_chan_rx.recv().unwrap();
        assert_eq!(notification, ("new_mail".to_string(), "".to_string()));
    }

    #[test]
    #[should_panic]
    fn test_client_logic_connected_unreachable() {
        let (_client, mut logic) = make_client_logic();
        let bytes = Codec::encode(&P::connected(true, true)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        logic.process_client_event(transport_recv);
    }

    #[test]
    fn test_client_logic_loop_close() {
        let (client, mut logic) = make_client_logic();
        drop(client.chan);
        let s = Arc::new(AtomicBool::new(false));
        let r = logic.start_loop(s);
        assert_eq!(r, DisconnectedReason::ForceCloseSocket);
    }

    #[test]
    fn test_client_logic_close_on_incorrect_packet() {
        let (_client, mut logic) = make_client_logic();
        let bytes = Codec::encode(&P::heartbeat()).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        assert_eq!(
            logic.process_client_event(transport_recv).unwrap(),
            DisconnectedReason::ForceCloseSocket
        );
    }

    #[test]
    fn test_client_logic_disconnect_errors() {
        let (_client, mut logic) = make_client_logic();

        let bytes = Codec::encode(&P::server_close_connection()).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        assert_eq!(
            logic.process_client_event(transport_recv).unwrap(),
            DisconnectedReason::NormalDisconnect
        );

        let bytes = Codec::encode(&P::connect_failed(ConnectFailedError::ServiceDegraded)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        assert_eq!(
            logic.process_client_event(transport_recv).unwrap(),
            DisconnectedReason::ConnectFailed(ConnectFailedError::ServiceDegraded)
        );

        let bytes = Codec::encode(&P::unauthorized(UnauthorizedError::DeviceNotFound)).unwrap();
        let transport_recv = ClientEvent::TransportRecv(bytes);
        assert_eq!(
            logic.process_client_event(transport_recv).unwrap(),
            DisconnectedReason::Unauthorized(UnauthorizedError::DeviceNotFound)
        );
    }

    #[test]
    fn test_client_logic_authentication() {
        let (client, mut logic) = make_client_logic();
        let p = Codec::encode(&P::connected(true, true)).unwrap();
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();
        logic.wait_for_authentication().unwrap();
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_with_extra_packets() {
        let (client, mut logic) = make_client_logic();
        let mut p = Codec::encode(&P::connected(true, false)).unwrap();
        let mut p2 = Codec::encode(&P::heartbeat_ack(true)).unwrap();
        p.append(&mut p2);
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();

        logic.wait_for_authentication().unwrap();
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        client.ping_chan_rx.recv().unwrap();
        assert!(logic.authenticated.load(Ordering::SeqCst));
        assert_eq!(
            client.notify_chan_rx.recv().unwrap(),
            ("connected".to_string(), "".to_string())
        );
        assert_eq!(
            client.notify_chan_rx.recv().unwrap(),
            ("new_mail".to_string(), "".to_string())
        );
    }

    #[test]
    fn test_client_logic_authentication_with_extra_packets_and_stream_closure() {
        let (client, mut logic) = make_client_logic();
        let mut p = Codec::encode(&P::connected(false, true)).unwrap();
        let mut p2 = Codec::encode(&P::heartbeat_ack(true)).unwrap();
        let mut p3 = Codec::encode(&P::server_close_connection()).unwrap();
        p.append(&mut p2);
        p.append(&mut p3);
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::NormalDisconnect
        );

        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        client.ping_chan_rx.recv().unwrap();
        assert!(logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_connect_failed() {
        let (client, mut logic) = make_client_logic();
        let p = Codec::encode(&P::connect_failed(ConnectFailedError::ServiceUnavailable)).unwrap();
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ConnectFailed(ConnectFailedError::ServiceUnavailable)
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_unauthorized() {
        let (client, mut logic) = make_client_logic();
        let p = Codec::encode(&P::unauthorized(UnauthorizedError::DeviceSecretIncorrect)).unwrap();
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::Unauthorized(UnauthorizedError::DeviceSecretIncorrect)
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_failed_invalid_packet() {
        let (client, mut logic) = make_client_logic();
        let p = Codec::encode(&P::heartbeat_ack(true)).unwrap();
        client.chan.send(ClientEvent::TransportRecv(p)).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ForceCloseSocket
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_failed_invalid_client_event() {
        let (client, mut logic) = make_client_logic();
        client.chan.send(ClientEvent::TransportClose).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ForceCloseSocket
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_failed_invalid_client_event_2() {
        let (client, mut logic) = make_client_logic();
        client.chan.send(ClientEvent::Refresh).unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ForceCloseSocket
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_close_chan() {
        let (client, mut logic) = make_client_logic();
        drop(client.chan);

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ForceCloseSocket
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_close_transport_write_chan() {
        let (client, mut logic) = make_client_logic();
        drop(client.transport_write_chan_rx);

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::ForceCloseSocket
        );

        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_codec_can_handle_partial_packets_correctly() {
        let mut codec = Codec::new();
        codec.feed(&[3]);
        let packets = codec.process_packets().unwrap();
        assert!(packets.is_empty());
        codec.feed(&[3]);
        let packets = codec.process_packets().unwrap();
        assert!(!packets.is_empty());
        let target = P::connected(true, true);
        assert_eq!(*packets.first().unwrap(), target);
    }

    #[test]
    fn test_client_logic_authentication_partial_multiple_packets() {
        let (client, mut logic) = make_client_logic();
        client
            .chan
            .send(ClientEvent::TransportRecv(vec![3]))
            .unwrap();

        client
            .chan
            .send(ClientEvent::TransportRecv(vec![3, 9, 0]))
            .unwrap();

        logic.wait_for_authentication().unwrap();
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        client.ping_chan_rx.recv().unwrap();
        assert!(logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_client_logic_authentication_fail_partial_multiple_packets() {
        let (client, mut logic) = make_client_logic();
        client
            .chan
            .send(ClientEvent::TransportRecv(vec![4, 0]))
            .unwrap();

        client
            .chan
            .send(ClientEvent::TransportRecv(vec![3, 9, 0]))
            .unwrap();

        assert_eq!(
            logic.wait_for_authentication().unwrap_err(),
            DisconnectedReason::Unauthorized(UnauthorizedError::DeviceNotFound)
        );
        let bytes = client.transport_write_chan_rx.recv().unwrap();
        let (connect_packet, _) = Codec::decode(&bytes).unwrap().unwrap();
        assert!(matches!(connect_packet, MoonlightPacket::Connect { .. }));
        assert!(!logic.authenticated.load(Ordering::SeqCst));
    }

    #[test]
    fn test_moonlight_client_basics() {
        let m = MoonlightClient::new(
            gen_fleet_id(),
            gen_device_id(),
            gen_device_secret(),
            ConnectMode::Local(8484),
        );

        assert!(!m.shutdown_flag.load(Ordering::SeqCst));
        assert_eq!(*m.disconnected_reason.lock().unwrap(), None);
        assert_eq!(*m.reconnect_in.lock().unwrap(), None);
        assert!(m.mailbox_chan.lock().unwrap().is_none());

        assert!(m.status()["connected"] == false);

        m.stop();
        assert!(m.shutdown_flag.load(Ordering::SeqCst));
        assert!(m.status()["connected"] == false);

        *m.disconnected_reason.lock().unwrap() = Some(DisconnectedReason::ForceCloseSocket);
        assert!(m.status()["connected"] == false);
        assert!(m.status()["error"] == "connect_failed");
        assert!(
            m.status()["msg"]
                .to_string()
                .to_lowercase()
                .contains("unknown")
        );

        *m.disconnected_reason.lock().unwrap() = Some(DisconnectedReason::ConnectFailed(
            ConnectFailedError::ServiceDegraded,
        ));
        assert!(m.status()["connected"] == false);
        assert!(m.status()["error"] == "connect_failed");
        assert!(
            m.status()["msg"]
                .to_string()
                .to_lowercase()
                .contains("degraded")
        );

        assert_eq!(m.status()["reconnecting_in"], 0);

        *m.reconnect_in.lock().unwrap() = Some(Duration::from_secs(30));
        assert_eq!(m.status()["reconnecting_in"], 30_000);
        *m.reconnect_in.lock().unwrap() = None;

        *m.disconnected_reason.lock().unwrap() = Some(DisconnectedReason::Unauthorized(
            UnauthorizedError::TemporaryBan,
        ));
        assert!(m.status()["connected"] == false);
        assert!(m.status()["error"] == "unauthorized");
        assert!(
            m.status()["msg"]
                .to_string()
                .to_lowercase()
                .contains("banned")
        );

        *m.disconnected_reason.lock().unwrap() = None;
        m.authenticated.store(true, Ordering::SeqCst);
        assert!(m.status()["connected"] == true);
    }

    #[test]
    fn test_moonlight_client_send_cmd_failures() {
        let m = MoonlightClient::new(
            gen_fleet_id(),
            gen_device_id(),
            gen_device_secret(),
            ConnectMode::Local(8484),
        );

        let e = ReturnChanResult::Err("mailbox write failed".to_string());

        let (ret_tx, ret_rx) = channel();
        m.send_cmd(ClientCmd::SendPulse(
            PulseType::Data,
            "name".to_string(),
            Some(json!(null)),
            ret_tx,
        ));
        assert_eq!(ret_rx.recv().unwrap(), e);

        let (ret_tx, ret_rx) = channel();
        m.send_cmd(ClientCmd::MailboxNext(true, ret_tx));
        assert_eq!(ret_rx.recv().unwrap(), e);

        let (ret_tx, ret_rx) = channel();
        m.send_cmd(ClientCmd::MailOp(MailAckType::Ack, 1, ret_tx));
        assert_eq!(ret_rx.recv().unwrap(), e);
    }

    #[test]
    fn test_moonlight_client_backoff() {
        let mut m = MoonlightClient::new(
            gen_fleet_id(),
            gen_device_id(),
            gen_device_secret(),
            ConnectMode::Local(8484),
        );

        assert_eq!(m.backoff(false), Duration::from_millis(0));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(1))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(1));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_millis(2500))
        );

        assert_eq!(m.backoff(false), Duration::from_millis(2500));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(5))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(5));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(10))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(10));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(15))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(15));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(30))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(30));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(30))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(30));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(30))
        );

        assert_eq!(m.backoff(false), Duration::from_secs(30));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(30))
        );

        assert_eq!(m.backoff(true), Duration::from_secs(5 * 60));
        assert_eq!(
            *m.reconnect_in.lock().unwrap(),
            Some(Duration::from_secs(5 * 60))
        );
    }

    #[test]
    fn test_timer_proc() {
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_for_timer = shutdown_flag.clone();
        let (mailbox_tx, mailbox_rx) = channel();
        let (_ping_tx, ping_rx) = channel();

        let handle = std::thread::spawn(move || {
            MoonlightClient::timer_proc(shutdown_flag_for_timer, mailbox_tx, ping_rx);
        });

        assert!(matches!(mailbox_rx.recv().unwrap(), ClientEvent::Refresh));

        // Shutdown the task
        sleep(Duration::from_millis(100));
        shutdown_flag.store(true, Ordering::SeqCst);
        handle.join().unwrap();
    }

    fn call_timer_logic(
        last_refresh_sent: Option<Instant>,
        last_heartbeat_sent: Option<Instant>,
        last_heartbeat_ack: Option<Instant>,
    ) -> Receiver<ClientEvent> {
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let (mailbox_tx, mailbox_rx) = channel();
        let (_ping_tx, ping_rx) = channel();

        let mut last_refresh_sent = last_refresh_sent.unwrap_or(Instant::now());
        let mut last_heartbeat_sent = last_heartbeat_sent.unwrap_or(Instant::now());
        let mut last_heartbeat_ack = last_heartbeat_ack.unwrap_or(Instant::now());

        MoonlightClient::timer_logic(
            &shutdown_flag,
            &mailbox_tx,
            &ping_rx,
            &mut last_refresh_sent,
            &mut last_heartbeat_sent,
            &mut last_heartbeat_ack,
        );

        mailbox_rx
    }

    #[test]
    fn test_timer_logic_refresh() {
        let mb = call_timer_logic(
            Some(Instant::now() - Duration::from_millis(600)),
            None,
            None,
        );
        assert!(matches!(mb.try_recv().unwrap(), ClientEvent::Refresh));
    }

    #[test]
    fn test_timer_logic_heartbeat_miss() {
        let mb = call_timer_logic(
            None,
            Some(Instant::now() - Duration::from_secs(6)),
            Some(Instant::now() - Duration::from_secs(10)),
        );
        assert!(matches!(mb.try_recv().unwrap(), ClientEvent::HeartbeatTick));
    }

    #[test]
    fn test_timer_logic_heartbeat_miss_and_close() {
        let mb = call_timer_logic(None, None, Some(Instant::now() - Duration::from_secs(95)));
        assert!(matches!(
            mb.try_recv().unwrap(),
            ClientEvent::TransportClose
        ));
    }

    #[test]
    fn test_timer_logic_heartbeat_tick() {
        let mb = call_timer_logic(None, Some(Instant::now() - Duration::from_secs(31)), None);
        assert!(matches!(mb.try_recv().unwrap(), ClientEvent::HeartbeatTick));
    }

    #[test]
    fn test_timer_logic_heartbeat_ack_and_tick() {
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let (mailbox_tx, mailbox_rx) = channel();
        let (ping_tx, ping_rx) = channel();

        ping_tx.send(()).unwrap();

        let mut last_refresh_sent = Instant::now();
        let mut last_heartbeat_sent = Instant::now();
        let mut last_heartbeat_ack = Instant::now();

        MoonlightClient::timer_logic(
            &shutdown_flag,
            &mailbox_tx,
            &ping_rx,
            &mut last_refresh_sent,
            &mut last_heartbeat_sent,
            &mut last_heartbeat_ack,
        );

        assert!(matches!(
            mailbox_rx.try_recv().err().unwrap(),
            TryRecvError::Empty
        ));
    }
}
