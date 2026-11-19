//! GPU per le statistiche per nerd, su macOS: in IOKit ogni GPU è un servizio
//! "IOAccelerator", e nelle sue "PerformanceStatistics" c'è la percentuale di utilizzo
//! ("Device Utilization %"), sia sui Mac con Apple Silicon sia su quelli Intel/AMD. Non
//! servono permessi speciali. Con più GPU (MacBook Pro Intel con la GPU dedicata) vale
//! la più occupata.

use crate::nerd::LetturaGpu;
use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType};
use std::ffi::{c_char, c_void};
use std::ptr::NonNull;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const c_char) -> *mut c_void;
    /// prende per sé `matching` (non va rilasciato)
    fn IOServiceGetMatchingServices(main_port: u32, matching: *mut c_void, existing: *mut u32) -> i32;
    fn IOIteratorNext(iterator: u32) -> u32;
    fn IORegistryEntryCreateCFProperties(entry: u32, properties: *mut *mut c_void, allocator: *const c_void, options: u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
}

pub struct Gpu;

impl Gpu {
    pub fn new() -> Self {
        Gpu
    }

    pub fn leggi(&mut self) -> LetturaGpu {
        let mut l = LetturaGpu::default();
        let statistiche = CFString::from_static_str("PerformanceStatistics");
        let utilizzo = CFString::from_static_str("Device Utilization %");
        unsafe {
            let m = IOServiceMatching(c"IOAccelerator".as_ptr());
            if m.is_null() {
                return l;
            }
            let mut it = 0u32;
            // 0 = kIOMainPortDefault
            if IOServiceGetMatchingServices(0, m, &mut it) != 0 {
                return l;
            }
            loop {
                let s = IOIteratorNext(it);
                if s == 0 {
                    break;
                }
                let mut p: *mut c_void = std::ptr::null_mut();
                if IORegistryEntryCreateCFProperties(s, &mut p, std::ptr::null(), 0) == 0 {
                    if let Some(p) = NonNull::new(p as *mut CFDictionary<CFString, CFType>) {
                        // SAFETY: IORegistryEntryCreateCFProperties dà un dizionario nuovo
                        // (da rilasciare) con chiavi CFString
                        let d = CFRetained::from_raw(p);
                        let uso = d
                            .get(&statistiche)
                            .and_then(|v| v.downcast::<CFDictionary>().ok())
                            .map(|v| CFRetained::cast_unchecked::<CFDictionary<CFString, CFType>>(v))
                            .and_then(|st| st.get(&utilizzo)?.downcast::<CFNumber>().ok()?.as_i64());
                        if let Some(u) = uso {
                            let u = u.clamp(0, 100) as f32;
                            l.uso = Some(l.uso.map_or(u, |x: f32| x.max(u)));
                        }
                    }
                }
                IOObjectRelease(s);
            }
            IOObjectRelease(it);
        }
        l
    }
}
