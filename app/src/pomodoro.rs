//! Pomodoro: lavoro e pause a tempo (25 + 5 minuti, una pausa lunga ogni 4 pomodori).
//!
//! Lo stato vive qui, nel programma, uno solo per tutti gli schermi: alle pagine va
//! l'istante assoluto in cui finisce la fase (ms dall'epoch), o il tempo rimasto se è in
//! pausa, e ognuna disegna il conto alla rovescia da sola. "Ricarica sfondo" quindi non
//! lo azzera. Il giro periodico (ogni ~2 s) passa alla fase successiva quando il tempo è
//! finito. Tutte le funzioni ricevono l'ora: la logica si prova senza aspettare.

use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fase {
    Lavoro,
    Pausa,
    Lunga,
}

impl Fase {
    pub fn nome(self) -> &'static str {
        match self {
            Fase::Lavoro => "lavoro",
            Fase::Pausa => "pausa",
            Fase::Lunga => "pausa lunga",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stato {
    Fermo,
    /// la fase finisce all'istante `fine` (ms dall'epoch)
    InCorso { fine: u64 },
    /// messo in pausa con `rimasto` ms ancora da fare
    InPausa { rimasto: u64 },
}

/// durate delle fasi in ms; `ogni`: dopo quanti pomodori di lavoro la pausa lunga (0 = mai)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Durate {
    pub lavoro: u64,
    pub pausa: u64,
    pub lunga: u64,
    pub ogni: u32,
}

impl Durate {
    pub fn minuti(lavoro: u32, pausa: u32, lunga: u32, ogni: u32) -> Self {
        let ms = |m: u32| m.clamp(1, 600) as u64 * 60_000;
        Self { lavoro: ms(lavoro), pausa: ms(pausa), lunga: ms(lunga), ogni }
    }
}

/// una fase finita da meno di così parte esattamente dalla fine della precedente; se è
/// passato di più (computer sospeso) la nuova fase parte da adesso
const RITARDO_MAX: u64 = 60_000;

#[derive(Debug)]
pub struct Pomodoro {
    pub fase: Fase,
    pub stato: Stato,
    /// pomodori di lavoro finiti da quando è stato avviato
    pub fatti: u32,
    /// cambi di fase fatti dal giro periodico: la pagina lampeggia quando il numero cresce
    pub scatti: u64,
    durate: Durate,
}

impl Pomodoro {
    pub fn new(durate: Durate) -> Self {
        Self { fase: Fase::Lavoro, stato: Stato::Fermo, fatti: 0, scatti: 0, durate }
    }

    /// le durate nuove valgono dalla prossima fase (o da subito, se è fermo)
    pub fn set_durate(&mut self, durate: Durate) {
        self.durate = durate;
    }

    pub fn durata(&self, fase: Fase) -> u64 {
        match fase {
            Fase::Lavoro => self.durate.lavoro,
            Fase::Pausa => self.durate.pausa,
            Fase::Lunga => self.durate.lunga,
        }
    }

    /// riparte da capo con un pomodoro di lavoro
    pub fn avvia(&mut self, ora: u64) {
        self.fase = Fase::Lavoro;
        self.fatti = 0;
        self.stato = Stato::InCorso { fine: ora + self.durate.lavoro };
    }

    pub fn pausa(&mut self, ora: u64) {
        if let Stato::InCorso { fine } = self.stato {
            self.stato = Stato::InPausa { rimasto: fine.saturating_sub(ora) };
        }
    }

    pub fn riprendi(&mut self, ora: u64) {
        if let Stato::InPausa { rimasto } = self.stato {
            self.stato = Stato::InCorso { fine: ora + rimasto };
        }
    }

    /// la voce che cambia testo: Avvia, Pausa o Riprendi
    pub fn avvia_pausa_riprendi(&mut self, ora: u64) {
        match self.stato {
            Stato::Fermo => self.avvia(ora),
            Stato::InCorso { .. } => self.pausa(ora),
            Stato::InPausa { .. } => self.riprendi(ora),
        }
    }

    /// passa subito alla fase successiva, che parte adesso (anche se era in pausa)
    pub fn salta(&mut self, ora: u64) {
        if self.stato != Stato::Fermo {
            self.avanza(ora);
        }
    }

    pub fn ferma(&mut self) {
        self.stato = Stato::Fermo;
        self.fase = Fase::Lavoro;
        self.fatti = 0;
    }

    /// Dal giro periodico: se la fase è finita passa alla successiva. true se è cambiata.
    pub fn controlla(&mut self, ora: u64) -> bool {
        let Stato::InCorso { fine } = self.stato else { return false };
        if ora < fine {
            return false;
        }
        // il giro arriva fino a 2 s dopo: la fase nuova parte da quando è finita la vecchia,
        // così non si accumula ritardo; dopo una sospensione lunga parte da adesso
        let inizio = if ora - fine <= RITARDO_MAX { fine } else { ora };
        self.avanza(inizio);
        self.scatti += 1;
        true
    }

    fn avanza(&mut self, inizio: u64) {
        self.fase = match self.fase {
            Fase::Lavoro => {
                self.fatti += 1;
                let ogni = self.durate.ogni;
                if ogni > 0 && self.fatti % ogni == 0 {
                    Fase::Lunga
                } else {
                    Fase::Pausa
                }
            }
            Fase::Pausa | Fase::Lunga => Fase::Lavoro,
        };
        self.stato = Stato::InCorso { fine: inizio + self.durata(self.fase) };
    }

    /// ms che mancano alla fine della fase (None se è fermo)
    pub fn rimasto(&self, ora: u64) -> Option<u64> {
        match self.stato {
            Stato::Fermo => None,
            Stato::InCorso { fine } => Some(fine.saturating_sub(ora)),
            Stato::InPausa { rimasto } => Some(rimasto),
        }
    }

