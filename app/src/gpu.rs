//! GPU per le statistiche per nerd, su Windows: gli stessi contatori del Task Manager
//! (PDH). "GPU Engine" dà l'utilizzo di ogni motore della GPU (3D, copia, video…) per
//! ogni processo: si somma per motore e vale il motore più occupato, come nel Task
//! Manager. La memoria dedicata usata viene da "GPU Adapter Memory", quella totale da
//! DXGI. La temperatura Windows non la dà (solo i programmi dei produttori): non c'è.

use crate::nerd::LetturaGpu;
use std::collections::HashMap;
use windows::core::{w, PCWSTR};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_CSTATUS_NEW_DATA,
    PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

pub struct Gpu {
    /// la raccolta dei contatori: (raccolta, utilizzo dei motori, memoria dedicata usata)
    query: Option<(PDH_HQUERY, PDH_HCOUNTER, Option<PDH_HCOUNTER>)>,
    /// la GPU da mostrare (quella con più memoria dedicata): (il suo "luid_0x…_0x…" come
    /// nei nomi dei contatori, memoria dedicata totale)
    scheda: Option<(String, u64)>,
}

impl Gpu {
    pub fn new() -> Self {
        let query = unsafe {
            let mut q = PDH_HQUERY(std::ptr::null_mut());
            let mut uso = PDH_HCOUNTER::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) != 0 {
                None
            } else if PdhAddEnglishCounterW(q, w!("\\GPU Engine(*)\\Utilization Percentage"), 0, &mut uso) != 0 {
                PdhCloseQuery(q);
                None
            } else {
                let mut mem = PDH_HCOUNTER::default();
                let mem = (PdhAddEnglishCounterW(q, w!("\\GPU Adapter Memory(*)\\Dedicated Usage"), 0, &mut mem) == 0).then_some(mem);
                // la prima raccolta serve solo da punto di partenza
                PdhCollectQueryData(q);
                Some((q, uso, mem))
            }
        };
        Self { query, scheda: scheda() }
    }

    pub fn leggi(&mut self) -> LetturaGpu {
        let mut l = LetturaGpu::default();
        let Some((q, uso, mem)) = self.query else { return l };
        let mia = |nome: &str| self.scheda.as_ref().is_none_or(|(luid, _)| nome.contains(luid.as_str()));
        unsafe {
            if PdhCollectQueryData(q) != 0 {
                return l;
            }
            // "pid_1234_luid_0x00000000_0x0000c3c1_phys_0_eng_0_engtype_3d": il motore è
            // tutto quello che viene dopo "luid_"
            let mut motori: HashMap<String, f64> = HashMap::new();
            for (nome, v) in valori(uso) {
                if let Some(i) = nome.find("luid_") {
                    if mia(&nome) {
                        *motori.entry(nome[i..].to_string()).or_default() += v;
                    }
                }
            }
            if !motori.is_empty() {
                l.uso = Some(motori.values().fold(0.0f64, |a, &b| a.max(b)).clamp(0.0, 100.0) as f32);
            }
            if let (Some(mem), Some((_, totale))) = (mem, self.scheda.as_ref()) {
                let usata: f64 = valori(mem).into_iter().filter(|(n, _)| mia(n)).map(|(_, v)| v).sum();
                if *totale > 0 {
                    l.vram = Some((usata.max(0.0) as u64, *totale));
                }
            }
        }
        l
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        if let Some((q, _, _)) = self.query.take() {
            unsafe { PdhCloseQuery(q) };
        }
    }
}

/// (nome dell'istanza in minuscolo, valore) di un contatore con "(*)"
unsafe fn valori(c: PDH_HCOUNTER) -> Vec<(String, f64)> {
    // tra le due chiamate possono comparire istanze nuove: si riprova
    for _ in 0..3 {
        let (mut byte, mut n) = (0u32, 0u32);
        if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut byte, &mut n, None) != PDH_MORE_DATA || byte == 0 {
            return Vec::new();
        }
        // in u64, così il buffer è allineato per gli elementi
        let mut buf: Vec<u64> = vec![0; (byte as usize).div_ceil(8)];
        let elementi = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        match PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut byte, &mut n, Some(elementi)) {
            0 => {
                return std::slice::from_raw_parts(elementi, n as usize)
                    .iter()
                    .filter(|e| matches!(e.FmtValue.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA))
                    .map(|e| (e.szName.to_string().unwrap_or_default().to_lowercase(), e.FmtValue.Anonymous.doubleValue))
                    .collect();
            }
            PDH_MORE_DATA => continue,
            _ => return Vec::new(),
        }
    }
    Vec::new()
}

/// la GPU con più memoria dedicata (la scheda video, se c'è, non quella integrata);
/// niente GPU "software" (Microsoft Basic Render Driver)
fn scheda() -> Option<(String, u64)> {
    unsafe {
        let f: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut scelta: Option<(String, u64)> = None;
        let mut i = 0;
        while let Ok(a) = f.EnumAdapters1(i) {
            i += 1;
            let Ok(d) = a.GetDesc1() else { continue };
            if d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let luid = format!("luid_0x{:08x}_0x{:08x}", d.AdapterLuid.HighPart as u32, d.AdapterLuid.LowPart);
            let mem = d.DedicatedVideoMemory as u64;
            if scelta.as_ref().is_none_or(|(_, m)| mem > *m) {
                scelta = Some((luid, mem));
            }
        }
        scelta
    }
}
