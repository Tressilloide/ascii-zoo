//! GPU per le statistiche per nerd, su Linux: dai file del kernel in /sys/class/drm,
//! senza permessi speciali.
//! - AMD (amdgpu): gpu_busy_percent, mem_info_vram_used/total, temperatura da hwmon
//! - Intel (i915): quanto tempo la GPU è stata a riposo (RC6) tra una lettura e l'altra
//! - Intel (xe): lo stesso, da gtidle/idle_residency_ms
//! - NVIDIA (driver proprietario): la sua libreria NVML (libnvidia-ml.so.1), caricata solo
//!   se c'è; niente nvidia-smi lanciato ogni 2 secondi
//!
//! Una GPU che dorme (la GPU dedicata dei portatili, quando non serve) non si sveglia per
//! leggerla: vale 0%.

use crate::nerd::LetturaGpu;
use std::ffi::{c_char, c_void, CString};
use std::path::{Path, PathBuf};
use std::time::Instant;

enum Tipo {
    Amd,
    /// il file con i ms a riposo, e la lettura precedente (ms a riposo, quando)
    Intel { riposo: PathBuf, prima: Option<(u64, Instant)> },
    /// `sempre`: la libreria resta aperta. Non sui portatili dove la GPU può dormire
    /// (power/control = auto): finché la libreria è aperta la GPU resterebbe sveglia, e
    /// allora la si apre e la si chiude a ogni lettura, solo se la GPU è già sveglia.
    Nvidia { bus: String, sempre: bool, nvml: Option<Nvml> },
}

struct Scheda {
    /// la cartella del dispositivo PCI
    dev: PathBuf,
    tipo: Tipo,
}

pub struct Gpu {
    scheda: Option<Scheda>,
}

fn leggi_num(p: &Path) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

fn dorme(dev: &Path) -> bool {
    std::fs::read_to_string(dev.join("power/runtime_status")).is_ok_and(|s| s.trim() == "suspended")
}

impl Gpu {
    pub fn new() -> Self {
        Self { scheda: trova() }
    }

    pub fn leggi(&mut self) -> LetturaGpu {
        let mut l = LetturaGpu::default();
        let Some(s) = self.scheda.as_mut() else { return l };
        if dorme(&s.dev) {
            if let Tipo::Nvidia { sempre: false, nvml, .. } = &mut s.tipo {
                *nvml = None;
            }
            if let Tipo::Intel { prima, .. } = &mut s.tipo {
                *prima = None;
            }
            l.uso = Some(0.0);
            return l;
        }
        match &mut s.tipo {
            Tipo::Amd => {
                l.uso = leggi_num(&s.dev.join("gpu_busy_percent")).map(|v| v as f32);
                if let (Some(u), Some(t)) = (leggi_num(&s.dev.join("mem_info_vram_used")), leggi_num(&s.dev.join("mem_info_vram_total"))) {
                    if t > 0 {
                        l.vram = Some((u, t));
                    }
                }
                l.temp = temperatura(&s.dev);
            }
            Tipo::Intel { riposo, prima } => {
                if let Some(ms) = leggi_num(riposo) {
                    let ora = Instant::now();
                    if let Some((ms0, t0)) = *prima {
                        let passati = ora.duration_since(t0).as_secs_f64() * 1000.0;
                        if passati > 100.0 && ms >= ms0 {
                            l.uso = Some((100.0 - (ms - ms0) as f64 * 100.0 / passati).clamp(0.0, 100.0) as f32);
                        }
                    }
                    *prima = Some((ms, ora));
                }
                l.temp = temperatura(&s.dev);
            }
            Tipo::Nvidia { bus, sempre, nvml } => {
                if nvml.is_none() {
                    *nvml = Nvml::apri(bus);
                }
                if let Some(n) = nvml.as_ref() {
                    n.leggi(&mut l);
                }
                if !*sempre {
                    *nvml = None;
                }
            }
        }
        l
    }
}

/// La GPU da mostrare: NVIDIA, poi AMD, poi Intel (le dedicate prima delle integrate).
fn trova() -> Option<Scheda> {
    let mut trovate: Vec<(u8, Scheda)> = Vec::new();
    for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
        let nome = e.file_name().to_string_lossy().into_owned();
        // card0, card1…; non card1-HDMI-A-1 (le uscite) né renderD128
        if !nome.strip_prefix("card").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
            continue;
        }
        let card = e.path();
        let Ok(dev) = std::fs::canonicalize(card.join("device")) else { continue };
        let driver = std::fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let scheda = match driver.as_str() {
            "nvidia" => {
                let bus = dev.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let sempre = std::fs::read_to_string(dev.join("power/control")).map_or(true, |c| c.trim() != "auto");
                (0, Tipo::Nvidia { bus, sempre, nvml: None })
            }
            "amdgpu" => (1, Tipo::Amd),
            "i915" => {
                // RC6 spento (rc6_enable = 0): il conto del riposo non dice niente
                let base = [card.join("gt/gt0"), card.join("power")].into_iter().find(|b| b.join("rc6_residency_ms").exists());
                let Some(base) = base else { continue };
                if leggi_num(&base.join("rc6_enable")).or_else(|| leggi_num(&card.join("power/rc6_enable"))) == Some(0) {
                    continue;
                }
                (2, Tipo::Intel { riposo: base.join("rc6_residency_ms"), prima: None })
            }
            "xe" => {
                let riposo = dev.join("tile0/gt0/gtidle/idle_residency_ms");
                if !riposo.exists() {
                    continue;
                }
                (2, Tipo::Intel { riposo, prima: None })
            }
            _ => continue,
        };
        trovate.push((scheda.0, Scheda { dev, tipo: scheda.1 }));
    }
    trovate.sort_by_key(|(p, _)| *p);
    trovate.into_iter().next().map(|(_, s)| s)
}

