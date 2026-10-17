# rust_boardgame_player

Dette repoet er avhengig av rust_boardgame_game repoet pga Playable trait. Mulig det bør lages enda et repo med felles komponenter som er rot repoet.

Dette repoet inneholder brain og er nok her det vil være mest utvikling.

TRENING
=======
Trening kjøres via `cli`-binæren i rust_boardgame_bin
(`cli train`, `cli evolve`, `cli rank`, `cli report`).
Strategier er:
tilfeldig
tilfeldig avvik
finn bad/good possition
- persepsjonslag trening + siste lags trening
- Fikse tree search player. Bruke den som fasit og lage evaluering av alle gatene besert på om de hjelper til å velge riktig eller fører til feil.
- - For hvert spill, endre gate til invert i siste laget de som motarbeider riktig trekk.
- - For hvert spill, i de andre lagene regenerere gates som ikke gir noe verdi

HJERNEDESIGN
============
- Tønne (like lag) ->tilfeldig foreldre
- Fast diamant (persepsjonslaget er størst, deretter en halvvering for hvert lag, størrelser er (antall lag, antall bits til slutt))->alle forgjengere er unike->tilfeldig persepsjonslag

FUNN
====
Ved tilfeldig generering er et lag nok. Er bare siste laget som må optimaliseres på å gi riktig resultat. De andre lagene må vurderes basert på ar de merker et bra og evt dårlig valg.

Mer utfordrende å lage en god AI på mer kompliserte spill som "4 på rad". Med tree search blir det ikke helt håpløst, men ingen utfordring å vinne, hvis man følger litt med.

Plain ved plain tree search spiller AI dårlig i starten og bedre når det er få trekk igjen.

TRENINGSTRATEGI: VEILEDER FOR DIAMOND
=====================================
Det bør fungere å få til bedre planlegging ved å bruke eksisterende LGNN_DIAMOND_TREE_SEARCH

Etter noen iterasjoner så bør Nevralnettverket klare å skille mellom en god og en dårlig posisjon litt ut i spillet. Og bli bedre og bedre til det.
Et antatt annet problem er at det trolig er mange "døde gates". Altså gates som har samme verdi uansett hva som skjer i spillet. Kan se det ut i fra at evalueringsscoren endres lite fra posisjon til posisjon.

Det som er fint med DIAMOND er at det bare er første lag hvor det er aktuelt å endre armadressene. Og da kan det være lettere og lage en bestemt logikk der.


Saudo kode:
AI m/TS spiller et spill mot seg selv. Spillets statistikker og tilstander lagres i minne.
Kun AI uten TS gjør en vurdering av alle tilstandene i spillrunden.

SCOREAVIK = Gjennomsnitt per trekk avviket for LGNND vs LGNNDmTS.
Summer opp alle gates evalueringene og finn gjennomsnitt diff til 50% av maks vurdering/CELL_SIZE

CHANGE_GATES = Antall gates som skal endres er det største tallet av SCOREAVVIK og diff snitt score vs 50% CELL_SIZE
Deretter ranger alle gates etter følgende formell:

ENDRINGSSCORE=
Sum alle vuredringer ( kun hvis gate trekker i feilretning: avvik AI vs AImTS)
+
Sum(når overvekt resultat av false eller true) * diffsnitt scorevs 50%

Så velges antall gates av gates som har høyest endringscore.

Når en gate i siste laget er plukket ut til å endres/evolvere, så ser man om gaten er død eller ikke.

Hvis levende, så ser man på innputt verdiene til gaten og bytter til den typen som vil bidra mest til riktig score(altså i retning LGNNDmTS sin score)

(TODO: Hvis allerede beste gate type så jobb med forrige lag osv. Siste instans er å endre armene til første laget.)

Hvis død så gå til første lag for den aktuelle gaten (64 gates). Finn alle døde armer og velg en tilfeldig arm til BIT_STATE som kommer fra spillet.
(TODO: Hvis ingen døde armer, jobb da med gate-type. Endre døde gater til gate type som gir mest balanse mellom true og false
Hvis gatene er allerede er over en terskelverdi i varians, så gå til gate typene i neste lag osv.)

---
