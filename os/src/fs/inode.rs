use crate::drivers::BLOCK_DEVICE;
use easy_fs::{
    EasyFileSystem,
    Inode,
    DiskInodeType,
};
use alloc::sync::Arc;
use lazy_static::*;
use bitflags::*;
use alloc::vec::Vec;
use super::{File, StatMode};
use crate::mm::UserBuffer;
use crate::sync::UPIntrFreeCell;

pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPIntrFreeCell<OSInodeInner>,
}

pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPIntrFreeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }

    pub fn is_readble(&self) -> bool {return self.readable;}
    pub fn is_writable(&self) -> bool {return self.writable;}
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/")
}

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;
        const WRONLY = 1 << 0;
        const RDWR = 1 << 1;
        const CREATE = 1 << 9;
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

/// dir is the directory in which the file is to be created. But in hCore, there is only one dir --- ROOT.
/// So the value of dir is always "/".
pub fn create_hard_link(dir: &str, old_name: &str, new_name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if dir != "/" {
        return None;
    }
    if flags.contains(OpenFlags::CREATE) {
        if let Some(old_inode) = ROOT_INODE.find(old_name) {
            ROOT_INODE.link(new_name, old_inode)
                .map(|inode| {
                    Arc::new(OSInode::new(
                        readable,
                        writable,
                        inode,
                    ))
                })
        } else {
            return None;
        }
    } else {
        return None;
    }
}


/// dir is the directory in which the file is to be created. But in hCore, there is only one dir --- ROOT.
/// So the value of dir is always "/".
pub fn unlink_file(dir: &str, name: &str) -> isize {
    if dir != "/" {
        return -1;
    }
    if let Some(inode) = ROOT_INODE.find(name) {
        let nlink = ROOT_INODE.unlink(&inode);
        if nlink == 0 {
            return ROOT_INODE.delete_dir_entry(name, inode);
        }
        else {
            return nlink;
        }
    } else {
        return -1;
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }

    fn stat(&self, stat: &mut super::Stat) -> isize {
        let inner = self.inner.exclusive_access();
        let (ftype, nlink) = inner.inode.status();
        let node_id = inner.inode.get_disk_node_id();
        //println!("[kernel] in function stat node_id = {}, nlink = {}", node_id, nlink);
        stat.ino = node_id as u64;
        stat.mode = if ftype == DiskInodeType::Directory {StatMode::DIR} else {StatMode::FILE};
        stat.nlink = nlink;
        0
    }
}
