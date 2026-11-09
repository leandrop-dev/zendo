# A janela do Zendo em português de Portugal: as mesmas mensagens de
# en.ftl, pela mesma ordem. Ver https://projectfluent.org.

## Contagens e tempos

count-games = { $count ->
    [one] { $count } jogo
   *[other] { $count } jogos
}
count-sources = { $count ->
    [one] { $count } origem
   *[other] { $count } origens
}
count-files = { $count ->
    [one] { $count } ficheiro
   *[other] { $count } ficheiros
}
time-minutes = { $minutes } min
time-hours = { $hours } h
time-hours-minutes = { $hours } h { $minutes } min
ago-now = agora mesmo
ago-minutes = há { $count } min
ago-hours = há { $count } h
ago-today = hoje
ago-yesterday = ontem
ago-days = há { $count } dias
ago-weeks = há { $count } semanas
ago-months = há { $count } meses
ago-years = há { $count } anos

## Páginas e barra lateral

page-library = Biblioteca
page-store = Loja
page-downloads = Transferências
page-accounts = Contas
page-settings = Definições
page-about = Sobre
sidebar-browse = NAVEGAR
sidebar-system = SISTEMA
sidebar-signed-in = { $count } de { $stores }
sidebar-your-library = A SUA BIBLIOTECA
sidebar-summary = { $sources } · { $time }

## Origens e ordens

source-desktop = Ambiente de trabalho
source-added = Adicionados
sort-recent = Recentes
sort-most-played = Mais jogados
sort-title = A–Z

## Biblioteca

library-looking = À procura de jogos…
library-none-found = Nenhum jogo encontrado
library-none-found-about = O Zendo lê as bibliotecas do Steam, os jogos que o Heroic instalou e os ficheiros .desktop da categoria Game.
library-no-match = Nada corresponde a “{ $search }”
library-none-here = Não há jogos aqui · escolha Todos para ver todas as lojas
library-all-hidden = Todos os jogos estão ocultos · Ctrl+H ou Start mostra-os
library-subtitle = { $games } de { $sources }
library-subtitle-hidden = { $hidden ->
    [one] { $subtitle } · 1 oculto
   *[other] { $subtitle } · { $hidden } ocultos
}
library-scanning = A procurar…
library-refresh = Atualizar
library-search = Pesquisar na biblioteca
library-add-game = Adicionar jogo
library-all = Todos
library-sort = Ordenar
library-show-hidden = Mostrar ocultos
library-hidden = Oculto
badge-running = A CORRER
badge-pinned = FIXADO
played-never = Ainda não jogado
played-when = Jogado { $when }
played-time = { $time } jogados

## Sobre

about-source-code = Código-fonte
about-report-problem = Comunicar um problema
about-version = Versão { $version }
about-intro = Todos os jogos deste computador numa só biblioteca: Steam, Epic, GOG, Amazon e os restantes, cada um iniciado como o seu próprio lançador o faria.
about-licence = Licença
about-licence-text = MIT. O Zendo lê os ficheiros dos lançadores e nunca escreve neles.
about-built-on = Feito com
about-built-iced = Iced, a janela e tudo o que nela está
about-built-gilrs = gilrs, para os comandos
about-built-keyvalues = keyvalues-parser, para os ficheiros do Steam
about-built-ureq = ureq, serde e toml
about-built-steamgriddb = SteamGridDB, para as capas que nenhuma loja tem
about-built-fluent = Fluent, para os idiomas da janela
about-ideas-from = Ideias de
about-ideas-hikari = Hikari: a barra lateral, a biblioteca e as páginas dos jogos
about-ideas-omarchy = omarchy-games-menu: escrever em qualquer lado para filtrar
about-ideas-heroic = Heroic: onde guarda os jogos e as suas ligações de lançamento

## A janela

game-gone = Este jogo já não está na biblioteca
owned-gone = Este jogo já não está nas listas das suas lojas
images-failed = { $count ->
    [one] Não foi possível transferir { $count } imagem; `zendo covers` diz porquê
   *[other] Não foi possível transferir { $count } imagens; `zendo covers` diz porquê
}
images-downloading = { $count ->
    [one] A transferir imagens, falta 1
   *[other] A transferir imagens, faltam { $count }
}
footer-unreadable = { $count ->
    [one] Não foi possível ler { $count } ficheiro; as Definições indicam qual
   *[other] Não foi possível ler { $count } ficheiros; as Definições indicam quais
}
status-hid = { $title } ocultado · Ctrl+H mostra os jogos ocultos
status-unhid = { $title } voltou à biblioteca
status-pinned = { $title } fixado: aparece primeiro na biblioteca
status-unpinned = { $title } deixou de estar fixado
hints-library = Enter abrir · Tab ordenar · Ctrl+P fixar · Del ocultar · Ctrl+H ocultos · F11 ecrã inteiro
hints-sheet = Enter guardar · Esc cancelar
hints-game = Enter jogar · Esc voltar · Ctrl+P fixar · Del ocultar · F11 ecrã inteiro
hints-store = Enter abrir · escreva para pesquisar · Esc limpar · F11 ecrã inteiro
hints-owned = Enter instalar ou abrir · Esc voltar · F11 ecrã inteiro
hints-closing = Enter fechar mesmo assim · Esc continuar a transferir
hints-other = Esc voltar · F11 ecrã inteiro
panicked = O Zendo encontrou um erro ao fazer isso; o terminal tem os detalhes

## Definições

