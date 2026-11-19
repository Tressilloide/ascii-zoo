//! "In riproduzione" su Windows: i controlli multimediali di sistema (quelli del
//! riquadro del volume), GlobalSystemMediaTransportControlsSessionManager. Ci passano
//! Spotify, il lettore multimediale, i browser e le altre app che li usano.
//!
//! Gira sul thread di riproduzione.rs, in un appartamento COM multithread: lì le
//! chiamate asincrone di WinRT si possono aspettare con .get() senza bloccare niente.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Sessione,
    GlobalSystemMediaTransportControlsSessionManager as Gestore,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Stato,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::riproduzione::{Brano, Lettura};

/// ogni quanto al massimo si chiede a Windows
pub const OGNI: Duration = Duration::from_millis(1500);

/// i tempi di WinRT contano in unità di 100 ns; DateTime parte dal 1601
const DAL_1601_AL_1970: i64 = 116_444_736_000_000_000;

pub struct Lettore {
    gestore: Option<Gestore>,
    com: bool,
}

impl Lettore {
    pub fn new() -> Self {
        let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        // Windows 10 1809 o successivo; se non c'è il widget resta vuoto
        let gestore = Gestore::RequestAsync().and_then(|op| op.get()).ok();
        Self { gestore, com }
    }

    /// la sessione che sta suonando; se nessuna suona, quella in pausa
    pub fn leggi(&mut self) -> Lettura {
        let Some(g) = &self.gestore else { return Lettura::Niente };
        let mut scelta: Option<(Sessione, bool)> = None;
        if let Ok(sessioni) = g.GetSessions() {
            for s in &sessioni {
                match s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()) {
                    Ok(Stato::Playing) => {
                        scelta = Some((s, true));
                        break;
                    }
                    Ok(Stato::Paused) if scelta.is_none() => scelta = Some((s, false)),
                    _ => {}
                }
            }
        }
        let Some((s, in_corso)) = scelta else { return Lettura::Niente };
        brano(&s, in_corso).map_or(Lettura::Niente, Lettura::Brano)
    }
}

impl Drop for Lettore {
    fn drop(&mut self) {
        self.gestore = None;
        if self.com {
            unsafe { CoUninitialize() };
        }
    }
}

fn brano(s: &Sessione, in_corso: bool) -> Option<Brano> {
    let p = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()).ok()?;
    let titolo = p.Title().map(|t| t.to_string_lossy()).unwrap_or_default();
    let artista = p.Artist().map(|t| t.to_string_lossy()).unwrap_or_default();
    if titolo.trim().is_empty() && artista.trim().is_empty() {
        return None;
    }
    let (mut posizione, mut durata) = (None, None);
    if let Ok(t) = s.GetTimelineProperties() {
        let inizio = t.StartTime().map(|x| x.Duration).unwrap_or(0);
        let fine = t.EndTime().map(|x| x.Duration).unwrap_or(0);
        if fine > inizio {
            durata = Some(((fine - inizio) / 10_000) as u64);
        }
        if let Ok(pos) = t.Position() {
            let mut p = pos.Duration - inizio;
            // molte app aggiornano la posizione solo quando cambia lo stato: si aggiunge il
            // tempo passato da LastUpdatedTime
            if in_corso {
                if let Ok(agg) = t.LastUpdatedTime() {
                    let ora = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as i64 / 100).unwrap_or(0)
                        + DAL_1601_AL_1970;
                    if agg.UniversalTime > 0 && ora > agg.UniversalTime {
                        p += ora - agg.UniversalTime;
                    }
                }
            }
            if fine > inizio {
                p = p.min(fine - inizio);
            }
            if p >= 0 && (durata.is_some() || p > 0) {
                posizione = Some((p / 10_000) as u64);
            }
        }
    }
    Some(Brano { titolo, artista, in_corso, posizione, durata })
}
