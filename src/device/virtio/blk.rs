use alloc::{sync::Arc, vec::Vec};
use spin::Mutex;


use crate::{device::{VIRTQ_DESC_F_WRITE, vm_ipa2pa}, memory::PAGE_SIZE_4K};

use super::{Virtq, VirtioMmio};

pub const VIRTQUEUE_BLK_MAX_SIZE: usize = 256;
pub const VIRTQUEUE_NET_MAX_SIZE: usize = 256;

/* VIRTIO_BLK_FEATURES*/
pub const VIRTIO_BLK_F_SIZE_MAX: usize = 1 << 1;
pub const VIRTIO_BLK_F_SEG_MAX: usize = 1 << 2;

/* BLOCK PARAMETERS*/
/// 块设备的扇区大小
pub const SECTOR_BSIZE: usize = 512; 
pub const BLOCKIF_SIZE_MAX: usize = 128 * PAGE_SIZE_4K;
pub const BLOCKIF_IOV_MAX: usize = 512;

/* BLOCK REQUEST TYPE*/
/// read from device
pub const VIRTIO_BLK_T_IN: u32 = 0;
/// write to device
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTIO_BLK_T_FLUSH: u32 = 4;
pub const VIRTIO_BLK_T_GET_ID: u32 = 8;

/* BLOCK REQUEST STATUS*/
pub const VIRTIO_BLK_S_OK: usize = 0;
// pub const VIRTIO_BLK_S_IOERR: usize = 1;
/// 不支持的请求类型
pub const VIRTIO_BLK_S_UNSUPP: usize = 2;

#[derive(Clone)]
pub struct BlkDesc {
    inner: Arc<Mutex<BlkDescInner>>,
}

impl BlkDesc {
    pub fn default() -> BlkDesc {
        BlkDesc {
            inner: Arc::new(Mutex::new(BlkDescInner::default())),
        }
    }

    pub fn cfg_init(&self, bsize: usize) {
        let mut inner = self.inner.lock();
        inner.cfg_init(bsize);
    }

    pub fn start_addr(&self) -> usize {
        let inner = self.inner.lock();
        &inner.capacity as *const _ as usize
    }