settings-subtitle = O idioma, as capas, o Nexus Mods, o Proton, as origens que o Zendo lê e os seus próprios ficheiros
language-title = Idioma
language-about = O idioma da janela. Sistema segue o do seu ambiente de trabalho; a linha de comandos fica em inglês.
language-system = Sistema ({ $language })
key-removed = Chave removida
key-saved-unchecked = Guardada, mas não verificada: { $error }
key-paste-first = Cole primeiro uma chave
key-hide = Ocultar
key-show = Mostrar
key-checking = A verificar…
key-check = Verificar
key-save = Guardar
key-none = Nenhuma chave definida
kept-keyring = Guardada no porta-chaves do sistema.
kept-file = Guardada em config.toml, legível apenas por si.
kept-file-why = Guardada em config.toml, legível apenas por si: { $why }.
kept-unread = Não foi possível ler o porta-chaves: { $why }.
steamgriddb-saved = Guardada. O SteamGridDB aceita esta chave.
steamgriddb-accepted = O SteamGridDB aceita esta chave; Guardar mantém-na.
steamgriddb-rejected = O SteamGridDB recusou esta chave. Copie-a de novo das suas preferências do SteamGridDB.
steamgriddb-rejected-unsaved = O SteamGridDB recusou esta chave, por isso não foi guardada. Copie-a de novo das suas preferências do SteamGridDB.
steamgriddb-paste = Cole a sua chave da API do SteamGridDB
steamgriddb-get-key = Obtenha uma chave gratuita nas suas preferências do SteamGridDB
steamgriddb-about = Capas e imagens largas para jogos cuja loja não as tem. A chave fica no porta-chaves do sistema ou, quando não há nenhum, no seu ficheiro de definições, legível apenas por si.
report-title = Diagnóstico
report-about = Um relatório do que o Zendo encontrou, para colar num relatório de erro: o sistema, as ferramentas e as suas versões, os Protons, as definições (nunca as chaves), se tem sessão iniciada em cada loja (nunca a conta), as contagens da biblioteca, os prefixos Wine e os registos dos três últimos lançamentos. A sua pasta pessoal aparece como ~. Leia-o antes de o partilhar: o registo de um jogo pode revelar mais. `zendo report` mostra o mesmo e `zendo logs` lista os registos. O relatório está em inglês.
report-gathering = A reunir…
report-copy = Copiar relatório
report-open-logs = Abrir registos de lançamento
report-copied = { $count ->
    [one] Relatório copiado: 1 linha.
   *[other] Relatório copiado: { $count } linhas.
}
images-none = Não há imagens para transferir
images-left = { $count ->
    [one] falta 1
   *[other] faltam { $count }
}
covers-title = Capas
covers-about = As do Steam vêm da sua própria cache. A Epic, a GOG e a Amazon publicam as suas; as restantes vêm do SteamGridDB. Um jogo sem nenhuma em lado nenhum volta a ser procurado ao fim de uma semana.
covers-download = Transferir as capas e imagens largas em falta
covers-download-now = Transferir já as imagens em falta
sources-title = Origens
sources-about = Onde o Zendo procura jogos. Lê-as e nunca escreve nelas; Atualizar, na biblioteca, procura de novo.
sources-epic = Epic Games, através do Heroic
sources-gog = GOG, através do Heroic
sources-amazon = Amazon Games, através do Heroic
sources-heroic = Adicionados ao Heroic à mão
sources-desktop = Ficheiros .desktop da categoria Game
sources-local = Adicionados à mão no Zendo
sources-none-found = nenhum encontrado
sources-unreadable = Não foi possível ler { $problem }
files-title = Ficheiros
files-about = Os ficheiros do próprio Zendo. Remover um repõe o que ele guarda. Os jogos das lojas são instalados na pasta de jogos; `install_dir` no ficheiro de definições muda-a.
files-no-home = sem pasta pessoal
files-open-folder = Abrir pasta
files-settings = Definições
files-state = Lançamentos, fixados, ocultos
files-images = Imagens transferidas
files-queue = Fila de transferências
files-sign-ins = Sessões das lojas
files-games = Jogos

## Nexus Mods