    /// per zooSetting('pomodoro', …): null se è fermo
    pub fn json(&self) -> Value {
        let (fine, rimasto) = match self.stato {
            Stato::Fermo => return Value::Null,
            Stato::InCorso { fine } => (Some(fine), None),
            Stato::InPausa { rimasto } => (None, Some(rimasto)),
        };
        json!({
            "fase": match self.fase { Fase::Lavoro => "lavoro", Fase::Pausa => "pausa", Fase::Lunga => "lunga" },
            "fine": fine,
            "rimasto": rimasto,
            "durata": self.durata(self.fase),
            "fatti": self.fatti,
            "ogni": self.durate.ogni,
            "scatti": self.scatti,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const MIN: u64 = 60_000;

    fn nuovo() -> Pomodoro {
        Pomodoro::new(Durate::minuti(25, 5, 15, 4))
    }

    #[test]
    fn avvio_pausa_e_ripresa() {
        let mut p = nuovo();
        assert_eq!(p.rimasto(0), None);
        assert!(p.json().is_null());
        p.avvia_pausa_riprendi(1_000);
        assert_eq!(p.stato, Stato::InCorso { fine: 1_000 + 25 * MIN });
        // in pausa dopo 10 minuti: ne restano 15, anche se passa un'ora
        p.avvia_pausa_riprendi(1_000 + 10 * MIN);
        assert_eq!(p.stato, Stato::InPausa { rimasto: 15 * MIN });
        assert!(!p.controlla(1_000 + 70 * MIN));
        assert_eq!(p.rimasto(1_000 + 70 * MIN), Some(15 * MIN));
        let j = p.json();
        assert_eq!((j["fine"].is_null(), j["rimasto"].as_u64()), (true, Some(15 * MIN)));
        // ripresa: finisce 15 minuti dopo
        p.avvia_pausa_riprendi(2_000_000);
        assert_eq!(p.stato, Stato::InCorso { fine: 2_000_000 + 15 * MIN });
    }

    #[test]
    fn fasi_e_pausa_lunga_ogni_4() {
        let mut p = nuovo();
        p.avvia(0);
        let mut ora = 0;
        let mut fasi = Vec::new();
        for _ in 0..9 {
            let Stato::InCorso { fine } = p.stato else { panic!() };
            assert!(!p.controlla(fine - 1));
            ora = fine + 1_500; // il giro arriva un po' dopo
            assert!(p.controlla(ora));
            fasi.push(p.fase);
        }
        use Fase::*;
        assert_eq!(fasi, [Pausa, Lavoro, Pausa, Lavoro, Pausa, Lavoro, Lunga, Lavoro, Pausa]);
        assert_eq!((p.fatti, p.scatti), (5, 9));
        // senza ritardo accumulato: 5 lavori, 3 pause, 1 lunga e la pausa in corso
        let atteso = 5 * 25 * MIN + 3 * 5 * MIN + 15 * MIN + 5 * MIN;
        assert_eq!(p.stato, Stato::InCorso { fine: atteso });
        assert!(ora < atteso);
    }

    #[test]
    fn senza_pausa_lunga() {
        let mut p = Pomodoro::new(Durate::minuti(25, 5, 15, 0));
        p.avvia(0);
        for _ in 0..10 {
            let Stato::InCorso { fine } = p.stato else { panic!() };
            p.controlla(fine);
            assert_ne!(p.fase, Fase::Lunga);
        }
    }

    #[test]
    fn salta_e_ferma() {
        let mut p = nuovo();
        p.salta(0);
        assert_eq!(p.stato, Stato::Fermo); // fermo: non fa niente
        p.avvia(0);
        p.salta(3 * MIN);
        assert_eq!((p.fase, p.stato, p.fatti), (Fase::Pausa, Stato::InCorso { fine: 8 * MIN }, 1));
        // saltare non è un cambio "da solo": la pagina non lampeggia
        assert_eq!(p.scatti, 0);
        // saltata da in pausa: la fase nuova parte subito
        p.pausa(4 * MIN);
        p.salta(10 * MIN);
        assert_eq!((p.fase, p.stato), (Fase::Lavoro, Stato::InCorso { fine: 35 * MIN }));
        p.ferma();
        assert_eq!((p.fase, p.stato, p.fatti), (Fase::Lavoro, Stato::Fermo, 0));
        assert!(!p.controlla(u64::MAX / 2));
    }

    #[test]
    fn dopo_una_sospensione_lunga_si_avanza_di_una_fase_sola() {
        let mut p = nuovo();
        p.avvia(0);
        // il computer dorme tre ore: alla ripresa una sola fase in più, che parte da adesso
        let sveglia = 3 * 60 * MIN;
        assert!(p.controlla(sveglia));
        assert_eq!((p.fase, p.stato, p.scatti), (Fase::Pausa, Stato::InCorso { fine: sveglia + 5 * MIN }, 1));
        assert!(!p.controlla(sveglia + 2_000));
    }

    #[test]
    fn durate_nuove_dalla_fase_dopo() {
        let mut p = nuovo();
        p.avvia(0);
        p.set_durate(Durate::minuti(50, 10, 30, 4));
        assert_eq!(p.stato, Stato::InCorso { fine: 25 * MIN });
        p.controlla(25 * MIN);
        assert_eq!(p.stato, Stato::InCorso { fine: 35 * MIN });
        // minuti fuori misura dalla config: almeno 1
        assert_eq!(Durate::minuti(0, 0, 0, 4).lavoro, MIN);
    }
}
