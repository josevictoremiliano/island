# Documentação da Ilha

Comece por aqui. Cada arquivo responde a uma pergunta:

| Arquivo | Para quê |
|---|---|
| [arquitetura.md](arquitetura.md) | Como a Ilha é montada: janelas, módulos em Rust, eventos, onde fica cada coisa |
| [integracoes.md](integracoes.md) | Como ela conversa com o Claude Code e o Codex (ganchos, status line, permissões) |
| [desenvolvimento.md](desenvolvimento.md) | Rodar, testar, depurar e adicionar um app novo na ilha |
| [atualizacoes-e-releases.md](atualizacoes-e-releases.md) | O sistema de atualização automática e como lançar uma versão |
| [privacidade.md](privacidade.md) | O que fica no PC, o que sai e para onde |

## Em uma frase

A Ilha é um app **Tauri 2** (Rust + WebView2) com duas janelas: a **ilha** (transparente, sempre no topo, que só aceita clique onde está desenhada) e as **Configurações**. Um **servidor HTTP local** recebe os ganchos do Claude Code e do Codex; vários **módulos em Rust** leem o Windows (música, rede, volume, bateria, área de transferência) e avisam a interface por **eventos**.

## Mapa rápido

```
island/
├─ src/                    interface (sem bundler)
│  ├─ island.html|css|js   a ilha: barra recolhida, painel com abas, cartões de aviso
│  ├─ settings.html|css|js as Configurações
│  └─ sounds.js            os sons (gerados na hora, 4 pacotes)
├─ src-tauri/
│  ├─ src/                 backend em Rust (um módulo por assunto, ver arquitetura.md)
│  ├─ tauri.conf.json      configuração do app e do atualizador
│  └─ icons/
├─ scripts/release.ps1     lança uma versão
├─ .github/workflows/      CI e release (build, assinatura e publicação)
└─ docs/                   esta documentação
```
