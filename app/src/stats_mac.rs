//! CPU, RAM e batteria per il widget "pc" (macOS: chiamate mach del kernel e IOKit).

use std::ffi::{c_char, c_void};

extern "C" {
    fn mach_host_self() -> u32;
    fn host_page_size(host: u32, size: *mut usize) -> i32;
    fn host_statistics(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn host_statistics64(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn sysctlbyname(name: *const c_char, old: *mut c_void, oldlen: *mut usize, new: *mut c_void, newlen: usize) -> i32;
}

const HOST_CPU_LOAD_INFO: i32 = 3;
const HOST_VM_INFO64: i32 = 4;
/// vm_statistics64 è lunga 152 byte = 38 interi da 32 bit
const VM_INFO64_COUNT: u32 = 38;

pub struct Stats {
    host: u32,
    /// tick della lettura precedente: (inattivo, totale)
    prima: Option<(u64, u64)>,
}

impl Default for Stats {
    fn default() -> Self {
        Self { host: unsafe { mach_host_self() }, prima: None }
    }
}

impl Stats {
    /// i valori come oggetto JSON per zooSetting('statistiche', ...)
    pub fn leggi(&mut self) -> serde_json::Value {
        let mut v = serde_json::Map::new();
        unsafe {
            // tick della CPU: utente, sistema, inattivo, nice
            let mut cpu = [0i32; 4];
            let mut n = 4u32;
            if host_statistics(self.host, HOST_CPU_LOAD_INFO, cpu.as_mut_ptr(), &mut n) == 0 {
                let t: Vec<u64> = cpu.iter().map(|&x| x as u32 as u64).collect();
                let ora = (t[2], t.iter().sum::<u64>());
                if let Some(p) = self.prima {
                    let tot = ora.1.wrapping_sub(p.1);
                    if tot > 0 && tot < u32::MAX as u64 {
                        let usato = tot.saturating_sub(ora.0.wrapping_sub(p.0));
                        v.insert("cpu".into(), (usato as f64 * 100.0 / tot as f64).round().into());
                    }
                }
                self.prima = Some(ora);
            }

            // memoria usata come la conta Monitoraggio Attività:
            // memoria delle app (interna - eliminabile) + wired + compressa
            let mut vm = [0u64; 20];
            let mut n = VM_INFO64_COUNT;
            let mut pagina = 0usize;
            let mut totale = 0u64;
            let mut len = std::mem::size_of::<u64>();
            if host_statistics64(self.host, HOST_VM_INFO64, vm.as_mut_ptr().cast(), &mut n) == 0
                && host_page_size(self.host, &mut pagina) == 0
                && sysctlbyname(c"hw.memsize".as_ptr(), (&mut totale as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0) == 0
                && totale > 0
            {
                let p = std::slice::from_raw_parts(vm.as_ptr().cast::<u32>(), 38);
                let (wired, eliminabile, compressa, interna) = (p[3], p[22], p[32], p[35]);
                let usate = interna.saturating_sub(eliminabile) as u64 + wired as u64 + compressa as u64;
                let perc = (usate * pagina as u64) as f64 * 100.0 / totale as f64;
                v.insert("ram".into(), perc.round().min(100.0).into());
            }
        }
        if let Some(b) = crate::power::batteria() {
            v.insert("batteria".into(), b.into());
            v.insert("carica".into(), crate::power::on_ac().into());
        }
        v.into()
    }
}
