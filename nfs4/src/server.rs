use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use std::os::unix::fs::MetadataExt;
use nix::unistd::{Uid, Gid};

use crate::protocol::*;

#[derive(Clone)]
pub struct NfsServer {
    export_root: PathBuf,
    handles: Arc<RwLock<HashMap<Vec<u8>, PathBuf>>>,
    stateids: Arc<RwLock<HashMap<[u8; 16], FileState>>>,
}

#[derive(Debug)]
struct FileState {
    path: PathBuf,
    open_mode: u32,
    seqid: u32,
    file: Option<File>,
}

impl NfsServer {
    pub fn new(export_root: PathBuf) -> Self {
        Self {
            export_root,
            handles: Arc::new(RwLock::new(HashMap::new())),
            stateids: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_compound(&self, request: CompoundRequest) -> Result<CompoundResponse> {
        let mut results = Vec::new();
        let mut current_status = NfsStatus::Ok;
        let mut current_fh: Option<NfsFileHandle> = None;

        for operation in request.operations {
            if current_status != NfsStatus::Ok {
                break;
            }

            let result = match operation {
                NfsOperation::Access(args) => self.handle_access(args, &current_fh).await,
                NfsOperation::Close(args) => self.handle_close(args).await,
                NfsOperation::Commit(args) => self.handle_commit(args, &current_fh).await,
                NfsOperation::Create(args) => self.handle_create(args, &current_fh).await,
                NfsOperation::GetAttr(args) => self.handle_getattr(args, &current_fh).await,
                NfsOperation::GetFh(args) => {
                    let res = self.handle_getfh(args).await?;
                    if let Some(OperationData::GetFh(ref fh)) = res.result {
                        current_fh = Some(fh.clone());
                    }
                    Ok(res)
                }
                NfsOperation::Lookup(args) => {
                    let res = self.handle_lookup(args, &current_fh).await?;
                    if res.status == NfsStatus::Ok {
                        // Update current filehandle after successful lookup
                        if let Some(OperationData::GetFh(ref fh)) = res.result {
                            current_fh = Some(fh.clone());
                        }
                    }
                    Ok(res)
                }
                NfsOperation::Open(args) => self.handle_open(args, &current_fh).await,
                NfsOperation::Read(args) => self.handle_read(args).await,
                NfsOperation::Write(args) => self.handle_write(args).await,
                _ => Ok(OperationResult {
                    status: NfsStatus::Error,
                    result: None,
                }),
            }?;

            current_status = result.status;
            results.push(result);
        }

        Ok(CompoundResponse {
            tag: request.tag,
            status: current_status,
            results,
        })
    }

    async fn handle_access(&self, args: AccessOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        if let Some(fh) = current_fh {
            let handles = self.handles.read().await;
            if let Some(path) = handles.get(&fh.data) {
                if let Ok(metadata) = fs::metadata(path).await {
                    let uid = Uid::current().as_raw();
                    let gid = Gid::current().as_raw();

                    let mode = metadata.mode();
                    let file_uid = metadata.uid();
                    let file_gid = metadata.gid();

                    let mut allowed_access = 0u32;

                    // Owner
                    if uid == file_uid {
                        if mode & 0o400 != 0 { allowed_access |= ACCESS4_READ; }
                        if mode & 0o200 != 0 { allowed_access |= ACCESS4_MODIFY | ACCESS4_EXTEND; }
                        if mode & 0o100 != 0 { allowed_access |= ACCESS4_EXECUTE; }
                    }
                    // Group
                    else if gid == file_gid {
                        if mode & 0o040 != 0 { allowed_access |= ACCESS4_READ; }
                        if mode & 0o020 != 0 { allowed_access |= ACCESS4_MODIFY | ACCESS4_EXTEND; }
                        if mode & 0o010 != 0 { allowed_access |= ACCESS4_EXECUTE; }
                    }
                    // Others
                    else {
                        if mode & 0o004 != 0 { allowed_access |= ACCESS4_READ; }
                        if mode & 0o002 != 0 { allowed_access |= ACCESS4_MODIFY | ACCESS4_EXTEND; }
                        if mode & 0o001 != 0 { allowed_access |= ACCESS4_EXECUTE; }
                    }

                    Ok(OperationResult {
                        status: NfsStatus::Ok,
                        result: Some(OperationData::Access(allowed_access & args.access)),
                    })
                } else {
                    Ok(OperationResult {
                        status: NfsStatus::NoEnt,
                        result: None,
                    })
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::StaleFileHandle,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadHandle,
                result: None,
            })
        }
    }

    async fn handle_close(&self, args: CloseOperation) -> Result<OperationResult> {
        let mut stateids = self.stateids.write().await;
        if let Some(state) = stateids.remove(&args.open_stateid) {
            Ok(OperationResult {
                status: NfsStatus::Ok,
                result: None,
            })
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadStateid,
                result: None,
            })
        }
    }