cancel = Cancelar
nexus-no-game = Chegou do Nexus um mod para { $name }, mas nenhum jogo instalado aceita mods
nexus-updates-failed = Não foi possível pedir atualizações ao Nexus: { $error }
nexus-no-exe = não foi possível saber onde está o Zendo: { $error }
nexus-links-open-in = As ligações do Nexus abrem agora em { $exe }
nexus-key-saved = Guardada. { $who }.
nexus-key-accepted = { $who }; Guardar mantém a chave.
nexus-key-rejected = O Nexus recusou esta chave. Copie-a de novo de API Access no Nexus.
nexus-key-rejected-unsaved = O Nexus recusou esta chave, por isso não foi guardada. Copie-a de novo de API Access no Nexus.
nexus-known-as = O Nexus conhece esta chave como { $name }
nexus-known-as-premium = O Nexus conhece esta chave como { $name }, com Premium
nexus-mod-arrived = Chegou um mod do Nexus…
nexus-download-name = { $title } · ficheiro #{ $file }
nexus-downloading = A transferir um mod para { $title } do Nexus…
nexus-download-stopped = Transferência parada
nexus-download-failed = Não foi possível transferir o mod: { $error }
nexus-downloads-title = Mods do Nexus
nexus-paste = Cole a sua chave pessoal da API do Nexus Mods
nexus-get-key = Obtenha a sua chave pessoal da API em API Access no Nexus Mods
steam-sign-in-about = Inicie sessão na própria página do Steam, com a palavra-passe e o Steam Guard ou o código QR da aplicação Steam; o Zendo nunca vê a palavra-passe. A Loja passa a listar os jogos que tem e os que a sua família Steam partilha, e instala-os através do Steam. O Zendo guarda a sessão no porta-chaves do sistema e a página do Steam lembra-se dela.
steam-sign-in = Iniciar sessão com o Steam
steam-signing-in = Inicie sessão na página do Steam, na janela que abriu
steam-signed-in = Sessão iniciada: a Loja está a pedir os seus jogos ao Steam
steam-signed-in-about = A Loja lista os jogos desta conta e os que a sua família Steam partilha.
steam-signed-out = Sessão do Steam terminada
steam-sign-in-again = A sessão do Steam terminou: inicie sessão de novo
steam-family = Família Steam: { $name } · { $count ->
    [one] { $count } membro
   *[other] { $count } membros
}
steam-family-failed = Não foi possível listar os jogos da sua família: { $error }
store-shared = Partilhado pela sua família
steam-asked = Pedido ao Steam para instalar { $title }: escolha onde na janela do Steam
steam-installed = { $title } está instalado
steam-downloads-title = Steam
steam-open-downloads = Abrir as transferências do Steam
steam-downloads-about = É o Steam que as transfere; pause-as ou cancele-as no Steam.
steam-waiting-dialog = À espera da janela de instalação do Steam
steam-stop-waiting = Deixar de esperar
steam-ask-again = Pedir de novo ao Steam
steam-uninstall = Desinstalar com o Steam
steam-verify = Verificar com o Steam
steam-ask-again-uninstall = Pedir de novo ao Steam para desinstalar
steam-asked-uninstall = Pedido ao Steam para desinstalar { $title }: responda à pergunta dele
steam-asked-verify = Pedido ao Steam para verificar os ficheiros de { $title }: o progresso aparece no Steam
steam-waiting-uninstall = À espera de que o Steam desinstale
steam-uninstalled = { $title } foi desinstalado
steam-updated = { $title } foi atualizado
steam-verifying = A verificar no Steam
steam-waiting-verify = À espera de que o Steam comece a verificar
steam-verified = O Steam verificou os ficheiros de { $title }
steam-state-waiting = Em fila no Steam
steam-state-downloading = A transferir, { $progress }
steam-state-installing = A instalar, { $progress }
steam-state-paused = Em pausa no Steam, { $progress }
nexus-handler-zendo = O Zendo abre as ligações Mod manager download.
nexus-handler-other = As ligações Mod manager download abrem em { $other }, não no Zendo.
nexus-handler-other-zendo = As ligações Mod manager download abrem noutra cópia do Zendo, { $program }; prima Abri-las com o Zendo para usar esta.
nexus-handler-nothing = Ainda nada abre as ligações Mod manager download.
nexus-become-handler = Abri-las com o Zendo
nexus-about = Os mods dos botões Mod manager download do Nexus vão diretamente para os mods do jogo, e o Zendo avisa quando têm atualizações. Sem Premium, o Nexus só dá ficheiros através desse botão no seu site. A chave fica no porta-chaves do sistema ou, quando não há nenhum, no seu ficheiro de definições, legível apenas por si.
nexus-progress = { $done } de { $total } MiB
nexus-progress-unknown = { $done } MiB
nexus-stopping = A parar…
nexus-stop = Parar
nexus-choose-title = Para que jogo é este mod?
nexus-choose-none = O Nexus diz que este mod é para { $name }, e nenhum jogo instalado tem esse nome. Escolha o jogo a que se destina; o Zendo lembra-se disso para os próximos mods deste jogo.
nexus-choose-several = O Nexus diz que este mod é para { $name }, e há mais de um jogo instalado que corresponde.
nexus-updated-to = { $name } atualizado para { $version }
nexus-updated = { $name } atualizado
nexus-added-version = { $name } { $version } adicionado a { $title }
nexus-added = { $name } adicionado a { $title }

## Mods

saving = A guardar…
deleting = A eliminar…
delete = Eliminar
really-delete = Eliminar mesmo
profile-made = Criado { $now }, uma cópia do perfil anterior
profile-switched = Mudou para { $now }{ $state ->
    [applied] ; { $now } está aplicado
    [changed] ; Aplicar põe-no no jogo
   *[other] {""}
}
profile-renamed = Perfil renomeado para { $name }
profile-deleted = { $name } eliminado; agora em { $now }{ $state ->
    [applied] ; { $now } está aplicado
    [changed] ; Aplicar põe-no no jogo
   *[other] {""}
}
mods-applied-count = { $count ->
    [one] { $count } mod aplicado a { $title }
   *[other] { $count } mods aplicados a { $title }
}
mods-vanilla = { $title } voltou ao original
mods-applied = Aplicado a { $title }
mods-choosing = A escolher um mod…
mods-choose = Escolha um mod
mods-deleted = Mod eliminado
mods-new-folder = Os mods vão para a nova pasta
mods-overwrite-deleted = Eliminado o que o jogo escreveu
mods-reading-installer = A ler o instalador…
mods-applying = A aplicar…
mods-restoring = A voltar ao original…
mods-added = { $name } adicionado
mods-unpacking = A extrair…
mods-switching = A mudar…
mods-close-game-first = Feche primeiro o jogo: os seus ficheiros estão em uso
mods-reading = A ler…
mods-add = Adicionar mod…
mods-title = Mods
mods-find-on-nexus = Procurar mods no Nexus
mods-search-nexus = Pesquisar no Nexus
search-title = Nexus Mods de { $title }
search-placeholder = Palavras no nome do mod, ou nenhuma para todos
search-order-endorsed = Mais recomendados
search-order-downloaded = Mais transferidos
search-order-updated = Atualizados recentemente
search-adult = Mods para adultos
search-adult-badge = Adultos
search-have = Nos seus mods
search-by = de { $author }
search-updated = atualizado em { $date }
search-endorsements = { $count } recomendações
search-downloads = { $count } transferências
search-files = Ficheiros
search-hide-files = Esconder ficheiros
search-page = Página no Nexus
search-asking-files = A pedir os ficheiros ao Nexus…
search-no-files = O Nexus não lista ficheiros atuais para este mod.
search-download = Transferir
search-get-on-nexus = Obter no Nexus
search-file-main = principal
search-file-optional = opcional
search-file-update = atualização
search-file-misc = diversos
search-asking = A pesquisar no Nexus…
search-nothing = O Nexus não encontrou mods.
search-count = { $shown } de { $total } mods
search-downloading = A transferir do Nexus…
search-needs-key = Defina a sua chave do Nexus nas Definições para ver os ficheiros de um mod e transferi-los daqui.
search-not-premium = Sem Premium, Obter no Nexus abre a página do ficheiro numa janela do Zendo: inicie sessão no Nexus aí uma vez, carregue em Slow download e o mod vem para o Zendo.
search-have-file = Já tem este ficheiro
search-site-open = A página do Nexus está aberta numa janela própria: carregue em Slow download e o Zendo adiciona o mod.
nexus-window-title = Nexus Mods · Zendo
nexus-window-downloading = A transferir… · Nexus Mods · Zendo
nexus-window-download-failed = A transferência da página do Nexus falhou.
mods-place-known-root = O Zendo conhece este jogo: os mods vão para a pasta do próprio jogo.
mods-place-known = O Zendo conhece este jogo: os mods vão para { $folder }.
mods-place-custom = Os mods vão para { $folder }, como definiu.
mods-place-root = Os mods vão para a pasta do próprio jogo; defina outra abaixo se pertencerem a outro lado.
mods-none = Ainda não há mods. Adicionar mod… aceita um arquivo (zip, 7z, rar) que tenha transferido.
mods-overwrite = Ficheiros que o jogo escreveu com mods: { $count }, aplicados depois de todos os mods
mods-from-nexus-version = versão { $version } do Nexus
mods-from-nexus = do Nexus
mods-wins = substitui { $count } de { $other }
mods-loses = { $count ->
    [one] 1 substituído por { $other }
   *[other] { $count } substituídos por { $other }
}
mods-options = Opções…
mods-update-to = Atualizar para { $version }
mods-update = Atualizar
mods-profile = Perfil
mods-profile-rename = Um novo nome para este perfil
mods-profile-new-name = Um nome para o novo perfil, uma cópia deste
mods-profile-new = Novo…
mods-profile-rename-button = Renomear…
mods-folder = Pasta dos mods
mods-folder-placeholder = a pasta do próprio jogo
mods-folder-known = Como o Zendo sabe
mods-state-no = Não aplicado: o jogo está como estava
mods-state-applied = { $count ->
    [one] Aplicado: { $count } mod no jogo
   *[other] Aplicado: { $count } mods no jogo
}
mods-state-applied-plugins = { $count ->
    [one] Aplicado: { $count } mod e a ordem dos plugins no jogo
   *[other] Aplicado: { $count } mods e a ordem dos plugins no jogo
}
mods-state-changed = Alterado desde que foi aplicado: Aplicar põe a lista no jogo
mods-state-running = O jogo está a correr: feche-o para aplicar ou voltar ao original
mods-back-to-vanilla = Voltar ao original
mods-apply = Aplicar
plugins-title = Plugins
plugins-note-dates = Aplicar escreve esta ordem na lista de plugins do jogo, abaixo, e data os plugins nela, que é como este jogo lê a ordem.
plugins-note = Aplicar escreve esta ordem na lista de plugins do jogo, abaixo.
plugins-note-no-prefix = Inicie o jogo uma vez, para que o seu prefixo seja criado; depois, Aplicar escreve esta ordem lá.
plugins-none = Não há plugins na pasta Data do jogo nem nos mods ativos.
plugins-from = de { $name }
plugins-games-own = do jogo
plugins-master = master
plugins-always = carrega sempre
plugins-needs = precisa de { $names }, que não está ativo
plugins-before = carrega antes de { $names }, de que precisa
plugins-unreadable = não é possível ler o seu cabeçalho

