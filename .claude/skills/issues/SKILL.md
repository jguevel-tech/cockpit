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

## Principe

**Tu vas jusqu'au bout, seul.** Jimmy ne tranche QU'UNE chose : les demandes de
nouvelle fonctionnalite. Tout le reste — corriger un bug, demander une precision,
expliquer qu'une fonctionnalite existe deja, releaser, repondre, fermer — se fait
sans lui poser la question.

Ne lui remonte pas un tableau a valider ligne par ligne. Tu agis, puis tu annonces
ce qui est parti.

## Etape 0 — Suivi des issues en attente de retour (TOUJOURS en premier)

Le label `attente-retour` veut dire : on a repondu, la balle est chez l'auteur. Rien
d'autre ne se passe sur ces issues tant qu'il ne parle pas — **on ne relance pas une
deuxieme fois**.

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
(`fichier:ligne`), cause, correction proposee, cout, langue de l'auteur, doute.

**Un agent peut se mettre en attente sans rien rendre** (vu le 2026-08-20 sur deux
des neuf agents). Ce n'est pas une raison de refaire son travail toi-meme : relance-le
par `SendMessage` en lui redemandant la fiche au format, et en lui donnant la question
precise qui tranche son classement. Il a deja lu le code, il repond vite.

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

## Etape 5 — Une seule release, puis les reponses

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

Puis, issue par issue — **commenter et poser le label, sans fermer** :

```bash
gh issue comment <N> --repo jguevel-tech/cockpit --body "..."
gh issue edit    <N> --repo jguevel-tech/cockpit --add-label attente-retour
```

Le commentaire doit demander a l'auteur de verifier chez lui et de repondre. Sans
cette demande explicite, le label ne veut rien dire : personne ne sait qu'on attend
quelque chose.

## Etape 6 — Rapport a Jimmy

En fin de traitement, une reponse courte :

- ce qui a ete corrige et dans quelle version
- les issues fermees
- les issues laissees ouvertes en attente d'infos
- **les demandes de nouvelle fonctionnalite, avec pour chacune : ce que ca donnerait,
  ce que ca touche, le cout, et ta recommandation.** C'est la seule partie ou il a
  quelque chose a decider.
- **ce qui a ete corrige dans le skill ou les agents pendant le run**, en une ligne
  par regle ajoutee. S'il n'y a rien, ne rien inventer — mais si quelque chose s'est
  mal passe et que rien n'a bouge, c'est que l'etape 4bis a ete sautee.

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