    pub fn offset_data(&self, offset: usize) -> u32 {
        let start_addr = self.start_addr();
        if start_addr + offset < 0x1000 {
            panic!("illegal addr {:x}", start_addr + offset);
        }
        let value = unsafe { *((start_addr + offset) as *const u32) };
        return value;
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct BlkDescInner {
    /// Number of 512 Bytes sectors
    capacity: usize,
    /// Maximum size of any single segment
    size_max: u32,
    /// Maximum number of segments in a request
    seg_max: u32,
    geometry: BlkGeometry,
    blk_size: usize,
    topology: BlkTopology,
    writeback: u8,
    unused0: [u8; 3],
    max_discard_sectors: u32,
    max_discard_seg: u32,
    discard_sector_alignment: u32,
    max_write_zeroes_sectors: u32,
    max_write_zeroes_seg: u32,
    write_zeroes_may_unmap: u8,
    unused1: [u8; 3],
}

impl BlkDescInner {
    pub fn default() -> BlkDescInner {
        BlkDescInner {
            capacity: 0,
            size_max: 0,
            seg_max: 0,
            geometry: BlkGeometry::default(),
            blk_size: 0,
            topology: BlkTopology::default(),
            writeback: 0,
            unused0: [0; 3],
            max_discard_sectors: 0,
            max_discard_seg: 0,
            discard_sector_alignment: 0,
            max_write_zeroes_sectors: 0,
            max_write_zeroes_seg: 0,
            write_zeroes_may_unmap: 0,
            unused1: [0; 3],
        }
    }

    pub fn cfg_init(&mut self, bsize: usize) {
        self.capacity = bsize;
        self.size_max = BLOCKIF_SIZE_MAX as u32;
        self.seg_max = BLOCKIF_IOV_MAX as u32;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct BlkGeometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl BlkGeometry {
    fn default() -> BlkGeometry {
        BlkGeometry {
            cylinders: 0,
            heads: 0,
            sectors: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct BlkTopology {
    // # of logical blocks per physical block (log2)
    physical_block_exp: u8,
    // offset of first aligned logical block
    alignment_offset: u8,
    // suggested minimum I/O size in blocks
    min_io_size: u16,
    // optimal (suggested maximum) I/O size in blocks
    opt_io_size: u32,
}

impl BlkTopology {
    fn default() -> BlkTopology {
        BlkTopology {
            physical_block_exp: 0,
            alignment_offset: 0,
            min_io_size: 0,
            opt_io_size: 0,
        }
    }
}
/// the first buffer in the head of descriptor's list to describe the request
#[repr(C)]
#[derive(Clone)]
pub struct VirtioBlkReqNode {
    /// 请求类型
    req_type: u32,
    reserved: u32,
    /// 偏移量（需要乘512）
    sector: usize,
    /// 描述符链头
    desc_chain_head_idx: u32,
    // io请求向量，for backend req to real driver
    iov: Vec<BlkIov>,
    // sum up byte for req
    iov_sum_up: usize,
    // total byte for current req. May be removed later, same as iov_sum_up
    iov_total: usize,
}

impl VirtioBlkReqNode {
    pub fn default() -> VirtioBlkReqNode {
        VirtioBlkReqNode {
            req_type: 0,
            reserved: 0,
            sector: 0,
            desc_chain_head_idx: 0,
            iov: vec![],
            iov_sum_up: 0,
            iov_total: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct BlkIov {
    pub data_bg: usize,
    pub len: u32,
}

/// frontend向后端发出queue notify的最终处理函数
pub fn virtio_blk_notify_handler(vq: Virtq, blk: VirtioMmio) -> bool {
    // if vm.id() == 0 && active_vm_id() == 0 {
    //     panic!("src vm should not be 0");
    // }
    info!("enter virtio-blk notify handler");
    let avail_idx = vq.avail_idx();

    if vq.ready() == 0 {
        error!("blk virt_queue is not ready!");
        return false;
    }

    let _dev = blk.dev();
    // let req = match dev.req() {
    //     super::DevReq::BlkReq(blk_req) => blk_req,
    //     _ => {
    //         panic!("virtio_blk_notify_handler: illegal req");
    //     }
    // };
    let mut req_list: Vec<VirtioBlkReqNode> = Vec::new();

    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    let mut _process_count: i32 = 0; // 当前已经处理了多少描述符链

    // 持续从可用环中取出描述符链头
    while next_desc_idx_opt.is_some() {
        let mut next_desc_idx = next_desc_idx_opt.unwrap() as usize;
        vq.disable_notify();
        if vq.check_avail_idx(avail_idx) {
            // if this is the last request
            vq.enable_notify();
        }

        let mut head = true;

        let mut req_node = VirtioBlkReqNode::default();
        req_node.desc_chain_head_idx = next_desc_idx as u32;
        // println!(
        //     "avail idx {} desc_chain_head {} avail flag {}",
        //     vq.last_avail_idx() - 1,
        //     req_node.desc_chain_head_idx,
        //     vq.avail_flags()
        // );

        loop {
            // 描述符链还不到末尾
            if vq.desc_has_next(next_desc_idx) {
                // 处理描述符链头
                if head {
                    if vq.desc_is_writable(next_desc_idx) {
                        error!(
                            "Failed to get virt blk queue desc header, idx = {}, flag = {:x}",
                            next_desc_idx,
                            vq.desc_flags(next_desc_idx)
                        );
                        // blk.notify(vm);
                        return false;
                    }
                    head = false;
                    let vreq_addr = unsafe { vm_ipa2pa(vq.desc_addr(next_desc_idx)) };
                    if vreq_addr == 0 {
                        error!("virtio_blk_notify_handler: failed to get vreq");
                        return false;
                    }
                    // 获取链头的内容
                    let vreq = unsafe { &mut *(vreq_addr as *mut VirtioBlkReqNode) };
                    req_node.req_type = vreq.req_type;
                    req_node.sector = vreq.sector;
                } else {
                    /*data handler*/
                    if (vq.desc_flags(next_desc_idx) & VIRTQ_DESC_F_WRITE) as u32 >> 1 == req_node.req_type {
                        // check if buffer can be written as req_type declairs
                        error!(
                            "Failed to get virt blk queue desc data, idx = {}, req.type = {}, desc.flags = {}",
                            next_desc_idx,
                            req_node.req_type,
                            vq.desc_flags(next_desc_idx)
                        );
                        // blk.notify(vm);
                        return false;
                    }
                    // 数据缓冲区的物理地址
                    let data_bg = unsafe { vm_ipa2pa(vq.desc_addr(next_desc_idx)) };
                    if data_bg == 0 {
                        error!("virtio_blk_notify_handler: failed to get iov data begin");
                        return false;
                    }

                    let iov = BlkIov {
                        data_bg,
                        len: vq.desc_len(next_desc_idx),
                    };
                    req_node.iov_sum_up += iov.len as usize;
                    req_node.iov.push(iov);
                }
            } else {
                /*state handler*/
                if !vq.desc_is_writable(next_desc_idx) {
                    error!("Failed to get virt blk queue desc status, idx = {}", next_desc_idx);
                    // blk.notify(vm);
                    return false;
                }
                let vstatus_addr = unsafe { vm_ipa2pa(vq.desc_addr(next_desc_idx)) };
                if vstatus_addr == 0 {
                    error!("virtio_blk_notify_handler: vm failed to vstatus");
                    return false;
                }
                let vstatus = unsafe { &mut *(vstatus_addr as *mut u8) };
                // 如果请求类型不为in和out，且不为VIRTIO_BLK_T_GET_ID
                // 注意，目前失败是直接panic，其实不应该而是返回给driver VIRTIO_BLK_S_IOERR
                if req_node.req_type > 1 && req_node.req_type != VIRTIO_BLK_T_GET_ID as u32 {
                    *vstatus = VIRTIO_BLK_S_UNSUPP as u8;
                } else {
                    *vstatus = VIRTIO_BLK_S_OK as u8;
                }
                break;
            }
            next_desc_idx = vq.desc_next(next_desc_idx) as usize;
        }
        req_node.iov_total = req_node.iov_sum_up;
        req_list.push(req_node);

        _process_count += 1;
        // 获取下一个描述符链
        next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    }
    if !process_blk_requests(req_list, &vq) {
        error!("process_blk_requests error!");
        return false;
    }

    // if vq.avail_flags() == 0 && process_count > 0 && !req.mediated() {
    //     println!("virtio blk notify");
    //     blk.notify(vm);
    // }
    return true;
}

pub trait PlatOperation {
    fn blk_read(offset: usize, count: usize, buf: usize) -> bool;

    fn blk_write(offset: usize, count: usize, buf: usize) -> bool;
}

struct FakeBlkDevice;

impl PlatOperation for FakeBlkDevice {
    fn blk_read(offset: usize, count: usize, buf: usize) -> bool{
        if offset + count >= SECTOR_BSIZE * SECTORS_NUM {
            error!("blk requests exceed blk device");
            return false;
        }
        unsafe {
            let src: *const u8 = &BLOCK_DEVICE[offset] as *const _;
            let dst: *mut u8 = buf as *mut _;
            core::ptr::copy_nonoverlapping(src, dst, count);
        }
        true
    }

    fn blk_write(offset: usize, count: usize, buf: usize) -> bool {
        if offset + count >= SECTOR_BSIZE * SECTORS_NUM {
            error!("blk requests exceed blk device");
            return false;
        }
        unsafe {
            let src: *const u8 = buf as *const _;
            let dst: *mut u8 = &mut BLOCK_DEVICE[offset] as *mut _;
            core::ptr::copy_nonoverlapping(src, dst, count);
        }
        true
    }
}

const SECTORS_NUM: usize = 32;
/// a fake blk device
static mut BLOCK_DEVICE: [u8; SECTOR_BSIZE * SECTORS_NUM] = [0; SECTOR_BSIZE * SECTORS_NUM];

fn process_blk_requests(req_list: Vec<VirtioBlkReqNode>, vq: &Virtq) -> bool {
    for req in req_list {
        let mut write_len = 0;
        match req.req_type {
            VIRTIO_BLK_T_IN | VIRTIO_BLK_T_OUT => {
                let mut offset = req.sector * SECTOR_BSIZE;
                for aiov in req.iov {
                    if req.req_type == VIRTIO_BLK_T_IN as u32{
                        FakeBlkDevice::blk_read(offset, aiov.len as _, aiov.data_bg);
                        write_len += aiov.len;
                    } else {
                        FakeBlkDevice::blk_write(offset, aiov.len as _, aiov.data_bg);
                    }
                    offset += aiov.len as usize;
                }
            },
            VIRTIO_BLK_T_FLUSH => {

            },
            VIRTIO_BLK_T_GET_ID => {
                let data_bg = req.iov[0].data_bg as *mut u8;
                let name = "virtio-blk-0".as_ptr();
                unsafe {
                    core::ptr::copy_nonoverlapping(name, data_bg, 20);
                }
            },
            _ => {
                panic!("it shouldb't panic in process blk requests");
            }
        }
        if !vq.update_used_ring(write_len as u32, req.desc_chain_head_idx) {
            return false;
        }
    }
    return true;
}
