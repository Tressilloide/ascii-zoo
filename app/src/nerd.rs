//! Statistiche per nerd del widget "sistema": CPU per core, temperatura, frequenza,
//! memoria e swap in GB, GPU, velocità della rete, disco di sistema, da quanto è acceso,
//! processi, carico medio.
//!
//! Le legge il crate sysinfo, una parte per volta e solo quelle accese dal menu: gira
//! tutto il giorno ogni 2 secondi, quindi niente "refresh di tutto". Quello che su un
//! sistema non c'è (la temperatura su Windows, il carico medio su Windows) non va alla
//! pagina e la riga non compare.

use serde_json::{json, Map, Value};
use std::time::{Duration, Instant};
use sysinfo::System;

use crate::gpu::Gpu;

/// le voci di "Statistiche per nerd": (id per la pagina e la config, nome nel menu)
pub const VOCI: [(&str, &str); 10] = [
    ("core", "CPU per core"),
    ("temperatura", "Temperatura della CPU"),
    ("frequenza", "Frequenza della CPU"),
    ("memoria", "RAM e swap in GB"),
    ("gpu", "GPU (uso, memoria, temperatura)"),
    ("rete", "Rete (velocità ↓ ↑)"),
    ("disco", "Disco di sistema"),
    ("acceso", "Acceso da"),
    ("processi", "Processi"),
    ("carico", "Carico medio"),
];

/// la voce esiste su questo sistema? Su Windows la temperatura la darebbe solo WMI con i
/// permessi di amministratore, e il carico medio non esiste (sysinfo lo simula)
pub fn disponibile(id: &str) -> bool {
    !(cfg!(windows) && matches!(id, "temperatura" | "carico"))
}

/// il disco cambia piano: lo si rilegge ogni tanto
const OGNI_DISCO: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Nerd {
    /// creato alla prima lettura: finché le statistiche per nerd sono spente non costa niente
    sys: Option<System>,
    rete: Option<Rete>,
    #[cfg(not(windows))]
    sensori: Option<Sensori>,
    disco: Option<(Instant, Option<(u64, u64)>)>,
    gpu: Option<Gpu>,
}

/// Una lettura della GPU (moduli gpu_*.rs): quello che il sistema non dà resta None e la
/// sua parte di riga non compare
#[derive(Default, Debug)]
pub struct LetturaGpu {
    /// percentuale di utilizzo
    pub uso: Option<f32>,
    /// memoria della scheda video: (usata, totale) in byte
    pub vram: Option<(u64, u64)>,
    /// °C
    pub temp: Option<f32>,
}

impl Nerd {
    /// aggiunge a `v` i valori delle voci accese
    pub fn leggi(&mut self, voci: &[String], v: &mut Map<String, Value>) {
        let su = |id: &str| voci.iter().any(|x| x == id) && disponibile(id);
        if !VOCI.iter().any(|(id, _)| su(id)) {
            // tutto spento: si libera la memoria (rientrando si ricomincia da capo)
            *self = Self::default();
            return;
        }
        let sys = self.sys.get_or_insert_with(System::new);
        if su("core") {
            sys.refresh_cpu_usage();
            let core: Vec<u32> = sys.cpus().iter().map(|c| c.cpu_usage().clamp(0.0, 100.0).round() as u32).collect();
            if !core.is_empty() {
                v.insert("core".into(), json!(core));
            }
        }
        if su("frequenza") {
            sys.refresh_cpu_frequency();
            let mhz: Vec<u64> = sys.cpus().iter().map(|c| c.frequency()).filter(|&f| f > 0).collect();
            if !mhz.is_empty() {
                v.insert("freq".into(), json!(mhz.iter().sum::<u64>() / mhz.len() as u64));
            }
        }
        if su("memoria") {
            sys.refresh_memory();
            if sys.total_memory() > 0 {
                v.insert("mem".into(), json!([sys.used_memory(), sys.total_memory()]));
            }
            if sys.total_swap() > 0 {
                v.insert("swap".into(), json!([sys.used_swap(), sys.total_swap()]));
            }
        }
        if su("gpu") {
            let l = self.gpu.get_or_insert_with(Gpu::new).leggi();
            if let Some(u) = l.uso {
                v.insert("gpu".into(), json!(u.round() as u32));
            }
            if let Some((usata, totale)) = l.vram {
                v.insert("vram".into(), json!([usata, totale]));
            }
            if let Some(t) = l.temp {
                v.insert("gputemp".into(), json!((t * 10.0).round() / 10.0));
            }
        } else {
            self.gpu = None;
        }
        if su("rete") {
            match &mut self.rete {
                Some(r) => {
                    if let Some((giu, su)) = r.velocita() {
                        v.insert("rete".into(), json!([giu, su]));
                    }
                }
                // la prima lettura serve solo da punto di partenza
                None => self.rete = Some(Rete::new()),
            }
        } else {
            self.rete = None;
        }
        if su("disco") {
            let ora = Instant::now();
            if self.disco.as_ref().is_none_or(|(quando, _)| ora.duration_since(*quando) >= OGNI_DISCO) {
                self.disco = Some((ora, disco()));
            }
            if let Some((_, Some((usato, totale)))) = self.disco {
                v.insert("disco".into(), json!([usato, totale]));
            }
        }
        if su("acceso") {
            v.insert("acceso".into(), json!(System::uptime()));
        }
        if su("processi") {
            if let Some(n) = processi() {
                v.insert("processi".into(), json!(n));
            }
        }
        #[cfg(not(windows))]
        {
            if su("carico") {
                let c = System::load_average();
                v.insert("carico".into(), json!([c.one, c.five, c.fifteen]));
            }
            if su("temperatura") {
                if let Some(t) = self.sensori.get_or_insert_with(Sensori::new).cpu() {
                    v.insert("temp".into(), json!((t * 10.0).round() / 10.0));
                }
            } else {
                self.sensori = None;
            }
        }
    }
}

