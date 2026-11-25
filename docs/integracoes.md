# Integração com o Claude Code e o Codex

Nenhuma das duas IAs precisa de plugin: as duas sabem chamar **ganchos** (hooks) em momentos do trabalho. A Ilha só se registra como um desses ganchos.

## Claude Code

Ao ligar em **Configurações → Conexões**, `claude.rs` acrescenta em `~/.claude/settings.json`:

- **Ganchos HTTP** para `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `Notification`, `Stop`, `StopFailure` e `SessionEnd`, todos apontando para `http://127.0.0.1:47900/hook?agent=claude` com o cabeçalho `X-Island-Token`.
- Uma **status line** (`curl.exe … /status`) que manda os limites de 5 h e da semana, só se você ainda não tiver uma status line sua.

Regras:

- Só as entradas que apontam para o servidor da Ilha são tocadas; seus ganchos continuam como estão. Um backup é feito uma vez em `settings.json.ilha-backup`.
- Ao desligar, só os ganchos da Ilha saem.
- Toda resposta é um JSON válido. `{}` quer dizer "não decido nada, siga normalmente". Uma resposta vazia fazia o Claude Code avisar *"A hook gave no usable answer"*.

### Permissões

O `PermissionRequest` é o único gancho que **espera**: o servidor segura a resposta (até ~10 min) enquanto você decide na ilha.

| Botão | Resposta ao Claude Code |
|---|---|
| Permitir | `{"behavior":"allow"}` |
| Sempre permitir | `allow` + `updatedPermissions` com as regras que o próprio Claude Code sugeriu |
| Negar | `{"behavior":"deny","message":…}` |
| (respondeu no terminal) | a conexão fecha e o cartão some da ilha |

Se a Ilha estiver **fechada**, o gancho não tem quem responda; o Claude Code segue normalmente, mas pode mostrar um aviso. Por isso vale ligar *Iniciar com o Windows*.

### Uso do plano (anéis)

A status line só roda no **terminal**. Na aba Code do app Claude ela não roda; por isso existe a opção *Ler o uso pela sua conta* (`account_usage.rs`), que só funciona com o login do Claude Code do terminal (`claude` → `/login`).

## Codex

O Codex lê `~/.codex/hooks.json`, no mesmo formato, mas só aceita ganchos de **comando**. `codex.rs` registra, para `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop` e `SessionEnd`:

```
curl.exe -s -m 3 --noproxy 127.0.0.1 -H X-Island-Token:<token> -H Content-Type:application/json -d@- http://127.0.0.1:47900/codex
```

Sem aspas, sem `&` e sem `@` no começo de uma palavra, porque versões antigas do Codex e o PowerShell quebram com isso. Os ganchos rodam em segundo plano (`async`), então **as permissões do Codex são respondidas no próprio Codex**; a ilha só mostra que ele está esperando.

O Codex pede para você **confiar** em ganchos novos: no Codex, digite `/hooks` e confie nos da Ilha.

O uso do plano do Codex vem dos arquivos de sessão dele (`~/.codex/sessions/AAAA/MM/DD/rollout-*.jsonl`, campo `rate_limits`); `codex_usage.rs` lê o mais recente. Quando a janela de 5 h já reiniciou, o Codex não está em uso e a ilha esconde esses números.

## Testar sem uma IA de verdade

Com a Ilha aberta, mande um evento como o Claude Code mandaria:

```powershell
$t = Get-Content "$env:APPDATA\dev.ilha.desktop\token"
'{"hook_event_name":"UserPromptSubmit","session_id":"teste","cwd":"C:/projeto","prompt":"Olá"}' |
  curl.exe -s -H "X-Island-Token: $t" -H "Content-Type: application/json" --data-binary '@-' "http://127.0.0.1:47900/hook?agent=claude"
'{"hook_event_name":"Stop","session_id":"teste","cwd":"C:/projeto","last_assistant_message":"Pronto."}' |
  curl.exe -s -H "X-Island-Token: $t" -H "Content-Type: application/json" --data-binary '@-' "http://127.0.0.1:47900/hook?agent=claude"
```
