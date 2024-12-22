use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};

// RPC message types
pub const RPC_CALL: u32 = 0;
pub const RPC_REPLY: u32 = 1;

// RPC versions
pub const RPC_VERSION: u32 = 2;

// Reply status
pub const MSG_ACCEPTED: u32 = 0;
pub const MSG_DENIED: u32 = 1;

// Accept status
pub const SUCCESS: u32 = 0;
pub const PROG_UNAVAIL: u32 = 1;
pub const PROG_MISMATCH: u32 = 2;
pub const PROC_UNAVAIL: u32 = 3;
pub const GARBAGE_ARGS: u32 = 4;

// Auth flavors
pub const AUTH_NONE: u32 = 0;
pub const AUTH_SYS: u32 = 1;
pub const AUTH_SHORT: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcMsg {
    pub xid: u32,
    pub body: RpcMsgBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcMsgBody {
    Call(CallBody),
    Reply(ReplyBody),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallBody {
    pub rpc_vers: u32,
    pub prog: u32,
    pub prog_vers: u32,
    pub proc: u32,
    pub cred: OpaqueAuth,
    pub verf: OpaqueAuth,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyBody {
    pub reply_stat: u32,
    pub data: ReplyData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReplyData {
    Accepted(AcceptedReply),
    Rejected(RejectedReply),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedReply {
    pub verf: OpaqueAuth,
    pub stat: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedReply {
    pub stat: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpaqueAuth {
    pub flavor: u32,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSys {
    pub stamp: u32,
    pub machinename: String,
    pub uid: u32,
    pub gid: u32,
    pub gids: Vec<u32>,
}

impl RpcMsg {
    pub fn new_call(xid: u32, prog: u32, prog_vers: u32, proc: u32, data: Vec<u8>) -> Self {
        RpcMsg {
            xid,
            body: RpcMsgBody::Call(CallBody {
                rpc_vers: RPC_VERSION,
                prog,
                prog_vers,
                proc,
                cred: OpaqueAuth {
                    flavor: AUTH_NONE,
                    body: vec![],
                },
                verf: OpaqueAuth {
                    flavor: AUTH_NONE,
                    body: vec![],
                },
                data,
            }),
        }
    }

    pub fn new_success_reply(xid: u32, data: Vec<u8>) -> Self {
        RpcMsg {
            xid,
            body: RpcMsgBody::Reply(ReplyBody {
                reply_stat: MSG_ACCEPTED,
                data: ReplyData::Accepted(AcceptedReply {
                    verf: OpaqueAuth {
                        flavor: AUTH_NONE,
                        body: vec![],
                    },
                    stat: SUCCESS,
                    data,
                }),
            }),
        }
    }

    pub fn new_prog_mismatch_reply(xid: u32) -> Self {
        RpcMsg {
            xid,
            body: RpcMsgBody::Reply(ReplyBody {
                reply_stat: MSG_ACCEPTED,
                data: ReplyData::Accepted(AcceptedReply {
                    verf: OpaqueAuth {
                        flavor: AUTH_NONE,
                        body: vec![],
                    },
                    stat: PROG_MISMATCH,
                    data: vec![],
                }),
            }),
        }
    }

    pub fn new_garbage_args_reply(xid: u32) -> Self {
        RpcMsg {
            xid,
            body: RpcMsgBody::Reply(ReplyBody {
                reply_stat: MSG_ACCEPTED,
                data: ReplyData::Accepted(AcceptedReply {
                    verf: OpaqueAuth {
                        flavor: AUTH_NONE,
                        body: vec![],
                    },
                    stat: GARBAGE_ARGS,
                    data: vec![],
                }),
            }),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        serde_xdr::to_writer(&mut buf, self)?;
        Ok(buf)
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        Ok(serde_xdr::from_bytes(buf)?)
    }
}

// Helper function to read a complete RPC message from a buffer
pub fn read_rpc_message(buf: &mut BytesMut) -> Option<Result<RpcMsg>> {
    if buf.len() < 4 {
        return None;
    }

    let size = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < size + 4 {
        return None;
    }

    buf.advance(4);
    let msg_buf = buf.split_to(size);
    Some(RpcMsg::decode(&msg_buf))
}

// Helper function to write an RPC message to a buffer
pub fn write_rpc_message(msg: &RpcMsg, buf: &mut BytesMut) -> Result<()> {
    let encoded = msg.encode()?;
    let len = (encoded.len() as u32).to_be_bytes();
    buf.put_slice(&len);
    buf.put_slice(&encoded);
    Ok(())
}