## Instalador FOMOD

installer-installed = { $name } instalado
installer-installing = A instalar…
installer-choose = Escolha as opções de { $name }
installer-cancelled = { $name } não foi instalado
installer-exactly-one = escolha um
installer-at-most-one = escolha um ou nenhum
installer-at-least-one = escolha pelo menos um
installer-all = todos estes
installer-any = escolha qualquer um
installer-required = { $name } (obrigatório)
installer-recommended = { $name } (recomendado)
installer-not-usable = { $name } (não utilizável)
installer-may-not-work = { $name } (pode não funcionar)
installer-step = Passo { $at } de { $steps } · { $name }
installer-needs = O mod diz que precisa de ficheiros que o Zendo não encontrou; pode não funcionar.
installer-back = Anterior
installer-install = Instalar
installer-next = Seguinte
installer-no-description = Sem descrição.

## A página de um jogo

game-back = ← Biblioteca
game-ready = Pronto quando quiser
game-pin = Fixar
game-unpin = Desafixar
game-hide = Ocultar
game-unhide = Mostrar
game-more = Mais
game-less = Menos
game-technical = Técnico
game-change-art = Mudar imagens
game-last-played = Jogado pela última vez { $when }
game-about = Sobre
game-details = Detalhes do jogo
game-launch-options = Opções de lançamento
badge-update = ATUALIZAÇÃO DISPONÍVEL
badge-hidden = OCULTO
count-times = { $count ->
    [one] { $count } vez
   *[other] { $count } vezes
}
fact-download-size = Tamanho da transferência
fact-asking = A perguntar à loja…
fact-size-on-disk = Tamanho no disco
fact-space-needed = Espaço necessário (estimativa da loja)
fact-latest-version = Versão mais recente
fact-latest-build = Compilação mais recente
fact-store-details = Detalhes da loja
fact-store-failed = A loja não soube dizer: { $error }
fact-publisher = Editora
fact-released = Lançado
fact-genres = Géneros
fact-store = Loja
fact-started-by = Iniciado por
fact-platform = Plataforma
fact-version = Versão
fact-install-size = Tamanho instalado
fact-install-path = Caminho de instalação
fact-cloud-saves = Gravações na nuvem
fact-updates = Atualizações
fact-last-played = Jogado pela última vez
fact-never = Nunca
fact-play-time = Tempo de jogo
fact-status = Estado
fact-owner = De quem é
fact-recent-time = Últimas duas semanas
fact-played-on = Jogado em
system-linux = Linux
system-windows = Windows
system-mac = macOS
system-deck = Steam Deck
fact-not-installed = Não instalado
fact-started-from-zendo = Iniciado a partir do Zendo
fact-launch-command = Comando de lançamento
fact-zendo-id = Id no Zendo

## Jogos a correr