/// "ascii-zoo --statistiche": scrive tutte le statistiche per nerd che questo sistema dà
/// (due letture a un secondo l'una dall'altra, perché CPU, rete e GPU sono differenze tra
/// due letture). Serve per capire cosa vede il programma, e alla CI per provarle davvero
/// su Windows e macOS.
pub fn stampa() {
    let voci: Vec<String> = VOCI.iter().map(|(id, _)| id.to_string()).collect();
    let (mut n, mut v) = (Nerd::default(), Map::new());
    n.leggi(&voci, &mut v);
    std::thread::sleep(Duration::from_millis(1100));
    v.clear();
    n.leggi(&voci, &mut v);
    println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
}

/// Velocità della rete: byte ricevuti e inviati tra una lettura e l'altra, contando solo
/// le interfacce "vere": niente loopback, e niente interfacce virtuali (VPN, container,
/// ponti) che conterebbero un'altra volta lo stesso traffico.
struct Rete {
    quando: Instant,
    /// Linux: totali ricevuti e inviati alla lettura precedente
    #[cfg(target_os = "linux")]
    conti: (u64, u64),
    #[cfg(not(target_os = "linux"))]
    reti: sysinfo::Networks,
}

impl Rete {
    fn new() -> Self {
        Self {
            quando: Instant::now(),
            #[cfg(target_os = "linux")]
            conti: contatori_linux(),
            #[cfg(not(target_os = "linux"))]
            reti: sysinfo::Networks::new_with_refreshed_list(),
        }
    }

    /// byte al secondo (giù, su) dall'ultima lettura
    fn velocita(&mut self) -> Option<(u64, u64)> {
        let ora = Instant::now();
        let secondi = ora.duration_since(self.quando).as_secs_f64();
        if secondi < 0.5 {
            return None; // troppo vicino all'ultima: il conto sarebbe solo rumore
        }
        self.quando = ora;
        // dopo una pausa lunga (sfondo nascosto) la media sarebbe su tutta la pausa: si riparte
        let lunga = secondi > 10.0;
        #[cfg(target_os = "linux")]
        let (giu, su) = {
            let conti = contatori_linux();
            // un'interfaccia tolta fa scendere i totali: per quel giro conta zero
            let d = (conti.0.saturating_sub(self.conti.0), conti.1.saturating_sub(self.conti.1));
            self.conti = conti;
            d
        };
        #[cfg(not(target_os = "linux"))]
        let (giu, su) = {
            // true: le interfacce sparite si tolgono, quelle nuove arrivano con il loro primo conto
            self.reti.refresh(true);
            self.reti
                .iter()
                .filter(|(nome, _)| conta_interfaccia(nome))
                .fold((0u64, 0u64), |(g, s), (_, d)| (g + d.received(), s + d.transmitted()))
        };
        if lunga {
            return None;
        }
        Some(((giu as f64 / secondi).round() as u64, (su as f64 / secondi).round() as u64))
    }
}

