// nome: Neve
//
// Un'altra scena di esempio per ASCII Zoo (la prima, con le spiegazioni, è lucciole.js):
// copiala nella cartella delle scene (menu "Animale" > "Apri la cartella delle scene…"),
// esci da ASCII Zoo e riaprilo.
//
// Nevica su un paesino di notte. I fiocchi vicini sono più grandi e più veloci di quelli
// lontani, il vento cambia piano piano, e la neve si posa: sui tetti e per terra cresce,
// e ogni tanto una raffica ne porta via un po'. Qualche finestra si accende e si spegne.
// Le coordinate di w.put sono intere: Math.floor sulle posizioni.

scena({
  colori: ['#f4f8ff', '#a9b8d4', '#5d6b88', '#dfe8f7', '#8a97b3', '#2b3247', '#ffcf73', '#c98f3c'],
  //        0 fiocco    1 lontano  2 lontanissimo 3 neve  4 neve in ombra 5 case 6 finestra 7 finestra fioca
  puntino: '#dfe8f7',

  init: function (w) {
    var s = { fiocchi: [], case: [], finestre: [], tetti: [], terra: [], vento: 0, ventoVerso: 0 };
    // il paesino: case di altezze diverse lungo il fondo, con il tetto a punta
    var i = 0;
    while (i < w.cols) {
      var larga = 6 + Math.floor(hash2(i, 3) * 9), alta = 3 + Math.floor(hash2(i, 5) * Math.min(7, w.rows * 0.12));
      if (hash2(i, 9) < 0.25) { i += 2 + Math.floor(hash2(i, 11) * 5); continue; } // un po' di spazio
      var c = { x0: i, x1: Math.min(w.cols - 1, i + larga - 1), alta: alta };
      s.case.push(c);
      for (var x = c.x0 + 1; x < c.x1; x += 3) {
        for (var y = 1; y < alta - 1; y += 2) {
          if (hash2(x, y + i) < 0.55) s.finestre.push({ x: x, y: y, casa: c, accesa: hash2(x * 7, y) < 0.6, fra: 5 + 25 * hash2(y, x) });
        }
      }
      i += larga + 1;
    }
    // quanta neve c'è su ogni colonna (in celle, anche frazioni): per terra e sui tetti
    for (var k = 0; k < w.cols; k++) { s.terra.push(0.3 + 0.4 * hash2(k, 1)); s.tetti.push(0); }
    var n = clamp(Math.round(w.cols * w.rows / 50), 60, 900);
    for (var f = 0; f < n; f++) s.fiocchi.push(this.fiocco(w, w.rnd() * w.rows));
    return s;
  },

  // un fiocco nuovo, all'altezza y: piano 0 = vicino, 1 = lontano, 2 = lontanissimo
  fiocco: function (w, y) {
    var piano = w.rnd() < 0.25 ? 0 : (w.rnd() < 0.55 ? 1 : 2);
    return { x: w.rnd() * w.cols, y: y, piano: piano, vy: [4.5, 2.6, 1.4][piano] * (0.8 + 0.4 * w.rnd()), fase: w.rnd() * TAU };
  },

  // quanto sale il tetto della casa c sopra la colonna i (a due falde, poco ripido)
  tetto: function (c, i) {
    return Math.max(0, Math.floor(((c.x1 - c.x0) / 2 - Math.abs(i - (c.x0 + c.x1) / 2)) * 0.45));
  },

  // l'altezza del suolo (riga) sotto la colonna i: la terra o il tetto di una casa
  suolo: function (w, s, i) {
    for (var k = 0; k < s.case.length; k++) {
      var c = s.case[k];
      if (i >= c.x0 && i <= c.x1) return w.rows - 1 - c.alta - this.tetto(c, i) - Math.floor(s.tetti[i]);
    }
    return w.rows - 1 - Math.floor(s.terra[i]);
  },

  tick: function (w, dt, t, s) {
    // il vento: va verso un valore nuovo ogni tanto, con qualche raffica
    if (w.rnd() < dt * 0.08) s.ventoVerso = (w.rnd() - 0.5) * 6;
    s.vento += (s.ventoVerso - s.vento) * Math.min(1, dt * 0.5);
    for (var k = 0; k < s.fiocchi.length; k++) {
      var f = s.fiocchi[k], forza = [1, 0.6, 0.35][f.piano];
      f.y += f.vy * dt;
      f.x += (s.vento * forza + Math.sin(t * 1.3 + f.fase) * 0.8) * dt;
      if (f.x < 0) f.x += w.cols; else if (f.x >= w.cols) f.x -= w.cols;
      var i = Math.floor(f.x);
      // solo i fiocchi vicini si posano; gli altri spariscono dietro le case
      if (f.piano === 0 && f.y >= this.suolo(w, s, i)) {
        var sulTetto = this.suolo(w, s, i) < w.rows - 1 - Math.floor(s.terra[i]);
        if (sulTetto) s.tetti[i] = Math.min(2, s.tetti[i] + 0.12);
        else s.terra[i] = Math.min(w.rows * 0.15, s.terra[i] + 0.06);
        s.fiocchi[k] = this.fiocco(w, -1);
      } else if (f.y >= w.rows) {
        s.fiocchi[k] = this.fiocco(w, -1);
      }
    }
    // la neve si assesta: le colonne vicine si pareggiano, e il vento forte ne porta via
    for (var j = 1; j < w.cols - 1; j++) {
      var media = (s.terra[j - 1] + s.terra[j + 1]) / 2;
      s.terra[j] += (media - s.terra[j]) * Math.min(1, dt * 0.3);
      if (Math.abs(s.vento) > 2.2) s.terra[j] = Math.max(0.3, s.terra[j] - dt * 0.02);
      s.tetti[j] = Math.max(0, s.tetti[j] - dt * (Math.abs(s.vento) > 2.2 ? 0.05 : 0.002));
    }
    // le finestre: ogni tanto una si accende o si spegne
    for (var q = 0; q < s.finestre.length; q++) {
      var fin = s.finestre[q];
      fin.fra -= dt;
      if (fin.fra <= 0) { fin.accesa = !fin.accesa; fin.fra = 8 + w.rnd() * 40; }
    }
  },

  disegna: function (w, t, A, s) {
    var C = this.c0, i, j;
    // le case, scure, con il tetto a due falde
    for (var k = 0; k < s.case.length; k++) {
      var c = s.case[k];
      for (i = c.x0; i <= c.x1; i++) {
        var punta = this.tetto(c, i);
        for (j = w.rows - 1 - c.alta - punta; j < w.rows; j++) w.put(i, j, j < w.rows - c.alta ? 0.9 : 0.75, C + 5, A);
        // la neve sul tetto
        var neve = Math.floor(s.tetti[i] + 0.5);
        for (var h = 1; h <= neve; h++) w.put(i, w.rows - 1 - c.alta - punta - h, 0.7, C + 3, A);
      }
    }
    for (var q = 0; q < s.finestre.length; q++) {
      var fin = s.finestre[q], tremola = 0.85 + 0.15 * Math.sin(t * 3 + fin.x);
      if (fin.accesa) w.put(fin.x, w.rows - 1 - fin.y, 0.9, C + 6, A * tremola, true);
      else w.put(fin.x, w.rows - 1 - fin.y, 0.5, C + 7, A * 0.25, true);
    }
    // la neve per terra: sopra chiara, sotto in ombra
    for (i = 0; i < w.cols; i++) {
      var alto = Math.floor(s.terra[i]);
      for (j = 0; j <= alto; j++) w.put(i, w.rows - 1 - j, j === alto ? 0.55 : 0.8, j === alto ? C + 3 : C + 4, A);
    }
    // i fiocchi: i vicini pieni e chiari, i lontani piccoli e fiochi
    for (var n = 0; n < s.fiocchi.length; n++) {
      var f = s.fiocchi[n];
      w.put(Math.floor(f.x), Math.floor(f.y), [0.6, 0.35, 0.2][f.piano], C + f.piano, A * [1, 0.7, 0.45][f.piano]);
    }
  }
});
