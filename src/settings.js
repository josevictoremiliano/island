'use strict';
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = s => document.querySelector(s);
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const I = d => `<svg viewBox="0 0 24 24">${d}</svg>`;
const ICON = {
  home: I('<path d="M3 10.5L12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/>'),
  plug: I('<path d="M9 2v6M15 2v6M6 8h12v3a6 6 0 0 1-12 0zM12 17v5"/>'),
  grid: I('<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>'),
  sliders: I('<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>'),
  bell: I('<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9M10.3 21a1.94 1.94 0 0 0 3.4 0"/>'),
  island: I('<rect x="5" y="3" width="14" height="6" rx="3"/><path d="M4 14h16M7 19h10"/>'),
  check: I('<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.7 2.7L16 9.8"/>'),
  cal: I('<rect x="3" y="4" width="18" height="17" rx="2.5"/><path d="M3 9h18M8 2v4M16 2v4M9 15l2 2 4-4"/>'),
  flame: I('<path d="M12 22c4 0 7-3 7-7 0-5-5-7-5-12-3 2-6 6-6 9-1-1-2-2-2-4-1 2-1 4-1 5 0 5 3 9 7 9z"/>'),
  bot: I('<rect x="4" y="8" width="16" height="12" rx="3"/><path d="M12 4v4M9 13v2M15 13v2"/>'),
  clock: I('<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>'),
  gauge: I('<path d="M12 14l4-4"/><path d="M3.3 17a10 10 0 1 1 17.4 0"/>'),
  timer: I('<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2M10 2h4"/>'),
  note: I('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6M8 13h8M8 17h5"/>'),
  music: I('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>'),
  clip: I('<rect x="8" y="2" width="8" height="4" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>'),
  tray: I('<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.5 5h13L22 12v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6z"/>'),
  wifi: I('<path d="M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M2 9a15 15 0 0 1 20 0M12 20h.01"/>'),
  cpu: I('<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>'),
  alert: I('<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16v.5"/>'),
  link: I('<path d="M14 3h7v7M21 3l-9 9M19 14v5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h5"/>'),
  shield: I('<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>'),
  mic: I('<rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10a7 7 0 0 0 14 0M12 17v5"/>'),
  q: I('<circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 1-1 1.7M12 17h.01"/>')
};

const PAGES = {
  home: { t: 'Início', s: '', icon: 'home', sec: 'Ilha' },
  connect: { t: 'Conexões', s: 'Quais IAs a ilha acompanha para você', icon: 'plug', sec: 'Ilha' },
  apps: { t: 'Apps', s: 'Escolha o que aparece na sua ilha', icon: 'grid', sec: 'Ilha' },
  general: { t: 'Geral', s: 'Seu nome, inicialização e atalhos', icon: 'sliders', sec: 'Configurações' },
  notify: { t: 'Notificações', s: 'Como a ilha te avisa', icon: 'bell', sec: 'Configurações' },
  island: { t: 'Ilha', s: 'Cores e aparência da ilha', icon: 'island', sec: 'Configurações' }
};
const DEFAULT_COLORS = { busy: '#ff9f43', question: '#ff5d8f', done: '#3ddc97', error: '#ff4d4f' };

let S = null, page = 'home', query = '', recording = false, WEATHER = null;

