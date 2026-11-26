'use strict';
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = s => document.querySelector(s);
const island = $('#island');
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const ICON = {
  check: '<svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>',
  alert: '<svg viewBox="0 0 24 24"><path d="M12 8v5M12 16.5v.5"/><circle cx="12" cy="12" r="9"/></svg>'
};

let S = null;          // snapshot from Rust
let M = null;          // what is playing
let SYS = null;        // cpu, ram, battery
let SHELF = [];        // file tray
let CLIPS = [];        // last copies
let NET = null;        // wifi / bluetooth
let dragging = false;  // a file is being dragged over the island
let LY = null;         // lyrics of the song playing
let CAL = null;        // calendar events
let distraction = null; // focus guard: { word, title, until }
let W = null;          // weather
let wasOpen = false;
let hover = false;     // mouse is over the visible island
let armed = false;     // you meant to open it (rested the mouse on it, or clicked)
let intentTimer = 0;
const HOVER_DELAY = 380;
let pinned = false;
let tab = 'ai';
let collapseTimer = 0;
let testUntil = 0;
const seen = new Set(); // finished sessions you already looked at
const errors = {};      // session id → error text from a decision

// ---------- helpers ----------
const color = state => ({ running: 'var(--busy)', question: 'var(--question)', done: 'var(--done)', error: 'var(--error)' }[state] || '#555');
const seenKey = s => s.id + ':' + s.ended_at;
const fmtDur = ms => {
  const t = Math.max(0, Math.floor(ms / 1000)), h = Math.floor(t / 3600), m = Math.floor(t % 3600 / 60), s = t % 60;
  return h ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`;
};
// "auto" follows Windows' own clock format; "12" / "24" are fixed
const clock = d => {
  const mode = S?.cfg.clock_mode || (S?.cfg.hour12 ? '12' : 'auto');
  if (mode === '12') return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' }).replace(/\s?[AP]M$/, '');
  if (mode === '24') return d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', hour12: false });
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }).replace(/\s?[AP]M$/i, '');
};
const dateStr = d => {
  const wd = d.toLocaleDateString('pt-BR', { weekday: 'short' }).replace(/\.|,/g, '');
  const mo = d.toLocaleDateString('pt-BR', { month: 'short' }).replace(/\./g, '');
  return `${wd} ${String(d.getDate()).padStart(2, '0')} ${mo}`;
};
const resetText = v => {
  if (v == null) return '';
  const ms = typeof v === 'number' ? (v < 1e12 ? v * 1000 : v) : Date.parse(v);
  if (!Number.isFinite(ms)) return '';
  const d = new Date(ms), sameDay = d.toDateString() === new Date().toDateString();
  return 'reinicia ' + (sameDay ? 'às ' + clock(d) : d.toLocaleDateString('pt-BR', { weekday: 'short' }).replace('.', '') + ' ' + clock(d));
};
const resetMs = v => v == null ? NaN : typeof v === 'number' ? (v < 1e12 ? v * 1000 : v) : Date.parse(v);
// the saved numbers may be old: once the window has reset, the use starts again at 0
const fmtAge = ms => { const m = Math.round(ms / 60000); return m < 60 ? `${m} min` : m < 1440 ? `${Math.round(m / 60)} h` : `${Math.round(m / 1440)} d`; };
function limit(l) {
  if (!l) return null;
  const ms = resetMs(l.resets_at);
  return Number.isFinite(ms) && Date.now() > ms ? { used: 0, resets_at: null, fresh: true } : l;
}
function ring(pct, c) {
  const r = 7.5, len = 2 * Math.PI * r, used = Math.min(100, Math.max(0, pct)) / 100 * len;
  return `<svg class="ring" viewBox="0 0 20 20"><circle class="bg" cx="10" cy="10" r="${r}"/><circle class="fg" cx="10" cy="10" r="${r}" style="stroke:${c};stroke-dasharray:${used} ${len}"/></svg>`;
}
const limitColor = p => p >= 90 ? 'var(--error)' : p >= 70 ? 'var(--busy)' : 'var(--done)';

// the session that matters most right now
function focusSession() {
  const list = S?.sessions || [];
  return list.find(s => s.permission) || list.find(s => s.state === 'question')
    || list.find(s => s.state === 'running') || list.find(s => s.state === 'error' && !seen.has(seenKey(s)))
    || list.find(s => s.state === 'done' && !seen.has(seenKey(s))) || null;
}
const needsYou = () => (S?.sessions || []).some(s => s.permission);

// ---------- compact bar ----------
// ---------- resting island: what goes left / right (Settings → Island) ----------
const WMO = c => c === 0 ? '☀️' : c <= 2 ? '🌤️' : c === 3 ? '☁️' : c <= 48 ? '🌫️' : c <= 67 ? '🌧️' : c <= 77 ? '❄️' : c <= 82 ? '🌦️' : c <= 86 ? '🌨️' : '⛈️';
function usageChips() {
  const five = limit(S.usage.five_hour), cx = codexActive() ? limit(S.codex_usage.five_hour) : null, both = five && cx, out = [];
  if (five) out.push(`<span class="chip" title="Claude · limite de 5 horas">${ring(five.used, limitColor(five.used))}${both ? '<small>C</small>' : ''}${five.used}%</span>`);
  if (cx) out.push(`<span class="chip" title="Codex · limite de 5 horas">${ring(cx.used, limitColor(cx.used))}${both || !five ? '<small>CX</small>' : ''}${cx.used}%</span>`);
  return out.join('');
}
function slot(kind, now) {
  switch (kind) {
    case 'date': return `<span class="date">${esc(dateStr(now).toUpperCase())}</span>`;
    case 'clock': return `<span class="clock">${clock(now)}</span>`;
    case 'weather': return W && W.temp != null ? `<span class="chip" title="${esc(W.city)}">${WMO(W.code)} ${Math.round(W.temp)}°</span>` : '';
    case 'battery': return SYS?.battery != null ? `<span class="chip">${SYS.charging ? '⚡' : '🔋'} ${SYS.battery}%</span>` : '';
    case 'cpu': return SYS ? `<span class="chip">CPU ${SYS.cpu}%</span>` : '';
    case 'ram': return SYS ? `<span class="chip">RAM ${SYS.ram}%</span>` : '';
    case 'net': return NET ? `<span class="chip net">${netIcon()}</span>` : '';
    case 'usage': return S.cfg.apps.usage ? usageChips() : '';
    default: return '';
  }
}

// volume bar: shown for a moment after the volume changes
let VOL = null, volUntil = 0;
const SPEAKER = m => m ? '<svg viewBox="0 0 24 24"><path d="M11 5L6 9H2v6h4l5 4zM23 9l-6 6M17 9l6 6"/></svg>' : '<svg viewBox="0 0 24 24"><path d="M11 5L6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14"/></svg>';

function renderBar() {
  const s = focusSession(), now = new Date(), parts = [];
  let resting = false;
  if (VOL && Date.now() < volUntil) {
    const lvl = VOL.muted ? 0 : VOL.level;
    parts.push(`<span class="check" style="--c:#fff">${SPEAKER(VOL.muted || VOL.level === 0)}</span><span class="vol"><i style="width:${lvl}%"></i></span><span class="muted">${VOL.muted ? 'mudo' : VOL.level + '%'}</span>`);
    $('#bar').innerHTML = parts.join('');
    return;
  }
  if (Date.now() < testUntil) {
    parts.push(`<span class="check" style="--c:var(--done)">${ICON.check}</span><span class="txt"><b>Teste</b> · assim ela avisa</span>`);
  } else if (s) {
    const c = color(s.state), name = `<b>${esc(s.project || (s.agent === 'codex' ? 'Codex' : 'Claude Code'))}</b>`;
    if (s.permission) parts.push(`<i class="dot pulse" style="--c:${c}"></i><span class="txt">${name} quer permissão</span>`);
    else if (s.state === 'question') parts.push(`<i class="dot pulse" style="--c:${c}"></i><span class="txt">${name} precisa de você</span>`);
    else if (s.state === 'running') parts.push(`<i class="spin" style="--c:${c}"></i><span class="txt">${name} · ${esc(s.detail)}</span><span class="muted">${fmtDur(Date.now() - s.started_at)}</span>`);
    else if (s.state === 'error') parts.push(`<span class="check" style="--c:${c}">${ICON.alert}</span><span class="txt">${name}: erro</span>`);
    else parts.push(`<span class="check" style="--c:${c}">${ICON.check}</span><span class="txt">${name} pronto</span><span class="muted">${fmtDur(s.ended_at - s.started_at)}</span>`);
  } else if (S?.cfg.apps.calendar && nextEvent() && nextEvent().start - Date.now() <= 15 * 60000) {
    const e = nextEvent(), mins = Math.max(0, Math.round((e.start - Date.now()) / 60000));
    parts.push(`<i class="dot${mins <= 5 ? ' pulse' : ''}" style="--c:#7fb2ff"></i><span class="txt">${e.start <= Date.now() ? 'Agora' : mins === 0 ? 'Começando' : `Em ${mins} min`} · <b>${esc(e.title)}</b></span>`);
  } else if (S?.cfg.apps.music && M?.playing) {
    parts.push(`<i class="mini-art" style="${M.art ? `background-image:url('${M.art}')` : ''}"></i><span class="txt"><b>${esc(M.title)}</b>${M.artist ? ' · ' + esc(M.artist) : ''}</span><span class="eq"><i></i><i></i><i></i></span>`);
  } else if (S) {
    // nothing going on: your chosen left · clock · right
    resting = true;
    const left = slot(S.cfg.rest_left, now), right = slot(S.cfg.rest_right, now);
    const mid = S.cfg.rest_clock !== false && S.cfg.apps.clock !== false ? `<span class="clock">${clock(now)}</span>` : '';
    parts.push(left, mid, right);
    if (!left && !mid && !right) parts.push(`<i class="dot" style="--c:#444"></i>`);
  }
  if (S?.cfg.apps.timer && timer.running) parts.push(`<span class="chip">⏱ ${fmtDur(timer.left())}</span>`);
  // while something happens, the usage rings stay on the right (at rest they are a slot you choose)
  if (S?.cfg.apps.usage && !resting) parts.push(usageChips());
  $('#bar').innerHTML = parts.filter(Boolean).join('');
  const glow = s && Date.now() >= testUntil ? (s.state === 'done' || s.state === 'error' ? color(s.state) : color(s.state)) : Date.now() < testUntil ? 'var(--done)' : 'transparent';
  island.style.setProperty('--glow', s || Date.now() < testUntil ? glow : 'transparent');
}

// ---------- AI tab ----------
function renderSessions() {
  const list = S?.sessions || [];
  if (!list.length) {
    $('#tab-ai').innerHTML = (S?.claude.installed || S?.codex?.installed)
      ? `<div class="empty"><b>Tudo calmo por aqui</b>Quando o Claude Code trabalhar, você acompanha aqui.</div>`
      : `<div class="empty"><b>Conecte sua IA</b>Abra as configurações → Conexões e ligue o Claude Code ou o Codex.</div>`;
    return;
  }
  $('#tab-ai').innerHTML = list.slice(0, 4).map(s => {
    const c = color(s.state), end = s.ended_at || Date.now();
    const finished = s.state === 'done' || s.state === 'error' || (s.state === 'question' && !s.permission && s.ended_at);
    let body = `<div class="detail">${esc(s.detail || '')}</div>`;
    if (s.prompt) body += `<div class="prompt">“${esc(s.prompt)}”</div>`;
    if (s.permission) {
      const p = s.permission;
      body += `<div class="perm"><div class="tool">${esc(p.tool)}</div>${p.what ? `<div class="what">${esc(p.what)}</div>` : ''}
        <div class="btns">
          <button class="btn deny" data-decide="deny" data-id="${esc(s.id)}">Negar</button>
          ${p.always ? `<button class="btn" data-decide="always" data-id="${esc(s.id)}">Sempre permitir</button>` : ''}
          <button class="btn allow" data-decide="allow" data-id="${esc(s.id)}">${p.kind === 'plan' ? 'Aprovar plano' : 'Permitir'}</button>
        </div></div>`;
      if (errors[s.id]) body += `<div class="err">${esc(errors[s.id])}</div>`;
    } else if (s.question && s.state === 'question') {
      body += `<div class="msg">${esc(s.question)}</div>`;
    } else if (finished && s.last_message) {
      body += `<div class="msg">${esc(s.last_message)}</div>`;
    }
    if (!finished && s.steps.length) {
      body += `<ul class="steps">${s.steps.slice(-4).map(st => `<li class="${esc(st.state)}"><i></i><span>${esc(st.label)} ${esc(st.target)}</span></li>`).join('')}</ul>`;
    }
    if (finished && s.files.length) body += `<div class="files">${s.files.length} arquivo${s.files.length > 1 ? 's' : ''} alterado${s.files.length > 1 ? 's' : ''}: ${esc(s.files.slice(0, 4).join(', '))}${s.files.length > 4 ? '…' : ''}</div>`;
    return `<div class="sess" style="--c:${c}">
      <div class="row"><i class="dot${s.state === 'question' ? ' pulse' : ''}" style="--c:${c}"></i><b>${esc(s.project || (s.agent === 'codex' ? 'Codex' : 'Claude Code'))}</b>
        ${s.agent === 'codex' ? '<span class="tag">Codex</span>' : ''}${s.model ? `<span class="tag">${esc(s.model)}</span>` : ''}${s.context != null ? `<span class="tag">contexto ${s.context}%</span>` : ''}
        <span class="time" data-start="${s.started_at}" data-end="${s.ended_at || ''}">${fmtDur(end - s.started_at)}</span>
        ${finished ? `<button class="x" data-dismiss="${esc(s.id)}" title="Dispensar">${ICON.x}</button>` : ''}
      </div>${body}</div>`;
  }).join('');
}

const hasUsage = u => !!(u && (u.five_hour || u.seven_day));
// Codex's numbers come from its last session file: once its 5-hour window has reset, Codex is not in use,
// so its numbers are only an old leftover and stay out of sight
const codexActive = () => { const l = limit(S?.codex_usage?.five_hour); return !!l && !l.fresh; };
function renderUsage() {
  const box = $('#usage');
  if (!S?.cfg.apps.usage) { box.innerHTML = ''; return; }
  const u = S.usage, cx = S.codex_usage, fromAccount = S.cfg.usage_from_account;
  const lim = (label, raw) => {
    const l = limit(raw); if (!l) return '';
    return `<div class="lim" style="--c:${limitColor(l.used)}"><div class="top"><span>${label} <b>${l.used}%</b></span><span>${l.fresh ? 'já reiniciou' : esc(resetText(l.resets_at))}</span></div><div class="bar"><i style="width:${Math.min(100, l.used)}%"></i></div></div>`;
  };
  const cxOn = codexActive();
  const both = hasUsage(u) && cxOn;
  let html = '';
  // Claude
  if (hasUsage(u)) {
    const age = Date.now() - (u.updated_at || 0);
    const note = fromAccount && S.usage_error ? esc(S.usage_error)
      : age > 10 * 60 * 1000 ? `Atualizado há ${fmtAge(age)}${fromAccount ? '' : ' · chega quando você usa o Claude Code no terminal'}` : '';
    html += `${both ? '<div class="who">Claude</div>' : ''}<div class="lims">${lim(both ? '5h' : 'Claude 5h', u.five_hour)}${lim('Semana', u.seven_day)}</div>${note ? `<div class="hint">${note}</div>` : ''}`;
  } else if (fromAccount && S.usage_error) {
    html += `<div class="hint">Claude: ${esc(S.usage_error)}</div>`;
  }
  // Codex (read from its own session files)
  if (cxOn) {
    html += `${both ? '<div class="who">Codex</div>' : ''}<div class="lims">${lim(both ? '5h' : 'Codex 5h', cx.five_hour)}${lim('Semana', cx.seven_day)}</div>`;
  }
  if (!html && S.claude.installed) {
    html = `<div class="hint">O uso do Claude aparece depois de usar o Claude Code no terminal (a aba Code do app não envia esses números).</div>`;
  }
  box.innerHTML = html;
}

// ---------- music ----------
const PLAY = '<svg viewBox="0 0 24 24"><path d="M7 4l13 8-13 8z"/></svg>';
const PAUSE = '<svg viewBox="0 0 24 24"><path d="M7 4h3v16H7zM14 4h3v16h-3z"/></svg>';
function mediaPos() {
  if (!M) return 0;
  const p = M.position_ms + (M.playing ? Date.now() - M.position_at : 0);
  return M.duration_ms > 0 ? Math.min(Math.max(0, p), M.duration_ms) : Math.max(0, p);
}
function renderMusic() {
  $('#player').hidden = !M; $('#mEmpty').hidden = !!M;
  if (!M) return;
  $('#mArt').style.backgroundImage = M.art ? `url('${M.art}')` : '';
  $('#mTitle').textContent = M.title;
  $('#mArtist').textContent = [M.artist, M.app].filter(Boolean).join(' · ');
  $('#mToggle').innerHTML = M.playing ? PAUSE : PLAY;
  document.querySelector('[data-media="next"]').disabled = !M.can_next;
  document.querySelector('[data-media="prev"]').disabled = !M.can_prev;
  renderProgress();
}
function renderProgress() {
  if (!M) return;
  const pos = mediaPos(), dur = M.duration_ms;
  $('#mPos').textContent = fmtDur(pos);
  $('#mDur').textContent = dur > 0 ? fmtDur(dur) : '';
  $('#mBar').style.width = dur > 0 ? (pos / dur * 100).toFixed(2) + '%' : '0%';
}
function renderSys() {
  const box = $('#sys');
  if (!S?.cfg.apps.system || !SYS) { box.innerHTML = ''; return; }
  const bat = SYS.battery != null ? `<span>Bateria <b>${SYS.battery}%</b>${SYS.charging ? ' ⚡' : ''}</span>` : '';
  box.innerHTML = `<span>CPU <b>${SYS.cpu}%</b></span><span>RAM <b>${SYS.ram}%</b></span>${bat}`;
}

// ---------- toast ----------
let toastTimer = 0;
function toast(text, bad) {
  const el = $('#toast'); el.textContent = text; el.classList.toggle('bad', !!bad); el.classList.add('on');
  clearTimeout(toastTimer); toastTimer = setTimeout(() => el.classList.remove('on'), 1600);
}
const ago = at => {
  const s = (Date.now() - at) / 1000;
  return s < 60 ? 'agora' : s < 3600 ? `${Math.floor(s / 60)} min` : s < 86400 ? `${Math.floor(s / 3600)} h` : `${Math.floor(s / 86400)} d`;
};

// ---------- file tray ----------
const EXT_COLOR = [
  [/^(png|jpe?g|gif|webp|svg|bmp|heic|ico)$/, '#ff9fd1'], [/^(mp4|mov|mkv|avi|webm)$/, '#c49bff'], [/^(mp3|wav|flac|m4a|ogg)$/, '#6ee7a0'],
  [/^(pdf)$/, '#ff7a7a'], [/^(docx?|odt|rtf|txt|md)$/, '#7fb2ff'], [/^(xlsx?|csv|ods)$/, '#5fd38d'], [/^(pptx?|key)$/, '#ffb066'],
  [/^(zip|rar|7z|tar|gz)$/, '#e8d27a'], [/^(exe|msi|bat|cmd|ps1)$/, '#9b9ba3'], [/^(js|ts|rs|py|json|html|css|go|java|cs|c|cpp)$/, '#7ee0e0']
];
const fileSize = b => b >= 1e9 ? (b / 1e9).toFixed(1) + ' GB' : b >= 1e6 ? (b / 1e6).toFixed(1) + ' MB' : b >= 1e3 ? Math.round(b / 1e3) + ' KB' : b + ' B';
const SHELF_ICONS = {
  open: '<svg viewBox="0 0 24 24"><path d="M14 3h7v7M21 3l-9 9M19 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>'
};
function renderShelf() {
  $('#shelfCount').textContent = SHELF.length ? SHELF.length : '';
  if (!SHELF.length) {
    $('#shelfList').innerHTML = `<div class="drop"><b>${dragging ? 'Solte aqui' : 'Bandeja vazia'}</b>Arraste arquivos para a ilha e pegue de volta depois, arrastando para onde quiser.</div>`;
    return;
  }
  $('#shelfList').innerHTML = `<div class="files-grid">${SHELF.map(f => {
    const ext = f.dir ? 'pasta' : (f.name.includes('.') ? f.name.split('.').pop().toLowerCase() : '');
    const color = f.dir ? '#f0d66e' : (EXT_COLOR.find(([re]) => re.test(ext))?.[1] || '#9b9ba3');
    return `<div class="file${f.missing ? ' missing' : ''}" data-file="${esc(f.id)}" title="${esc(f.path)}">
      <div class="ext" style="--x:${color}">${f.dir ? '' : esc(ext.slice(0, 4))}</div>
      <b>${esc(f.name)}</b><span>${f.missing ? 'não encontrado' : f.dir ? 'pasta' : fileSize(f.size)}</span>
      <div class="acts">
        <button data-fact="open" title="Abrir">${SHELF_ICONS.open}</button>
        <button data-fact="reveal" title="Mostrar na pasta">${SHELF_ICONS.folder}</button>
        <button data-fact="remove" title="Tirar da bandeja">${ICON.x}</button>
      </div></div>`;
  }).join('')}</div>
  <div class="foot"><span>${dragging ? 'Solte para adicionar' : 'Arraste um arquivo para fora para usar'}</span><button data-shelf-clear>Limpar bandeja</button></div>`;
}

// ---------- clipboard ----------
function renderClips() {
  if (!CLIPS.length) {
    $('#clipList').innerHTML = `<div class="empty"><b>Nenhuma cópia ainda</b>Copie um texto (Ctrl+C) e ele aparece aqui. Fica só na memória, nunca no disco.</div>`;
    return;
  }
  $('#clipList').innerHTML = `<div class="clips">${CLIPS.map(c => `<div class="clip" data-clip="${c.id}" title="Clique para copiar de novo">
      <p>${esc(c.text.length > 400 ? c.text.slice(0, 400) + '…' : c.text)}</p><span>${ago(c.at)}</span>
      <button class="x" data-clip-remove="${c.id}" title="Apagar">${ICON.x}</button></div>`).join('')}</div>
    <div class="foot"><span>${CLIPS.length} cópia${CLIPS.length > 1 ? 's' : ''} · clique para copiar</span><button data-clip-clear>Limpar</button></div>`;
}

// ---------- network ----------
const NET_ICONS = {
  wifi: '<svg viewBox="0 0 24 24"><path d="M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M2 9a15 15 0 0 1 20 0M12 20h.01"/></svg>',
  wifiOff: '<svg viewBox="0 0 24 24"><path d="M2 2l20 20M8.5 16a5 5 0 0 1 7 0M5 12.5a10 10 0 0 1 5-2.7M2 9a15 15 0 0 1 4.2-2.8M14.5 9.6a10 10 0 0 1 4.5 2.9M10.7 5.1A15 15 0 0 1 22 9M12 20h.01"/></svg>',
  cable: '<svg viewBox="0 0 24 24"><rect x="6" y="3" width="12" height="9" rx="1.5"/><path d="M9 3v3M12 3v3M15 3v3M12 12v4M8 21h8M12 16v5"/></svg>',
  bt: '<svg viewBox="0 0 24 24"><path d="M7 7l10 10-5 5V2l5 5L7 17"/></svg>'
};
function netIcon() {
  if (!NET || !NET.online) return NET_ICONS.wifiOff;
  return NET.wired ? NET_ICONS.cable : NET_ICONS.wifi;
}
function renderNet() {
  $('#netBtn').innerHTML = netIcon();
  if (!NET) { $('#netBox').innerHTML = '<div class="empty">Lendo a rede…</div>'; return; }
  const n = NET;
  const wifiText = n.wifi == null ? (n.online ? `Sem Wi‑Fi neste PC · ${esc(n.network)}` : 'Sem Wi‑Fi neste PC') : !n.wifi ? 'Desligado' : n.wired ? esc(n.network) : n.network ? esc(n.network) : 'Ligado, sem rede';
  const btText = n.bluetooth == null ? 'Indisponível neste PC' : !n.bluetooth ? 'Desligado' : n.devices.length ? esc(n.devices.join(', ')) : 'Nenhum dispositivo conectado';
  $('#netBox').innerHTML = `<div class="netcards">
    <div class="ncard"><div class="top"><div class="ic ${n.online ? 'on' : ''}">${n.wired ? NET_ICONS.cable : n.wifi ? NET_ICONS.wifi : NET_ICONS.wifiOff}</div>
      <div class="tx"><b>${n.wifi == null ? 'Rede' : 'Wi‑Fi'}</b><span>${wifiText}</span></div>
      ${n.wifi == null ? '' : `<label class="sw"><input type="checkbox" data-radio="wifi" ${n.wifi ? 'checked' : ''}></label>`}</div>
      <button class="link" data-netset="${n.wifi == null ? 'network' : 'wifi'}">${n.wifi == null ? 'Configurações de rede' : 'Redes Wi‑Fi'}</button></div>
    <div class="ncard"><div class="top"><div class="ic ${n.bluetooth ? 'on' : ''}">${NET_ICONS.bt}</div>
      <div class="tx"><b>Bluetooth</b><span>${btText}</span></div>
      ${n.bluetooth == null ? '' : `<label class="sw"><input type="checkbox" data-radio="bluetooth" ${n.bluetooth ? 'checked' : ''}></label>`}</div>
      <button class="link" data-netset="bluetooth">Dispositivos Bluetooth</button></div>
  </div>`;
}

// ---------- your own sites and programs ----------
function renderLinks() {
  const links = S?.cfg.apps.links ? (S.cfg.links || []) : [];
  const html = links.map((l, i) => {
    const icon = LINK_ICONS.get(l.target);
    if (icon === undefined) fetchLinkIcon(l.target);
    const letter = esc((l.name || l.target).replace(/^https?:\/\/(www\.)?/, '').charAt(0).toUpperCase());
    return `<button data-link="${i}" title="${esc(l.name || l.target)}">${icon ? `<img src="${esc(icon)}" alt="">` : letter}</button>`;
  }).join('');
  if ($('#links').innerHTML !== html) $('#links').innerHTML = html;
  // the island grows with the links so the clock and the gear keep their room
  $('#island').style.setProperty('--links-w', links.length ? links.length * 31 + 12 + 'px' : '0px');
}
// target -> data URL, or null when it has none (then the letter stays)
const LINK_ICONS = new Map();
async function fetchLinkIcon(target) {
  LINK_ICONS.set(target, null);
  try {
    const url = await invoke('link_icon', { target });
    if (url) { LINK_ICONS.set(target, url); renderLinks(); }
  } catch {}
}

// ---------- lyrics ----------
function renderLyrics() {
  const box = $('#lyrics');
  if (!S?.cfg.apps.lyrics || !M || !LY || LY.key !== `${M.title}|${M.artist}`) { box.innerHTML = ''; return; }
  if (!LY.found) { box.innerHTML = '<div class="none">Letra não encontrada para esta música.</div>'; return; }
  if (!LY.synced) {
    if (!box.querySelector('.plain')) box.innerHTML = `<div class="plain">${esc(LY.plain)}</div>`;
    return;
  }
  const pos = mediaPos();
  let i = LY.lines.findIndex(l => l.t > pos) - 1;
  if (i === -2) i = LY.lines.length - 1; // past the last stamp
  const line = k => (k >= 0 && k < LY.lines.length ? LY.lines[k].text : '') || '♪';
  const html = i < 0 ? `<p class="now">♪</p><p>${esc(line(0))}</p>` : `<p>${esc(i > 0 ? line(i - 1) : '')}</p><p class="now">${esc(line(i))}</p><p>${esc(line(i + 1))}</p>`;
  if (box.innerHTML !== html) box.innerHTML = html;
}

// ---------- calendar ----------
const dayLabel = ms => {
  const d = new Date(ms), t = new Date(), y = new Date(); y.setDate(t.getDate() + 1);
  if (d.toDateString() === t.toDateString()) return 'Hoje';
  if (d.toDateString() === y.toDateString()) return 'Amanhã';
  return d.toLocaleDateString('pt-BR', { weekday: 'long', day: '2-digit', month: 'short' }).replace('.', '');
};
const hm = ms => clock(new Date(ms));
function nextEvent() {
  const now = Date.now();
  return (CAL?.events || []).find(e => !e.all_day && e.end > now) || null;
}
function renderCal() {
  const box = $('#calList');
  if (!S?.cfg.calendar_url) { box.innerHTML = `<div class="empty"><b>Agenda não conectada</b>Cole o link iCal da sua agenda em Configurações → Apps → Agenda.</div>`; return; }
  if (!CAL) { box.innerHTML = '<div class="empty">Lendo sua agenda…</div>'; return; }
  const now = Date.now(), events = CAL.events.filter(e => e.end > now);
  let html = CAL.error ? `<div class="hint" style="margin-bottom:6px">${esc(CAL.error)}</div>` : '';
  if (!events.length) html += '<div class="empty"><b>Nada nos próximos 14 dias</b>Aproveite.</div>';
  let day = '';
  for (const e of events.slice(0, 25)) {
    const label = dayLabel(e.start);
    if (label !== day) { day = label; html += `<div class="day">${esc(label)}</div>`; }
    const live = e.start <= now && e.end > now;
    const when = e.all_day ? 'dia todo' : live ? 'agora' : `${hm(e.start)}–${hm(e.end)}`;
    html += `<div class="ev${live ? ' now' : ''}"><i></i><div class="tx"><b>${esc(e.title)}</b>${e.location ? `<span>${esc(e.location)}</span>` : ''}</div><div class="when">${when}</div></div>`;
  }
  box.innerHTML = `<div class="cal">${html}</div>`;
}

// ---------- focus guard ----------
function renderGuard() {
  const box = $('#guard');
  if (!distraction || Date.now() > distraction.until) { distraction = null; box.hidden = true; return; }
  box.hidden = false;
  const html = `<div class="tx"><b>Isso te distrai: ${esc(distraction.word)}</b><span>${esc(distraction.title || '')}</span></div>
    <div class="btns"><button class="btn" data-guard="snooze">5 min</button><button class="btn allow" data-guard="leave">Voltar ao foco</button></div>`;
  if (box.dataset.k !== distraction.word + distraction.title) { box.innerHTML = html; box.dataset.k = distraction.word + distraction.title; }
}

function renderHeader() {
  const now = new Date();
  $('#pDate').textContent = dateStr(now);
  $('#pTime').textContent = clock(now);
  document.querySelector('[data-tab="music"]').hidden = !S?.cfg.apps.music;
  document.querySelector('[data-tab="timer"]').hidden = !S?.cfg.apps.timer;
  document.querySelector('[data-tab="note"]').hidden = !S?.cfg.apps.note;
  document.querySelector('[data-tab="shelf"]').hidden = !S?.cfg.apps.shelf;
  document.querySelector('[data-tab="clips"]').hidden = !S?.cfg.apps.clipboard;
  $('#netBtn').hidden = !S?.cfg.apps.net;
  document.querySelector('[data-tab="cal"]').hidden = !S?.cfg.apps.calendar;
  const allowed = { music: 'music', timer: 'timer', note: 'note', shelf: 'shelf', clips: 'clipboard', net: 'net', cal: 'calendar' };
  if (allowed[tab] && !S?.cfg.apps[allowed[tab]]) setTab('ai');
  $('#pin').classList.toggle('on', pinned);
  $('#netBtn').classList.toggle('on', tab === 'net');
}

function applyColors() {
  const c = S?.cfg.colors; if (!c) return;
  for (const k of ['busy', 'question', 'done', 'error']) document.documentElement.style.setProperty('--' + k, c[k]);
  applyLook();
}

// ---------- look: size, attached / floating, black / glass, transparency, music tint ----------
let artColor = null, artFor = '';
function albumColor(src) {
  if (!src || src === artFor) return;
  artFor = src;
  const img = new Image();
  img.onload = () => {
    const cv = document.createElement('canvas'); cv.width = cv.height = 16;
    const ctx = cv.getContext('2d'); ctx.drawImage(img, 0, 0, 16, 16);
    const d = ctx.getImageData(0, 0, 16, 16).data;
    // the most colorful pixels say more about a cover than the plain average
    let r = 0, g = 0, b = 0, w = 0;
    for (let i = 0; i < d.length; i += 4) {
      const mx = Math.max(d[i], d[i + 1], d[i + 2]), mn = Math.min(d[i], d[i + 1], d[i + 2]);
      const weight = 1 + (mx - mn) * 4 / 255 + (mx > 30 ? 0.5 : 0);
      r += d[i] * weight; g += d[i + 1] * weight; b += d[i + 2] * weight; w += weight;
    }
    artColor = `rgb(${Math.round(r / w)}, ${Math.round(g / w)}, ${Math.round(b / w)})`;
    applyLook();
  };
  img.src = src;
}
function applyLook() {
  const c = S?.cfg; if (!c) return;
  const root = document.documentElement;
  root.dataset.size = c.size || 'normal';
  root.dataset.style = c.style || 'attached';
  root.dataset.material = c.material || 'black';
  root.style.setProperty('--alpha', String(1 - Math.min(80, c.transparency || 0) / 100));
  // glass tint: the album cover while music plays, else what the AI is doing
  const s = focusSession();
  const tint = c.music_tint && M?.playing && artColor && !s ? artColor : s ? color(s.state) : '#5b5b66';
  root.style.setProperty('--tint', tint);
  if (M?.art && c.music_tint) albumColor(M.art);
}

// ---------- expand / collapse ----------
function shouldExpand() {
  const guard = !!distraction && Date.now() < distraction.until;
  if (tucked) return pinned || dragging || guard || (S?.cfg.auto_expand && needsYou());
  return armed || pinned || dragging || guard || (S?.cfg.auto_expand && needsYou());
}

// ---------- tucked: right click folds the island into a thin line at the top ----------
let tucked = localStorage.getItem('ilha:tucked') === '1';
function setTucked(on) {
  tucked = on; localStorage.setItem('ilha:tucked', on ? '1' : '0');
  armed = false; pinned = false; clearTimeout(intentTimer);
  render();
}
function renderLine() {
  island.classList.toggle('tucked', tucked && !shouldExpand());
  if (!tucked) return;
  const s = focusSession();
  const used = Math.max(limit(S?.usage.five_hour)?.used ?? -1, limit(S?.codex_usage?.five_hour)?.used ?? -1);
  const active = s && (s.state === 'running' || s.state === 'question' || (s.state !== 'idle' && !seen.has(seenKey(s))));
  const fill = used >= 0 ? used : active ? 100 : 0;
  const c = active ? color(s.state) : used >= 0 ? limitColor(used) : '#555';
  $('#line').style.setProperty('--fill', Math.min(100, Math.max(fill, active ? 6 : 0)) + '%');
  $('#line').style.setProperty('--lc', c);
  $('#line').classList.toggle('busy', !!(s && s.state === 'running'));
}
function layout() {
  const open = shouldExpand();
  // opening with no AI work but music playing: show the player
  if (open && !wasOpen && tab === 'ai' && !(S?.sessions || []).length && S?.cfg.apps.music && M) setTab('music');
  wasOpen = open;
  island.classList.toggle('expanded', open);
  island.classList.toggle('collapsed', !open);
  if (open && armed) for (const s of S?.sessions || []) if (s.state === 'done' || s.state === 'error') seen.add(seenKey(s));
}
function render() {
  if (!S) return;
  applyColors();
  renderBar();
  renderHeader();
  renderSessions();
  renderUsage();
  renderMusic();
  renderSys();
  renderShelf();
  renderClips();
  renderNet();
  renderLinks();
  renderLyrics();
  renderCal();
  renderGuard();
  island.classList.toggle('dropping', dragging);
  layout();
  renderLine();
}

function setTab(name) {
  tab = name;
  // the AI's usage belongs to the AI tab
  $('#usage').hidden = name !== 'ai';
  document.querySelectorAll('#tabs button').forEach(b => b.classList.toggle('on', b.dataset.tab === name));
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('on', t.id === 'tab-' + name));
  $('#netBtn').classList.toggle('on', name === 'net');
  if (name === 'note') setTimeout(() => $('#note').focus(), 50);
}

// tell Rust which part of the window is the visible island, so clicks elsewhere pass through
let rectQueued = false;
function sendRect() {
  if (rectQueued) return; rectQueued = true;
  requestAnimationFrame(() => {
    rectQueued = false;
    const r = island.getBoundingClientRect();
    const rects = [{ x: r.x, y: r.y, w: r.width, h: r.height }];
    // the notice cards: clickable, but resting on them does not open the island
    const box = $('#notices');
    if (box.children.length) {
      box.style.top = (r.bottom + 10) + 'px';
      const n = box.getBoundingClientRect();
      rects.push({ x: n.x, y: n.y, w: n.width, h: n.height });
    }
    invoke('set_hit_rects', { rects });
  });
}
new ResizeObserver(sendRect).observe(island);
new ResizeObserver(sendRect).observe($('#notices'));
island.addEventListener('transitionend', sendRect);

// ---------- notice cards (Settings → Notifications chooses card / Windows / off per kind) ----------
const NOTICE = {
  ai_done:       { c: '#2b6cff', ic: 'check', stay: 7, tab: 'ai' },
  ai_permission: { c: '#ffb020', ic: 'ask', stay: 12, tab: 'ai' },
  ai_question:   { c: '#ffb020', ic: 'ask', stay: 10, tab: 'ai' },
  ai_error:      { c: '#ff4d4f', ic: 'warn', stay: 8, tab: 'ai' },
  limit:         { c: '#ffb020', ic: 'warn', stay: 8, sound: 'question' },
  focus_done:    { c: '#9b6bff', ic: 'timer', stay: 8, tab: 'timer' },
  guard_back:    { c: '#9b6bff', ic: 'shield', stay: 3 },
  calendar:      { c: '#3b82f6', ic: 'cal', stay: 10, tab: 'cal', sound: 'question' },
  battery:       { c: '#ff4d4f', ic: 'battery', stay: 8, sound: 'question' },
  keys_taken:    { c: '#8d8d96', ic: 'key', stay: 8 },
  update:        { c: '#3ddc97', ic: 'download', stay: 8 }
};
const NIC = {
  check: '<svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>',
  ask: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 1-1 1.7M12 17h.01"/></svg>',
  warn: '<svg viewBox="0 0 24 24"><path d="M12 3l10 18H2z"/><path d="M12 10v4M12 17.5v.5"/></svg>',
  timer: '<svg viewBox="0 0 24 24"><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2M10 2h4"/></svg>',
  shield: '<svg viewBox="0 0 24 24"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>',
  cal: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="17" rx="2.5"/><path d="M3 9h18M8 2v4M16 2v4"/></svg>',
  battery: '<svg viewBox="0 0 24 24"><rect x="2" y="7" width="17" height="10" rx="2"/><path d="M22 11v2M6 10v4"/></svg>',
  key: '<svg viewBox="0 0 24 24"><circle cx="7.5" cy="15.5" r="4.5"/><path d="M10.7 12.3L21 2M16 7l3 3M18 5l2 2"/></svg>',
  download: '<svg viewBox="0 0 24 24"><path d="M12 3v12M7 10l5 5 5-5M5 21h14"/></svg>'
};

// ---------- update download: a card with a progress bar under the island, while it downloads ----------
const mb = b => (b / 1048576).toFixed(1).replace('.', ',') + ' MB';
listen('update', e => {
  const u = e.payload, box = $('#notices');
  let card = document.getElementById('updCard');
  if (u.status !== 'downloading') { if (card) { card.classList.add('out'); setTimeout(() => { card.remove(); sendRect(); }, 240); } return; }
  if (!card) {
    card = document.createElement('div');
    card.id = 'updCard'; card.className = 'notice'; card.style.setProperty('--nc', '#3ddc97');
    card.innerHTML = `<div class="nic">${NIC.download}</div><div class="ntx"><div class="ntop"><b></b><time></time></div><div class="uprog"><i></i></div><p></p></div>`;
    box.appendChild(card); sendRect();
  }
  const pct = u.total ? Math.min(100, Math.round(u.downloaded / u.total * 100)) : 0;
  card.querySelector('b').textContent = `Baixando a versão v${u.version}`;
  card.querySelector('time').textContent = u.total ? pct + '%' : '';
  card.querySelector('.uprog i').style.width = pct + '%';
  card.querySelector('p').textContent = u.total ? `${mb(u.downloaded)} de ${mb(u.total)}` : 'Começando…';
});
let noticeSeq = 0;
function showNotice(n) {
  const kind = NOTICE[n.id] || { c: '#8d8d96', ic: 'check', stay: 6 };
  if (kind.sound) chime(kind.sound);
  // open island: everything is in view already
  if (island.classList.contains('expanded')) return;
  const box = $('#notices');
  while (box.children.length >= 3) box.firstElementChild.remove();
  const el = document.createElement('div');
  el.className = 'notice'; el.dataset.n = ++noticeSeq; el.style.setProperty('--nc', kind.c);
  el.innerHTML = `<div class="nic">${NIC[kind.ic] || NIC.check}</div>
    <div class="ntx"><div class="ntop"><b>${esc(n.title)}</b><time>${clock(new Date(n.at || Date.now()))}</time></div>${n.body ? `<p>${esc(n.body)}</p>` : ''}</div>
    <button class="nx" title="Fechar">${ICON.x}</button>`;
  const close = () => { el.classList.add('out'); setTimeout(() => { el.remove(); sendRect(); }, 240); };
  el.addEventListener('click', e => {
    close();
    if (e.target.closest('.nx')) return;
    if (kind.tab) setTab(kind.tab);
    if (tucked) setTucked(false);
    armed = true; render();
  });
  let timer = setTimeout(close, kind.stay * 1000);
  // resting the mouse on a card keeps it
  el.addEventListener('mouseenter', () => clearTimeout(timer));
  el.addEventListener('mouseleave', () => { timer = setTimeout(close, 2500); });
  box.appendChild(el);
  sendRect();
}
listen('notice', e => showNotice(e.payload));

// ---------- sound ----------
// sounds.js: the chosen pack, at the chosen volume
function chime(kind) {
  if (!S?.cfg.sound) return;
  window.playChime(kind, S.cfg.sound_pack, S.cfg.sound_volume);
}

// ---------- timer ----------
const timer = {
  total: 25 * 60 * 1000, endAt: 0, rest: 25 * 60 * 1000, running: false,
  left() { return this.running ? Math.max(0, this.endAt - Date.now()) : this.rest; }
};
function renderTimer() {
  $('#tDisplay').textContent = fmtDur(timer.left());
  $('#tStart').textContent = timer.running ? 'Pausar' : timer.rest < timer.total ? 'Continuar' : 'Iniciar';
}
$('#tStart').addEventListener('click', () => {
  if (timer.running) { timer.rest = timer.left(); timer.running = false; }
  else { timer.endAt = Date.now() + timer.rest; timer.running = true; }
  renderTimer(); renderBar();
});
$('#tReset').addEventListener('click', () => { timer.running = false; timer.rest = timer.total; renderTimer(); renderBar(); });
document.querySelectorAll('.presets button').forEach(b => b.addEventListener('click', () => {
  document.querySelectorAll('.presets button').forEach(x => x.classList.toggle('on', x === b));
  timer.total = timer.rest = Number(b.dataset.min) * 60 * 1000; timer.running = false; renderTimer();
}));

// ---------- note ----------
let noteTimer = 0;
$('#note').addEventListener('input', e => {
  clearTimeout(noteTimer);
  noteTimer = setTimeout(() => invoke('set_note', { text: e.target.value }), 400);
});
invoke('get_note').then(t => { $('#note').value = t; });

// ---------- events ----------
document.querySelectorAll('#tabs button, #netBtn').forEach(b => b.addEventListener('click', () => setTab(b.dataset.tab)));

// file tray: open / reveal / remove, and drag a file out to another app
$('#tab-shelf').addEventListener('click', async e => {
  if (e.target.closest('[data-shelf-clear]')) return invoke('shelf_remove', { id: null });
  const act = e.target.closest('[data-fact]'), file = e.target.closest('[data-file]');
  if (!act || !file) return;
  const id = file.dataset.file;
  try {
    if (act.dataset.fact === 'open') await invoke('shelf_open', { id });
    else if (act.dataset.fact === 'reveal') await invoke('shelf_reveal', { id });
    else await invoke('shelf_remove', { id });
  } catch (err) { toast(String(err), true); }
});
let pressed = null;
$('#tab-shelf').addEventListener('mousedown', e => {
  const file = e.target.closest('[data-file]');
  if (file && !e.target.closest('[data-fact]') && e.button === 0) pressed = { id: file.dataset.file, x: e.clientX, y: e.clientY };
});
addEventListener('mouseup', () => { pressed = null; });
addEventListener('mousemove', e => {
  if (!pressed || Math.hypot(e.clientX - pressed.x, e.clientY - pressed.y) < 5) return;
  const id = pressed.id; pressed = null;
  pinned = true; // keep the island open while you drag
  invoke('shelf_drag', { ids: [id] }).catch(err => toast(String(err), true)).finally(() => { pinned = false; render(); });
});

// clipboard: click copies again
$('#tab-clips').addEventListener('click', async e => {
  if (e.target.closest('[data-clip-clear]')) return invoke('clip_remove', { id: null });
  const rm = e.target.closest('[data-clip-remove]');
  if (rm) return invoke('clip_remove', { id: Number(rm.dataset.clipRemove) });
  const clip = e.target.closest('[data-clip]');
  if (clip) {
    try { await invoke('clip_copy', { id: Number(clip.dataset.clip) }); toast('Copiado'); }
    catch (err) { toast(String(err), true); }
  }
});

// network: switches and Windows' own lists
$('#tab-net').addEventListener('change', async e => {
  const r = e.target.closest('[data-radio]'); if (!r) return;
  try { await invoke('net_set', { kind: r.dataset.radio, on: r.checked }); }
  catch (err) { r.checked = !r.checked; toast('O Windows não deixou. Abri as configurações.', true); }
});
$('#tab-net').addEventListener('click', e => {
  const b = e.target.closest('[data-netset]'); if (b) invoke('net_settings', { which: b.dataset.netset });
});
$('#gear').addEventListener('click', () => invoke('open_settings_cmd'));
$('#pin').addEventListener('click', () => { pinned = !pinned; render(); });
$('#tab-ai').addEventListener('click', async e => {
  const d = e.target.closest('[data-decide]');
  if (d) {
    delete errors[d.dataset.id];
    try { await invoke('decide', { sessionId: d.dataset.id, behavior: d.dataset.decide }); }
    catch (err) { errors[d.dataset.id] = String(err); render(); }
    return;
  }
  const x = e.target.closest('[data-dismiss]');
  if (x) invoke('dismiss', { sessionId: x.dataset.dismiss });
});

document.querySelector('.ctl').addEventListener('click', async e => {
  const b = e.target.closest('[data-media]'); if (!b || !M) return;
  if (b.dataset.media === 'toggle') { // answer at once; Windows confirms a moment later
    M = { ...M, position_ms: mediaPos(), position_at: Date.now(), playing: !M.playing };
    renderMusic(); renderBar();
  }
  try { await invoke('media_control', { action: b.dataset.media }); } catch {}
});

listen('state', e => { S = e.payload; render(); });
listen('media', e => { M = e.payload; if (S) { renderMusic(); renderBar(); applyLook(); } });
listen('sys', e => { SYS = e.payload; if (S) renderSys(); });
listen('shelf', e => { SHELF = e.payload || []; if (S) renderShelf(); });
listen('clips', e => { CLIPS = e.payload || []; if (S) renderClips(); });
listen('net', e => { NET = e.payload; if (S) renderNet(); });
listen('lyrics', e => { LY = e.payload; if (S) renderLyrics(); });
listen('weather', e => { W = e.payload; if (S) renderBar(); });
invoke('get_weather').then(v => { W = v; if (S) renderBar(); });
listen('volume', e => {
  VOL = e.payload; volUntil = Date.now() + 1600;
  if (!S) return;
  renderBar();
  setTimeout(() => { if (Date.now() >= volUntil) renderBar(); }, 1650);
});
listen('calendar', e => { CAL = e.payload && e.payload.updated_at ? e.payload : null; if (S) { renderCal(); renderBar(); } });
listen('distraction', e => {
  distraction = { ...e.payload, until: Date.now() + 20000 };
  chime('question');
  if (S) render();
});
invoke('get_lyrics').then(v => { LY = v; if (S) renderLyrics(); });
invoke('get_calendar').then(v => { CAL = v; if (S) renderCal(); });

// your own sites and programs
$('#links').addEventListener('click', async e => {
  const b = e.target.closest('[data-link]'); if (!b) return;
  const link = S?.cfg.links?.[Number(b.dataset.link)]; if (!link) return;
  try { await invoke('open_link', { target: link.target }); armed = false; render(); }
  catch (err) { toast(String(err), true); }
});
// focus guard
$('#guard').addEventListener('click', e => {
  const b = e.target.closest('[data-guard]'); if (!b) return;
  if (b.dataset.guard === 'leave') {
    invoke('focus_leave');
    invoke('notice', { id: 'guard_back', title: 'Boa! De volta ao foco', body: `Você voltou em vez de abrir ${distraction?.word || 'a distração'}.` });
  }
  else { invoke('focus_snooze', { minutes: 5 }); toast('Ok, 5 minutos'); }
  distraction = null; render();
});
// the guard needs to know whether the focus timer runs
let timerWas = null;
setInterval(() => { if (timer.running !== timerWas) { timerWas = timer.running; invoke('focus_timer', { running: timer.running }); } }, 500);
listen('dragging', e => {
  dragging = !!e.payload;
  if (dragging) { setTab('shelf'); armed = true; }
  if (S) render();
});
invoke('get_shelf').then(v => { SHELF = v || []; if (S) renderShelf(); });
invoke('get_clips').then(v => { CLIPS = v || []; if (S) renderClips(); });
invoke('get_net').then(v => { NET = v; if (S) renderNet(); });
invoke('get_media').then(m => { M = m; if (S) render(); });
invoke('get_sys').then(s => { SYS = s; if (S) renderSys(); });
// Passing the mouse by does not open the island: it opens after resting on it a moment (or on a click).
listen('hover', e => {
  hover = e.payload;
  clearTimeout(collapseTimer); clearTimeout(intentTimer);
  if (hover) {
    if (island.classList.contains('expanded')) { armed = true; return; }
    if (S?.cfg.open_mode !== 'click' && !tucked) intentTimer = setTimeout(() => { if (hover) { armed = true; render(); } }, HOVER_DELAY);
  } else {
    collapseTimer = setTimeout(() => { armed = false; render(); }, 220);
  }
});
$('#bar').addEventListener('click', () => { armed = true; clearTimeout(intentTimer); render(); });
$('#line').addEventListener('click', () => setTucked(false));
// No browser menu ("Inspect"…): right click folds / unfolds the island instead. Text fields keep their menu.
document.addEventListener('contextmenu', e => {
  if (e.target.closest('textarea, input')) return;
  e.preventDefault();
  setTucked(!tucked);
});
listen('toggle', () => { pinned = !pinned; render(); });
listen('chime', e => chime(e.payload));
listen('test', () => { testUntil = Date.now() + 4000; render(); setTimeout(render, 4100); });

setInterval(() => {
  if (!S) return;
  if (timer.running && timer.left() === 0) {
    timer.running = false; timer.rest = timer.total; chime('done');
    invoke('notice', { id: 'focus_done', title: 'Foco concluído!', body: `${fmtDur(timer.total)} de foco. Faça uma pausa.` });
  }
  renderBar(); renderHeader(); renderTimer(); renderProgress(); renderLyrics(); renderGuard();
  if (shouldExpand() !== island.classList.contains('expanded')) layout();
  document.querySelectorAll('.sess .time').forEach(t => {
    const end = t.dataset.end ? Number(t.dataset.end) : Date.now();
    t.textContent = fmtDur(end - Number(t.dataset.start));
  });
}, 1000);

invoke('get_state').then(s => { S = s; render(); renderTimer(); });
