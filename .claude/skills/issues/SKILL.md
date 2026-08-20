---
name: issues
description: Traiter les issues GitHub ouvertes du repo cockpit de bout en bout — lire les captures, reproduire, corriger, releaser, repondre a l'auteur dans sa langue et fermer. A utiliser quand Jimmy demande de traiter/regarder les issues, ou quand une issue precise est citee.
---

# Traiter les issues GitHub

Repo : `jguevel-tech/cockpit`. `gh` est authentifie dessus.

**Le fichier sur disque fait autorite.** Si la copie de ce skill qui t'a ete fournie
au lancement ne contient pas l'etape 0 ou le label `attente-retour`, c'est un
instantane perime : relis
`.claude/skills/issues/SKILL.md` et `.claude/agents/issue-*.md` avant de commencer,
et suis le fichier. Ca arrive quand le skill a ete modifie pendant la session en
cours — donc a chaque fois que l'etape 4bis fait son travail.

## REGLE NUMERO 1 — ALLER JUSQU'AU BOUT

**Un correctif committe mais non publie est un travail non fait.** Pas « presque
fait » : non fait. Tant que le tag n'est pas pousse, la CI verte et l'auteur prevenu,
il ne s'est rien passe pour personne.

La chaine complete, et elle ne se coupe pas en son milieu :

```
corriger -> verifier -> CLAUDE.md -> commiter -> RELEASER -> pousser le tag
         -> attendre la CI -> verifier la version servie -> PREVENIR L'AUTEUR -> label
```