run-its-launcher = o seu lançador
run-starting-game = A iniciar { $title }…
run-handed = { $title } entregue a { $launcher }
run-running = { $title } está a correr
run-waiting = À espera que { $launcher } inicie { $title }…
run-fetching-runtime = A transferir o runtime { $runtime } para { $title }…
run-runtime-failed = { $title } está a correr sem o runtime anti-batota: { $error }
run-missed = { $title } entregue a { $launcher }; o Zendo não o viu iniciar
run-closed = { $minutes ->
    [0] { $title } fechou
    [one] { $title } fechou ao fim de um minuto
   *[other] { $title } fechou ao fim de { $minutes } minutos
}
run-asked-close = Foi pedido a { $title } que fechasse; pode pedir para guardar primeiro
run-asked-end = Foi pedido a { $title } que terminasse
run-play = Jogar
run-state-running = A correr
run-state-starting = A iniciar…
run-stop = Parar
run-force-stop = Forçar paragem
run-wine-prefix = Prefixo Wine
run-last-log = Registo do último lançamento
run-open = Abrir

## ProtonDB

protondb-no-reports-yet = Ainda sem relatórios do Proton
protondb-reports = { $count ->
    [one] Proton: { $tier } segundo 1 relatório
   *[other] Proton: { $tier } segundo { $count } relatórios
}
protondb-native = PROTONDB NATIVO
protondb-native-about = Tem versão para Linux · { $reports } · abre o ProtonDB
protondb-tier-about = { $reports } · abre o ProtonDB
protondb-none = PROTONDB: SEM RELATÓRIOS
protondb-none-about = Ainda ninguém relatou nada · abre o ProtonDB

## Palavras comuns

save = Guardar
add = Adicionar
edit = Editar
remove = Remover
really-remove = Remover mesmo
choose = Escolher…
choosing = A escolher…

## Jogos adicionados à mão

local-no-record = O Zendo não tem registo deste jogo
local-choose-program = Escolha o programa do jogo
local-added = { $title } adicionado
local-saved = { $title } guardado
local-removed = { $title } removido da biblioteca; os ficheiros ficam onde estavam
local-edit-game = Editar jogo
local-add-game = Adicionar um jogo
local-about-empty = Um jogo que instalou por si: o .exe de um jogo de Windows, que corre através do Proton num prefixo próprio, ou o programa de um jogo de Linux.
local-about-windows = Um jogo de Windows: corre através do umu-launcher e do Proton, num prefixo próprio.
local-about-linux = Um programa de Linux: corre tal como está.
local-program = Programa
local-program-placeholder = /home/utilizador/Jogos/Um Jogo/Jogo.exe
local-title = Título
local-title-placeholder = Um Jogo
local-arguments = Argumentos
local-arguments-placeholder = Passados ao jogo, divididos como numa shell
local-cover-note = A capa vem do SteamGridDB pelo título quando as Definições têm uma chave, e Mudar imagens na página do jogo escolhe outra. O Proton, o MangoHud e o resto estão nas opções de lançamento.

## Opções de lançamento

