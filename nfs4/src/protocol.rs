use serde::{Deserialize, Serialize};

// NFSv4 constants
pub const NFS_VERSION: u32 = 4;
pub const NFS_PROGRAM: u32 = 100003;

// NFSv4 procedures
#[derive(Debug, Clone, Copy)]
pub enum NfsProcedure {
    Null = 0,
    Compound = 1,
}

// File types
pub const NF4REG: u32 = 1; // Regular file
pub const NF4DIR: u32 = 2; // Directory
pub const NF4BLK: u32 = 3; // Block device
pub const NF4CHR: u32 = 4; // Character device
pub const NF4LNK: u32 = 5; // Symbolic link
pub const NF4SOCK: u32 = 6; // Socket
pub const NF4FIFO: u32 = 7; // Named pipe
pub const NF4ATTRDIR: u32 = 8; // Attribute directory
pub const NF4NAMEDATTR: u32 = 9; // Named attribute

// Access rights
pub const ACCESS4_READ: u32 = 0x00000001;
pub const ACCESS4_LOOKUP: u32 = 0x00000002;
pub const ACCESS4_MODIFY: u32 = 0x00000004;
pub const ACCESS4_EXTEND: u32 = 0x00000008;
pub const ACCESS4_DELETE: u32 = 0x00000010;
pub const ACCESS4_EXECUTE: u32 = 0x00000020;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NfsFileHandle {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NfsTime {
    pub seconds: u64,
    pub nseconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NfsFileAttributes {
    pub type_: u32,
    pub mode: u32,
    pub size: u64,
    pub space_used: u64,
    pub time_access: NfsTime,
    pub time_modify: NfsTime,
    pub owner: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NfsOperation {
    Access(AccessOperation),
    Close(CloseOperation),
    Commit(CommitOperation),
    Create(CreateOperation),
    GetAttr(GetAttrOperation),
    GetFh(GetFhOperation),
    Lookup(LookupOperation),
    Lookupp(LookuppOperation),
    Open(OpenOperation),
    OpenConfirm(OpenConfirmOperation),
    Read(ReadOperation),
    Write(WriteOperation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessOperation {
    pub access: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseOperation {
    pub seqid: u32,
    pub open_stateid: [u8; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitOperation {
    pub offset: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOperation {
    pub object_type: u32,
    pub object_name: String,
    pub attributes: NfsFileAttributes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAttrOperation {
    pub attr_request: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFhOperation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupOperation {
    pub object_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookuppOperation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenOperation {
    pub seqid: u32,
    pub share_access: u32,
    pub share_deny: u32,
    pub owner: Vec<u8>,
    pub open_claim: OpenClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpenClaim {
    Null(String),
    Previous(String),
    Delegate(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenConfirmOperation {
    pub open_stateid: [u8; 16],
    pub seqid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadOperation {
    pub stateid: [u8; 16],
    pub offset: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOperation {
    pub stateid: [u8; 16],
    pub offset: u64,
    pub stable: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundRequest {
    pub tag: String,
    pub minor_version: u32,
    pub operations: Vec<NfsOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundResponse {
    pub tag: String,
    pub status: NfsStatus,
    pub results: Vec<OperationResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NfsStatus {
    Ok = 0,
    Error = 1,
    BadHandle = 10001,
    BadType = 10002,
    NoEnt = 10003,
    IoError = 10004,
    NoSpace = 10005,
    BadName = 10006,
    RoFs = 10007,
    StaleFileHandle = 10008,
    BadStateid = 10009,
    BadSeqid = 10010,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub status: NfsStatus,
    pub result: Option<OperationData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationData {
    Access(u32),
    GetAttr(NfsFileAttributes),
    GetFh(NfsFileHandle),
    Read(Vec<u8>),
    Write(u32),
    Open([u8; 16]), // stateid
}