/// Linux: totali ricevuti e inviati delle schede vere (Ethernet, Wi-Fi, USB: quelle con
/// un dispositivo dietro), da /sys/class/net. sysinfo leggerebbe sette file per ogni
/// interfaccia, comprese tutte quelle dei container.
#[cfg(target_os = "linux")]
fn contatori_linux() -> (u64, u64) {
    let mut tot = (0, 0);
    let Ok(dir) = std::fs::read_dir("/sys/class/net") else { return tot };
    for e in dir.flatten() {
        let p = e.path();
        if e.file_name() == "lo" || !p.join("device").exists() {
            continue;
        }
        let n = |f: &str| -> u64 {
            std::fs::read_to_string(p.join("statistics").join(f)).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
        };
        tot.0 += n("rx_bytes");
        tot.1 += n("tx_bytes");
    }
    tot
}

#[cfg(target_os = "macos")]
fn conta_interfaccia(nome: &str) -> bool {
    // en0, en1…: Wi-Fi ed Ethernet; utun (VPN), awdl (AirDrop), bridge, lo0 no
    nome.starts_with("en")
}
#[cfg(windows)]
fn conta_interfaccia(nome: &str) -> bool {
    // sysinfo tiene già solo le schede vere, senza i filtri che le duplicano
    !nome.contains("Loopback")
}

/// spazio usato e totale del disco di sistema, in byte
#[cfg(not(windows))]
fn disco() -> Option<(u64, u64)> {
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c"/".as_ptr(), &mut s) } != 0 {
        return None;
    }
    let blocco = s.f_frsize as u64;
    let totale = s.f_blocks as u64 * blocco;
    // come df: usato = totale - libero (anche lo spazio riservato a root conta come usato)
    let usato = totale.saturating_sub(s.f_bfree as u64 * blocco);
    (totale > 0).then_some((usato, totale))
}
#[cfg(windows)]
fn disco() -> Option<(u64, u64)> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let unita = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let (mut totale, mut libero) = (0u64, 0u64);
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(format!("{unita}\\")), None, Some(&mut totale), Some(&mut libero)) }
        .ok()?;
    (totale > 0).then_some((totale.saturating_sub(libero), totale))
}

/// quanti processi ci sono (senza leggerli uno per uno)
#[cfg(target_os = "linux")]
fn processi() -> Option<usize> {
    let dir = std::fs::read_dir("/proc").ok()?;
    Some(dir.flatten().filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()))).count())
}
#[cfg(target_os = "macos")]
fn processi() -> Option<usize> {
    // con il buffer vuoto dice quanto spazio serve (con un po' di margine); poi li conta davvero
    let stima = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if stima <= 0 {
        return None;
    }
    let mut pid = vec![0i32; stima as usize + 64];
    let n = unsafe { libc::proc_listallpids(pid.as_mut_ptr().cast(), (pid.len() * 4) as i32) };
    (n > 0).then_some(n as usize)
}
#[cfg(windows)]
fn processi() -> Option<usize> {
    use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    let mut p = PERFORMANCE_INFORMATION::default();
    unsafe { GetPerformanceInfo(&mut p, std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32) }.ok()?;
    Some(p.ProcessCount as usize)
}

/// I sensori di temperatura della CPU. La lista dei sensori si fa una volta sola; poi
/// si rileggono solo quelli della CPU.
#[cfg(not(windows))]
struct Sensori {
    lista: sysinfo::Components,
    cpu: Vec<usize>,
}

#[cfg(not(windows))]
impl Sensori {
    fn new() -> Self {
        let lista = sysinfo::Components::new_with_refreshed_list();
        let cpu = sensori_cpu(lista.iter().map(|c| c.label()));
        Self { lista, cpu }
    }

    /// la temperatura più alta tra i sensori della CPU, in °C
    fn cpu(&mut self) -> Option<f32> {
        let mut max: Option<f32> = None;
        for &i in &self.cpu {
            let Some(c) = self.lista.list_mut().get_mut(i) else { continue };
            c.refresh();
            if let Some(t) = c.temperature().filter(|t| t.is_finite() && *t > 0.0 && *t < 150.0) {
                max = Some(max.map_or(t, |m| m.max(t)));
            }
        }
        max
    }
}

