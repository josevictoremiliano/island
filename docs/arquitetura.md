# Arquitetura

## Janelas

| Janela | Rótulo | Como é |
|---|---|---|
| Ilha | `island` | 720×560, sem borda, **transparente**, sempre no topo, fora da barra de tarefas, centralizada no topo do monitor principal |
| Configurações | `settings` | Janela normal, criada sob demanda (bandeja, engrenagem, segunda instância) |

A janela da ilha é bem maior que a pílula visível. Para os cliques passarem pela parte vazia, `hover.rs` confere a posição do mouse a cada 40 ms e liga/desliga `set_ignore_cursor_events`. A interface informa quais retângulos são "a ilha" e "os cartões de aviso" com `set_hit_rects`. Só repousar o mouse sobre **a ilha** a abre; os cartões são só clicáveis.

## Backend (`src-tauri/src`)

| Módulo | Faz |
|---|---|
| `main.rs` | Inicia o app, plugins, janelas, ícone da bandeja e registra os comandos chamados pela interface |
| `core.rs` | O estado central: sessões das IAs, permissões esperando resposta, histórico, uso, avisos (`notice`) e configurações |
| `config.rs` | `Config` salva em `%APPDATA%\dev.ilha.desktop\config.json` (todos os campos têm padrão) |
| `server.rs` | Servidor HTTP em `127.0.0.1:47900`: `/hook` (Claude Code), `/codex`, `/status` (status line). Exige o token |
| `claude.rs` / `codex.rs` | Liga e desliga os ganchos em `~/.claude/settings.json` e `~/.codex/hooks.json` |
| `hover.rs` | Clique atravessando a parte vazia da janela |
| `media.rs` | Música (controles de mídia do Windows, WinRT): título, capa, posição, tocar/pausar/pular |
| `lyrics.rs` | Letras do lrclib.net |
| `system.rs` | CPU, RAM, bateria, esconder em tela cheia, aviso de bateria baixa |
| `volume.rs` | Volume principal (Core Audio) para a barra de volume |
| `net.rs` | Wi‑Fi e Bluetooth (WinRT Radios) e a rede atual |
| `clipboard.rs` | Histórico de cópias (só na memória, ignora conteúdo marcado como privado) |
| `shelf.rs` | Bandeja de arquivos (guarda só caminhos) e arrastar para fora (`drag`) |
| `links.rs` | Seus próprios sites e programas |
| `focus.rs` | Guarda do Foco (título da janela em primeiro plano) |
| `calendar.rs` | Agenda iCal (eventos repetidos via `rrule`) e aviso 10 min antes |
| `weather.rs` | Clima pelo Open-Meteo |
| `account_usage.rs` | Opcional: uso do plano Claude pelo login do Claude Code (só se você ligar) |
| `codex_usage.rs` | Uso do plano do Codex lido dos arquivos de sessão dele |
| `updater.rs` | Atualização automática pelo GitHub Releases |

Cada módulo que observa algo roda numa **thread própria** (ou tarefa assíncrona) e manda o resultado para a interface com `app.emit("nome", dados)`. Os módulos guardam o último valor numa `static Mutex`, para a interface pedir com `get_*` ao abrir.

## Interface (`src/`)

HTML, CSS e JavaScript puros, sem bundler. A API do Tauri vem de `window.__TAURI__` (`withGlobalTauri`).

- **Comandos** (`invoke('nome', {...})`): a interface pede algo ao Rust (`decide`, `save_config`, `media_control`…).
- **Eventos** (`listen('nome', ...)`): o Rust avisa mudanças (`state`, `media`, `notice`, `update`…).

O evento `state` carrega o `Snapshot` inteiro (sessões, uso, histórico, config). Os outros carregam só o próprio assunto.

## Avisos

Tudo que avisa passa por `Core::notice(id, título, texto)`. A escolha por tipo (cartão, Windows ou desligado) fica em `config.notices`. Com a ilha escondida (tela cheia), um cartão vira notificação do Windows. A lista de tipos está em `settings.js` (`NOTICES`) e as cores/ícones em `island.js` (`NOTICE`).
