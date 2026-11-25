# Desenvolvimento

## Preparar

- Windows 10 ou 11 com WebView2 (já vem no Windows 11)
- [Rust](https://rustup.rs) estável (MSVC) e [Node.js](https://nodejs.org) 20+

```powershell
npm install        # só o CLI do Tauri
npm run dev        # compila e abre a Ilha (recompila ao salvar arquivos do Rust)
```

A primeira compilação leva alguns minutos. A interface (`src/`) é embutida no binário: mudou HTML/CSS/JS, rode `cargo build` de novo (o `npm run dev` faz isso sozinho).

## Testes

```powershell
cd src-tauri
cargo test         # inclui o leitor de agenda iCal (eventos repetidos, exclusões, cancelados)
cargo check        # o mesmo que o CI roda
```

## Depurar

- `src-tauri\target\debug\ilha.exe --probe` imprime o que a Ilha lê agora: música, sistema, tela cheia, rede, uso.
- Os dados ficam em `%APPDATA%\dev.ilha.desktop\`: `config.json`, `history.json`, `usage.json`, `shelf.json`, `note.txt`, `token`.
- O build de depuração abre um console com as mensagens do Rust; `F12` não funciona de propósito (o menu de navegador está bloqueado).
- Para simular o Claude Code, veja o fim de [integracoes.md](integracoes.md).

## Adicionar um app novo na ilha (receita)

1. **Config**: um campo em `Apps` (e o padrão) em `config.rs`.
2. **Rust** (se precisar ler algo do Windows): um módulo `novo.rs` com `spawn(app, core)` que roda em thread e faz `app.emit("novo", dados)`, mais um `static CURRENT` e um comando `get_novo` em `main.rs`.
3. **Ilha**: em `island.html` uma aba (`<button data-tab="novo">` e `<section id="tab-novo">`); em `island.js` um `renderNovo()`, um `listen('novo', …)` e o nome no mapa `allowed` de `renderHeader`.
4. **Configurações**: um cartão em `appsPage()` (`settings.js`).
5. **Avisos** (se o app avisar algo): `core.notice("novo", título, texto)`, mais uma linha em `NOTICES` (`settings.js`) e em `NOTICE` (`island.js`).

## Convenções

- Textos da interface em **português do Brasil**; comentários do código em inglês curto.
- Nada sai do PC sem a pessoa ligar a opção e sem ser dito na tela para onde vai (veja [privacidade.md](privacidade.md)).
- Toda resposta aos ganchos das IAs é JSON válido.
