use crate::GuestPhysAddr;
use crate::HostPhysAddr;

pub use self::blk::*;
pub use self::dev::*;
pub use self::mmio::*;
pub use self::queue::*;

mod blk;
mod dev;
mod mmio;
mod queue;

extern "C" {
    pub fn vm_ipa2pa(gpa: GuestPhysAddr) -> HostPhysAddr;
}