// ---------- shared ----------
let toastTimer = 0;
function toast(text, bad) {
  const el = $('#toast'); el.textContent = text; el.classList.toggle('bad', !!bad); el.classList.add('on');
  clearTimeout(toastTimer); toastTimer = setTimeout(() => el.classList.remove('on'), 2200);
}
async function save(patch) {
  const cfg = structuredClone(S.cfg);
  for (const [k, v] of Object.entries(patch)) cfg[k] = (v && typeof v === 'object' && !Array.isArray(v)) ? { ...cfg[k], ...v } : v;
  try { S = await invoke('save_config', { cfg }); paintChrome(); if (S.shortcut_error) toast(S.shortcut_error, true); else toast('Salvo'); }
  catch (e) { toast(String(e), true); }
  render();
}
const sw = (id, on, disabled) => `<label class="sw"><input type="checkbox" id="${id}" ${on ? 'checked' : ''} ${disabled ? 'disabled' : ''}></label>`;
const greeting = h => h < 5 ? 'Boa noite' : h < 12 ? 'Bom dia' : h < 18 ? 'Boa tarde' : 'Boa noite';
const ago = at => {
  const d = new Date(at), diff = (Date.now() - at) / 1000;
  if (diff < 60) return 'agora';
  if (diff < 3600) return `há ${Math.floor(diff / 60)} min`;
  if (d.toDateString() === new Date().toDateString()) return d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
  return d.toLocaleDateString('pt-BR', { day: '2-digit', month: 'short' }).replace('.', '') + ' ' + d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
};
const dur = s => s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.round(s % 3600 / 60)} min` : s >= 60 ? `${Math.round(s / 60)} min` : `${s} s`;

// ---------- chrome ----------
function paintNav() {
  let html = '', sec = '';
  for (const [id, p] of Object.entries(PAGES)) {
    if (p.sec !== sec) { sec = p.sec; html += `<div class="sec">${sec}</div>`; }
    const alert = id === 'connect' && S && !S.claude.installed && !S.codex.installed;
    html += `<button data-page="${id}" class="${id === page ? 'active' : ''}">${ICON[p.icon]}${p.t}${alert ? '<i class="alert"></i>' : ''}</button>`;
  }
  $('#nav').innerHTML = html;
}
function paintChrome() {
  if (!S) return;
  const name = S.cfg.name || 'Você';
  $('#meName').textContent = name;
  $('#avatar').textContent = name.trim().charAt(0).toUpperCase();
  $('#meStats').textContent = `${S.stats.total} tarefa${S.stats.total === 1 ? '' : 's'} concluída${S.stats.total === 1 ? '' : 's'}`;
  for (const [k, v] of Object.entries(S.cfg.colors)) document.documentElement.style.setProperty('--' + k, v);
  paintNav();
}
function heroTick() {
  const d = new Date();
  $('#heroDate').textContent = d.toLocaleDateString('pt-BR', { weekday: 'short', day: '2-digit', month: 'short' }).replace(/\./g, '').toUpperCase();
  const mode = S?.cfg.clock_mode || 'auto';
  $('#heroTime').textContent = mode === '12'
    ? d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' }).replace(/\s?[AP]M$/, '')
    : mode === '24' ? d.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', hour12: false })
    : d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }).replace(/\s?[AP]M$/i, '');
}
setInterval(heroTick, 10000);

// ---------- pages ----------
function homePage() {
  const name = S.cfg.name ? ', ' + esc(S.cfg.name) : '';
  let h = `<h2>${greeting(new Date().getHours())}${name}</h2><p class="lead">A ilha fica de olho na sua IA e avisa quando ela termina ou tem uma pergunta.</p>`;
  if (!S.claude.installed && !S.codex.installed) {
    h += `<div class="card connect"><div class="ic">${ICON.plug}</div><div class="tx"><b>Conecte sua IA</b><span>Ligue o Claude Code. A ilha avisa quando ele termina e deixa você aprovar ações sem abrir o terminal.</span></div><button class="btn white" data-go="connect">Conectar</button></div>`;
  }
  const st = S.stats;
  h += `<div class="stats">
    <div class="card stat"><div class="ic" style="background:#25301f;color:#b5e08a">${ICON.check}</div><div><b>${st.total}</b><span>tarefas concluídas</span></div></div>
    <div class="card stat"><div class="ic" style="background:#1f2538;color:#9fb4ff">${ICON.cal}</div><div><b>${st.today}</b><span>hoje</span></div></div>
    <div class="card stat"><div class="ic" style="background:#3a2616;color:#ffb066">${ICON.flame}</div><div><b>${st.streak}</b><span>dias seguidos</span></div></div>
  </div>`;
  h += `<div class="acthead"><h3>Atividade</h3><div style="display:flex;gap:8px"><input class="search" id="search" placeholder="Buscar na atividade" value="${esc(query)}">${S.history.length ? '<button class="btn" id="clearHist">Limpar</button>' : ''}</div></div>`;
  const q = query.trim().toLowerCase();
  const items = S.history.filter(it => !q || [it.project, it.prompt, it.message].some(t => t.toLowerCase().includes(q)));
  if (!items.length) {
    h += `<div class="card empty">${S.history.length ? 'Nada encontrado.' : 'Nada ainda. Quando sua IA terminar uma tarefa, ela aparece aqui.'}</div>`;
  } else {
    h += `<div class="card list">${items.slice(0, 80).map(it => {
      const c = it.result === 'error' ? 'var(--error)' : 'var(--done)';
      return `<div class="item act"><div class="ic" style="background:color-mix(in srgb, ${c} 18%, transparent);color:${c}">${it.result === 'error' ? ICON.alert : ICON.check}</div>
        <div class="tx"><b>${esc(it.project)}</b>${it.prompt ? ` <span style="color:var(--muted)">· ${esc(it.prompt)}</span>` : ''}<p>${esc(it.message || 'Sem resposta em texto.')}</p></div>
        <div class="meta">${ago(it.at)}<br>${dur(it.duration_s)}${it.files ? ` · ${it.files} arq.` : ''}</div></div>`;
    }).join('')}</div>`;
  }
  return h;
}

function connectPage() {
  const c = S.claude;
  let status;
  if (!c.found) status = '<span class="warn">Claude Code não encontrado neste PC. Instale e rode <b>claude</b> uma vez no terminal.</span>';
  else if (!c.installed) status = '<span>Acompanha o Claude Code no terminal, no VS Code e na aba Code do app Claude.</span>';
  else if (!c.seen) status = '<span class="ok">Conectado. Abra uma nova sessão do Claude Code (ou reinicie a atual) e dê uma tarefa.</span>';
  else status = '<span class="ok">Conectado e recebendo eventos.</span>';
  const x = S.codex;
  let codexStatus;
  if (!x.found) codexStatus = '<span class="warn">Codex não encontrado neste PC.</span>';
  else if (!x.installed) codexStatus = '<span>Acompanha o OpenAI Codex (app, terminal ou VS Code) enquanto ele programa.</span>';
  else if (!x.seen) codexStatus = '<span class="ok">Conectado. No Codex, digite <b>/hooks</b> e confie nos ganchos da Ilha, depois comece um chat novo.</span>';
  else codexStatus = '<span class="ok">Conectado e recebendo eventos. Permissões são respondidas no próprio Codex.</span>';
  let h = `<p class="lead">Ligue o que você usa. A ilha cuida do resto: sem janelas para escolher, sem código.</p>`;
  h += `<div class="label">IA de código</div><div class="card list">
    <div class="item"><div class="ic">${ICON.bot}</div><div class="tx"><b>Claude Code</b>${status}</div>${sw('swClaude', c.installed, !c.found && !c.installed)}</div>
    <div class="item"><div class="ic">${ICON.bot}</div><div class="tx"><b>Codex</b>${codexStatus}</div>${sw('swCodex', x.installed, !x.found && !x.installed)}</div>
  </div>`;
  const acc = S.cfg.usage_from_account;
  const accStatus = !acc
    ? '<span>Mostra os limites de 5 horas e da semana também quando você usa a aba Code do app Claude.</span>'
    : S.usage_error ? `<span class="warn">${esc(S.usage_error)}</span>`
    : S.usage?.updated_at ? '<span class="ok">Ligado. Atualiza a cada 5 minutos.</span>' : '<span class="ok">Ligado. Buscando seu uso…</span>';
  h += `<div class="label">Uso do plano</div><div class="card list">
    <div class="item"><div class="ic">${ICON.gauge}</div><div class="tx"><b>Ler o uso pela sua conta Claude</b>${accStatus}</div>${sw('usage_from_account', acc)}</div>
    <div class="note">Usa o login que o Claude Code já guardou neste PC para perguntar à Anthropic quanto do limite você usou, a mesma consulta do comando <code>/usage</code>.
      Não é uma conversa com o modelo, então <b>não gasta tokens</b>. O login só é lido (nunca alterado) e só vai para <code>api.anthropic.com</code>.
      O endereço não é documentado pela Anthropic e pode mudar.</div>
  </div>`;
  h += `<div class="label">Como funciona</div><div class="card"><div class="note">
    Ao ligar, a ilha adiciona ganchos (hooks) em <code>~/.claude/settings.json</code> que avisam um servidor local
    (<code>127.0.0.1:47900</code>, protegido por token). Nada sai do seu PC. Seus próprios hooks não são tocados, e um backup
    é salvo uma vez em <code>settings.json.ilha-backup</code>. Se você já tem uma status line, ela é mantida (o anel do limite de 5h só aparece com a da ilha).
    Ao desligar, só os ganchos da ilha são removidos.</div></div>`;
  if (S.server_error) h += `<div class="alertbox">${esc(S.server_error)}</div>`;
  return h;
}

function appsPage() {
  const a = S.cfg.apps;
  const app = (id, icon, bg, fg, title, text, on, fixed) => `<div class="card app"><div class="top"><div class="ic" style="background:${bg};color:${fg}">${ICON[icon]}</div>
    <div class="tx"><b>${title}</b><span>${fixed ? 'Sempre ligado' : on ? 'Na sua ilha' : 'Fora da ilha'}</span></div>${fixed ? '<span class="pill">Sempre</span>' : sw('app_' + id, on)}</div><p>${text}</p></div>`;
  const soon = (icon, title, text) => `<div class="card app soon"><div class="top"><div class="ic" style="background:#222228;color:#bbb">${ICON[icon]}</div><div class="tx"><b>${title}</b><span>Em breve</span></div></div><p>${text}</p></div>`;
  const links = S.cfg.links || [];
  const linksCard = `<div class="card owncard">
    <div class="top"><div class="ic" style="background:#232838;color:#9fb4ff">${ICON.link}</div>
      <div class="tx"><b>Seus próprios apps</b><span>Até 4 sites ou programas. Ficam na ilha ao lado das abas. Clique em um e ele abre.</span></div>${sw('app_links', a.links)}</div>
    ${links.length ? `<div class="ownlist">${links.map((l, i) => `<div class="own"><b>${esc(l.name || l.target)}</b><span>${esc(l.target)}</span><button class="btn" data-rmlink="${i}">Remover</button></div>`).join('')}</div>` : ''}
    ${links.length < 4 ? `<div class="ownform"><input class="input" id="linkName" placeholder="Nome (ex.: YouTube)" maxlength="30"><input class="input" id="linkUrl" placeholder="Endereço (ex.: youtube.com)">
      <button class="btn white" id="addLink">Adicionar</button><button class="btn" id="pickProgram">Escolher um programa…</button></div>` : '<div class="note" style="padding:8px 0 0">Você já tem 4. Remova um para adicionar outro.</div>'}
  </div>`;
  const f = S.cfg.focus;
  const focusCard = `<div class="card app wide"><div class="top"><div class="ic" style="background:#2a2236;color:#c9a8ff">${ICON.shield}</div>
      <div class="tx"><b>Guarda do Foco</b><span>${a.focus ? 'Na sua ilha' : 'Fora da ilha'}</span></div>${sw('app_focus', a.focus)}</div>
    <p>A ilha te para um pouco quando você abre algo que distrai. Olha só o título da janela e o nome do programa, aqui no PC.</p>
    ${a.focus ? `<div class="field" style="padding:4px 0"><div class="tx"><b>Só durante o timer de foco</b><span>Desligado: vale o tempo todo.</span></div>${sw('focus_only_timer', f.only_timer)}</div>
    <div class="tx"><b style="font-size:13px">O que distrai</b><span style="font-size:12.5px;color:var(--muted)">Uma palavra por linha (site, programa ou parte do título).</span></div>
    <textarea class="input words" id="focusWords" rows="4">${esc(f.words.join('\n'))}</textarea>` : ''}
  </div>`;
  const calCard = `<div class="card app wide"><div class="top"><div class="ic" style="background:#3a1f22;color:#ff8a8a">${ICON.cal}</div>
      <div class="tx"><b>Agenda</b><span>${a.calendar ? 'Na sua ilha' : 'Fora da ilha'}</span></div>${sw('app_calendar', a.calendar)}</div>
    <p>Seus próximos eventos na ilha e um aviso 5 minutos antes. Sem login: use o endereço secreto iCal da sua agenda.</p>
    ${a.calendar ? `<div class="ownform"><input class="input" id="calUrl" style="flex:1" placeholder="https://calendar.google.com/calendar/ical/…/basic.ics" value="${esc(S.cfg.calendar_url)}"><button class="btn white" id="saveCal">Salvar</button></div>
    <details class="howto"><summary>Onde acho esse link?</summary>
      <b>Google Agenda</b> (no computador): Configurações → clique na sua agenda (à esquerda) → “Integrar agenda” → copie o <b>Endereço secreto no formato iCal</b>.<br>
      <b>Outlook</b>: Configurações → Calendário → Calendários compartilhados → Publicar um calendário → copie o link <b>ICS</b>.<br>
      Esse link dá acesso de leitura à sua agenda: não compartilhe com ninguém.</details>` : ''}
  </div>`;
  return `${linksCard}<div class="grid">
    ${app('ai', 'bot', '#26301f', '#b5e08a', 'Assistente de IA', 'Mostra se o Claude Code está trabalhando, tem uma pergunta ou terminou. Permitir e negar direto na ilha.', true, true)}
    ${app('clock', 'clock', '#24213a', '#a99bff', 'Relógio', 'Data e hora na ilha quando nada está acontecendo.', a.clock)}
    ${app('usage', 'gauge', '#2f2416', '#ffb066', 'Uso das IAs', 'Anel com quanto do limite de 5 horas e da semana você já usou, do Claude e do Codex.', a.usage)}
    ${app('timer', 'timer', '#3a2616', '#ffb066', 'Timer de foco', 'Um timer para trabalho focado, dentro da ilha.', a.timer)}
    ${app('note', 'note', '#33301a', '#f0d66e', 'Nota rápida', 'Uma nota sempre à mão, salva automaticamente.', a.note)}
    ${app('music', 'music', '#16301f', '#6ee7a0', 'Música', 'O que está tocando, com capa e controles. Spotify, navegador e qualquer player do Windows.', a.music)}
    ${app('system', 'cpu', '#1f2538', '#9fb4ff', 'Sistema', 'CPU, memória e bateria num relance, no rodapé da ilha.', a.system)}
    ${app('clipboard', 'clip', '#16302e', '#6ee0d6', 'Área de transferência', 'Suas últimas cópias de texto. Clique em uma para copiar de novo. Fica só na memória e ignora senhas.', a.clipboard)}
    ${app('shelf', 'tray', '#1f2538', '#9fb4ff', 'Bandeja de arquivos', 'Solte arquivos na ilha e arraste de volta para onde quiser depois.', a.shelf)}
    ${app('net', 'wifi', '#1c2a40', '#7fb2ff', 'Wi‑Fi e Bluetooth', 'Ligar, desligar e ver a rede e os dispositivos conectados.', a.net)}
    ${app('lyrics', 'mic', '#16301f', '#6ee7a0', 'Letras', 'A letra da música que está tocando, sincronizada quando existir. Busca em lrclib.net enviando só título, artista e duração.', a.lyrics)}
  </div>
  <div class="grid" style="margin-top:12px">${focusCard}${calCard}</div>`;
}

function generalPage() {
  const c = S.cfg;
  return `<div class="label">Você</div><div class="card list">
    <div class="field"><div class="tx"><b>Seu nome</b><span>Usado na saudação. Fica só neste PC.</span></div><input class="input" id="name" value="${esc(c.name)}" maxlength="40" placeholder="Seu nome"></div>
  </div>
  <div class="label">Comportamento</div><div class="card list">
    <div class="field"><div class="tx"><b>Iniciar com o Windows</b><span>A ilha abre sozinha quando você liga o PC.</span></div>${sw('autostart', c.autostart)}</div>
    <div class="field"><div class="tx"><b>Esconder em tela cheia</b><span>Some durante jogos, vídeos e apresentações em tela cheia.</span></div>${sw('hide_fullscreen', c.hide_fullscreen)}</div>
  </div>
  <div class="label">Atalhos de teclado</div><div class="card list">
    <div class="field"><div class="tx"><b>Abrir e fixar a ilha</b><span>${recording ? 'Pressione as teclas… (Esc cancela, Delete desliga)' : 'Clique para gravar um novo atalho.'}</span>${S.shortcut_error ? `<span class="bad">${esc(S.shortcut_error)}</span>` : ''}</div>
      <button class="input key ${recording ? 'rec' : ''}" id="shortcut">${recording ? '…' : esc(c.shortcut || 'Desligado')}</button></div>
  </div>
  ${updateBlock()}
  <div class="label">Sobre</div><div class="card list">
    <div class="field"><div class="tx"><b>Ilha ${esc(S.version)}</b><span>Feita em Rust + Tauri. Tudo roda no seu PC. <a href="#" id="repo">Código no GitHub</a></span></div><button class="btn" id="quit">Sair da Ilha</button></div>
  </div>`;
}

// updates from GitHub Releases: status, progress, buttons
let UPD = null;
function updateBlock() {
  const u = UPD || { status: 'idle', current: S.version };
  const pct = u.total ? Math.min(100, Math.round(u.downloaded / u.total * 100)) : 0;
  const when = u.checked_at ? new Date(u.checked_at).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' }) : '';
  const text = {
    idle: 'Ainda não procurei nesta sessão. A Ilha procura sozinha 30 segundos depois de abrir e a cada 6 horas.',
    checking: 'Procurando no GitHub…',
    latest: `Você está na versão mais nova${when ? ` (conferido às ${when})` : ''}.`,
    downloading: `Baixando a versão v${esc(u.version)}… ${pct}%`,
    ready: `A versão v${esc(u.version)} está baixada e pronta para instalar.`,
    installing: `Instalando a versão v${esc(u.version)}…`,
    error: esc(u.error || 'Algo deu errado.')
  }[u.status] || '';
  const btn = u.status === 'ready' ? '<button class="btn white" id="updInstall">Instalar agora</button>'
    : `<button class="btn" id="updCheck" ${['checking', 'downloading', 'installing'].includes(u.status) ? 'disabled' : ''}>Procurar agora</button>`;
  return `<div class="label">Atualizações</div><div class="card list">
    <div class="field"><div class="tx"><b>Versão ${esc(u.current || S.version)}</b><span class="${u.status === 'error' ? 'warn' : u.status === 'latest' ? 'ok' : ''}">${text}</span>
      ${u.status === 'downloading' ? `<div class="bar upd"><i style="width:${pct}%"></i></div>` : ''}
      ${u.notes && ['downloading', 'ready'].includes(u.status) ? `<details class="howto"><summary>O que mudou</summary>${esc(u.notes)}</details>` : ''}</div>${btn}</div>
    <div class="field"><div class="tx"><b>Atualizar sozinho</b><span>Baixa e instala novas versões quando nenhuma IA estiver trabalhando ou esperando você.</span></div>${sw('auto_update', S.cfg.auto_update !== false)}</div>
  </div>`;
}

// every kind of notice: where it shows is your choice (a card under the island, Windows' own, or off)
const NOTICES = [
  { g: 'IA', id: 'ai_permission', t: 'Pedido de permissão', d: 'O Claude Code quer rodar algo e espera por você', ex: ['Claude Code quer permissão', 'island: Rodando npm test'] },
  { g: 'IA', id: 'ai_question', t: 'Pergunta da IA', d: 'A IA não continua sem uma resposta sua', ex: ['Claude Code tem uma pergunta', 'island: Qual arquivo devo mudar?'] },
  { g: 'IA', id: 'ai_done', t: 'IA terminou', d: 'Uma tarefa foi concluída', ex: ['Claude Code terminou', 'island: corrigi o total do carrinho'] },
  { g: 'IA', id: 'ai_error', t: 'IA com erro', d: 'Uma tarefa falhou', ex: ['Claude Code: erro', 'island: npm test falhou'] },
  { g: 'IA', id: 'limit', t: 'Limite quase cheio', d: 'Quando o uso do limite de 5h chega no nível abaixo', ex: ['Você usou 80% do limite de 5h do Claude', 'Às 14:30 ele volta a encher.'] },
  { g: 'Foco', id: 'focus_done', t: 'Timer de foco acabou', d: 'Seu timer chegou ao fim', ex: ['Foco concluído!', '25:00 de foco. Faça uma pausa.'] },
  { g: 'Foco', id: 'guard_back', t: 'De volta ao foco', d: 'Você voltou em vez de abrir a distração', ex: ['Boa! De volta ao foco', 'Você voltou em vez de abrir YouTube.'] },
  { g: 'Agenda', id: 'calendar', t: 'Evento em 10 minutos', d: '10 minutos antes de um evento', ex: ['Em 10 min: Reunião do time', 'Às 15:00 · Google Meet'] },
  { g: 'Sistema', id: 'battery', t: 'Bateria baixa', d: 'Em 20% e em 10%', ex: ['Bateria em 20%', 'Conecte o carregador.'] },
  { g: 'Sistema', id: 'update', t: 'Atualização', d: 'Nova versão encontrada, pronta ou instalando', ex: ['Nova versão disponível: v0.2.0', 'Baixando em segundo plano…'] },
  { g: 'Sistema', id: 'keys_taken', t: 'Atalho ocupado', d: 'Outro app já usa as teclas que você escolheu', ex: ['Atalho ocupado', 'Não consegui usar o atalho Ctrl+Alt+I'] }
];
function notifyPage() {
  const c = S.cfg, modes = c.notices || {};
  let rows = '', group = '';
  for (const n of NOTICES) {
    if (n.g !== group) { if (group) rows += '</div>'; group = n.g; rows += `<div class="label">${group}</div><div class="card list">`; }
    const m = modes[n.id] || 'card';
    rows += `<div class="field"><div class="tx"><b>${n.t}</b><span>${n.d}</span></div>
      <div class="seg">${[['card', 'Cartão'], ['windows', 'Windows'], ['off', 'Desligado']].map(([v, l]) => `<button data-nmode="${n.id}" data-v="${v}" class="${m === v ? 'on' : ''}">${l}</button>`).join('')}</div>
      <button class="btn" data-ntest="${n.id}" ${m === 'off' ? 'disabled' : ''}>Testar</button></div>`;
  }
  rows += '</div>';
  return `<p class="lead"><b>Cartão</b> aparece na tela, logo abaixo da ilha, e some sozinho. <b>Windows</b> usa a notificação do sistema. Com a ilha escondida (tela cheia), o cartão vira notificação do Windows.</p>
  <div class="label">Notificações e som</div><div class="card list">
    <div class="field"><div class="tx"><b>Volume dos sons da Ilha</b><span>Só os sons da Ilha. O volume do PC continua no Windows.</span></div>
      <input type="range" class="range" id="sound_volume" min="0" max="100" step="5" value="${c.sound_volume ?? 70}"><b class="rangev" id="soundV">${c.sound_volume ?? 70}%</b></div>
    <div class="field"><div class="tx"><b>Sons de semáforo</b><span>Ding ao terminar, pling para pergunta, tom grave para erro.</span></div>${sw('sound', c.sound)}</div>
    <div class="field"><div class="tx"><b>Pacote de sons</b><span>Como soam esses sons (clique para ouvir)</span></div>
      <div class="seg">${[['classic', 'Clássico'], ['8bit', '8-bit'], ['soft', 'Suave'], ['bells', 'Sinos']].map(([v, l]) => `<button data-pack="${v}" class="${(c.sound_pack || 'classic') === v ? 'on' : ''}">${l}</button>`).join('')}</div></div>
    <div class="field"><div class="tx"><b>Abrir a ilha sozinha</b><span>Quando o Claude Code pede permissão, a ilha se abre com os botões.</span></div>${sw('auto_expand', c.auto_expand)}</div>
    <div class="field"><div class="tx"><b>Avisar do limite em</b><span>Uso do limite de 5 horas do Claude ou do Codex.</span></div>
      <div class="seg">${[[70, '70%'], [80, '80%'], [90, '90%'], [0, 'Nunca']].map(([v, l]) => `<button data-limit="${v}" class="${(c.limit_warn ?? 80) === v ? 'on' : ''}">${l}</button>`).join('')}</div></div>
  </div>${rows}`;
}

function islandPage() {
  const c = S.cfg.colors;
  const row = (k, title, text) => `<div class="field"><div class="tx"><b>${title}</b><span>${text}</span></div><input type="color" data-color="${k}" value="${esc(c[k])}"></div>`;
  const f = S.cfg;
  const mode = f.open_mode === 'click' ? 'click' : 'hover';
  // a row of choices that saves `key` = value
  const seg = (key, current, options) => `<div class="seg wrap">${options.map(([v, l]) => `<button data-set="${key}" data-v="${v}" class="${String(current) === String(v) ? 'on' : ''}">${l}</button>`).join('')}</div>`;
  const SLOTS = [['none', 'Nada'], ['date', 'Data'], ['clock', 'Hora'], ['weather', 'Clima'], ['battery', 'Bateria'], ['cpu', 'CPU'], ['ram', 'RAM'], ['net', 'Rede'], ['usage', 'Uso da IA']];
  const transp = Math.min(80, f.transparency || 0);
  const w = WEATHER;
  return `<div class="label">Visual</div><div class="card list">
      <div class="field"><div class="tx"><b>Tamanho</b><span>O tamanho da ilha</span></div>${seg('size', f.size || 'normal', [['compact', 'Compacto'], ['normal', 'Normal'], ['large', 'Grande']])}</div>
      <div class="field"><div class="tx"><b>Estilo</b><span>Colada na borda de cima como o notch de um notebook, ou flutuando logo abaixo como no iPhone</span></div>${seg('style', f.style || 'attached', [['attached', 'Colada'], ['floating', 'Flutuante']])}</div>
      <div class="field"><div class="tx"><b>Material</b><span>Preto, ou vidro colorido: translúcido, com um brilho na cor da sua música ou do que a IA está fazendo</span></div>${seg('material', f.material || 'black', [['black', 'Preto'], ['glass', 'Vidro colorido']])}</div>
      <div class="field"><div class="tx"><b>Transparência</b><span>Quanto dá para ver através da ilha</span></div>
        <input type="range" class="range" id="transparency" min="0" max="80" step="5" value="${transp}"><b class="rangev" id="transpV">${transp}%</b></div>
      <div class="field"><div class="tx"><b>Cor da música</b><span>No vidro colorido, a ilha pega a cor da capa do álbum que está tocando</span></div>${sw('music_tint', f.music_tint !== false)}</div>
      <div class="field"><div class="tx"><b>Abrir</b><span>${mode === 'hover' ? 'Abre quando o mouse descansa sobre ela por um instante. Só passar por cima não abre.' : 'Abre só quando você clica nela.'}</span></div>
        <div class="seg"><button data-open="hover" class="${mode === 'hover' ? 'on' : ''}">Ao passar</button><button data-open="click" class="${mode === 'click' ? 'on' : ''}">Ao clicar</button></div></div>
    </div>
    <div class="label">Ilha em repouso</div><div class="card list">
      <div class="field stack"><div class="tx"><b>À esquerda</b><span>O que mostra quando não tem nada rolando</span></div>${seg('rest_left', f.rest_left || 'date', SLOTS)}</div>
      <div class="field"><div class="tx"><b>Hora no meio</b><span>O relógio no centro da ilha</span></div>${sw('rest_clock', f.rest_clock !== false)}</div>
      <div class="field stack"><div class="tx"><b>À direita</b><span>O que mostra quando não tem nada rolando</span></div>${seg('rest_right', f.rest_right || 'usage', SLOTS)}</div>
      <div class="field"><div class="tx"><b>Relógio</b><span>O formato da hora, na ilha e nas Configurações</span></div>${seg('clock_mode', f.clock_mode || 'auto', [['auto', 'Automático'], ['12', '12 horas · 6:30'], ['24', '24 horas · 18:30']])}</div>
    </div>
    <div class="label">Na ilha</div><div class="card list">
      <div class="field"><div class="tx"><b>Barra de volume</b><span>Mostra o volume na ilha quando você muda</span></div>${sw('volume_bar', f.volume_bar !== false)}</div>
      <div class="field"><div class="tx"><b>Clima</b><span>${w?.error ? `<span class="warn">${esc(w.error)}</span>` : w?.temp != null ? `Agora em ${esc(w.city)}: ${Math.round(w.temp)}° (mín ${Math.round(w.min)}° · máx ${Math.round(w.max)}°). Escolha "Clima" à esquerda ou à direita.` : 'Digite sua cidade. Busca no Open-Meteo, que é gratuito; só o nome da cidade vai para lá.'}</span></div>
        <input class="input" id="city" placeholder="Ex.: São Paulo" value="${esc(f.weather_city || '')}"><button class="btn" id="saveCity">Salvar</button></div>
    </div>
    <div class="label">Prévia das cores</div><div class="card preview">
      <div class="pv" style="--c:${c.busy}"><i></i>meu-projeto · Editando main.rs</div>
      <div class="pv" style="--c:${c.question}"><i></i>meu-projeto quer permissão</div>
      <div class="pv" style="--c:${c.done}"><i></i>meu-projeto pronto</div>
      <div class="pv" style="--c:${c.error}"><i></i>meu-projeto: erro</div>
    </div>
    <div class="label">Cores</div><div class="card list">
      ${row('busy', 'Trabalhando', 'Enquanto a IA lê, edita e roda comandos')}
      ${row('question', 'Precisa de você', 'Permissão ou pergunta')}
      ${row('done', 'Pronto', 'Quando a tarefa termina')}
      ${row('error', 'Erro', 'Quando algo dá errado')}
    </div>
    <div style="margin-top:14px"><button class="btn" id="resetColors">Voltar às cores padrão</button></div>`;
}

const RENDER = { home: homePage, connect: connectPage, apps: appsPage, general: generalPage, notify: notifyPage, island: islandPage };

function render(fresh) {
  if (!S) return;
  const p = PAGES[page];
  $('#title').textContent = page === 'home' ? 'Início' : p.t;
  $('#sub').textContent = p.s;
  $('#hero').classList.toggle('slim', page !== 'home');
  const box = $('#page');
  const focusId = document.activeElement?.id, caret = document.activeElement?.selectionStart;
  box.innerHTML = RENDER[page]();
  if (fresh) { box.style.animation = 'none'; box.offsetHeight; box.style.animation = ''; $('#scroll').scrollTop = 0; }
  if (focusId && document.getElementById(focusId)) {
    const el = document.getElementById(focusId); el.focus();
    if (caret != null && el.setSelectionRange) el.setSelectionRange(caret, caret);
  }
  paintNav();
}
function go(name) { page = name; recording = false; render(true); }

// ---------- events ----------
$('#nav').addEventListener('click', e => { const b = e.target.closest('[data-page]'); if (b) go(b.dataset.page); });
$('#page').addEventListener('click', async e => {
  const t = e.target;
  if (t.closest('[data-go]')) return go(t.closest('[data-go]').dataset.go);
  if (t.id === 'clearHist') { await invoke('clear_history'); toast('Atividade limpa'); return; }
  if (t.dataset.pack) {
    // hear it right away: done, then a question
    window.playChime('done', t.dataset.pack, S.cfg.sound_volume ?? 70);
    setTimeout(() => window.playChime('question', t.dataset.pack, S.cfg.sound_volume ?? 70), 700);
    return save({ sound_pack: t.dataset.pack });
  }
  if (t.dataset.nmode) return save({ notices: { ...(S.cfg.notices || {}), [t.dataset.nmode]: t.dataset.v } });
  if (t.dataset.limit != null) return save({ limit_warn: Number(t.dataset.limit) });
  if (t.dataset.ntest) {
    const n = NOTICES.find(x => x.id === t.dataset.ntest);
    await invoke('notice', { id: n.id, title: n.ex[0], body: n.ex[1] });
    toast((S.cfg.notices?.[n.id] || 'card') === 'card' ? 'Olhe embaixo da ilha 👀' : 'Notificação do Windows enviada');
    return;
  }
  if (t.id === 'resetColors') return save({ colors: DEFAULT_COLORS });
  if (t.dataset.open) return save({ open_mode: t.dataset.open });
  if (t.dataset.set) return save({ [t.dataset.set]: t.dataset.v });
  if (t.id === 'saveCity') { await save({ weather_city: $('#city').value.trim() }); return invoke('weather_refresh'); }
  if (t.id === 'quit') return invoke('quit');
  if (t.id === 'updCheck') { invoke('update_check'); return; }
  if (t.id === 'updInstall') { invoke('update_install'); return; }
  if (t.id === 'repo') { e.preventDefault(); invoke('open_link', { target: 'https://github.com/josevictoremiliano/island' }); return; }
  if (t.id === 'addLink') {
    const name = $('#linkName').value.trim(), target = $('#linkUrl').value.trim();
    if (!target) return toast('Cole um endereço (ex.: youtube.com) ou escolha um programa.', true);
    return save({ links: [...(S.cfg.links || []), { name: name || target.replace(/^https?:\/\/(www\.)?/, '').split('/')[0], target }] });
  }
  if (t.id === 'pickProgram') {
    const path = await invoke('pick_program');
    if (!path) return;
    const name = path.split('\\').pop().replace(/\.(exe|lnk|bat|cmd|url)$/i, '');
    return save({ links: [...(S.cfg.links || []), { name, target: path }] });
  }
  if (t.dataset.rmlink) return save({ links: S.cfg.links.filter((_, i) => i !== Number(t.dataset.rmlink)) });
  if (t.id === 'saveCal') {
    await save({ calendar_url: $('#calUrl').value.trim() });
    return invoke('calendar_refresh');
  }
  if (t.id === 'shortcut') { recording = true; render(); $('#shortcut')?.focus(); }
});
$('#page').addEventListener('change', async e => {
  const t = e.target;
  if (t.id === 'swClaude') {
    try { S = await invoke('set_claude', { on: t.checked }); toast(t.checked ? 'Claude Code conectado' : 'Claude Code desconectado'); }
    catch (err) { toast(String(err), true); }
    paintChrome(); render(); return;
  }
  if (t.id === 'swCodex') {
    try { S = await invoke('set_codex', { on: t.checked }); toast(t.checked ? 'Codex conectado' : 'Codex desconectado'); }
    catch (err) { toast(String(err), true); }
    paintChrome(); render(); return;
  }
  if (t.id === 'focus_only_timer') return save({ focus: { only_timer: t.checked } });
  if (t.id === 'focusWords') return save({ focus: { words: t.value.split('\n').map(w => w.trim()).filter(Boolean) } });
  if (t.id?.startsWith('app_')) return save({ apps: { [t.id.slice(4)]: t.checked } });
  if (t.id === 'transparency') return save({ transparency: Number(t.value) });
  if (t.id === 'sound_volume') {
    window.playChime('done', S.cfg.sound_pack, Number(t.value));
    return save({ sound_volume: Number(t.value) });
  }
  if (['auto_update', 'music_tint', 'rest_clock', 'volume_bar', 'autostart', 'hide_fullscreen', 'usage_from_account', 'hour12', 'notify', 'notify_question', 'sound', 'auto_expand'].includes(t.id)) return save({ [t.id]: t.checked });
  if (t.id === 'name') return save({ name: t.value.trim() });
  if (t.dataset.color) return save({ colors: { [t.dataset.color]: t.value } });
});
$('#page').addEventListener('input', e => {
  if (e.target.id === 'transparency') { $('#transpV').textContent = e.target.value + '%'; return; }
  if (e.target.id === 'sound_volume') { $('#soundV').textContent = e.target.value + '%'; return; }
  if (e.target.id === 'search') { query = e.target.value; render(); }
});
document.addEventListener('keydown', e => {
  if (!recording) return;
  e.preventDefault();
  if (e.key === 'Escape') { recording = false; return render(); }
  if (e.key === 'Delete' || e.key === 'Backspace') { recording = false; return save({ shortcut: '' }); }
  if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;
  const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean);
  if (!mods.length) return toast('Use também Ctrl, Alt, Shift ou Win.', true);
  let key = e.code.replace(/^Key/, '').replace(/^Digit/, '');
  if (!/^([A-Z0-9]|F\d{1,2}|Space)$/.test(key)) return toast('Escolha uma letra, número ou tecla F.', true);
  recording = false;
  save({ shortcut: [...mods, key].join('+') });
});

listen('state', e => {
  S = e.payload; paintChrome();
  const typing = ['INPUT', 'TEXTAREA'].includes(document.activeElement?.tagName) && document.activeElement.type !== 'checkbox';
  if (!typing && !recording) render();
});

// no browser menu ("Inspect"…) outside text fields
document.addEventListener('contextmenu', e => { if (!e.target.closest('input, textarea')) e.preventDefault(); });

listen('update', e => { UPD = e.payload; if (page === 'general' && !recording) render(); });
invoke('update_get').then(u => { UPD = u; });
listen('weather', e => { WEATHER = e.payload; if (page === 'island' && !['INPUT'].includes(document.activeElement?.tagName)) render(); });
invoke('get_weather').then(w => { WEATHER = w; });
invoke('get_state').then(s => { S = s; paintChrome(); heroTick(); render(true); });
