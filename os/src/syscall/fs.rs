use crate::fs::{make_pipe, open_file, OpenFlags, create_hard_link, unlink_file, Stat};
use crate::mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer};
use crate::task::{current_process, current_user_token, current_task};
use alloc::sync::Arc;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let process = current_process();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

pub fn sys_pipe(pipe: *mut usize) -> isize {
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}

pub fn sys_dup(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}
// YOUR JOB: 扩展 easy-fs 和内核以实现以下三个 syscall
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    let task = current_process();
    let token = current_user_token();
    let st = translated_refmut(token, _st);

    let inner = task.inner_exclusive_access();

    if _fd < inner.fd_table.len() {
        let res = inner.fd_table[_fd].clone();
        match res {
            None => {
                println!("[kernel] fd = {} is None in fd_table of process = {}", _fd, task.pid.0);
                return -1;
            },
            Some(node) => {
                //println!("[kernel] fd = {} is valid in fd_table of process = {}", _fd, task.pid.0);
                node.stat(st);
                return 0;
            }
        }
    }
    else {
        println!("[kernel] fd = {} not found in fd_table of process = {}", _fd, task.pid.0);
        return -1;
    }
}

pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    let process = current_process();
    let token = current_user_token();
    let old_path = translated_str(token, _old_name);
    let new_path = translated_str(token, _new_name);

    if let Some(old_inode) = open_file(
        old_path.as_str(),
        OpenFlags::RDONLY
    ) {
        let mut flags = OpenFlags::CREATE;
        if old_inode.is_readble() {flags = flags | OpenFlags::RDONLY;}
        if old_inode.is_writable() {flags = flags | OpenFlags::WRONLY;}

        let res_inode = create_hard_link("/", old_path.as_str(),new_path.as_str(), flags);
        //println!("[kernel] create hard link finished, old_path = {}, new_path = {}", old_path.as_str(), new_path.as_str());
        match res_inode{
            None => return -1,
            Some(new_node) => {
                let mut inner = process.inner_exclusive_access();
                let fd = inner.alloc_fd();
                //println!("[kernel] create hard link success, fd = {}", fd);
                inner.fd_table[fd] = Some(new_node);
                return fd as isize;
            }
        }
    }
    else {
        return -1;
    }
}

pub fn sys_unlinkat(_name: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, _name);
    return unlink_file("/", path.as_str());
}