    async fn handle_commit(&self, args: CommitOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        if let Some(fh) = current_fh {
            let handles = self.handles.read().await;
            if let Some(path) = handles.get(&fh.data) {
                if let Ok(mut file) = File::open(path).await {
                    file.sync_all().await?;
                    Ok(OperationResult {
                        status: NfsStatus::Ok,
                        result: None,
                    })
                } else {
                    Ok(OperationResult {
                        status: NfsStatus::IoError,
                        result: None,
                    })
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::StaleFileHandle,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadHandle,
                result: None,
            })
        }
    }

    async fn handle_create(&self, args: CreateOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        if let Some(fh) = current_fh {
            let handles = self.handles.read().await;
            if let Some(parent_path) = handles.get(&fh.data) {
                let new_path = parent_path.join(&args.object_name);

                match args.object_type {
                    NF4REG => {
                        if let Ok(file) = File::create(&new_path).await {
                            // Generate new file handle
                            let mut handle_data = vec![0u8; 16];
                            rand::thread_rng().fill(&mut handle_data[..]);

                            let mut handles = self.handles.write().await;
                            handles.insert(handle_data.clone(), new_path);

                            Ok(OperationResult {
                                status: NfsStatus::Ok,
                                result: Some(OperationData::GetFh(NfsFileHandle { data: handle_data })),
                            })
                        } else {
                            Ok(OperationResult {
                                status: NfsStatus::IoError,
                                result: None,
                            })
                        }
                    },
                    NF4DIR => {
                        if let Ok(_) = fs::create_dir(&new_path).await {
                            let mut handle_data = vec![0u8; 16];
                            rand::thread_rng().fill(&mut handle_data[..]);

                            let mut handles = self.handles.write().await;
                            handles.insert(handle_data.clone(), new_path);

                            Ok(OperationResult {
                                status: NfsStatus::Ok,
                                result: Some(OperationData::GetFh(NfsFileHandle { data: handle_data })),
                            })
                        } else {
                            Ok(OperationResult {
                                status: NfsStatus::IoError,
                                result: None,
                            })
                        }
                    },
                    _ => Ok(OperationResult {
                        status: NfsStatus::BadType,
                        result: None,
                    }),
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::StaleFileHandle,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadHandle,
                result: None,
            })
        }
    }

    async fn handle_getattr(&self, args: GetAttrOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        if let Some(fh) = current_fh {
            let handles = self.handles.read().await;
            if let Some(path) = handles.get(&fh.data) {
                if let Ok(metadata) = fs::metadata(path).await {
                    let attrs = NfsFileAttributes {
                        type_: if metadata.is_dir() { NF4DIR } else { NF4REG },
                        mode: metadata.mode(),
                        size: metadata.len(),
                        space_used: metadata.blocks() * 512,
                        time_access: NfsTime {
                            seconds: metadata.atime() as u64,
                            nseconds: metadata.atime_nsec() as u32,
                        },
                        time_modify: NfsTime {
                            seconds: metadata.mtime() as u64,
                            nseconds: metadata.mtime_nsec() as u32,
                        },
                        owner: metadata.uid().to_string(),
                        group: metadata.gid().to_string(),
                    };

                    Ok(OperationResult {
                        status: NfsStatus::Ok,
                        result: Some(OperationData::GetAttr(attrs)),
                    })
                } else {
                    Ok(OperationResult {
                        status: NfsStatus::NoEnt,
                        result: None,
                    })
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::StaleFileHandle,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadHandle,
                result: None,
            })
        }
    }