/// Quali sensori sono della CPU, dai nomi. Linux: "coretemp Package id 0" (Intel),
/// "k10temp Tctl" (AMD), "cpu_thermal temp1" (Raspberry Pi e altri ARM); macOS Intel:
/// "PECI CPU", "CPU Proximity"; Apple Silicon: "PMU tdie…", "pACC/eACC MTR Temp Sensor…".
/// Si prende il gruppo più preciso che c'è; dischi, batteria, GPU e scheda madre mai.
#[cfg(not(windows))]
fn sensori_cpu<'a>(etichette: impl Iterator<Item = &'a str>) -> Vec<usize> {
    let etichette: Vec<String> = etichette.map(|s| s.to_lowercase()).collect();
    let gruppi: [&[&str]; 4] = [
        &["coretemp package", "k10temp tctl", "k10temp tdie", "zenpower tdie", "zenpower tctl"],
        &["coretemp core", "cpu_thermal", "cpu-thermal", "soc_thermal", "tdie", "pacc", "eacc"],
        &["k10temp", "zenpower", "peci cpu", "cpu proximity", "cpu die"],
        &["cpu"],
    ];
    let estranei = ["gpu", "nvme", "batt", "wifi", "iwlwifi", "acpitz", "pch", "nand", "ssd", "gas gauge"];
    for gruppo in gruppi {
        let trovati: Vec<usize> = etichette
            .iter()
            .enumerate()
            .filter(|(_, e)| gruppo.iter().any(|g| e.contains(g)) && !estranei.iter().any(|x| e.contains(x)))
            .map(|(i, _)| i)
            .collect();
        if !trovati.is_empty() {
            return trovati;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn sceglie_i_sensori_della_cpu() {
        let intel = ["acpitz temp1", "nvme Composite WD", "coretemp Package id 0", "coretemp Core 0", "coretemp Core 1"];
        assert_eq!(sensori_cpu(intel.into_iter()), vec![2]);
        let amd = ["amdgpu edge", "k10temp Tctl", "k10temp Tccd1", "nvme Composite"];
        assert_eq!(sensori_cpu(amd.into_iter()), vec![1]);
        let mac = ["gas gauge battery", "PMU tdie1", "PMU tdie2", "NAND CH0 temp", "PMU tdev1"];
        assert_eq!(sensori_cpu(mac.into_iter()), vec![1, 2]);
        let pi = ["cpu_thermal temp1", "rp1_adc temp1"];
        assert_eq!(sensori_cpu(pi.into_iter()), vec![0]);
        // solo disco e scheda madre: nessuna temperatura della CPU, la riga non compare
        assert!(sensori_cpu(["acpitz temp1", "nvme Composite"].into_iter()).is_empty());
    }

    #[test]
    fn spento_non_legge_niente() {
        let mut n = Nerd::default();
        let mut v = Map::new();
        n.leggi(&[], &mut v);
        assert!(v.is_empty() && n.sys.is_none());
    }

    /// quanto costa una lettura, con tutto acceso e voce per voce (letture a 0,6 s
    /// l'una dall'altra, come nel programma; il tempo di attesa non conta):
    /// cargo test --release nerd::test::costo -- --ignored --nocapture
    #[cfg(not(windows))]
    #[test]
    #[ignore]
    fn costo() {
        let misura = |voci: &[&str]| {
            let voci: Vec<String> = voci.iter().map(|s| s.to_string()).collect();
            let (mut n, mut v) = (Nerd::default(), Map::new());
            n.leggi(&voci, &mut v);
            let (giri, mut cpu) = (8, 0.0);
            for _ in 0..giri {
                std::thread::sleep(Duration::from_millis(600));
                v.clear();
                let cpu0 = cpu_processo();
                n.leggi(&voci, &mut v);
                cpu += cpu_processo() - cpu0;
            }
            (cpu / giri as f64 * 1000.0, v)
        };
        let tutte: Vec<&str> = VOCI.iter().map(|v| v.0).collect();
        let (ms, v) = misura(&tutte);
        println!("tutto: {ms:.3} ms di CPU a lettura\n{}", serde_json::to_string(&v).unwrap());
        for id in tutte {
            println!("{id}: {:.3} ms", misura(&[id]).0);
        }
    }

    /// secondi di CPU usati finora dal processo
    #[cfg(not(windows))]
    fn cpu_processo() -> f64 {
        let mut u: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) };
        let t = |x: libc::timeval| x.tv_sec as f64 + x.tv_usec as f64 / 1e6;
        t(u.ru_utime) + t(u.ru_stime)
    }
}
