# Atualizações e releases

## Como a Ilha se atualiza

```
Ilha instalada ──(30 s após abrir, depois a cada 6 h)──▶ GitHub: releases/latest/download/latest.json
      │                                                        │ versão maior que a minha?
      ▼                                                        ▼
aviso "Nova versão disponível" ◀── sim ── baixa o instalador mostrando o progresso (cartão + Configurações)
      │
      ▼
confere a assinatura com a chave pública (tauri.conf.json → plugins.updater.pubkey)
      │
      ▼
espera nenhuma IA estar trabalhando/esperando ──▶ instala (instalador NSIS em modo passivo) ──▶ a Ilha reabre
```

- `src-tauri/src/updater.rs` faz tudo isso, com o plugin oficial `tauri-plugin-updater`.
- **Configurações → Geral → Atualizações** mostra a versão, o estado e o progresso, e tem os botões *Procurar agora* / *Instalar agora*.
- *Atualizar sozinho* desligado: a Ilha baixa e avisa, mas só instala quando você clicar.
- Um instalador que não foi assinado com a chave do projeto é **recusado**. Ninguém consegue empurrar uma versão falsa.

## A chave de assinatura

| Parte | Onde fica |
|---|---|
| Chave **pública** | `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`), no repositório |
| Chave **privada** | no PC de quem publica, em `~/.tauri/ilha.key`, e no segredo `TAURI_SIGNING_PRIVATE_KEY` do repositório no GitHub |

**Nunca** coloque a chave privada no repositório. Se ela se perder, as Ilhas já instaladas não aceitam mais atualizações (seria preciso reinstalar manualmente uma versão com uma chave nova). Guarde uma cópia em lugar seguro.

Gerar uma chave nova (só se precisar trocar):

```powershell
npx tauri signer generate --ci -w "$env:USERPROFILE\.tauri\ilha.key"
gh secret set TAURI_SIGNING_PRIVATE_KEY --repo josevictoremiliano/island < "$env:USERPROFILE\.tauri\ilha.key"
# e troque a pubkey em tauri.conf.json pelo conteúdo de ilha.key.pub
```

## Lançar uma versão

1. Escreva o que mudou no `CHANGELOG.md`, numa seção `## [0.2.0] - AAAA-MM-DD`, e faça o commit.
2. Rode:

   ```powershell
   .\scripts\release.ps1 0.2.0
   ```

   O script confere se está tudo commitado e na `main`, troca a versão em `package.json`, `tauri.conf.json` e `Cargo.toml`, faz o commit, cria a tag `v0.2.0` e envia.
3. O workflow **Release** (`.github/workflows/release.yml`) roda no Windows do GitHub: compila em modo release, assina, cria a Release com o instalador `Ilha_0.2.0_x64-setup.exe`, a assinatura `.sig` e o `latest.json`.
4. Em até 6 horas todas as Ilhas abertas encontram a versão nova (ou na hora, com *Procurar agora*).

Acompanhe em [Actions](https://github.com/josevictoremiliano/island/actions). O build leva ~10–15 minutos (menos com o cache).

## Permissões do repositório

- O workflow de release usa `permissions: contents: write` para criar a Release com o `GITHUB_TOKEN` automático (não precisa de token pessoal).
- O CI usa só `contents: read`.
- Segredos: `TAURI_SIGNING_PRIVATE_KEY` (obrigatório) e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (vazio: a chave atual não tem senha).