    async fn handle_getfh(&self, _args: GetFhOperation) -> Result<OperationResult> {
        let mut handle_data = vec![0u8; 16];
        rand::thread_rng().fill(&mut handle_data[..]);

        let handle = NfsFileHandle { data: handle_data };

        Ok(OperationResult {
            status: NfsStatus::Ok,
            result: Some(OperationData::GetFh(handle)),
        })
    }

    async fn handle_lookup(&self, args: LookupOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        if let Some(fh) = current_fh {
            let handles = self.handles.read().await;
            if let Some(parent_path) = handles.get(&fh.data) {
                let path = parent_path.join(&args.object_name);
                if path.exists() {
                    let mut handle_data = vec![0u8; 16];
                    rand::thread_rng().fill(&mut handle_data[..]);

                    let mut handles = self.handles.write().await;
                    handles.insert(handle_data.clone(), path);

                    Ok(OperationResult {
                        status: NfsStatus::Ok,
                        result: Some(OperationData::GetFh(NfsFileHandle { data: handle_data })),
                    })
                } else {
                    Ok(OperationResult {
                        status: NfsStatus::NoEnt,
                        result: None,
                    })
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::StaleFileHandle,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadHandle,
                result: None,
            })
        }
    }

    async fn handle_open(&self, args: OpenOperation, current_fh: &Option<NfsFileHandle>) -> Result<OperationResult> {
        let mut stateid = [0u8; 16];
        rand::thread_rng().fill(&mut stateid[..]);

        let mut stateids = self.stateids.write().await;
        match &args.open_claim {
            OpenClaim::Null(path) => {
                let full_path = self.export_root.join(path);
                let file = OpenOptions::new()
                    .read((args.share_access & ACCESS4_READ) != 0)
                    .write((args.share_access & (ACCESS4_MODIFY | ACCESS4_EXTEND)) != 0)
                    .create(true)
                    .open(&full_path)
                    .await;

                match file {
                    Ok(file) => {
                        stateids.insert(
                            stateid,
                            FileState {
                                path: full_path,
                                open_mode: args.share_access,
                                seqid: args.seqid,
                                file: Some(file),
                            },
                        );

                        Ok(OperationResult {
                            status: NfsStatus::Ok,
                            result: Some(OperationData::Open(stateid)),
                        })
                    }
                    Err(_) => Ok(OperationResult {
                        status: NfsStatus::IoError,
                        result: None,
                    }),
                }
            }
            _ => Ok(OperationResult {
                status: NfsStatus::Error,
                result: None,
            }),
        }
    }

    async fn handle_read(&self, args: ReadOperation) -> Result<OperationResult> {
        let stateids = self.stateids.read().await;
        if let Some(state) = stateids.get(&args.stateid) {
            if let Some(ref file) = state.file {
                let mut file = file.try_clone().await?;
                file.seek(std::io::SeekFrom::Start(args.offset)).await?;

                let mut buf = vec![0u8; args.count as usize];
                match file.read(&mut buf).await {
                    Ok(n) => {
                        buf.truncate(n);
                        Ok(OperationResult {
                            status: NfsStatus::Ok,
                            result: Some(OperationData::Read(buf)),
                        })
                    }
                    Err(_) => Ok(OperationResult {
                        status: NfsStatus::IoError,
                        result: None,
                    }),
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::IoError,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadStateid,
                result: None,
            })
        }
    }

    async fn handle_write(&self, args: WriteOperation) -> Result<OperationResult> {
        let stateids = self.stateids.read().await;
        if let Some(state) = stateids.get(&args.stateid) {
            if let Some(ref file) = state.file {
                let mut file = file.try_clone().await?;
                file.seek(std::io::SeekFrom::Start(args.offset)).await?;

                match file.write_all(&args.data).await {
                    Ok(_) => {
                        if args.stable != 0 {
                            file.sync_all().await?;
                        }
                        Ok(OperationResult {
                            status: NfsStatus::Ok,
                            result: Some(OperationData::Write(args.data.len() as u32)),
                        })
                    }
                    Err(_) => Ok(OperationResult {
                        status: NfsStatus::IoError,
                        result: None,
                    }),
                }
            } else {
                Ok(OperationResult {
                    status: NfsStatus::IoError,
                    result: None,
                })
            }
        } else {
            Ok(OperationResult {
                status: NfsStatus::BadStateid,
                result: None,
            })
        }
    }
}