options-proton-default = Predefinido ({ $name })
options-proton-default-umu = Predefinido (UMU-Proton, transferido pelo umu)
options-proton-missing = { $name } (não instalado; corre o predefinido)
options-size-both = Indique ao gamescope a largura e a altura, ou nenhuma
options-saved = Opções de lançamento de { $title } guardadas
options-title = { $title } · Opções de lançamento
options-proton-about = Corre o jogo de Windows. O GE-Proton traz mais correções; as Definições escolhem o predefinido.
options-mangohud-about = Uma sobreposição com a taxa de fotogramas, os tempos de fotograma e as temperaturas.
options-gamemode-about = Pede ao sistema o desempenho máximo enquanto o jogo corre.
options-gamescope-about = Corre o jogo num compositor próprio: ecrã inteiro, no tamanho que escolher, para jogos que não passam a ecrã inteiro sozinhos.
options-xbox = Comandos como comandos Xbox
options-xbox-about = Para jogos que ignoram um comando PlayStation ou outro. O jogo deixa então de receber dele o touchpad, o movimento e os gatilhos adaptativos.
options-width = Largura
options-height = Altura
options-full-screen = Ecrã inteiro
options-arguments-about = Passados ao jogo, divididos como numa shell.
options-environment = Ambiente
options-environment-about = Pares NOME=valor para o jogo, separados por espaços.
options-not-installed = Não instalado.
saves-around = Gravações na nuvem
saves-around-about = Traz as gravações mais recentes da nuvem antes de o jogo começar e envia as mais recentes daqui depois de terminar, através do legendary ou do gogdl. O jogo tem de ter sido iniciado uma vez.
saves-sync-now = Sincronizar agora
saves-upload = Enviar
saves-download = Transferir
saves-really = { $action }: substituir o outro lado?
saves-syncing = A sincronizar as gravações…
saves-synced = As gravações estão sincronizadas
cloud-off = Desligadas (ligue-as nas Opções de lançamento)
cloud-not-yet = Ainda não sincronizadas
cloud-after-first-start = Sincronizam depois de o jogo ter arrancado
cloud-synced = Sincronizadas { $when }
cloud-none = A { $store } não guarda gravações na nuvem para este jogo
cloud-failed = Falhou { $when }: { $error }
options-offline = Sem ligação
options-offline-about = Inicia sem entrar na Epic, para jogar sem ligação; as partes online do jogo e as conquistas ficam desligadas.
options-offline-not-approved = A Epic não aprovou este jogo para jogar sem ligação: pode não iniciar.
options-skip-update = Iniciar com uma atualização à espera
options-skip-update-about = O legendary recusa-se a iniciar um jogo cuja atualização não está instalada; isto inicia a versão instalada.
options-eos-overlay = Sobreposição da Epic
options-eos-overlay-about = A sobreposição EOS da Epic sobre o jogo (amigos, convites, avisos de conquistas). Ligada no prefixo do jogo quando inicia, a partir do segundo início.
options-eos-overlay-missing = Instale-a primeiro nas Definições.
overlay-title = Sobreposição da Epic
overlay-about = A sobreposição EOS da Epic, que o legendary instala uma vez para todos os jogos da Epic; cada jogo liga-a nas suas opções de arranque.
overlay-working = A trabalhar com o legendary…
overlay-is-installed = Instalada em { $path }
overlay-not-installed = Não instalada
overlay-no-legendary = É preciso o legendary: entre na Epic na página Contas, ou instale o legendary.
overlay-install = Instalar
overlay-update = Atualizar
overlay-installed = A sobreposição da Epic está instalada
overlay-updated = A sobreposição da Epic está atualizada
overlay-removed = A sobreposição da Epic foi removida
options-gamescope-shows = Mostra-se como
options-gamescope-window = Uma janela
options-gamescope-borderless = Uma janela sem moldura
options-output-size = O seu tamanho
options-game-size = O do jogo
options-sizes-about = Vazio: o do próprio jogo. Um tamanho menor para o jogo é ampliado para o do gamescope.
options-game-size-both = Dê ao tamanho do jogo uma largura e uma altura, ou nenhuma
options-scaler = Ajuste
options-filter = Ampliação
options-gamescope-own = O do gamescope
options-scaler-auto = Automático
options-scaler-integer = Passos inteiros (pixel art)
options-scaler-fit = Caber dentro
options-scaler-fill = Preencher, cortando
options-scaler-stretch = Esticar
options-filter-linear = Suave
options-filter-nearest = Píxel mais próximo
options-filter-pixel = Pixel art
options-gamescope-softest = 20 (mais suave)
options-gamescope-fps = Fotogramas por segundo
options-gamescope-fps-empty = Vazio: os do ecrã
options-gamescope-more = Mais opções
options-gamescope-more-about = Qualquer uma das do gamescope, como numa shell: --adaptive-sync, --force-grab-cursor.
options-runtime = Steam Linux Runtime
options-runtime-about = Corre o jogo num dos contentores do Steam, com as bibliotecas para que foi feito: jogos nativos mais antigos precisam muitas vezes do 1.0 (scout).
options-runtime-none = Nenhum: as bibliotecas do sistema
options-runtime-missing = { $name } (não instalado)
options-no-umu = Sem o umu
options-no-umu-about = O Zendo corre o Proton no Steam Linux Runtime 4.0, como o umu faria: para quando o umu-launcher falta ou falha.
options-no-umu-runtime = O Steam Linux Runtime 4.0 não está instalado: instale-o no Steam.
options-language = Idioma
options-language-about = O idioma que o legendary pede ao jogo, como código: fr, de, pt-BR.
options-language-empty = Vazio: o do sistema
options-task = Iniciar com
options-task-about = A forma da loja de iniciar o jogo, ou outra que ela indique (sem o iniciador, um modo sem anti-batota).
options-task-default = A da loja
options-task-gone = { $name } (já não indicado; corre o da loja)
options-task-none = A loja não indica outra forma de iniciar este jogo.
options-executable = Programa
options-executable-about = Corre este programa em vez do da loja: um que esteja na pasta do jogo.
options-executable-empty = Vazio: o da loja
options-executable-browse = Procurar…
options-executable-choose = Escolher o programa do jogo
options-before = Antes do jogo
options-before-about = Um comando corrido antes de o jogo começar, como numa shell; o jogo espera por ele, e não começa se falhar. Recebe ZENDO_GAME_ID, ZENDO_GAME_TITLE, ZENDO_GAME_FOLDER e o WINEPREFIX do jogo.
options-after = Depois do jogo
options-after-about = Um comando corrido quando o jogo termina, com os mesmos nomes; só com a janela do Zendo aberta.
options-offload = Descarga para a NVIDIA
options-offload-about = Desenha o jogo na placa NVIDIA de um computador com duas (PRIME render offload), como na maioria dos portáteis.
options-offload-none = Este computador não tem uma segunda placa com o controlador da NVIDIA.
options-proton-switches = Proton
options-build-ignores = O { $build } não lê { $names }.
options-wayland = Controlador Wayland
options-wayland-about = O Wine desenha através do Wayland em vez do X11: mais nítido em ecrãs com escala, e necessário para HDR fora do gamescope.
options-hdr-about = Para jogos que oferecem HDR, num ecrã e ambiente de trabalho com HDR ligado; precisa do controlador Wayland ou do gamescope.
options-fsr-about = O FSR 1 do Wine: um jogo em ecrã inteiro abaixo da resolução do ecrã é ampliado com o FSR da AMD. Não com o controlador Wayland.
options-fsr-sharpening = Nitidez
options-fsr-sharpest = 0 (mais nítido)
options-fsr-softest = 5 (mais suave)
options-fsr-default = 2 (o do Wine)
options-fps-counter = Contador de FPS
options-fps-counter-about = A taxa de fotogramas do DXVK, no canto superior esquerdo, para jogos Direct3D 8 a 11; o MangoHud mostra mais.
options-frame-cap = Limite de fotogramas
options-frame-cap-about = O máximo de fotogramas por segundo que o jogo desenha, através do DXVK e do VKD3D-Proton.
options-frame-cap-empty = Vazio: sem limite
options-wow64-about = Corre código Windows de 32 bits sem as bibliotecas de 32 bits do sistema. Alguns jogos antigos correm pior assim.
options-no-sync = Esync e fsync desligados
options-no-sync-about = Para jogos que bloqueiam ou falham com as formas mais rápidas de espera do Wine.

## Mais palavras comuns

installed = Instalado
download = Transferir
stop = Parar
stopping = A parar…
open = Abrir
close = Fechar
try-again = Tentar de novo
size-of = { $done } de { $total }

## Proton

