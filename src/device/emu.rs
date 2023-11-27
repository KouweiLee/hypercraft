use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::in_range;

use super::VirtioMmio;

pub const EMU_DEV_NUM_MAX: usize = 32;
pub static EMU_DEVS_LIST: Mutex<Vec<EmuDevEntry>> = Mutex::new(Vec::new());
/// EmuDevs of all vms
pub static VM_EMU_DEVS: Mutex<Vec<EmuDevs>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmuDeviceType {
    // EmuDeviceTConsole = 0,
    // EmuDeviceTGicd = 1,
    // EmuDeviceTGPPT = 2,
    EmuDeviceTVirtioBlk = 3,
    EmuDeviceTVirtioNet = 4,
    // EmuDeviceTVirtioConsole = 5,
    // EmuDeviceTShyper = 6,
    // EmuDeviceTVirtioBlkMediated = 7,
    // EmuDeviceTIOMMU = 8,
}

pub type EmuDevHandler = fn(usize, &EmuContext) -> bool;


pub struct EmuContext {
    pub address: usize,
    pub width: usize,
    pub write: bool,
    pub sign_ext: bool,
    pub reg: usize,
    pub reg_width: usize,
}

pub struct EmuDevEntry {
    pub emu_type: EmuDeviceType,
    // pub vm_id: usize,
    pub id: usize,
    pub ipa: usize,
    pub size: usize,
    pub handler: EmuDevHandler,
}

#[derive(Clone)]
pub enum EmuDevs {
    // Vgic(Arc<Vgic>),
    VirtioBlk(VirtioMmio),
    // VirtioNet(VirtioMmio),
    // VirtioConsole(VirtioMmio),
    None,
}

pub fn emu_handler(emu_ctx: &EmuContext) -> bool {
    let ipa = emu_ctx.address;
    let emu_devs_list = EMU_DEVS_LIST.lock();
    // TODO: multi cpus and vms
    for emu_dev in &*emu_devs_list {        
        if in_range(ipa, emu_dev.ipa, emu_dev.size - 1) {
            let handler = emu_dev.handler;
            let id = emu_dev.id;
            drop(emu_devs_list);
            return handler(id, emu_ctx);
        }
    }
    error!(
        "emu_handler: no emul handler for Core {} data abort ipa 0x{:x}",
        0,
        ipa
    );
    return false;
}
/// register a emu dev's info
pub fn emu_register_dev(
    emu_type: EmuDeviceType,
    // vm_id: usize,
    dev_id: usize,
    address: usize,
    size: usize,
    handler: EmuDevHandler,
) {
    info!("emu_register_dev");
    let mut emu_devs_list = EMU_DEVS_LIST.lock();
    if emu_devs_list.len() >= EMU_DEV_NUM_MAX {
        panic!("emu_register_dev: can't register more devs");
    }

    for emu_dev in &*emu_devs_list {
        // if vm_id != emu_dev.vm_id {
        //     continue;
        // }
        if in_range(address, emu_dev.ipa, emu_dev.size - 1) || in_range(emu_dev.ipa, address, size - 1) {
            panic!("emu_register_dev: duplicated emul address region: prev address 0x{:x} size 0x{:x}, next address 0x{:x} size 0x{:x}", emu_dev.ipa, emu_dev.size, address, size);
        }
    }

    emu_devs_list.push(EmuDevEntry {
        emu_type,
        // vm_id,
        id: dev_id,
        ipa: address,
        size,
        handler,
    });

    // let mut vm_emus = VM_EMU_DEVS.lock();
    // vm_emus.push()
}