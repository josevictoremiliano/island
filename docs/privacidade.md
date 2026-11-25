# Privacidade

A Ilha não tem conta, servidor próprio nem telemetria. Quase tudo acontece no PC.

## Fica só no PC

- Atividade do Claude Code e do Codex (chega por `127.0.0.1`, protegida por um token aleatório)
- Histórico, nota, bandeja de arquivos (só os caminhos), configurações: `%APPDATA%\dev.ilha.desktop\`
- Área de transferência: **só na memória**, nunca no disco; ignora o que gerenciadores de senha marcam como privado
- Guarda do Foco: lê o título da janela da frente e o nome do programa, localmente
- Música, volume, rede, bateria, CPU e RAM: lidos do Windows

## Sai do PC (só o necessário, e só se o recurso estiver ligado)

| Recurso | Para onde | O que vai |
|---|---|---|
| Atualizações | github.com | Um pedido do `latest.json` e o download do instalador |
| Letras | lrclib.net | Título, artista e duração da música |
| Clima | open-meteo.com | O nome da cidade que você digitou e as coordenadas dela |
| Agenda | o endereço iCal que você colou | Um download da sua agenda |
| Uso pela conta Claude (desligado por padrão) | api.anthropic.com | O login que o Claude Code guardou, só para perguntar quanto do limite você usou. Não é uma conversa com o modelo e não gasta tokens |

Nada disso passa por um servidor da Ilha, porque não existe um.