protons-installed-note = { $name } está instalado; as opções de lançamento podem escolhê-lo
protons-stopped = Transferência de { $name } parada
protons-removed-default = { $name } removido; era o predefinido, por isso passa a ser usado o Automático
protons-removed = { $name } removido
protons-umu = UMU-Proton, que o umu transfere
protons-default-set = Os jogos sem Proton próprio passam a correr com { $name }
protons-automatic = Automático: o GE-Proton mais recente primeiro ({ $name })
protons-automatic-umu = Automático: UMU-Proton, transferido pelo umu
protons-missing = { $name } (não instalado; é usado o Automático)
protons-default = Predefinido
protons-default-about = Para os jogos de Windows cujas opções de lançamento deixam o Proton como vem.
protons-here = Neste computador
protons-none = Ainda nenhum: o umu transfere o UMU-Proton da primeira vez que um jogo de Windows inicia.
protons-releases = Versões do GE-Proton
protons-check-again = Verificar de novo
protons-asking = A perguntar ao GitHub…
protons-show-older = Mostrar versões mais antigas
protons-about = Corre os jogos de Windows que o Zendo instalou; as opções de lançamento de cada jogo escolhem um. O GE-Proton traz mais correções e vai para a pasta de ferramentas de compatibilidade do Steam, onde o Steam e o Heroic também o encontram.
protons-chosen-by = { $count ->
    [one] 1 jogo escolheu-o; vai correr com o predefinido
   *[other] { $count } jogos escolheram-no; vão correr com o predefinido
}
protons-downloading = A transferir { $name }
runtimes-title = Anti-batota
runtimes-about = Os jogos que trazem Easy Anti-Cheat ou BattlEye precisam do respetivo runtime para jogar online com o Proton. O Zendo usa o do Steam quando o Steam o tem; caso contrário, transfere o seu do Lutris na primeira vez que um desses jogos arranca.
runtimes-from-steam = Do Steam
runtimes-downloaded-on = Transferido pelo Zendo, compilado a { $date }
runtimes-downloaded-undated = Transferido pelo Zendo
runtimes-not-here = Não está neste computador; é transferido quando um jogo precisar
runtimes-update = Atualizar para { $date }
runtimes-downloading = A transferir…
runtimes-asking = A perguntar ao Lutris…
runtimes-downloaded = O runtime { $runtime } está pronto
runtimes-removed = Removida a cópia do Zendo do runtime { $runtime }

## O seletor de imagens

art-no-game = O SteamGridDB não tem nenhum jogo chamado “{ $query }”; experimente outro título
art-no-images = { $kind ->
    [cover] Este jogo ainda não tem capa no SteamGridDB
   *[other] Este jogo ainda não tem imagem larga no SteamGridDB
}
art-needs-key = Pesquisar no SteamGridDB precisa de uma chave: adicione a sua nas Definições
art-searching = A pesquisar…
art-looking = A procurar imagens…
art-new = { $kind ->
    [cover] Nova capa para { $title }
   *[other] Nova imagem larga para { $title }
}
art-back-to-automatic = { $kind ->
    [cover] De volta à capa automática de { $title }
   *[other] De volta à imagem larga automática de { $title }
}
art-cover = Capa
art-hero = Imagem larga
art-search-placeholder = Um título a pesquisar no SteamGridDB
art-search = Pesquisar
art-title = Imagens de { $title }
art-show-more = Mostrar mais
art-open-settings = Abrir Definições
art-use-automatic = { $kind ->
    [cover] Usar a capa automática
   *[other] Usar a imagem larga automática
}
art-by = { $about }, de { $author }

## Contas

accounts-signed-in-to = Sessão iniciada em { $store } como { $name }
accounts-signed-out-of = Sessão terminada em { $store }
accounts-closed = Início de sessão em { $store } fechado
accounts-no-web-view = { $error }. Inicie sessão no navegador.
accounts-no-code-epic = Não há código aí: cole a página inteira que a Epic mostrou
accounts-no-code-address = Não há código aí: cole o endereço em que a página terminou
accounts-about = Inicie sessão para ver e instalar os jogos que tem. Estas sessões são do Zendo, à parte das do Heroic.
accounts-signed-out = Sem sessão iniciada
accounts-signed-in-as = Sessão iniciada como { $name }
accounts-sign-out = Terminar sessão
accounts-sign-in = Iniciar sessão
accounts-through = Através de { $path }
accounts-gog-playtime = Enviar o tempo de jogo para a sua conta GOG
accounts-engine-missing = { $engine } não encontrado: instale-o, ou o Heroic, que o inclui
accounts-preparing-amazon = A preparar a página da Amazon…
accounts-signing-in = A iniciar sessão…
accounts-in-window = Inicie sessão em { $store } na janela que abriu; o Zendo termina sozinho.
accounts-opening-window = A abrir a janela de início de sessão…
accounts-use-browser = Usar antes o meu navegador
accounts-step-one = 1. Inicie sessão na página da loja, no seu navegador.
accounts-step-two-epic = 2. A Epic mostra depois uma página de texto. Copie-a toda, ou o valor authorizationCode.
accounts-step-two-gog = 2. A GOG mostra depois uma página vazia. Copie o endereço da barra de endereço.
accounts-step-two-amazon = 2. A Amazon vai depois para a página inicial. Copie o endereço dessa página da barra de endereço.
accounts-step-three = 3. Cole-o abaixo. Ctrl+V funciona em qualquer parte desta página.
accounts-paste-here = Cole aqui
accounts-finish = Concluir
accounts-open-again = Abrir a página de novo
accounts-preparing = A preparar…

## A loja

install = Instalar
installing = A instalar…
resume = Retomar
queued = Na fila
see-downloads = Ver transferências
badge-installed = INSTALADO
fact-platforms = Plataformas
fact-developer = Programador
store-owned = { $count ->
    [one] 1 jogo seu
   *[other] { $count } jogos seus
}
store-installed = { $count ->
    [one] 1 instalado
   *[other] { $count } instalados
}
store-asking-stores = A perguntar a { $stores } pelos seus jogos…
store-search = Pesquisar os seus jogos
store-asking = A perguntar…
store-sign-in-first = Inicie sessão numa loja em Contas para ver os jogos que tem
store-go-accounts = Ir para Contas
store-asking-all = A perguntar às lojas pelos seus jogos…
store-nothing = Nada aqui
store-not-installed-only = Só os não instalados
store-open-in-library = Abrir na biblioteca
store-into = Em { $path }
store-installs-into = Instala em { $path }
store-install-with-steam = Instalar com o Steam
store-steam-installs = O Steam pergunta onde instalar; o Zendo acompanha a transferência e junta o jogo quando terminar
store-steam-installs-needs = O Steam pergunta onde instalar; o Zendo acompanha a transferência e junta o jogo quando terminar · precisa de { $size }
store-installs-into-needs = Instala em { $path } · precisa de { $size }
store-with-dlc = Instalar também os DLC ({ $count })
store-with-dlc-size = Instalar também os DLC ({ $count }, { $size })