/// temperatura del sensore della GPU (hwmon del dispositivo): "edge" se c'è, se no il primo
fn temperatura(dev: &Path) -> Option<f32> {
    let hw = std::fs::read_dir(dev.join("hwmon")).ok()?.flatten().next()?.path();
    let mut scelto = None;
    for i in 1..=4 {
        let input = hw.join(format!("temp{i}_input"));
        if !input.exists() {
            continue;
        }
        let etichetta = std::fs::read_to_string(hw.join(format!("temp{i}_label"))).unwrap_or_default();
        if scelto.is_none() || etichetta.trim() == "edge" {
            scelto = Some(input);
        }
    }
    let t = leggi_num(&scelto?)? as f32 / 1000.0;
    (t > 0.0 && t < 150.0).then_some(t)
}

// ---------------------------------------------------------------- NVML (NVIDIA)

#[repr(C)]
#[derive(Default)]
struct Utilizzo {
    gpu: u32,
    memoria: u32,
}

#[repr(C)]
#[derive(Default)]
struct Memoria {
    totale: u64,
    libera: u64,
    usata: u64,
}

type FnInit = unsafe extern "C" fn() -> i32;
type FnPerBus = unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32;
type FnUtilizzo = unsafe extern "C" fn(*mut c_void, *mut Utilizzo) -> i32;
type FnMemoria = unsafe extern "C" fn(*mut c_void, *mut Memoria) -> i32;
type FnTemp = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32;

/// libnvidia-ml.so.1 aperta e inizializzata, con il dispositivo da leggere
struct Nvml {
    lib: *mut c_void,
    dev: *mut c_void,
    utilizzo: FnUtilizzo,
    memoria: FnMemoria,
    temp: FnTemp,
    chiudi: FnInit,
}

impl Nvml {
    fn apri(bus: &str) -> Option<Self> {
        unsafe {
            let lib = libc::dlopen(c"libnvidia-ml.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if lib.is_null() {
                return None;
            }
            let simbolo = |nome: &std::ffi::CStr| {
                let p = libc::dlsym(lib, nome.as_ptr());
                (!p.is_null()).then_some(p)
            };
            let tutto = (|| {
                Some((
                    simbolo(c"nvmlInit_v2")?,
                    simbolo(c"nvmlDeviceGetHandleByPciBusId_v2")?,
                    simbolo(c"nvmlDeviceGetUtilizationRates")?,
                    simbolo(c"nvmlDeviceGetMemoryInfo")?,
                    simbolo(c"nvmlDeviceGetTemperature")?,
                    simbolo(c"nvmlShutdown")?,
                ))
            })();
            let Some((init, per_bus, utilizzo, memoria, temp, chiudi)) = tutto else {
                libc::dlclose(lib);
                return None;
            };
            // SAFETY: i tipi sono quelli di nvml.h per queste funzioni
            let init: FnInit = std::mem::transmute::<*mut c_void, FnInit>(init);
            let per_bus: FnPerBus = std::mem::transmute::<*mut c_void, FnPerBus>(per_bus);
            let chiudi: FnInit = std::mem::transmute::<*mut c_void, FnInit>(chiudi);
            if init() != 0 {
                libc::dlclose(lib);
                return None;
            }
            let mut dev = std::ptr::null_mut();
            let bus = CString::new(bus).ok()?;
            if per_bus(bus.as_ptr(), &mut dev) != 0 || dev.is_null() {
                chiudi();
                libc::dlclose(lib);
                return None;
            }
            Some(Self {
                lib,
                dev,
                utilizzo: std::mem::transmute::<*mut c_void, FnUtilizzo>(utilizzo),
                memoria: std::mem::transmute::<*mut c_void, FnMemoria>(memoria),
                temp: std::mem::transmute::<*mut c_void, FnTemp>(temp),
                chiudi,
            })
        }
    }

    fn leggi(&self, l: &mut LetturaGpu) {
        unsafe {
            let mut u = Utilizzo::default();
            if (self.utilizzo)(self.dev, &mut u) == 0 {
                l.uso = Some(u.gpu.min(100) as f32);
            }
            let mut m = Memoria::default();
            if (self.memoria)(self.dev, &mut m) == 0 && m.totale > 0 {
                l.vram = Some((m.usata, m.totale));
            }
            // 0 = NVML_TEMPERATURE_GPU
            let mut t = 0u32;
            if (self.temp)(self.dev, 0, &mut t) == 0 && t > 0 && t < 150 {
                l.temp = Some(t as f32);
            }
        }
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        unsafe {
            (self.chiudi)();
            libc::dlclose(self.lib);
        }
    }
}
