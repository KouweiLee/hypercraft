use alloc::sync::Arc;
use spin::Mutex;

use super::{VIRTIO_BLK_F_SIZE_MAX, VIRTIO_BLK_F_SEG_MAX, VIRTIO_F_VERSION_1, BlkDesc};


pub const VIRTIO_IPA: [usize;2] = [0xa004000, 0];


#[derive(Copy, Clone, Debug)]
pub enum VirtioDeviceType {
    None = 0,
    Net = 1,
    Block = 2,
    Console = 3,
}

#[derive(Clone)]
pub struct VirtDev {
    inner: Arc<Mutex<VirtDevInner>>,
}

impl VirtDev {
    pub fn default() -> VirtDev {
        VirtDev {
            inner: Arc::new(Mutex::new(VirtDevInner::default())),
        }
    }

    pub fn desc(&self) -> DevDesc {
        let inner = self.inner.lock();
        inner.desc.clone()
    }

    pub fn init(&self, dev_type: VirtioDeviceType) {
        let mut inner = self.inner.lock();
        inner.init(dev_type);
    }

    pub fn features(&self) -> usize {
        let inner = self.inner.lock();
        inner.features
    }

    pub fn generation(&self) -> usize {
        let inner = self.inner.lock();
        inner.generation
    }

    pub fn activated(&self) -> bool {
        let inner = self.inner.lock();
        inner.activated
    }

    pub fn set_activated(&self, activated: bool) {
        let mut inner = self.inner.lock();
        inner.activated = activated;
    }
}

pub struct VirtDevInner {
    activated: bool,
    dev_type: VirtioDeviceType,
    features: usize,
    generation: usize,
    // int_id: usize,
    desc: DevDesc,
    // req: DevReq,
    // cache: Option<PageFrame>,
    // stat: DevStat,
}

impl VirtDevInner {
    pub fn default() -> VirtDevInner {
        VirtDevInner {
            activated: false,
            dev_type: VirtioDeviceType::None,
            features: 0,
            generation: 0,
            // int_id: 0,
            desc: DevDesc::None,
            // req: DevReq::None,
            // cache: None,
            // stat: DevStat::None,
        }
    }

    pub fn init(&mut self, dev_type: VirtioDeviceType) {
        self.dev_type = dev_type;
        let blk_desc = BlkDesc::default();
        // 初始化32个扇区
        blk_desc.cfg_init(32);
        self.desc = DevDesc::BlkDesc(blk_desc);

        match self.dev_type {
            VirtioDeviceType::Block => {
                self.features |= VIRTIO_BLK_F_SIZE_MAX | VIRTIO_BLK_F_SEG_MAX | VIRTIO_F_VERSION_1;
            }, 
            _ => {
                panic!("ERROR: Wrong virtio device type");
            }
        }
    }
}

#[derive(Clone)]
pub enum DevDesc {
    BlkDesc(BlkDesc),
    // NetDesc(NetDesc),
    // ConsoleDesc(ConsoleDesc),
    None,
}