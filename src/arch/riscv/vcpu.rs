use core::arch::global_asm;
use core::marker::PhantomData;
use core::mem::size_of;
use memoffset::offset_of;

use alloc::sync::Arc;
use riscv::register::{hstatus, htinst, htval, scause, sstatus, stval};

use crate::HyperCraftHal;

use super::regs::{GeneralPurposeRegisters, GprIndex};
use super::Guest;

/// Hypervisor GPR and CSR state which must be saved/restored when entering/exiting virtualization.
#[derive(Default)]
#[repr(C)]
struct HypervisorCpuState {
    gprs: GeneralPurposeRegisters,
    sstatus: u64,
    hstatus: u64,
    scounteren: u64,
    stvec: u64,
    sscratch: u64,
}

/// Guest GPR and CSR state which must be saved/restored when exiting/entering virtualization.
#[derive(Default)]
#[repr(C)]
struct GuestCpuState {
    gprs: GeneralPurposeRegisters,
    sstatus: u64,
    hstatus: u64,
    scounteren: u64,
    sepc: u64,
}

/// The CSRs that are only in effect when virtualization is enabled (V=1) and must be saved and
/// restored whenever we switch between VMs.
#[derive(Default)]
#[repr(C)]
pub struct GuestVsCsrs {
    htimedelta: u64,
    vsstatus: u64,
    vsie: u64,
    vstvec: u64,
    vsscratch: u64,
    vsepc: u64,
    vscause: u64,
    vstval: u64,
    vsatp: u64,
    vstimecmp: u64,
}

/// Virtualized HS-level CSRs that are used to emulate (part of) the hypervisor extension for the
/// guest.
#[derive(Default)]
#[repr(C)]
pub struct GuestVirtualHsCsrs {
    hie: u64,
    hgeie: u64,
    hgatp: u64,
}

/// CSRs written on an exit from virtualization that are used by the hypervisor to determine the cause
/// of the trap.
#[derive(Default, Clone)]
#[repr(C)]
pub struct VmCpuTrapState {
    pub scause: u64,
    pub stval: u64,
    pub htval: u64,
    pub htinst: u64,
}

/// (v)CPU register state that must be saved or restored when entering/exiting a VM or switching
/// between VMs.
#[derive(Default)]
#[repr(C)]
struct VmCpuRegisters {
    // CPU state that's shared between our's and the guest's execution environment. Saved/restored
    // when entering/exiting a VM.
    hyp_regs: HypervisorCpuState,
    guest_regs: GuestCpuState,

    // CPU state that only applies when V=1, e.g. the VS-level CSRs. Saved/restored on activation of
    // the vCPU.
    vs_csrs: GuestVsCsrs,

    // Virtualized HS-level CPU state.
    virtual_hs_csrs: GuestVirtualHsCsrs,

    // Read on VM exit.
    trap_csrs: VmCpuTrapState,
}

#[allow(dead_code)]
const fn hyp_gpr_offset(index: GprIndex) -> usize {
    offset_of!(VmCpuRegisters, hyp_regs)
        + offset_of!(HypervisorCpuState, gprs)
        + (index as usize) * size_of::<u64>()
}

#[allow(dead_code)]
const fn guest_gpr_offset(index: GprIndex) -> usize {
    offset_of!(VmCpuRegisters, guest_regs)
        + offset_of!(GuestCpuState, gprs)
        + (index as usize) * size_of::<u64>()
}

#[allow(unused_macros)]
macro_rules! hyp_csr_offset {
    ($reg:tt) => {
        offset_of!(VmCpuRegisters, hyp_regs) + offset_of!(HypervisorCpuState, $reg)
    };
}

#[allow(unused_macros)]
macro_rules! guest_csr_offset {
    ($reg:tt) => {
        offset_of!(VmCpuRegisters, guest_regs) + offset_of!(GuestCpuState, $reg)
    };
}

pub struct VCpu<H: HyperCraftHal> {
    regs: VmCpuRegisters,
    pub guest: Arc<Guest>,
    marker: PhantomData<H>,
}

// const hyp_ra: usize = hyp_gpr_offset(GprIndex::RA);
// const hyp_gp: usize = hyp_gpr_offset(GprIndex::GP);
// const hyp_tp: usize = hyp_gpr_offset(GprIndex::TP);
// const hyp_s0: usize = hyp_gpr_offset(GprIndex::S0);
// const hyp_s1: usize = hyp_gpr_offset(GprIndex::S1);
// const hyp_a1: usize = hyp_gpr_offset(GprIndex::A1);
// const hyp_a2: usize = hyp_gpr_offset(GprIndex::A2);
// const hyp_a3: usize = hyp_gpr_offset(GprIndex::A3);
// const hyp_a4: usize = hyp_gpr_offset(GprIndex::A4);
// const hyp_a5: usize = hyp_gpr_offset(GprIndex::A5);
// const hyp_a6: usize = hyp_gpr_offset(GprIndex::A6);
// const hyp_a7: usize = hyp_gpr_offset(GprIndex::A7);
// const hyp_s2: usize = hyp_gpr_offset(GprIndex::S2);
// const hyp_s3: usize = hyp_gpr_offset(GprIndex::S3);
// const hyp_s4: usize = hyp_gpr_offset(GprIndex::S4);
// const hyp_s5: usize = hyp_gpr_offset(GprIndex::S5);
// const hyp_s6: usize = hyp_gpr_offset(GprIndex::S6);
// const hyp_s7: usize = hyp_gpr_offset(GprIndex::S7);
// const hyp_s8: usize = hyp_gpr_offset(GprIndex::S8);
// const hyp_s9: usize = hyp_gpr_offset(GprIndex::S9);
// const hyp_s10: usize = hyp_gpr_offset(GprIndex::S10);
// const hyp_s11: usize = hyp_gpr_offset(GprIndex::S11);
// const hyp_sp: usize = hyp_gpr_offset(GprIndex::SP);

