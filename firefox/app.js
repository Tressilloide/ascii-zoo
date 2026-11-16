/* ASCII Zoo — interfaccia: selettore, saluto, ricerca, impostazioni. */
(function () {
  'use strict';

  var Zoo = window.AsciiZoo, SPECIES = Zoo.SPECIES;
  var root = document.documentElement;
  var wallpaper = root.hasAttribute('data-sfondo');
  var KEY = 'asciizoo.v1';
  var $ = function (id) { return document.getElementById(id); };

  var saved = {};
  try { saved = JSON.parse(localStorage.getItem(KEY)) || {}; } catch (e) { saved = {}; }
  var state = {
    mode: typeof saved.mode === 'string' ? saved.mode : '',
    name: typeof saved.name === 'string' ? saved.name : '',
    auto: !!saved.auto
  };
  function save() {
    try { localStorage.setItem(KEY, JSON.stringify(state)); } catch (e) { /* senza memoria va bene lo stesso */ }
  }

  function known(id) { return SPECIES.some(function (s) { return s.id === id; }); }
  function fromHash() { return decodeURIComponent((location.hash || '').replace(/^#/, '')).toLowerCase(); }

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var canvas = $('zoo');
  var zoo = new Zoo(canvas, $('glow'), { fps: wallpaper ? 30 : 0, speed: reduced ? 0.4 : 1 });

  function fit() {
    var cell = zoo.resize(root.clientWidth, root.clientHeight);
    root.style.setProperty('--cell', cell + 'px');
  }
  fit();
  if (document.fonts && document.fonts.load) {
    document.fonts.load('500 12px "IBM Plex Mono"').then(function () { zoo.buildAtlas(); }, function () {});
  }
  var resizeTimer = 0;
  window.addEventListener('resize', function () {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(fit, 120);
  });

  /* ---- animali ---- */
  var nav = $('modes'), hint = $('hint'), buttons = {};
  var touch = window.matchMedia && window.matchMedia('(pointer: coarse)').matches;

  SPECIES.forEach(function (sp, i) {
    var b = document.createElement('button');
    b.type = 'button';
    b.style.setProperty('--dot', sp.dot);
    b.setAttribute('aria-pressed', 'false');
    b.setAttribute('aria-label', sp.label);
    b.title = sp.label + ' (' + (i + 1) + ')';
    var s = document.createElement('span');
    s.textContent = sp.label;
    b.appendChild(s);
    b.addEventListener('click', function () { setMode(sp.id); });
    nav.appendChild(b);
    buttons[sp.id] = b;
  });

  function setMode(id, keepHash) {
    if (!known(id)) return;
    zoo.setMode(id);
    state.mode = id;
    save();
    var sp = zoo.species;
    Object.keys(buttons).forEach(function (k) { buttons[k].setAttribute('aria-pressed', k === id ? 'true' : 'false'); });
    root.style.setProperty('--accent', sp.dot);
    var act = touch ? 'Tocca' : 'Clic';
    hint.textContent = (sp.feeds ? act + ': dai da mangiare alle koi' : act + ': si disperdono') +
      (touch ? '' : ' · tasti 1–7: cambia animale');
    if (!keepHash && fromHash() !== id) {
      try { location.replace('#' + id); } catch (e) { /* non indispensabile */ }
    }
  }

  var fixed = known(fromHash());
  setMode(fixed ? fromHash() : known(state.mode) ? state.mode : SPECIES[0].id, true);
  window.addEventListener('hashchange', function () {
    var h = fromHash();
    if (known(h) && h !== state.mode) setMode(h, true);
  });

  canvas.addEventListener('pointerdown', function (e) {
    if (e.button) return;
    var r = canvas.getBoundingClientRect();
    zoo.poke(e.clientX - r.left, e.clientY - r.top);
    closePanel();
  });

  function nextMode() {
    var i = SPECIES.indexOf(zoo.species);
    setMode(SPECIES[(i + 1) % SPECIES.length].id, wallpaper);
  }
  setInterval(function () {
    var on = wallpaper ? !fixed : state.auto;
    if (on && !document.hidden) nextMode();
  }, wallpaper ? 120000 : 60000);

  zoo.start();
  window.asciiZoo = zoo;
  if (wallpaper) return;

  /* ---- saluto ---- */
  var greeting = $('greeting'), clock = $('clock');
  function tick() {
    var d = new Date(), h = d.getHours();
    var late = h < 5 || h >= 23;
    var g = late ? 'Ancora in piedi' : h < 12 ? 'Buongiorno' : h < 18 ? 'Buon pomeriggio' : 'Buonasera';
    greeting.textContent = g + (state.name ? ', ' + state.name : '') + (late ? '?' : '');
    clock.textContent =
      d.toLocaleDateString('it-IT', { weekday: 'short', day: 'numeric', month: 'short' }).replace(/\./g, '') +
      ' · ' + d.toLocaleTimeString('it-IT', { hour: '2-digit', minute: '2-digit' });
  }
  tick();
  setInterval(tick, 15000);

  /* ---- ricerca ---- */
  var form = $('search'), input = $('q'), link = $('search-link');
  var framed = true;
  try { framed = window.top !== window.self; } catch (e) { framed = true; }

  function go(url, label) {
    if (framed) {
      // dentro una pagina incorporata non si può navigare: si offre il link
      link.href = url;
      link.textContent = label + ' ↗';
      link.hidden = false;
      link.focus();
    } else {
      location.assign(url);
    }
  }

  form.addEventListener('submit', function (e) {
    e.preventDefault();
    var q = input.value.trim();
    if (!q) return;
    if (/^(https?:\/\/)?[\w-]+(\.[\w-]+)+(:\d+)?(\/\S*)?$/i.test(q)) {
      go(/^https?:\/\//i.test(q) ? q : 'https://' + q, 'Apri ' + q);
      return;
    }
    var api = typeof browser !== 'undefined' && browser.search ? browser.search : null;
    if (api) {
      // nell'estensione: motore di ricerca predefinito di Firefox
      if (api.query) api.query({ text: q, disposition: 'CURRENT_TAB' });
      else api.search({ query: q });
      return;
    }
    go('https://duckduckgo.com/?q=' + encodeURIComponent(q), 'Cerca «' + q + '» su DuckDuckGo');
  });
  input.addEventListener('input', function () { link.hidden = true; });

  /* ---- impostazioni ---- */
  var ui = $('ui'), panel = $('panel'), gear = $('gear');
  var nameInput = $('name'), autoInput = $('auto');
  nameInput.value = state.name;
  autoInput.checked = state.auto;

  function closePanel() {
    if (!panel) return; // versione sfondo: nessun pannello
    panel.hidden = true;
    gear.setAttribute('aria-expanded', 'false');
  }
  function setBare(on) {
    ui.classList.toggle('bare', on);
    gear.setAttribute('aria-label', on ? "Mostra l'interfaccia" : 'Impostazioni');
    gear.title = on ? "Mostra l'interfaccia (H)" : 'Impostazioni';
    if (on) closePanel();
  }
  setBare(false);

  gear.addEventListener('click', function () {
    if (ui.classList.contains('bare')) { setBare(false); return; }
    panel.hidden = !panel.hidden;
    gear.setAttribute('aria-expanded', panel.hidden ? 'false' : 'true');
    if (!panel.hidden) nameInput.focus();
  });
  nameInput.addEventListener('input', function () {
    state.name = nameInput.value.trim();
    save();
    tick();
  });
  autoInput.addEventListener('change', function () {
    state.auto = autoInput.checked;
    save();
  });
  $('hide').addEventListener('click', function () { setBare(true); });

  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') {
      closePanel();
      if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
      return;
    }
    var tag = e.target && e.target.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key >= '1' && e.key <= String(SPECIES.length)) {
      setMode(SPECIES[+e.key - 1].id);
    } else if (e.key === 'h' || e.key === 'H') {
      setBare(!ui.classList.contains('bare'));
    } else if (e.key === '/') {
      e.preventDefault();
      setBare(false);
      input.focus();
    }
  });
})();
