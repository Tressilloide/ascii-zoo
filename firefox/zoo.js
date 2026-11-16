/* ASCII Zoo — motore.
 * Ogni animale è una forma procedurale campionata su una griglia di celle:
 * ogni cella accesa diventa un carattere da tastiera, colorato. */
(function (global) {
  'use strict';

  var TAU = Math.PI * 2;
  var FLOW = -Math.PI / 4; // direzione "di corrente": in alto a destra
  var RAMP_A = '.,:-~;=+*oxX%#&@';
  var RAMP_B = "`'.:_!|/<>aeO08$";
  var LEVELS = 16;

  // Colori comuni a tutte le modalità: 0 = onda del clic, 1 = cibo
  var PALETTE = ['#aab3c5', '#f0d9a0'];

  function clamp(v, a, b) { return v < a ? a : v > b ? b : v; }
  function lerp(a, b, t) { return a + (b - a) * t; }
  function angDiff(a, b) { return Math.atan2(Math.sin(a - b), Math.cos(a - b)); }
  function hash2(i, j) {
    var n = (Math.imul(i, 374761393) + Math.imul(j, 668265263)) | 0;
    n = Math.imul(n ^ (n >>> 13), 1274126177);
    return ((n ^ (n >>> 16)) >>> 0) / 4294967296;
  }
  function steer(c, target, rate, dt) {
    c.th += clamp(angDiff(target, c.th), -rate * dt, rate * dt);
  }
  // distanza punto-segmento; _t = posizione lungo il segmento (0..1)
  var _t = 0;
  function segD(px, py, ax, ay, bx, by) {
    var vx = bx - ax, vy = by - ay;
    var t = ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy);
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    _t = t;
    var dx = px - ax - vx * t, dy = py - ay - vy * t;
    return Math.sqrt(dx * dx + dy * dy);
  }
  function advance(c, dt) {
    c.x += Math.cos(c.th) * c.sp * dt;
    c.y += Math.sin(c.th) * c.sp * dt;
  }
  function defaultPoke(c, x, y, w) {
    var dx = c.x - x, dy = c.y - y, d = Math.sqrt(dx * dx + dy * dy);
    if (d > w.pokeR) return false;
    c.flee = 0.8 + 0.9 * (1 - d / w.pokeR);
    c.fleeTh = d < 0.01 ? w.rnd() * TAU : Math.atan2(dy, dx);
    return true;
  }

  /* ------------------------------------------------------------------ */
  /* Specie. Coordinate locali: x in avanti, y di lato, unità = taglia. */
  /* shape() scrive o.v (intensità 0..1) e o.c (indice colore locale).   */
  /* ------------------------------------------------------------------ */

  var SPECIES = [];

  SPECIES.push({
    id: 'farfalle', label: 'Farfalle', dot: '#5aa7ff',
    pal: ['#eaf4ff', '#a9d6ff', '#58a4ff', '#2f66d0'],
    dens: 560, min: 7, max: 26, smin: 3.6, smax: 11, off: 0, rad: 0.78,
    spawn: function (c, w) {
      c.th = w.rnd() * TAU; c.rate = 8 + w.rnd() * 5; c.dth = 0;
    },
    update: function (c, dt, t, w) {
      var fl = c.flee > 0;
      c.ph += c.rate * (fl ? 1.9 : 1) * dt;
      c.dth += (w.rnd() - 0.5) * 10 * dt;
      c.dth *= Math.pow(0.2, dt);
      if (fl) steer(c, c.fleeTh, 7, dt);
      else c.th += (c.dth + Math.sin(t * 0.9 + c.seed * 9) * 0.6) * dt;
      var base = (2.2 + 3.8 * c.z) * (1 + 0.45 * Math.sin(c.ph));
      c.sp += ((fl ? base * 3.6 : base) - c.sp) * Math.min(1, 5 * dt);
      advance(c, dt);
    },
    shape: function (x, y, c, o) {
      var ay = y < 0 ? -y : y;
      if (ay < Math.max(0.045, c.mt * 0.9) && x > -0.34 && x < 0.36) { o.v = 0.7; o.c = 3; return; }
      var f = 0.22 + 0.78 * Math.abs(Math.cos(c.ph));
      var wy = ay / f;
      var ax = (x - 0.2) / 0.24, bx = (wy - 0.36) / 0.3;
      var d1 = ax * ax + bx * bx;
      ax = (x + 0.24) / 0.17; bx = (wy - 0.24) / 0.19;
      var d2 = ax * ax + bx * bx;
      var d = d1 < d2 ? d1 : d2;
      if (d < 1) {
        o.v = 1 - 0.5 * d;
        o.c = d < 0.3 ? 1 : d < 0.8 ? 2 : 3;
      }
    }
  });

  SPECIES.push({
    id: 'meduse', label: 'Meduse', dot: '#e45bd0',
    pal: ['#ffd9f7', '#ee6fdc', '#b03aa6', '#8d86a6'],
    dens: 720, min: 6, max: 20, smin: 3.6, smax: 11, off: -0.95, rad: 1.5,
    spawn: function (c, w) {
      c.home = FLOW + (w.rnd() - 0.5) * 0.5; c.th = c.home; c.rate = 1.9 + w.rnd() * 0.9;
    },
    update: function (c, dt, t, w) {
      var fl = c.flee > 0;
      c.ph += c.rate * (fl ? 2.2 : 1) * dt;
      if (fl) steer(c, c.fleeTh, 3.5, dt);
      else steer(c, c.home + Math.sin(t * 0.4 + c.seed * 7) * 0.25, 0.8, dt);
      var push = Math.max(0, Math.sin(c.ph));
      var target = (0.9 + 2.6 * c.z) * (0.45 + 1.5 * push) * (fl ? 3 : 1);
      c.sp += (target - c.sp) * Math.min(1, 3 * dt);
      advance(c, dt);
    },
    shape: function (x, y, c, o, h) {
      var p = Math.sin(c.ph);
      if (x >= -0.12) {
        var rx = 0.44 * (1 - 0.12 * p), ry = 0.48 * (1 + 0.14 * p);
        var d = (x / rx) * (x / rx) + (y / ry) * (y / ry);
        if (d < 1) {
          o.v = 1 - 0.45 * d;
          o.c = x < -0.04 ? 2 : (x > 0.12 && d < 0.42) ? 0 : d < 0.8 ? 1 : 2;
        }
        return;
      }
      if (x < -2.3 || h < 0.38) return;
      var spread = 0.16 * (1 + 0.12 * p), fade = 1 + x / 2.3;
      var th = Math.max(0.05, c.mt);
      for (var k = -2; k <= 2; k++) {
        var wig = 0.035 * Math.sin(x * 5 + c.ph * 1.5 + k * 1.7) * -x;
        if (Math.abs(y - k * spread - wig) < th) {
          o.v = 0.05 + 0.13 * fade; o.c = 3; return;
        }
      }
    }
  });

  SPECIES.push({
    id: 'libellule', label: 'Libellule', dot: '#3fe0b0',
    pal: ['#f2fff9', '#62f0b4', '#27b99a', '#8fe6cf', '#3d8f86'],
    dens: 600, min: 7, max: 22, smin: 4.2, smax: 10, off: -0.85, rad: 1.9,
    spawn: function (c, w) {
      c.home = FLOW + (w.rnd() - 0.5) * 0.9; c.th = c.home;
      c.state = w.rnd() < 0.7 ? 1 : 0; c.timer = w.rnd() * 1.5; c.next = c.th;
    },
    update: function (c, dt, t, w) {
      c.ph += 38 * dt;
      c.timer -= dt;
      if (c.flee > 0) { c.state = 1; c.next = c.fleeTh; if (c.timer < c.flee) c.timer = c.flee; }
      if (c.timer <= 0) {
        if (c.state === 1) { c.state = 0; c.timer = 0.35 + w.rnd() * 1.2; }
        else {
          c.state = 1; c.timer = 0.6 + w.rnd() * 1.6;
          c.next = w.rnd() < 0.8 ? c.home + (w.rnd() - 0.5) * 1.7 : w.rnd() * TAU;
        }
      }
      steer(c, c.next, c.state === 1 ? 9 : 3, dt);
      var target = c.state === 1 ? (9 + 13 * c.z) * (c.flee > 0 ? 1.6 : 1) : 0.5;
      c.sp += (target - c.sp) * Math.min(1, (c.state === 1 ? 7 : 4) * dt);
      advance(c, dt);
    },
    shape: function (x, y, c, o, h) {
      var ay = y < 0 ? -y : y, mt = c.mt;
      var hx = x - 0.3;
      if (hx * hx + y * y < Math.max(0.008, mt * mt * 0.5)) { o.v = 1; o.c = 0; return; }
      var tx = (x - 0.14) / 0.16, ty = y / 0.09;
      if (tx * tx + ty * ty < 1) { o.v = 0.92; o.c = 1; return; }
      if (ay < Math.max(0.035, mt)) {
        if (x > -0.95 && x < 0.05) { o.v = 0.62; o.c = 2; return; }
        var tl = 0.3 + c.sp * 0.07;
        if (x <= -0.95 && x > -0.95 - tl && h > 0.45) {
          o.v = 0.04 + 0.16 * (1 + (x + 0.95) / tl); o.c = 4; return;
        }
      }
      if (ay > 0.05 && ay < 0.64) {
        var taper = 1 - (ay / 0.64) * (ay / 0.64);
        var th = Math.max(0.03 + 0.09 * taper, mt * 0.7);
        var fore = Math.abs(x - (0.22 + 0.16 * ay));
        var hind = Math.abs(x - (0.02 - 0.3 * ay));
        if ((fore < th || hind < th) && h > 0.18) {
          o.v = 0.42 + 0.22 * Math.sin(c.ph + ay * 9); o.c = 3;
        }
      }
    }
  });

  SPECIES.push({
    id: 'koi', label: 'Koi', dot: '#ffb347',
    pal: ['#f6f2e9', '#e8402f', '#ff8f2e', '#dc5a16', '#ffd447', '#6d7282', '#c9ced8'],
    dens: 520, min: 7, max: 26, smin: 4.6, smax: 12.5, off: -0.12, rad: 0.78,
    feeds: true,
    spawn: function (c, w) {
      c.home = FLOW + (w.rnd() - 0.5) * 1.1; c.th = c.home; c.dth = 0;
      var r = w.rnd();
      c.kind2 = r < 0.3 ? 0 : r < 0.7 ? 1 : r < 0.82 ? 2 : 3;
    },
    update: function (c, dt, t, w) {
      var base = 2.2 + 3.6 * c.z, target = base;
      var best = 1e9, food = null, i, f;
      for (i = 0; i < w.food.length; i++) {
        f = w.food[i];
        var d2 = (f.x - c.x) * (f.x - c.x) + (f.y - c.y) * (f.y - c.y);
        if (d2 < best) { best = d2; food = f; }
      }
      if (food && best < w.pokeR * w.pokeR * 1.4) {
        steer(c, Math.atan2(food.y - c.y, food.x - c.x), 2.8, dt);
        target = base * 2.3;
        var mx = c.x + Math.cos(c.th) * 0.45 * c.S, my = c.y + Math.sin(c.th) * 0.45 * c.S;
        var reach = Math.max(1.3, c.S * 0.16);
        if ((food.x - mx) * (food.x - mx) + (food.y - my) * (food.y - my) < reach * reach) {
          food.eaten = true;
          w.ripple(food.x, food.y, 2.2 + c.S * 0.2, 0.7, 0);
        }
      } else {
        c.dth += (w.rnd() - 0.5) * 3 * dt;
        c.dth *= Math.pow(0.3, dt);
        c.th += c.dth * dt;
        steer(c, c.home + Math.sin(t * 0.25 + c.seed * 11) * 0.6, 0.35, dt);
      }
      c.sp += (target - c.sp) * Math.min(1, 2.5 * dt);
      c.ph += (3.5 + c.sp * 0.9) * dt;
      advance(c, dt);
    },
    shape: function (x, y, c, o) {
      if (x > 0.5 || x < -0.78) return;
      var s = (0.5 - x) / 0.9; // 0 al muso, 1 al peduncolo
      var amp = 0.015 + 0.15 * Math.pow(clamp(s / 1.4, 0, 1), 1.6);
      var u = y - amp * Math.sin(c.ph - 4.2 * (0.5 - x));
      var au = u < 0 ? -u : u;
      if (x >= -0.4) {
        var wd = 0.17 * (s < 0.3 ? Math.sqrt(s / 0.3) : 1 - 0.74 * Math.pow((s - 0.3) / 0.7, 1.3));
        wd = Math.max(wd, c.mt * 0.8);
        if (au < wd) {
          var q = au / wd;
          o.v = 0.96 - 0.3 * q * q;
          var sd = c.seed * 40;
          var n = Math.sin(x * 7 + sd) * Math.cos(u * 9 + sd * 0.7) + 0.5 * Math.sin(x * 3.3 + sd * 1.9);
          if (c.kind2 === 0) o.c = au < 0.035 ? 3 : 2;
          else if (c.kind2 === 1) o.c = n > 0.12 ? 1 : 0;
          else if (c.kind2 === 2) o.c = q > 0.7 ? 0 : 4;
          else o.c = n > 0.3 ? 1 : n < -0.55 ? 5 : 0;
          return;
        }
        if (x > 0.08 && x < 0.3) {
          var fin = wd + 0.13 * Math.sin(Math.PI * (x - 0.08) / 0.22);
          if (au < fin) { o.v = 0.4; o.c = c.kind2 === 0 ? 2 : c.kind2 === 2 ? 4 : 6; }
        }
        return;
      }
      var s2 = (-0.4 - x) / 0.36;
      if (au < 0.045 + 0.17 * s2 && !(s2 > 0.55 && au < 0.1 * (s2 - 0.55) / 0.45)) {
        o.v = 0.5 - 0.15 * s2; o.c = c.kind2 === 0 ? 2 : c.kind2 === 2 ? 4 : 6;
      }
    }
  });

  SPECIES.push({
    id: 'rane', label: 'Rane', dot: '#6fdc44',
    pal: ['#f6ffe9', '#a2ec6c', '#62cf3c', '#3c9a2c'],
    dens: 620, min: 6, max: 22, smin: 5, smax: 15, off: -0.3, rad: 1.08,
    spawn: function (c, w) {
      c.th = w.rnd() * TAU; c.state = 0; c.timer = w.rnd() * 4; c.air = 0; c.pend = 0; c.u = 0;
    },
    update: function (c, dt, t, w) {
      c.ph += 2.4 * dt;
      if (c.state === 0) {
        c.timer -= dt;
        if (c.timer <= 0) {
          var hop;
          if (c.pend > 0) {
            c.th = c.fleeTh + (w.rnd() - 0.5) * 0.9; hop = c.S * (3.6 + w.rnd() * 2); c.pend--;
          } else {
            c.th += (w.rnd() - 0.5) * 2.6; hop = c.S * (1.6 + w.rnd() * 2.2);
          }
          c.state = 1; c.u = 0; c.hop = hop; c.dur = 0.36 + hop * 0.006;
          c.x0 = c.x; c.y0 = c.y;
        }
      } else {
        c.u += dt / c.dur;
        var u = c.u >= 1 ? 1 : c.u;
        var e = u * u * (3 - 2 * u);
        c.x = c.x0 + Math.cos(c.th) * c.hop * e;
        c.y = c.y0 + Math.sin(c.th) * c.hop * e;
        c.air = Math.sin(Math.PI * u);
        if (c.u >= 1) {
          c.state = 0; c.air = 0;
          c.timer = c.pend > 0 ? 0.05 + w.rnd() * 0.12 : 0.7 + w.rnd() * 4.2;
        }
      }
    },
    poke: function (c, x, y, w) {
      var dx = c.x - x, dy = c.y - y, d = Math.sqrt(dx * dx + dy * dy);
      if (d > w.pokeR) return;
      c.fleeTh = Math.atan2(dy, dx);
      c.pend = d < w.pokeR * 0.5 ? 2 : 1;
      if (c.state === 0) c.timer = Math.min(c.timer, w.rnd() * 0.18);
    },
    shape: function (x, y, c, o) {
      var e = c.air, k = 1 + 0.22 * e;
      x /= k; y /= k;
      var ay = y < 0 ? -y : y, mt = c.mt;
      var ex = x - 0.34, ey = ay - 0.15;
      if (ex * ex + ey * ey < Math.max(0.0058, mt * mt * 0.6)) { o.v = 1; o.c = 0; return; }
      var hx = (x - 0.27) / 0.17, hy = y / 0.2;
      if (hx * hx + hy * hy < 1) { o.v = 0.9; o.c = 1; return; }
      var bx = x / (0.3 + 0.07 * e), by = y / (0.25 - 0.04 * e);
      var d = bx * bx + by * by;
      if (d < 1) {
        o.v = 0.95 - 0.3 * d;
        var sd = c.seed * 30;
        o.c = Math.sin(x * 21 + sd) * Math.cos(y * 19 + sd * 1.3) > 0.45 ? 3 : 2;
        return;
      }
      // zampe posteriori: piegate da ferma, distese in salto
      var kx = lerp(-0.04, -0.56, e), ky = lerp(0.37, 0.21, e);
      var fx = lerp(-0.3, -0.98, e), fy = lerp(0.39, 0.25, e);
      var lt = Math.max(0.07, mt);
      if (segD(x, ay, -0.2, 0.16, kx, ky) < lt || segD(x, ay, kx, ky, fx, fy) < lt * 0.85) {
        o.v = 0.7; o.c = 3; return;
      }
      if (segD(x, ay, fx, fy, fx - 0.1, fy + 0.1) < lt * 0.7) { o.v = 0.55; o.c = 3; return; }
      if (segD(x, ay, 0.14, 0.2, lerp(0.33, 0.08, e), lerp(0.36, 0.31, e)) < Math.max(0.05, mt * 0.9)) {
        o.v = 0.6; o.c = 3;
      }
    }
  });

  SPECIES.push({
    id: 'aironi', label: 'Aironi', dot: '#e6e9f0',
    pal: ['#fbfcff', '#cfd4de', '#858b99', '#f0483e', '#5d6270'],
    dens: 640, min: 6, max: 20, smin: 4.6, smax: 12.5, off: 0, rad: 0.92,
    spawn: function (c, w) {
      c.home = FLOW + (w.rnd() - 0.5) * 0.45; c.th = c.home; c.rate = 2 + w.rnd() * 1.2;
    },
    update: function (c, dt, t, w) {
      var fl = c.flee > 0;
      c.ph += c.rate * (fl ? 2.4 : 1) * dt;
      if (fl) steer(c, c.fleeTh, 2.6, dt);
      else steer(c, c.home + Math.sin(t * 0.3 + c.seed * 5) * 0.18, 0.7, dt);
      var target = (4.5 + 7 * c.z) * (fl ? 2.3 : 1);
      c.sp += (target - c.sp) * Math.min(1, 2.5 * dt);
      advance(c, dt);
    },
    shape: function (x, y, c, o, h) {
      var ay = y < 0 ? -y : y, mt = c.mt;
      if (ay < Math.max(0.03, mt)) {
        if (x > 0.57 && x < 0.69) { o.v = 1; o.c = 3; return; }
        if (x >= 0.69 && x < 0.82) { o.v = 0.45; o.c = 2; return; }
        if (x > 0.12 && x <= 0.57) { o.v = 0.82; o.c = 0; return; }
        if (x < -0.2 && x > -0.78 && h > 0.35) { o.v = 0.3; o.c = 4; return; }
      }
      var bx = x / 0.22, by = y / 0.075;
      if (bx * bx + by * by < 1) { o.v = 0.95; o.c = 0; return; }
      var f = Math.cos(c.ph);
      var span = 0.72 * (0.74 + 0.26 * Math.abs(f));
      var d = segD(x, ay, 0.02, 0.05, -0.12 - 0.13 * f, span);
      var t = _t, chord = Math.max(lerp(0.13, 0.05, t), mt);
      if (d < chord) {
        o.v = 0.92 - 0.3 * t - 0.25 * (d / chord);
        o.c = t > 0.66 ? 2 : (d / chord > 0.6 ? 1 : 0);
      }
    }
  });

  SPECIES.push({
    id: 'granchi', label: 'Granchi', dot: '#ff6a52',
    pal: ['#fff0e6', '#ff9a78', '#ee5540', '#b0342c'],
    dens: 430, min: 8, max: 30, smin: 4.2, smax: 8, off: 0.08, rad: 0.72,
    spawn: function (c, w) {
      c.th = w.rnd() * TAU; c.thT = c.th; c.dir = w.rnd() < 0.5 ? -1 : 1;
      c.state = w.rnd() < 0.6 ? 1 : 0; c.timer = w.rnd() * 2.5; c.leg = w.rnd() * 6; c.claw = w.rnd() * 6;
    },
    update: function (c, dt, t, w) {
      var fl = c.flee > 0;
      c.timer -= dt;
      if (fl) { c.state = 1; if (c.timer < c.flee) c.timer = c.flee; }
      if (c.timer <= 0) {
        if (c.state === 1) { c.state = 0; c.timer = 0.4 + w.rnd() * 1.8; }
        else {
          c.state = 1; c.timer = 0.9 + w.rnd() * 2.8;
          if (w.rnd() < 0.5) c.dir = -c.dir;
          c.thT = c.th + (w.rnd() - 0.5) * 1.5;
        }
      }
      steer(c, c.thT, fl ? 6 : 1.6, dt);
      var target = c.state === 1 ? (2.4 + 3.4 * c.z) * (fl ? 3.4 : 1) : 0;
      c.sp += (target - c.sp) * Math.min(1, 8 * dt);
      c.leg += (4 + c.sp * 3.2) * dt * (c.state === 1 ? 1 : 0.1);
      c.claw += (c.state === 1 ? 1.5 : 5) * dt;
      // i granchi camminano di lato
      c.x += -Math.sin(c.th) * c.dir * c.sp * dt;
      c.y += Math.cos(c.th) * c.dir * c.sp * dt;
    },
    poke: function (c, x, y, w) {
      if (!defaultPoke(c, x, y, w)) return;
      var a = c.fleeTh;
      var dot = -Math.sin(c.th) * Math.cos(a) + Math.cos(c.th) * Math.sin(a);
      c.dir = dot >= 0 ? 1 : -1;
      c.thT = a - c.dir * Math.PI / 2;
    },
    shape: function (x, y, c, o) {
      var ay = y < 0 ? -y : y, sg = y < 0 ? -1 : 1, mt = c.mt;
      var ex = x - 0.25, ey = ay - 0.1;
      if (ex * ex + ey * ey < 0.0026) { o.v = 1; o.c = 0; return; }
      var bx = x / 0.2, by = y / 0.31;
      var d = bx * bx + by * by;
      if (d < 1) { o.v = 0.96 - 0.3 * d; o.c = d < 0.25 ? 1 : d < 0.78 ? 2 : 3; return; }
      var open = 0.5 + 0.5 * Math.sin(c.claw + sg);
      var cy = 0.31 - 0.03 * open;
      var qx = (x - 0.43) / 0.13, qy = (ay - cy) / 0.1;
      if (qx * qx + qy * qy < 1) {
        if (x > 0.42 && Math.abs(ay - cy) < 0.035 * open) return;
        o.v = 0.9; o.c = 1; return;
      }
      if (segD(x, ay, 0.12, 0.25, 0.34, 0.35) < Math.max(0.05, mt * 0.8)) { o.v = 0.75; o.c = 2; return; }
      var lt = Math.max(0.035, mt * 0.75);
      for (var k = 0; k < 3; k++) {
        var rx = -0.13 + k * 0.1;
        var tip = rx * 1.9 - 0.05 + 0.07 * Math.sin(c.leg + k * 2.1 + (sg > 0 ? 0 : Math.PI));
        if (segD(x, ay, rx, 0.25, tip, 0.56) < lt) { o.v = 0.6; o.c = 3; return; }
      }
    }
  });

  SPECIES.forEach(function (sp) {
    sp.c0 = PALETTE.length;
    Array.prototype.push.apply(PALETTE, sp.pal);
  });

  /* ------------------------------------------------------------------ */

  function hexRgb(hex) {
    var n = parseInt(hex.slice(1), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  }

  function Zoo(canvas, glow, opts) {
    opts = opts || {};
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.glow = glow || null;
    this.gctx = glow ? glow.getContext('2d') : null;
    this.font = opts.font || '"IBM Plex Mono", ui-monospace, Menlo, Consolas, monospace';
    this.speed = opts.speed == null ? 1 : opts.speed;
    this.fps = opts.fps || 0;
    this.creatures = [];
    this.food = [];
    this.ripples = [];
    this.species = null;
    this.t = 0;
    this.frameNo = 0;
    this.last = 0;
    this.out = { v: 0, c: 0 };
    this.rgb = PALETTE.map(hexRgb);
    this._seed = (Date.now() ^ 0x9e3779b9) >>> 0;
    this.running = false;
    this._tick = this._tick.bind(this);
    this.resize();
  }

  Zoo.SPECIES = SPECIES;

  Zoo.prototype.rnd = function () {
    // mulberry32
    var t = (this._seed += 0x6d2b79f5) >>> 0;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };

  Zoo.prototype.resize = function (w, h) {
    w = w || global.innerWidth || 1280;
    h = h || global.innerHeight || 720;
    var dpr = Math.min(global.devicePixelRatio || 1, 2);
    var cell = clamp(Math.round(w / 128), 8, 14);
    var tile = Math.max(6, Math.round(cell * dpr));
    var oldCols = this.cols, oldRows = this.rows;
    this.cell = tile / dpr;
    this.tile = tile;
    this.cols = Math.ceil(w / this.cell);
    this.rows = Math.ceil(h / this.cell);
    this.canvas.width = this.cols * tile;
    this.canvas.height = this.rows * tile;
    this.canvas.style.width = this.cols * this.cell + 'px';
    this.canvas.style.height = this.rows * this.cell + 'px';
    var n = this.cols * this.rows;
    this.bv = new Float32Array(n);
    this.ba = new Float32Array(n);
    this.bw = new Float32Array(n);
    this.bc = new Uint8Array(n);
    this.stamp = new Int32Array(n).fill(-1);
    this.touched = new Int32Array(n);
    this.nt = 0;
    this.pokeR = Math.max(this.cols, this.rows) * 0.45;
    if (this.glow) {
      this.glow.width = this.cols;
      this.glow.height = this.rows;
      this.glow.style.width = this.cols * this.cell + 'px';
      this.glow.style.height = this.rows * this.cell + 'px';
      this.gimg = this.gctx.createImageData(this.cols, this.rows);
    }
    this.buildAtlas();
    if (oldCols && this.creatures.length) {
      var sx = this.cols / oldCols, sy = this.rows / oldRows;
      this.creatures.forEach(function (c) { c.x *= sx; c.y *= sy; });
      this.food.length = 0;
      this.ripples.length = 0;
      this.populate(true);
    }
    return this.cell;
  };

  Zoo.prototype.buildAtlas = function () {
    var tile = this.tile, a = document.createElement('canvas');
    a.width = LEVELS * 2 * tile;
    a.height = PALETTE.length * tile;
    var g = a.getContext('2d');
    var gap = Math.max(1, Math.round(tile * 0.07));
    g.font = '500 ' + Math.round(tile * 0.95) + 'px ' + this.font;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    for (var ci = 0; ci < PALETTE.length; ci++) {
      var rgb = this.rgb[ci];
      var bright = 'rgb(' + Math.round(lerp(rgb[0], 255, 0.34)) + ',' +
        Math.round(lerp(rgb[1], 255, 0.34)) + ',' + Math.round(lerp(rgb[2], 255, 0.34)) + ')';
      for (var r = 0; r < 2; r++) {
        var ramp = r ? RAMP_B : RAMP_A;
        for (var l = 0; l < LEVELS; l++) {
          var x = (r * LEVELS + l) * tile, y = ci * tile;
          var fill = 0.07 + 0.68 * Math.pow(l / (LEVELS - 1), 1.4);
          g.fillStyle = 'rgba(' + rgb[0] + ',' + rgb[1] + ',' + rgb[2] + ',' + fill.toFixed(3) + ')';
          g.fillRect(x + gap, y + gap, tile - gap * 2, tile - gap * 2);
          g.fillStyle = bright;
          g.fillText(ramp.charAt(l), x + tile / 2, y + tile / 2 + tile * 0.03);
        }
      }
    }
    this.atlas = a;
  };

  Zoo.prototype.target = function (sp) {
    return clamp(Math.round(this.cols * this.rows / sp.dens), sp.min, sp.max);
  };

  Zoo.prototype.make = function (sp, delay) {
    var u = Math.pow(this.rnd(), 1.7);
    var c = {
      kind: sp, seed: this.rnd(), z: 0.3 + 0.7 * u, S: lerp(sp.smin, sp.smax, u),
      x: this.rnd() * this.cols, y: this.rnd() * this.rows,
      th: 0, sp: 0, ph: this.rnd() * TAU, a: -delay, dying: false,
      flee: 0, fleeTh: 0, mt: 0
    };
    c.bright = 0.42 + 0.58 * c.z;
    sp.spawn(c, this);
    return c;
  };

  // porta il numero di animali vivi al valore giusto per lo schermo
  Zoo.prototype.populate = function (quick) {
    var sp = this.species;
    if (!sp) return;
    var alive = this.creatures.filter(function (c) { return c.kind === sp && !c.dying; });
    var want = this.target(sp), i;
    for (i = alive.length; i < want; i++) this.creatures.push(this.make(sp, quick ? 0 : 0.25 + this.rnd() * 0.5));
    for (i = want; i < alive.length; i++) alive[i].dying = true;
    this.creatures.sort(function (a, b) { return a.z - b.z; });
  };

  Zoo.prototype.setMode = function (id) {
    var sp = null;
    for (var i = 0; i < SPECIES.length; i++) if (SPECIES[i].id === id) sp = SPECIES[i];
    if (!sp || sp === this.species) return !!sp;
    this.creatures.forEach(function (c) { c.dying = true; });
    this.food.forEach(function (f) { f.life = Math.min(f.life, f.age + 0.4); });
    this.species = sp;
    this.populate(false);
    return true;
  };

  Zoo.prototype.ripple = function (x, y, r, life, color) {
    this.ripples.push({ x: x, y: y, r: r, life: life, age: 0, c: color });
  };

  // clic in pixel CSS relativi al canvas
  Zoo.prototype.poke = function (px, py) {
    var x = px / this.cell, y = py / this.cell, sp = this.species, self = this;
    if (!sp) return;
    if (sp.feeds) {
      var n = 5 + Math.floor(this.rnd() * 3);
      for (var i = 0; i < n; i++) {
        var a = this.rnd() * TAU, r = this.rnd() * 3.2;
        this.food.push({
          x: x + Math.cos(a) * r, y: y + Math.sin(a) * r,
          vx: Math.cos(a) * 0.5, vy: Math.sin(a) * 0.5, age: 0, life: 14 + this.rnd() * 4, eaten: false
        });
      }
      if (this.food.length > 90) this.food.splice(0, this.food.length - 90);
      this.ripple(x, y, 6, 0.9, 1);
      return;
    }
    this.ripple(x, y, 9, 0.7, 0);
    this.creatures.forEach(function (c) {
      if (c.dying || c.kind !== sp) return;
      (sp.poke || defaultPoke)(c, x, y, self);
    });
  };

  Zoo.prototype.put = function (i, j, v, col, a) {
    if (i < 0 || j < 0 || i >= this.cols || j >= this.rows) return;
    var k = j * this.cols + i, w = v * a;
    if (this.stamp[k] !== this.frameNo) { this.stamp[k] = this.frameNo; this.touched[this.nt++] = k; }
    else if (w < this.bw[k] * 0.85) return;
    this.bv[k] = v; this.bc[k] = col; this.ba[k] = a; this.bw[k] = w;
  };

  Zoo.prototype.step = function (dt) {
    var t = (this.t += dt), cs = this.creatures, i, c, sp, m;
    for (i = cs.length - 1; i >= 0; i--) {
      c = cs[i]; sp = c.kind;
      if (c.dying) { c.a -= 2.6 * dt; if (c.a <= 0) { cs.splice(i, 1); continue; } }
      else if (c.a < 1) c.a = Math.min(1, c.a + 1.8 * dt);
      if (c.flee > 0) c.flee -= dt;
      sp.update(c, dt, t, this);
      m = (sp.rad + Math.abs(sp.off)) * c.S + 1;
      if (c.x < -m) c.x = this.cols + m; else if (c.x > this.cols + m) c.x = -m;
      if (c.y < -m) c.y = this.rows + m; else if (c.y > this.rows + m) c.y = -m;
    }
    for (i = this.food.length - 1; i >= 0; i--) {
      var f = this.food[i];
      f.age += dt;
      f.x += f.vx * dt; f.y += f.vy * dt;
      f.vx *= Math.pow(0.3, dt); f.vy *= Math.pow(0.3, dt);
      if (f.eaten || f.age > f.life) this.food.splice(i, 1);
    }
    for (i = this.ripples.length - 1; i >= 0; i--) {
      this.ripples[i].age += dt;
      if (this.ripples[i].age > this.ripples[i].life) this.ripples.splice(i, 1);
    }
  };

  Zoo.prototype.drawCreature = function (c) {
    if (c.a <= 0) return;
    var sp = c.kind, S = c.S, cs = Math.cos(c.th), sn = Math.sin(c.th);
    var bx = c.x + cs * sp.off * S, by = c.y + sn * sp.off * S, R = sp.rad * S + 1;
    var i0 = Math.max(0, Math.floor(bx - R)), i1 = Math.min(this.cols - 1, Math.ceil(bx + R));
    var j0 = Math.max(0, Math.floor(by - R)), j1 = Math.min(this.rows - 1, Math.ceil(by + R));
    var a = c.a * c.bright, o = this.out, c0 = sp.c0, inv = 1 / S;
    c.mt = 0.52 * inv;
    for (var j = j0; j <= j1; j++) {
      var dy = j + 0.5 - c.y;
      for (var i = i0; i <= i1; i++) {
        var dx = i + 0.5 - c.x;
        o.v = 0;
        sp.shape((dx * cs + dy * sn) * inv, (dy * cs - dx * sn) * inv, c, o, hash2(i, j));
        if (o.v > 0) this.put(i, j, o.v, c0 + o.c, a);
      }
    }
  };

  Zoo.prototype.render = function () {
    var ctx = this.ctx, tile = this.tile, cols = this.cols, i, k;
    this.frameNo++;
    this.nt = 0;

    for (i = 0; i < this.ripples.length; i++) {
      var r = this.ripples[i], u = r.age / r.life;
      var rad = r.r * (1 - (1 - u) * (1 - u)), n = Math.ceil(TAU * rad * 1.3) + 4;
      for (k = 0; k < n; k++) {
        var ang = k / n * TAU;
        var ci = Math.floor(r.x + Math.cos(ang) * rad), cj = Math.floor(r.y + Math.sin(ang) * rad);
        if (hash2(ci, cj) > 0.3) this.put(ci, cj, 0.14, r.c, (1 - u) * 0.9);
      }
    }
    for (i = 0; i < this.creatures.length; i++) this.drawCreature(this.creatures[i]);
    for (i = 0; i < this.food.length; i++) {
      var f = this.food[i];
      var fa = Math.min(1, f.age * 5) * clamp((f.life - f.age) / 1.5, 0, 1);
      this.put(Math.floor(f.x), Math.floor(f.y), 0.6, 1, fa);
    }

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    var gd = this.gimg ? this.gimg.data : null;
    if (gd) gd.fill(0);
    var tw = Math.floor(this.t * 1.3);
    for (var q = 0; q < this.nt; q++) {
      k = this.touched[q];
      var ci2 = k % cols, cj2 = (k / cols) | 0;
      var h = hash2(ci2, cj2), v = this.bv[k], al = this.ba[k], col = this.bc[k];
      var lvl = clamp(Math.round(v * (LEVELS - 1) + (h - 0.5) * 2.2), 0, LEVELS - 1);
      var alt = hash2(ci2 + tw + ((h * 31) | 0), cj2 - tw) > 0.7 ? 1 : 0;
      if (col === 1) { lvl = 9; alt = 0; } // il cibo è sempre una 'o'
      ctx.globalAlpha = al > 1 ? 1 : al;
      ctx.drawImage(this.atlas, (alt * LEVELS + lvl) * tile, col * tile, tile, tile, ci2 * tile, cj2 * tile, tile, tile);
      if (gd) {
        var rgb = this.rgb[col], p = k * 4;
        gd[p] = rgb[0]; gd[p + 1] = rgb[1]; gd[p + 2] = rgb[2];
        gd[p + 3] = Math.round(255 * clamp(al * v * v, 0, 1));
      }
    }
    ctx.globalAlpha = 1;
    if (gd) this.gctx.putImageData(this.gimg, 0, 0);
  };

  Zoo.prototype._tick = function (now) {
    if (!this.running) return;
    global.requestAnimationFrame(this._tick);
    if (this.fps && now - this.last < 1000 / this.fps - 3) return;
    var dt = this.last ? (now - this.last) / 1000 : 0.016;
    this.last = now;
    this.step(Math.min(dt, 0.05) * this.speed);
    this.render();
  };

  Zoo.prototype.start = function () {
    if (this.running) return;
    this.running = true;
    this.last = 0;
    global.requestAnimationFrame(this._tick);
  };

  Zoo.prototype.stop = function () { this.running = false; };

  global.AsciiZoo = Zoo;
})(typeof window !== 'undefined' ? window : this);