// const hyp_sstatus: usize = hyp_csr_offset!(sstatus);
// const hyp_hstatus: usize = hyp_csr_offset!(hstatus);
// const hyp_scounteren: usize = hyp_csr_offset!(scounteren);
// const hyp_stvec: usize = hyp_csr_offset!(stvec);
// const hyp_sscratch: usize = hyp_csr_offset!(sscratch);

// const guest_ra: usize = guest_gpr_offset(GprIndex::RA);
// const guest_gp: usize = guest_gpr_offset(GprIndex::GP);
// const guest_tp: usize = guest_gpr_offset(GprIndex::TP);
// const guest_s0: usize = guest_gpr_offset(GprIndex::S0);
// const guest_s1: usize = guest_gpr_offset(GprIndex::S1);
// const guest_a0: usize = guest_gpr_offset(GprIndex::A0);
// const guest_a1: usize = guest_gpr_offset(GprIndex::A1);
// const guest_a2: usize = guest_gpr_offset(GprIndex::A2);
// const guest_a3: usize = guest_gpr_offset(GprIndex::A3);
// const guest_a4: usize = guest_gpr_offset(GprIndex::A4);
// const guest_a5: usize = guest_gpr_offset(GprIndex::A5);
// const guest_a6: usize = guest_gpr_offset(GprIndex::A6);
// const guest_a7: usize = guest_gpr_offset(GprIndex::A7);
// const guest_s2: usize = guest_gpr_offset(GprIndex::S2);
// const guest_s3: usize = guest_gpr_offset(GprIndex::S3);
// const guest_s4: usize = guest_gpr_offset(GprIndex::S4);
// const guest_s5: usize = guest_gpr_offset(GprIndex::S5);
// const guest_s6: usize = guest_gpr_offset(GprIndex::S6);
// const guest_s7: usize = guest_gpr_offset(GprIndex::S7);
// const guest_s8: usize = guest_gpr_offset(GprIndex::S8);
// const guest_s9: usize = guest_gpr_offset(GprIndex::S9);
// const guest_s10: usize = guest_gpr_offset(GprIndex::S10);
// const guest_s11: usize = guest_gpr_offset(GprIndex::S11);
// const guest_t0: usize = guest_gpr_offset(GprIndex::T0);
// const guest_t1: usize = guest_gpr_offset(GprIndex::T1);
// const guest_t2: usize = guest_gpr_offset(GprIndex::T2);
// const guest_t3: usize = guest_gpr_offset(GprIndex::T3);
// const guest_t4: usize = guest_gpr_offset(GprIndex::T4);
// const guest_t5: usize = guest_gpr_offset(GprIndex::T5);
// const guest_t6: usize = guest_gpr_offset(GprIndex::T6);
// const guest_sp: usize = guest_gpr_offset(GprIndex::SP);

// const guest_sstatus: usize = guest_csr_offset!(sstatus);
// const guest_hstatus: usize = guest_csr_offset!(hstatus);
// const guest_scounteren: usize = guest_csr_offset!(scounteren);
// const guest_sepc: usize = guest_csr_offset!(sepc);

