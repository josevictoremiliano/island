# Changelog

Todas as mudanças da Ilha. O formato segue [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e as versões seguem [SemVer](https://semver.org/lang/pt-BR/).

## [0.1.3] - 2026-11-26

### Corrigido
- Ícones dos seus apps ficavam um pouco tortos: o Windows entrega o ícone dentro de um quadrado maior e fora do centro. Agora a ilha corta a borda vazia e centraliza o desenho.
- As abas do painel agora ficam sempre no meio da ilha, mesmo desligando apps (como o timer de foco).
- Repouso com "Nada" dos dois lados: a hora agora fica no centro da ilha, e não encostada num lado.
- Tocando música, os anéis de uso da IA apareciam ao lado mesmo sem nenhuma IA rodando e sem "Uso da IA" escolhido no repouso. Agora só aparecem se a IA estiver trabalhando ou se você os escolheu.

### Mudado
- Sem nada para mostrar no repouso (nada, nada e sem hora), a ilha mostra dois olhinhos que piscam e olham em volta, em vez de um ponto.

## [0.1.2] - 2026-11-25

### Corrigido
- Seus próprios apps agora aparecem na ilha com o ícone de verdade (o do programa ou do atalho, ou o do site) em vez da inicial.
- Com apps adicionados, o cabeçalho da ilha não cabia: as abas cobriam a hora e a engrenagem das Configurações saía para fora. A ilha agora fica mais larga conforme a quantidade de apps.
- Configurações → Apps: caminhos longos de programas faziam a linha sair da caixa.

## [0.1.1] - 2026-11-25

### Corrigido
- Bandeja de arquivos: soltar arquivos na ilha não funcionava. A janela da ilha era criada escondida e ficava sem o receptor de arquivos do Tauri; agora ela já nasce visível e no lugar.
- Durante um arrastar e soltar, a ilha não virava "clicável" a tempo: a rotina que segue o mouse esperava a thread principal, ocupada com o arraste. Agora ela fala direto com o Windows.

## [0.1.0] - 2026-11-25

Primeira versão.

### IA
- Acompanha o Claude Code (ganchos HTTP) e o Codex (ganchos de comando) ao vivo: o que leem, editam e rodam
- Permitir, Sempre permitir, Negar e Aprovar plano direto na ilha
- Anéis do limite de 5 h e da semana (Claude pela status line ou pela conta, Codex pelos arquivos de sessão)
- Histórico com busca, tarefas de hoje e dias seguidos

### Ilha
- Barra recolhida, painel com abas, modo linha (botão direito) e cartões de aviso embaixo da ilha
- Colada no topo (com as curvas ligando à borda da tela) ou flutuante; preta ou vidro colorido na cor da música; tamanho e transparência
- O que aparece em repouso à esquerda e à direita; relógio automático, 12 h ou 24 h
- Abre ao repousar o mouse ou ao clicar; atalho de teclado; some em tela cheia

### Apps
- Música (qualquer player do Windows) com letras sincronizadas
- Bandeja de arquivos, área de transferência, nota, timer de foco, Guarda do Foco
- Agenda por link iCal, clima, Wi‑Fi e Bluetooth, CPU/RAM/bateria, barra de volume
- Até 4 atalhos para sites e programas

### Avisos e sons
- Por tipo: cartão, notificação do Windows ou desligado
- Volume próprio e 4 pacotes de sons

### Atualizações
- Atualização automática pelo GitHub Releases, assinada, com progresso, que espera a IA terminar antes de instalar
