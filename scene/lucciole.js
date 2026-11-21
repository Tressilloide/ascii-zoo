// nome: Lucciole
//
// Una scena di esempio per ASCII Zoo: copiala nella cartella delle scene
// (menu "Sfondo" > "Apri la cartella delle scene…"), esci da ASCII Zoo e riaprilo.
//
// La griglia ha w.cols colonne e w.rows righe. w.put(colonna, riga, v, colore, alfa)
// accende una cella: v (0..1) sceglie il carattere, dal più leggero al più pieno;
// colore = this.c0 + indice in `colori`; alfa (0..1) quanto si vede.
// A = quanto è visibile la scena (sale da 0 a 1 quando la si sceglie): moltiplica l'alfa.
// Aiuti già pronti: hash2(i, j) (numero fisso 0..1 per cella), clamp, lerp, TAU (2π),
// celle(w, lista, colore0, A) per disegnare una lista fissa [i, j, v, colore, alfa, ...].

scena({
  colori: ['#2f7a3a', '#1f4d2a', '#d8ff6a', '#9be05a'],
  puntino: '#d8ff6a', // il colore dei due punti dell'orologio

  // una volta, e di nuovo se cambia la dimensione della griglia
  init: function (w) {
    var s = { erba: [], lucciole: [] };
    for (var i = 0; i < w.cols; i++) {
      var alta = Math.round(w.rows * (0.12 + 0.08 * hash2(i, 7)));
      for (var j = w.rows - alta; j < w.rows; j++) {
        if (hash2(i * 3, j) < 0.7) s.erba.push(i, j, 0.3 + 0.3 * hash2(j, i), j < w.rows - alta * 0.5 ? 0 : 1, 0.8);
      }
    }
    var n = clamp(Math.round(w.cols * w.rows / 400), 10, 80);
    for (var k = 0; k < n; k++) {
      s.lucciole.push({ x: w.rnd() * w.cols, y: w.rows * (0.3 + 0.65 * w.rnd()), dir: w.rnd() * TAU, fase: w.rnd() * TAU });
    }
    return s;
  },

  // a ogni fotogramma, prima di disegnare: dt = secondi dall'ultimo, t = secondi in tutto
  tick: function (w, dt, t, s) {
    for (var k = 0; k < s.lucciole.length; k++) {
      var l = s.lucciole[k];
      l.dir += (w.rnd() - 0.5) * 3 * dt;
      l.x += Math.cos(l.dir) * 2 * dt;
      l.y += Math.sin(l.dir) * 1 * dt;
      if (l.x < 0) l.x += w.cols; else if (l.x >= w.cols) l.x -= w.cols;
      l.y = clamp(l.y, w.rows * 0.25, w.rows - 1);
    }
  },

  // disegna un fotogramma
  disegna: function (w, t, A, s) {
    var C = this.c0;
    celle(w, s.erba, C, A);
    for (var k = 0; k < s.lucciole.length; k++) {
      var l = s.lucciole[k], luce = Math.max(0, Math.sin(t * 1.7 + l.fase));
      if (luce < 0.05) continue;
      w.put(Math.floor(l.x), Math.floor(l.y), 0.5 + 0.5 * luce, C + 2, A * luce);
      w.put(Math.floor(l.x) + 1, Math.floor(l.y), 0.2, C + 3, A * luce * 0.5);
      w.put(Math.floor(l.x) - 1, Math.floor(l.y), 0.2, C + 3, A * luce * 0.5);
    }
  }
});