global_asm!(
    include_str!("guest.S"),
    hyp_ra = const hyp_gpr_offset(GprIndex::RA),
    hyp_gp = const hyp_gpr_offset(GprIndex::GP),
    hyp_tp = const hyp_gpr_offset(GprIndex::TP),
    hyp_s0 = const hyp_gpr_offset(GprIndex::S0),
    hyp_s1 = const hyp_gpr_offset(GprIndex::S1),
    hyp_a1 = const hyp_gpr_offset(GprIndex::A1),
    hyp_a2 = const hyp_gpr_offset(GprIndex::A2),
    hyp_a3 = const hyp_gpr_offset(GprIndex::A3),
    hyp_a4 = const hyp_gpr_offset(GprIndex::A4),
    hyp_a5 = const hyp_gpr_offset(GprIndex::A5),
    hyp_a6 = const hyp_gpr_offset(GprIndex::A6),
    hyp_a7 = const hyp_gpr_offset(GprIndex::A7),
    hyp_s2 = const hyp_gpr_offset(GprIndex::S2),
    hyp_s3 = const hyp_gpr_offset(GprIndex::S3),
    hyp_s4 = const hyp_gpr_offset(GprIndex::S4),
    hyp_s5 = const hyp_gpr_offset(GprIndex::S5),
    hyp_s6 = const hyp_gpr_offset(GprIndex::S6),
    hyp_s7 = const hyp_gpr_offset(GprIndex::S7),
    hyp_s8 = const hyp_gpr_offset(GprIndex::S8),
    hyp_s9 = const hyp_gpr_offset(GprIndex::S9),
    hyp_s10 = const hyp_gpr_offset(GprIndex::S10),
    hyp_s11 = const hyp_gpr_offset(GprIndex::S11),
    hyp_sp = const hyp_gpr_offset(GprIndex::SP),
    hyp_sstatus = const hyp_csr_offset!(sstatus),
    hyp_hstatus = const hyp_csr_offset!(hstatus),
    hyp_scounteren = const hyp_csr_offset!(scounteren),
    hyp_stvec = const hyp_csr_offset!(stvec),
    hyp_sscratch = const hyp_csr_offset!(sscratch),
    guest_ra = const guest_gpr_offset(GprIndex::RA),
    guest_gp = const guest_gpr_offset(GprIndex::GP),
    guest_tp = const guest_gpr_offset(GprIndex::TP),
    guest_s0 = const guest_gpr_offset(GprIndex::S0),
    guest_s1 = const guest_gpr_offset(GprIndex::S1),
    guest_a0 = const guest_gpr_offset(GprIndex::A0),
    guest_a1 = const guest_gpr_offset(GprIndex::A1),
    guest_a2 = const guest_gpr_offset(GprIndex::A2),
    guest_a3 = const guest_gpr_offset(GprIndex::A3),
    guest_a4 = const guest_gpr_offset(GprIndex::A4),
    guest_a5 = const guest_gpr_offset(GprIndex::A5),
    guest_a6 = const guest_gpr_offset(GprIndex::A6),
    guest_a7 = const guest_gpr_offset(GprIndex::A7),
    guest_s2 = const guest_gpr_offset(GprIndex::S2),
    guest_s3 = const guest_gpr_offset(GprIndex::S3),
    guest_s4 = const guest_gpr_offset(GprIndex::S4),
    guest_s5 = const guest_gpr_offset(GprIndex::S5),
    guest_s6 = const guest_gpr_offset(GprIndex::S6),
    guest_s7 = const guest_gpr_offset(GprIndex::S7),
    guest_s8 = const guest_gpr_offset(GprIndex::S8),
    guest_s9 = const guest_gpr_offset(GprIndex::S9),
    guest_s10 = const guest_gpr_offset(GprIndex::S10),
    guest_s11 = const guest_gpr_offset(GprIndex::S11),
    guest_t0 = const guest_gpr_offset(GprIndex::T0),
    guest_t1 = const guest_gpr_offset(GprIndex::T1),
    guest_t2 = const guest_gpr_offset(GprIndex::T2),
    guest_t3 = const guest_gpr_offset(GprIndex::T3),
    guest_t4 = const guest_gpr_offset(GprIndex::T4),
    guest_t5 = const guest_gpr_offset(GprIndex::T5),
    guest_t6 = const guest_gpr_offset(GprIndex::T6),
    guest_sp = const guest_gpr_offset(GprIndex::SP),

    guest_sstatus = const guest_csr_offset!(sstatus),
    guest_hstatus = const guest_csr_offset!(hstatus),
    guest_scounteren = const guest_csr_offset!(scounteren),
    guest_sepc = const guest_csr_offset!(sepc),

);

extern "C" {
    fn _run_guest(state: *mut VmCpuRegisters);
}

impl<H: HyperCraftHal> VCpu<H> {
    pub fn create(
        _entry: usize,
        _sp: usize,
        _hgatp: usize,
        _kernel_sp: usize,
        _trap_handler: usize,
        guest: Arc<Guest>,
    ) -> Self {
        let mut regs = VmCpuRegisters::default();
        // Set hstatus
        let mut hstatus = hstatus::read();
        hstatus.set_spv(true);
        regs.guest_regs.hstatus = hstatus.bits() as u64;

        // Set sstatus
        let mut sstatus = sstatus::read();
        sstatus.set_spp(sstatus::SPP::Supervisor);
        regs.guest_regs.sstatus = sstatus.bits() as u64;
        Self {
            regs,
            guest,
            marker: PhantomData,
        }
    }

    /// Runs this vCPU until traps.
    pub fn run(&mut self) {
        loop {
            let regs = &mut self.regs;
            unsafe {
                // Safe to run the guest as it only touches memory assigned to it by being owned
                // by its page table
                _run_guest(regs);
            }
            // Save off the trap information
            regs.trap_csrs.scause = scause::read().bits() as u64;
            regs.trap_csrs.stval = stval::read() as u64;
            regs.trap_csrs.htval = htval::read() as u64;
            regs.trap_csrs.htinst = htinst::read() as u64;
            // vm exit handler
            H::vmexit_handler(self);
        }
    }
}