**Interdit de s'arreter entre le commit et la release.** Interdit d'ecrire « je
publie ensuite », « je vais releaser », « puis je previens les auteurs » : ce sont des
phrases qui remplacent l'action. Soit tu le fais dans le meme tour, soit tu dis
clairement que ce n'est PAS fait et pourquoi. Le 2026-08-20, trois correctifs verifies
(#4, #7, #8) sont restes non publies pendant que le rapport annoncait leur
publication ; c'est Jimmy qui a du le remarquer, deux fois dans la meme session.

**Pourquoi c'est grave au-dela du retard** : le depot reste dans un etat que personne
ne sait lire.

- La session suivante trouve des entrees dans `[Unreleased]` sans pouvoir dire si
  c'est un lot en preparation ou un oubli. Elle peut releaser du travail qu'elle n'a
  pas verifie, ou au contraire attendre indefiniment.
- Les utilisateurs n'ont pas le correctif, et l'auteur de l'issue n'a aucune nouvelle
  alors que le travail est fait depuis des heures.
- Les corrections s'empilent, et la premiere release qui part embarque des choses que
  plus personne n'a en tete.

**Consequence pratique : on ne redige pas un rapport tant qu'il reste quelque chose de
publiable.** Publier d'abord, rendre compte ensuite. Un rapport qui dit « en cours de
publication » alors qu'aucun tag n'est pousse est un faux rapport.

Ce qui autorise legitimement a s'arreter, et rien d'autre : une verification qui
echoue, une question dont la reponse change ce qu'il faut livrer, ou un agent qui
travaille encore. Dans ces cas-la, le dire explicitement dans le bloc 5 du rapport.

## Principe

**Tu vas jusqu'au bout, seul.** Jimmy ne tranche QU'UNE chose : les demandes de
nouvelle fonctionnalite. Tout le reste — corriger un bug, demander une precision,
expliquer qu'une fonctionnalite existe deja, releaser, repondre, fermer — se fait
sans lui poser la question.

Ne lui remonte pas un tableau a valider ligne par ligne. Tu agis, puis tu annonces
ce qui est parti.

## L'ETAT DE CHAQUE ISSUE EST VISIBLE SUR GITHUB

**Toute issue ouverte porte EXACTEMENT UN label d'etat, tout le temps.** Jimmy doit
pouvoir ouvrir la liste des issues et voir d'un coup d'oeil ou en est chaque chose, sans
demander. Zero etat ou deux etats est une incoherence, pas une nuance.

| Etat | Ce que ca veut dire | La balle est chez |
|---|---|---|
| `a-trier` | arrivee, pas encore analysee | nous |
| `en-analyse` | un agent de triage travaille dessus | nous |
| `attente-arbitrage` | analysee, Jimmy doit trancher | **Jimmy** |
| `a-livrer` | retenue, pas encore livree | nous |
| `en-cours` | un agent ecrit la correction | nous |
| `attente-retour` | livree et publiee, on attend la confirmation | l'auteur |
| `attente-infos` | on attend des precisions pour avancer | l'auteur |
| `refuse` | pas retenu (l'issue se ferme) | — |

### Les transitions, et le geste qui va avec

```
ouverture ─> a-trier ─> en-analyse ─┬─> attente-arbitrage ─> a-livrer ou refuse
                                    ├─> a-livrer        (bug confirme, ou demande retenue)
                                    └─> attente-infos   (il manque une information)

a-livrer ─> en-cours ─> [release publiee + auteur prevenu] ─> attente-retour
                                                                    │
                                          confirmation ou 10 jours ─┴─> ferme
```

Chaque changement d'etat se fait **dans le meme geste que l'action** qui le provoque :

```bash
# retirer l'ancien et poser le nouveau, jamais l'un sans l'autre
gh issue edit <N> --repo jguevel-tech/cockpit --remove-label a-livrer --add-label en-cours
```

Deux moments ou on l'oublie systematiquement, donc a surveiller : quand on LANCE un agent
(l'issue passe `en-analyse` ou `en-cours`) et quand un agent REND son travail (elle sort de
cet etat). Un agent qui travaille sur une issue restee `a-livrer`, c'est un etat qui ment.

### Le controle automatique

`node scripts/issues-nouveautes.mjs` verifie les etats de TOUTES les issues ouvertes, y
compris celles ou rien n'a bouge — parce qu'une issue oubliee est justement celle qui n'a
rien de neuf. Il dit soit « toutes les issues ouvertes en portent exactement un », soit la
liste des fautives. **Corriger avant de continuer** : c'est l'absence de label qui a laissé
l'issue #6 sans reponse pendant des heures.

Les labels de TYPE (`bug`, `enhancement`, `documentation`...) sont independants et peuvent
coexister avec l'etat. Ne pas s'en servir comme etat.

## UNE PROMESSE EST UNE DETTE, ET ELLE SE SUIT

**Ecrire « c'est retenu, je vous previens quand c'est livre » cree une obligation.** Ce
n'est pas une formule de politesse : quelqu'un attend. Et rien ne le rappelle, puisque
l'issue n'attend plus l'auteur — elle nous attend.

D'ou le label **`a-livrer`** : promis, pas encore livre, la balle est chez nous. Il se
pose **dans le meme geste** que le commentaire qui promet, jamais plus tard.

```bash
gh issue comment <N> --repo jguevel-tech/cockpit --body-file <fichier>
gh issue edit    <N> --repo jguevel-tech/cockpit --add-label a-livrer
```

Il se retire quand c'est publie ET l'auteur prevenu — moment ou l'issue passe en
`attente-retour`, puisque la balle repart chez lui.

**Les deux labels sont symetriques et couvrent les deux sens de l'attente :**

| Label | Qui attend | Ce qui le retire |
|---|---|---|
| `attente-retour` | nous attendons l'auteur | sa reponse, ou dix jours de silence |
| `a-livrer` | l'auteur nous attend | la livraison, puis on le previent |

### Ce qui a rendu cette regle necessaire

Le 2026-08-20, trois issues (#2, #3, #6) ont recu « c'est retenu, je vous previens » puis
plus rien pendant des heures, pendant qu'un autre chantier demarrait. Aucun label ne les
suivait, aucun rapport ne les mentionnait, et c'est Jimmy qui a du les retrouver dans
l'interface GitHub.

**Le point important pour la suite** : ce jour-la, le nouveau chantier avait ete demande
par Jimmy lui-meme. Le lancer n'etait donc pas la faute. **La faute etait de ne pas dire
que des promesses etaient en attente.**

### Quand une nouvelle demande arrive alors qu'une promesse est en attente

Tu ne refuses pas, et tu ne repousses pas silencieusement la promesse. Tu **dis les
deux** et tu laisses Jimmy ordonner :

> « Je le fais. Note qu'il reste #2, #3 et #6 promises a gmarchault et non livrees — je
>   les prends avant ou apres ? »

Une seule phrase. Ce qui est interdit, c'est de partir sur la nouvelle demande comme si
les promesses n'existaient pas : elles disparaissent alors du rapport et de la memoire.

Et au retour du nouveau chantier, **les promesses reprennent la tete de file** sans qu'il
faille le redemander.

## ARME LA SURVEILLANCE AVANT TOUT LE RESTE

**Premiere action du run, avant meme l'etape 0** — parce que se fier a sa memoire a
echoue trois fois le 2026-08-20, et que Jimmy a du le signaler chaque fois :

```
Monitor({
  command: `cd <depot>
while true; do
  node scripts/issues-nouveautes.mjs --brut --marquer \\
    --repere=.claude/issues-vues-surveillance.json 2>&1 | grep -E "^(ISSUE #|⚠)" || true
  sleep 60
done`,
  description: "nouveaux commentaires sur les issues du repo cockpit",
  persistent: true,
})
```

Chaque commentaire arrive alors comme une notification dans la conversation, sans
qu'on ait a y penser. Le mode `--brut` se TAIT quand rien n'a bouge : une ligne emise
est un evenement a traiter.

Le repere de la surveillance est un fichier SEPARE de celui de la lecture manuelle
(`--repere=`). C'est voulu : la surveillance marque ce qu'elle annonce, et ne doit pas
marquer comme vu ce que la lecture manuelle n'a pas encore traite.

**Ce que ca remplace** : la regle « relire les issues avant chaque rapport » existait
deja et n'a pas suffi — j'ai lu, marque, puis discute une demi-heure, puis rendu un
rapport sans relire. Une reponse de Jimmy postee entre-temps est restee invisible
jusqu'a ce qu'il demande « faut lire l'issue ^^ ». Une regle qu'on oublie ne vaut rien
face a un mecanisme qui previent.

**Et une consequence a accepter** : la surveillance annonce AUSSI nos propres
commentaires, puisqu'on poste sous le compte de Jimmy et que rien ne les distingue. Ce
n'est pas un defaut a corriger, c'est le prix de la seule methode qui ne mente pas.

## Etape 0 — Suivi des issues en attente de retour (EN PREMIER, ET A CHAQUE PAUSE)

**Cette etape se rejoue a chaque fois que tu rends un rapport, pas seulement au
demarrage du run.** La jouer une seule fois au debut ne sert a rien : a ce moment-la tu
n'as encore rien demande a personne. Les reponses arrivent APRES tes questions, pendant
que tu travailles sur autre chose.

Le 2026-08-20, deux auteurs avaient repondu depuis deux heures et personne ne les avait
lus : les questions avaient ete posees a 11h00, l'etape 0 n'avait tourne qu'a 10h30.
C'est Jimmy qui a du le remarquer.

Donc : avant chaque rapport, tu relis les DEUX listes — celles ou l'on attend l'auteur et
celles ou l'auteur nous attend. C'est court, et c'est ce qui evite de laisser quelqu'un sans
reponse pendant qu'on s'active ailleurs.

```bash
gh issue list --repo jguevel-tech/cockpit --state open --label attente-retour --json number,title,comments
gh issue list --repo jguevel-tech/cockpit --state open --label a-livrer      --json number,title,comments
```

La deuxieme liste n'est pas informative : **c'est ta file de travail.** Une issue en
`a-livrer` passe avant toute issue neuve.

**Les reponses arrivent souvent par MAIL**, donc avec l'ancien message cite en dessous
et des mentions du fournisseur (« Yahoo Mail : Recherchez, organisez... »). Ne lis que
le haut du commentaire : le reste est du bruit de citation, pas du contenu.


Le label `attente-retour` veut dire une seule chose : **la balle est chez l'auteur**.
Rien d'autre ne se passe sur ces issues tant qu'il ne parle pas — **on ne relance pas
une deuxieme fois**.

**Des que la balle revient chez nous, le label saute.** Un auteur qui a repondu et une
demande que Jimmy a retenue : ce n'est plus une attente, c'est du travail a faire. Si le
label restait, le compteur des dix jours fermerait tout seul une issue sur laquelle on
est en train de bosser. On retire donc le label et on laisse l'issue ouverte jusqu'a la
livraison :

```bash
gh issue edit <N> --repo jguevel-tech/cockpit --remove-label attente-retour
```

```bash
gh issue list --repo jguevel-tech/cockpit --state open --label attente-retour \
  --json number,title,author,comments
```

Pour chacune, compare **notre dernier commentaire** (auteur `jguevel-tech`) et le
**dernier commentaire de l'auteur de l'issue** :

| Situation | Ce que tu fais |
|---|---|
| L'auteur a repondu apres nous que c'est bon | remercier, retirer le label, **fermer** |
| L'auteur a repondu que c'est toujours casse | retirer le label, reprendre l'issue au triage **avec les nouvelles informations** (souvent une capture de plus) |
| L'auteur a repondu autre chose (question, cas different) | repondre, garder le label — le compteur des dix jours repart de notre reponse |
| Aucune reponse de l'auteur, notre commentaire a **plus de 10 jours** | **fermer**, en disant que ca reste rouvrable |
| Aucune reponse, moins de 10 jours | **ne rien faire** — ne pas relancer, ne pas commenter |

Les dix jours se comptent depuis **notre dernier commentaire**, pas depuis
l'ouverture de l'issue :

```bash
# age en jours de notre dernier commentaire sur l'issue N
gh issue view <N> --repo jguevel-tech/cockpit --json comments \
  --jq '[.comments[] | select(.author.login=="jguevel-tech")] | last | .createdAt'
# comparer a : date -u -d '10 days ago' +%Y-%m-%dT%H:%M:%SZ
```

Commandes de fermeture et de label :

```bash
gh issue edit  <N> --repo jguevel-tech/cockpit --remove-label attente-retour
gh issue close <N> --repo jguevel-tech/cockpit --comment "..."
```

Un « ca ne marche toujours pas » n'est pas un echec du run precedent, c'est une
information neuve : le symptome a survecu a une correction verifiee, donc le
diagnostic etait incomplet. Reprendre au triage, pas re-patcher au meme endroit.

## Etape 1 — Collecter les issues ET leurs captures

```bash
gh issue list --repo jguevel-tech/cockpit --state open \
  --json number,title,body,author,createdAt,comments,labels --limit 50
```

**Les captures d'ecran ne sont pas optionnelles.** Il y en aura sur la plupart des
issues, et c'est souvent la seule preuve du symptome. Pour chaque URL d'image
trouvee dans un `body` ou un `comments[].body` (balises `<img src="...">` ou
markdown `![](...)`, domaine `github.com/user-attachments/assets/...`) :

```bash
curl -sL -o <scratchpad>/issue<N>_<i>.png "<url>"   # pas besoin de token, repo public
```

Puis **lis chaque fichier avec l'outil Read** — il affiche les images. Verifie que
`file` annonce bien une image avant de conclure quoi que ce soit ; un HTML de 2 ko
telecharge a la place d'un PNG veut dire que tu n'as rien vu.

Ne jamais ecrire « d'apres la capture » sans l'avoir ouverte.

## Etape 2 — Triage, un agent par issue, en parallele

Un agent `issue-triage` par issue (definition dans `.claude/agents/issue-triage.md`),
**tous lances dans le meme message** pour qu'ils tournent en parallele.

Donne a chaque agent : le numero, le titre, le corps, les commentaires, et le chemin
des captures deja telechargees. Le reste de ses consignes est dans son fichier — ne
le recopie pas dans le prompt, sinon les deux versions divergent et la correction
d'une regle ne prend plus effet.

Chaque agent rend la fiche decrite dans sa definition : classe, preuve
(`fichier:ligne`), cause, correction proposee, portee, langue de l'auteur, doute.

**DONNE A CHAQUE AGENT DE TRIAGE LE CHEMIN DU SCRATCHPAD** et demande-lui d'y ecrire sa
fiche dans un fichier (`triage-<numero>.md`). C'est le point le plus important de cette
etape : le 2026-08-20, onze agents en lecture seule sur treize ont termine sans que leur
texte final ne remonte, alors que TOUS les agents de correction ont livre — parce qu'un
correcteur depose son travail dans le depot au lieu de compter sur un message. Un fichier
arrive toujours, un message non. Tu lis ensuite les fichiers toi-meme.

**Le protocole normal est en DEUX temps : l'agent signale qu'il est disponible, puis
tu lui demandes sa fiche.** Ce n'est pas une panne, c'est ce qui se passe a chaque
fois — le 2026-08-20, sur neuf agents plus deux remplacants, tous ont annonce leur
disponibilite sans joindre la fiche. Ne t'en etonne pas et ne refais surtout pas leur
travail : reponds a chaque notification d'inactivite par une demande de fiche au
format, avec la question precise qui tranche le classement. Ils ont deja lu le code et
repondent vite — les fiches obtenues ainsi ont ete les meilleures du lot.

Prevois donc ce va-et-vient dans ton deroule : neuf issues, c'est neuf triages ET neuf
demandes de fiche.

Profite de la relance pour transmettre ce que les autres fiches ont deja etabli : sur
ce meme run, savoir que l'aller-retour marked/turndown perd le `<pre>` nu a oriente
utilement le triage des liens. Les agents ne se parlent pas entre eux, c'est a toi de
faire circuler.

**Deux DEMANDES EXPLICITES sans fiche = agent perdu, on le remplace.** Attention a ne
pas confondre avec le va-et-vient normal ci-dessus : ce qui compte, c'est une demande
de fiche restee sans reponse, pas une notification d'inactivite. Au deuxieme echec,
lance un agent NEUF sur la meme issue avec un prompt plus resserre — les questions
precises a trancher, et ce que les autres fiches ont deja etabli pour qu'il n'ait pas
a le redemontrer. Constate le 2026-08-20 sur les issues #8 et #9.

`ListAgents` ne sert a rien pour ca : il ne liste que les autres sessions Claude, pas
tes propres sous-agents. Leur etat ne se connait que par leurs notifications.

Regle de classement qui revient souvent : **une fonctionnalite qui existe mais que
l'utilisateur n'a pas trouvee est un probleme de decouvrabilite, pas un bug.** Elle
se traite par une reponse + un ajout dans la doc integree
(`src/lib/components/docs/DocsView.svelte`), pas par du code neuf.

## Etape 3 — Ce que tu fais de chaque classe

| Classe | Action | Fermer l'issue ? |
|---|---|---|
| `bug-confirme` | corriger (etape 4), releaser (etape 5), demander a l'auteur de verifier chez lui | **non** — label `attente-retour` |
| `bug-non-reproduit` | repondre en demandant ce qui manque | **non** — label `attente-retour` |
| `existe-deja` | expliquer ou c'est + completer la doc integree si elle est muette | **non** — label `attente-retour` |
| `nouvelle-fonctionnalite` | analyser et s'arreter — Jimmy tranche | non, jamais |

**On ne ferme JAMAIS une issue soi-meme au moment ou on la traite.** Ce n'est pas
regle parce que le code est corrige et publie : c'est regle quand ca marche chez la
personne qui l'a signalee. Une issue fermee de notre propre autorite oblige
l'utilisateur a rouvrir ou a en creer une deuxieme pour dire que ca ne va toujours
pas — et la plupart ne le font pas, ils arretent juste de signaler.

La fermeture se fait a l'etape 0 d'un run suivant, sur confirmation de l'auteur ou
apres dix jours de silence.

Pour `existe-deja` et `nouvelle-fonctionnalite`, si la doc integree est completee,
c'est une modification visible : elle passe par le changelog et part dans la release.

**Ne jamais ecrire a un auteur qu'une demande est refusee** sans l'accord de Jimmy.
Une demande non retenue reste ouverte, sans reponse.

### Si tu ne comprends pas l'issue : demande

**Une question a l'auteur coute un aller-retour ; se tromper coute une release, une
fonctionnalite a jeter et sa confiance.** Des qu'une fiche laisse un doute qui change
ce qu'il faudrait faire, tu ne devines pas : tu demandes, **dans la langue de
l'auteur**, puis tu poses le label `attente-retour` et tu passes a la suite.

Ca vaut pour tous les cas de figure :

- l'issue est vide ou tient en un titre, et plusieurs lectures sont possibles ;
- l'application a plusieurs endroits qui correspondent au titre (« arborescence »
  peut viser les notes, les dossiers de projets ou l'onglet Fichiers) ;
- le perimetre demande n'est pas clair (une zone precise, ou toute la mise en page) ;
- l'auteur decrit un symptome que le code contredit : demande ce qu'il a vu a
  l'ecran, pas une confirmation de ton hypothese.

Comment demander, en une question **fermee** quand c'est possible : proposer les
lectures possibles et lui faire choisir, plutot qu'un « pouvez-vous preciser ? » qui
lui laisse tout le travail. Deux ou trois options nommees avec le chemin dans
l'interface, il repond en un mot.

Ce qui reste interdit : poser la question ET coder l'hypothese en parallele. On
attend la reponse — sauf si une partie de l'issue est certaine, auquel cas cette
partie se corrige et la question ne porte que sur le reste.

Le champ `doute` des fiches de triage sert exactement a reperer ces cas. Une fiche
qui dit « a confirmer d'un mot avant d'ouvrir le chantier » est une question a poser,
pas une hypothese a retenir.

## Etape 4 — Corrections, EN SERIE

Un agent `issue-fix` par bug confirme (definition dans `.claude/agents/issue-fix.md`),
**lances un par un, jamais en parallele**. Raison : `cargo test`, `npm run check` et
`tauri build` se disputent `target/`, et deux agents qui touchent `CHANGELOG.md` en
meme temps fabriquent un conflit.

Donne a chaque agent : le numero d'issue, la fiche de triage, et les captures. Sa
sequence (reproduire, corriger, traduire, verifier les 5 points, commiter) est dans
son fichier.

Si un agent n'arrive pas a reproduire ce qu'un agent de triage avait classe
`bug-confirme`, l'issue redescend en `bug-non-reproduit` : on ne patche pas a
l'aveugle.

**Le `CLAUDE.md` du projet doit rester exact.** Il se met a jour dans le meme commit que
le correctif, jamais « plus tard » : une ligne perimee coute plus cher qu'une ligne
absente, parce qu'on la croit. Ca vaut pour les tableaux (stores, commandes, tables,
events, onglets), pour les comportements decrits, et pour toute affirmation qu'on
decouvre fausse — meme sans rapport avec la tache en cours. Ce qui a coute cher a
comprendre va dans « Pieges connus », avec le critere : quelqu'un qui arrive demain sur
ce fichier perdrait-il le meme temps ?

**Les defauts trouves en chemin font partie du lot.** Un triage qui ouvre un fichier
en trouve souvent plus que l'issue n'en demandait : un bug voisin, une chaine non
traduite, un garde-fou qui ne garde rien. Ca ne se met pas de cote au motif que
personne ne l'a signale — ca se corrige, ca va au changelog si c'est visible, et ca
part dans la meme release. Le champ `croise en chemin` du rendu des agents de
correction sert a en garder la trace.

Ce qui remonte a Jimmy plutot que d'etre fait : un remaniement dont l'ampleur depasse
largement l'issue. La aussi c'est une decision de perimetre, donc la sienne.

**Entre deux agents, applique l'etape 4bis.** C'est la raison d'etre de la serie.

## Etape 4bis — Corriger le skill et les agents, PENDANT le run

**Une erreur de workflow se corrige dans le fichier, tout de suite, pas dans ta
tete.** Ce skill et les deux definitions d'agents sont faits pour etre modifies en
cours de route. Une lecon qui ne vit que dans la conversation est perdue au prochain
run.

Declencheurs — des qu'un de ces cas se produit, tu edites avant de continuer :

- un agent a conclu sans ouvrir une capture ;
- un agent a patche sans reproduire ;
- un agent a oublie `en.ts`, le changelog, ou une des 4 commandes de verification ;
- un agent a annonce un succes que le log ne montre pas ;
- une consigne du skill s'est revelee fausse, ambigue ou absente (le champ
  `ecart au workflow` de la fiche `issue-fix` sert exactement a ca) ;
- une commande donnee ici n'a pas marche telle quelle ;
- Jimmy te reprend sur la facon de faire.

Ou ecrire la correction :

| Nature de l'erreur | Fichier a modifier |
|---|---|
| Un agent de triage s'est trompe de methode | `.claude/agents/issue-triage.md`, section « Regles ajoutees apres une erreur » |
| Un agent de correction a mal travaille | `.claude/agents/issue-fix.md`, meme section |
| L'enchainement, la release, les reponses, le decoupage | ce fichier |
| Une regle du projet qui manquait | `CLAUDE.md` du projet |

Comment ecrire l'entree : **la regle d'abord, la raison ensuite, en une ou deux
lignes.** Pas de recit. Ce qui compte est ce qu'il faut faire la prochaine fois, et
pourquoi — sans le pourquoi, la regle sera « simplifiee » plus tard par quelqu'un
qui la croit inutile.

Effet immediat :

- **Corrections (serie)** : l'agent suivant lit la version corrigee. C'est pour ca
  qu'elles ne sont pas parallelisees — une lecon du bug n°1 profite au bug n°2.
- **Triage (parallele)** : les agents deja lances ne verront pas la correction. Si
  l'erreur invalide une fiche, **relance cet agent-la** apres avoir edite. Sinon la
  lecon sert au prochain run.

Ces modifications partent dans le commit du lot, avec les corrections de code. Le
message dit ce qui a ete appris. Un skill qui n'a pas bouge apres un run ou quelque
chose s'est mal passe est un skill qui reproduira la meme erreur.

**Ne pas demander la permission de corriger le skill.** C'est de l'outillage interne.

## REGLE NUMERO 2 — NE JAMAIS ATTENDRE LES BRAS BALLANTS

**Attendre une CI ou une release n'occupe aucune ressource locale.** Le build tourne
chez GitHub pendant ~10 min : pendant ce temps la machine est libre, donc le sujet
suivant DEMARRE. Ne reste jamais a regarder un build.

Ce qui peut tourner en meme temps, sans se genner :

| En parallele | Pourquoi c'est sans risque |
|---|---|
| Une CI qui construit + un agent de correction local | le build est chez GitHub, la machine ne fait rien |
| Un agent de correction + des agents de TRIAGE | le triage est en lecture seule |
| Un agent de correction + les reponses aux auteurs d'un lot deja publie | ce sont des appels `gh`, aucun fichier touche |

La SEULE chose qui reste strictement sequentielle : **deux agents qui ECRIVENT du
code**. Ils se disputent `src-tauri/target/` pendant `cargo test` et `tauri build`, et
ils se marchent dessus sur `CHANGELOG.md`. Un seul agent de correction a la fois, donc —
mais jamais un seul agent en tout.

**Et toi non plus tu n'ecris pas dans les fichiers partages pendant qu'un agent de
correction tourne.** `CHANGELOG.md` et `CLAUDE.md` sont ecrits par les agents depuis que
la mise a jour de la doc est une etape de leur sequence : si tu y touches en meme temps,
ton `git add` embarque leur travail en cours sous un message de commit qui ne le decrit
pas. Constate le 2026-08-20 : une regle ajoutee au `CLAUDE.md` a emporte avec elle la
moitie de la documentation qu'un agent etait en train d'ecrire sur le double collage.
Les fichiers du skill et des agents (`.claude/`) restent libres, eux : personne d'autre
n'y touche.

Si une modification du `CLAUDE.md` ne peut pas attendre, commite en nommant
EXPLICITEMENT les fichiers voulus (`git add CLAUDE.md` seul ne suffit pas si l'agent
l'edite aussi) — ou attends la fin de l'agent, ce qui est plus simple. Et previens-le :
son `git diff` montrera moins que ce qu'il a ecrit.

Le deroule normal ressemble donc a ca, et non a une file d'attente :

```
sujet N : commit -> release -> tag pousse ─┐
                                            ├─> CI (~10 min, chez GitHub)
sujet N+1 : agent de correction lance ─────┘        │
triage des issues neuves en parallele              │
                                            CI verte -> prevenir les auteurs du sujet N
```

Quand la CI se termine, tu reviens prevenir les auteurs du lot precedent — sans
interrompre le sujet en cours. Utilise une attente en arriere-plan
(`run_in_background` avec une boucle `until`) pour etre notifie, plutot que de boucler
en avant-plan.

## Regle de cadence : FINIR CE QUI EST COMMENCE

**On ne s'ouvre pas un nouveau chantier tant que ceux en cours ne sont pas termines.**
Un sujet termine, c'est : corrige, verifie, commite, publie, auteur prevenu, label pose.
Tant qu'il manque une de ces etapes, le sujet n'est pas fini et il passe AVANT toute
nouvelle issue.

Le travers a eviter, constate le 2026-08-20 : neuf issues ouvertes de front, des
corrections verifiees mais pas publiees, des questions posees mais pas relancees, et un
agent lance sur un dixieme sujet pendant que rien n'aboutissait. Beaucoup de mouvement,
rien de livre.

Ce que ca veut dire concretement :

- **Le triage peut rester groupe** : il est en lecture seule et donne la vue d'ensemble
  necessaire pour ordonner le travail. C'est apres qu'il faut de la discipline.
- **La release peut grouper plusieurs sujets termines** — ce n'est pas la cadence de
  publication qui est en cause, c'est le fait de laisser des sujets en plan.
- **Avant de lancer un agent sur une nouvelle issue, regarde ce qui traine** : une
  correction non publiee, un auteur non prevenu, un rapport non rendu. On termine, puis
  on ouvre.
- Un sujet vraiment bloque (il attend une reponse de Jimmy ou d'un auteur) ne compte pas
  comme en cours : il est en attente, il figure dans le rapport, et il ne bloque pas la
  suite.

## Etape 5 — Release, puis les reponses

Quand toutes les corrections sont commitees :

```bash
npm run release -- <patch|minor|major>   # niveau deduit du contenu de [Unreleased]
git push origin main
git push origin vX.Y.Z                   # c'est ce tag qui publie
```

Le niveau se lit dans `[Unreleased]` : seulement `Fixed` -> `patch` ; au moins un
`Added`/`Changed` visible -> `minor`. En cas de doute, le niveau superieur.

**Attendre que la release soit reellement publiee avant de repondre aux gens.**
Annoncer une mise a jour qui n'existe pas encore est le pire resultat possible :

```bash
gh run list --repo jguevel-tech/cockpit --limit 3          # attendre la fin
curl -sL -o /dev/null -w "%{http_code}\n" \
  https://github.com/jguevel-tech/cockpit/releases/latest/download/latest.json
```

Le code doit etre `200`. Un `404` dans les 2 premieres minutes, c'est la propagation
du CDN GitHub — re-tester avant de conclure. Un `404` qui persiste veut dire que la
release est incomplete : la reparer (voir les pieges du `CLAUDE.md` du projet) AVANT
de dire a qui que ce soit de mettre a jour.

**Le code 200 NE SUFFIT PAS : il faut lire la VERSION servie.** La release sort en
brouillon, et `releases/latest` continue de servir la version PRECEDENTE jusqu'a ce
que le job `publier` leve le brouillon. Donc 200 pendant tout le build, avec l'ancienne
version dedans. Le 2026-08-20, `latest.json` repondait 200 avec `0.31.1` alors que la
`0.31.2` etait en cours de construction : repondre aux auteurs a ce moment-la, c'etait
leur dire de mettre a jour vers une version que GitHub ne servait pas encore.

```bash
curl -sL -o /tmp/latest.json \
  https://github.com/jguevel-tech/cockpit/releases/latest/download/latest.json
python3 -c "import json;d=json.load(open('/tmp/latest.json'));print(d['version'], sorted(d['platforms']))"
```

La version affichee doit etre CELLE QUE TU VIENS DE TAGUER, et `platforms` doit
contenir une entree `linux-*`. Tant que ce n'est pas le cas, on attend : `gh run list`
pour suivre, et rien n'est annonce a personne.

Ne pas lire ce fichier avec `curl | head` : le proxy `rtk` reformate la sortie et
remplace les valeurs par des longueurs de chaine. Ecrire dans un fichier, puis lire
avec `python3`.

Puis, issue par issue — **commenter et poser le label, sans fermer** :

```bash
gh issue comment <N> --repo jguevel-tech/cockpit --body "..."
gh issue edit    <N> --repo jguevel-tech/cockpit --add-label attente-retour
```

Le commentaire doit demander a l'auteur de verifier chez lui et de repondre. Sans
cette demande explicite, le label ne veut rien dire : personne ne sait qu'on attend
quelque chose.

## Etape 6 — Rapport a Jimmy : FORMAT IMPOSE

**A chaque fois que tu t'arretes de travailler, tu rends ce rapport.** Pas seulement a
la fin du run : a chaque pause, meme au milieu. Un point d'etape en prose ou il doit
deviner s'il a quelque chose a faire est un mauvais rapport, meme s'il est exact.

**Avant d'ecrire le rapport, rejoue l'etape 0.** Un rapport qui annonce « en attente de
retour » sur une issue ou l'auteur a repondu deux heures plus tot est un faux rapport.
C'est la premiere chose a faire, avant meme de rassembler ce que tu as termine.

Les cinq blocs ci-dessous, dans cet ORDRE, en sautant ceux qui sont vides — sauf le
premier et le dernier, jamais omis.

### 1. Ce que j'attends de toi

En premier, toujours. **Pose-les avec `AskUserQuestion`, en choix cliquables**, pas en
prose : sinon Jimmy doit recopier la question pour y repondre, il l'a demande le
2026-08-20. Une question par decision, ta recommandation en premiere option et marquee
comme telle.

**S'il n'y a rien, ecris-le franchement : « Rien de ton cote. »** C'est une
information, pas un blanc a combler.

**Formule le comportement par un EXEMPLE CONCRET, pas par la mecanique interne.**
Jimmy a du reformuler lui-meme une question mal posee sur l'issue #7, et sa version est
le modele a suivre :

> « si j'etais sur l'onglet fichier dans le projet toto, quand je vais sur le projet
> tata voir un truc et que je reviens sur toto, faut que je revienne sur l'onglet
> fichiers »

Ce que ca donne comme regle : raconte ce que fait l'utilisateur et ce qu'il voit, pas
le nom du store ni la ligne qui change. « L'onglet actif est remis a workspace dans
selectProject » ne dit pas ce que la personne vit ; « quand tu reviens sur un projet,
tu retombes sur Workspace au lieu de l'onglet ou tu etais » le dit. La mecanique va
apres, si elle sert a decider.

Cette regle vaut aussi pour les rapports et pour les questions posees aux auteurs
d'issues.

### 2. Termine

Tableau. Une ligne par chose finie, avec ce qui a ete livre et ou.

| Issue / sujet | Ce qui a ete fait | Version |
|---|---|---|

### 3. En cours

Tableau des agents qui travaillent en ce moment. S'il n'y en a aucun, dis-le : il doit
savoir si ca continue de bouger tout seul ou si tout est arrete.

| Agent | Sujet | Depuis |
|---|---|---|

### 4. Les deux sens de l'attente

**4a. On attend l'auteur** (label `attente-retour`) — avec la date de fermeture automatique.

| Issue | On attend quoi | Ferme le |
|---|---|---|

**4b. L'auteur nous attend** (label `a-livrer`) — promis et pas encore livre, avec la date
de la promesse. **Ce tableau ne s'omet jamais tant qu'il n'est pas vide**, meme si le sujet
du moment est ailleurs : c'est le seul endroit ou une promesse oubliee redevient visible.

| Issue | Ce qui a ete promis | Promis le |
|---|---|---|

### 5. Etat de la session

Une seule ligne, explicite, parmi :

- **Termine** — plus rien ne tourne, plus rien ne m'attend, tu peux fermer.
- **En cours** — des agents travaillent, je reviens avec la suite sans que tu fasses
  rien.
- **Bloque** — je ne peux pas avancer avant tes reponses du bloc 1.

Puis, s'il y a lieu, une derniere ligne : ce qui a ete corrige dans le skill ou les
agents pendant le run, une ligne par regle. Ne rien inventer s'il n'y a rien — mais si
quelque chose s'est mal passe et que rien n'a bouge, c'est que l'etape 4bis a ete
sautee.

### Ce qui est interdit dans un rapport

- Enterrer une question au milieu d'un paragraphe : elle va dans le bloc 1 ou nulle
  part.
- Laisser croire que quelque chose avance quand tout est arrete, ou l'inverse.
- Annoncer une version comme livree avant que `latest.json` reponde 200.
- Un pave. Les tableaux existent pour qu'il lise en dix secondes.

## Repondre a l'auteur

**Dans sa langue**, deduite du texte de son issue. Ton sobre et direct, premiere
personne, pas de tutoiement, pas de formule commerciale. **Aucune mention d'IA** —
ni signature, ni « genere par », rien : une reponse d'issue est un outil d'equipe.

Ne jamais brancher une reponse sur une supposition : si le symptome n'est pas
compris, on demande, on n'invente pas une explication credible.

Tous les modeles ci-dessous se terminent par une **question**. C'est voulu : c'est
elle qui justifie le label `attente-retour` et qui declenche la reponse permettant de
fermer.

### Bug corrige et publie (label `attente-retour`, pas de fermeture)

> Ca devrait etre regle dans la v0.32.0, qui vient de sortir.
>
> [une ou deux phrases sur ce qui se passait reellement]
>
> Vous pouvez mettre a jour — la cloche en haut a droite propose la mise a jour,
> sinon Parametres -> General -> Verifier les mises a jour — et me dire si c'est bon
> chez vous ? Je laisse l'issue ouverte en attendant votre retour.
>
> Merci pour le signalement.

Version anglaise :

> This should be fixed in v0.32.0, which was just released.
>
> [what was actually happening]
>
> Could you update — the bell in the top right offers the update, or
> Settings -> General -> Check for updates — and let me know if it works for you
> now? I'll leave the issue open until you confirm.
>
> Thanks for reporting it.

Ne pas ecrire « corrige » tout court : tant que ca n'a pas ete constate sur la
machine de l'auteur, c'est « ca devrait etre regle ». On ne sait pas encore.

### Bug non reproduit

Demander precisement ce qui manque, jamais « pouvez-vous donner plus de details » :
la version installee (Parametres -> General), la distribution, les etapes exactes,
et une capture si le symptome est visuel. Meme label, meme delai de dix jours.

### Question a l'auteur (issue ambigue)

Proposer les lectures possibles, ne pas lui demander de tout reformuler :

> Je veux etre sur de viser le bon endroit avant de m'y mettre : vous parlez de
> [option A, avec son chemin dans l'interface], de [option B] ou des deux ?
>
> [Si utile : ce qui existe deja aujourd'hui pour chacune.]

Version anglaise :

> I want to make sure I'm looking at the right place before starting: do you mean
> [option A, with where it is in the app], [option B], or both?

Pour un symptome que le code contredit, demander ce qu'il a vu, pas une confirmation :

> Chez moi [ce que fait le code]. Qu'est-ce que vous voyez a l'ecran a ce
> moment-la — un message d'erreur, rien du tout, autre chose ? Une capture
> m'aiderait beaucoup.

### Fonctionnalite qui existe deja

Dire ou c'est, en une phrase et un chemin cliquable dans l'interface, puis demander
si ca repond au besoin — il arrive que la demande soit en realite plus large que ce
qui existe. Si la doc integree n'en parlait pas, le dire : c'est un manque cote
projet, pas une erreur de l'utilisateur.

### Fermeture apres confirmation

> Parfait, merci pour la confirmation. Je ferme.

> Great, thanks for confirming. Closing this one.

### Fermeture apres dix jours sans reponse

> Sans retour de votre cote, je ferme — le correctif est parti dans la v0.32.0. Si
> le probleme est toujours la, repondez ici et l'issue sera rouverte.

> No news on your side, so I'm closing this one — the fix shipped in v0.32.0. If the
> problem is still there, just reply here and the issue will be reopened.

Ton neutre. Ne pas reprocher le silence, ne pas dire « faute de retour de votre
part ». L'issue nous a rendu service, la personne ne doit rien.

## Pieges

- **Une capture non regardee est une issue mal comprise.** Telecharger et lire,
  toujours.
- **Ne jamais fermer une issue au moment de la traiter** — seulement a l'etape 0,
  sur confirmation de l'auteur ou apres dix jours de silence.
- **Ne pas relancer deux fois.** Une issue en `attente-retour` de trois jours ne se
  commente pas « alors ? ». On attend.
- **Ne pas oublier de poser le label** apres avoir commente : sans lui, l'issue
  sort du suivi et personne ne la ferme jamais.
- **Ne pas compter les dix jours depuis l'ouverture de l'issue** mais depuis notre
  dernier commentaire.
- **Ne pas paralleliser les corrections** — conflits sur `CHANGELOG.md` et
  `package.json`, et `release.mjs` refuse un arbre sale.
- **Ne pas creer de branche** : sur ce repo, `main` en direct est la regle, et un
  push de branche ne declenche aucun deploiement.
- Plusieurs issues corrigees ensemble partent dans **une seule** version. Chaque
  reponse cite cette version.
- Une issue peut en cacher deux (un bug + une demande). La traiter comme telle :
  corriger la partie bug, remonter l'autre a Jimmy, et le dire dans la reponse.