## The DLC card on a game's page

dlc-title = DLC
dlc-count = { $installed } de { $total } instalados
dlc-install-all = Instalar todos
dlc-comes-with = Vem com o jogo
dlc-not-installed = Não instalado
dlc-in-heroic = O Heroic instala e remove estes.
dlc-sign-in = Inicie sessão na { $store } em Contas para ver todos os DLC que tem.
dlc-asking = A perguntar à loja pelos DLC que tem…
store-no-home = Não há pasta pessoal onde instalar
store-back = ← Loja

## Transferências

pause = Pausar
pausing = A pausar…
cancelling = A cancelar…
action-label = { $action ->
    [install] Instalar
    [update] Atualizar
    [repair] Reparar
    [add-dlc] Instalar DLC
    [remove-dlc] Desinstalar DLC
   *[uninstall] Desinstalar
}
action-doing = { $action ->
    [install] A instalar
    [update] A atualizar
    [repair] A reparar
    [add-dlc] A instalar DLC de
    [remove-dlc] A desinstalar DLC de
    [add-dlc-alone] A instalar DLC
    [remove-dlc-alone] A desinstalar DLC
   *[uninstall] A desinstalar
}
downloads-paused-count = { $count ->
    [one] 1 transferência em pausa
   *[other] { $count } transferências em pausa
}
downloads-pausing = A pausar { $title }…
downloads-cancelling = A cancelar { $title }…
downloads-doing-percent = { $doing } { $title }, { $percent }%
downloads-doing-dots = { $doing } { $title }…
downloads-doing = { $doing } { $title }
downloads-cancelled-gone = { $title } cancelado; os ficheiros foram apagados
downloads-cancelled-but = { $title } cancelado, mas { $error }
downloads-pausing-closing = A pausar a transferência e depois a fechar…
downloads-cancelled-job = Cancelado: { $doing } { $title }
downloads-cancelled = Cancelado
downloads-already-queued = { $title } já está na fila
downloads-ahead = { $count ->
    [one] { $action } { $title }: 1 à frente
   *[other] { $action } { $title }: { $count } à frente
}
downloads-paused-job = Em pausa: { $doing } { $title }
downloads-done-job = { $action } { $title }: concluído
downloads-failed-job = { $action } { $title } falhou: { $error }
downloads-job-state = { $state ->
    [now] { $doing }: agora
    [paused] { $doing }: em pausa
   *[queued] { $doing }: na fila
}
downloads-press-again = Prima de novo para apagar os ficheiros
downloads-check-updates = Procurar atualizações
downloads-checking-updates = A procurar atualizações…
downloads-up-to-date = Atualizado
downloads-update-version = Atualização disponível: { $version }
downloads-update-available = Atualização disponível
downloads-update-failed = Não foi possível procurar atualizações: { $error }
downloads-nothing = Nada para transferir
downloads-in-queue = { $count } na fila
downloads-in-queue-paused = { $count } na fila, { $paused } em pausa
downloads-empty = Instale jogos a partir da Loja e eles aparecem aqui
downloads-go-store = Ir para a Loja
downloads-up-next = A SEGUIR
downloads-paused = EM PAUSA
downloads-finished = CONCLUÍDAS
downloads-clear = Limpar
downloads-running-title = Há uma transferência a decorrer
downloads-running-about = Fechar põe-na em pausa, com o que já tem; retome-a daqui da próxima vez
downloads-close-anyway = Fechar mesmo assim
downloads-keep = Continuar a transferir
downloads-downloaded = { $size } transferidos
downloads-left = faltam { $time }
downloads-starting = A iniciar { $engine }…
downloads-failed = Falhou: { $error }
downloads-done = Concluído

## A janela de escolher ficheiros e a de iniciar sessão

dialog-choose = Escolher
dialog-archives = Arquivos
dialog-windows-programs = Programas Windows
dialog-all-files = Todos os ficheiros
sign-in-window-title = Iniciar sessão em { $store } · Zendo
sign-in-popup-title = Iniciar sessão · Zendo

## Tema

theme-title = Tema
theme-about = As cores da janela: as do próprio Zendo, um tema seu (um ficheiro em { $folder }, lido quando as Definições abrem) ou, num computador com o hyprmonk, o tom do hyprmonk, que o Zendo segue quando muda.
theme-hyprmonk = Tom do hyprmonk
theme-gone = Já não há nenhum tema chamado “{ $theme }”, por isso aparecem as cores do próprio Zendo.
theme-not-read = Não é um tema: { $problem }
theme-missing = O hyprmonk não escreveu nenhum tom, por isso aparecem as cores do próprio Zendo.
theme-following = A seguir o tom do hyprmonk: { $tone }.
theme-failed = Não foi possível ler o tom do hyprmonk, por isso as cores ficam como estavam: { $error }

## O cartão do prefixo Wine

prefix-title = Prefixo Wine
prefix-about = As definições do Wine, o winetricks e programas Windows (instaladores, correções) são executados no prefixo deste jogo, com o seu Proton.
prefix-wine-settings = Definições do Wine
prefix-run-program = Executar um programa…
prefix-choose-program = Escolha um programa Windows para executar no prefixo do jogo
prefix-components-placeholder = Componentes do winetricks, como vcrun2022 d3dx9
prefix-install = Instalar
prefix-running = { $what } em execução no prefixo…
prefix-game-running = As ferramentas esperam que o jogo termine.
prefix-done = { $what } terminou
prefix-installed = Instalado: { $components }
prefix-all-there = Já instalado: { $components }
prefix-installed-some = Instalado: { $components }; { $already } já estava instalado
