# Comparaison du tool Edit : Sinew vs PI vs OpenCode

Objectif : comparer uniquement le tool d'édition par remplacement de texte existant. Je ne traite pas le tool `write`/`create`, sauf quand un agent mélange explicitement la création dans son `edit`.

Repos inspectés :

- Sinew, repo courant : `crates/sinew-app/src/edit.rs`, `read.rs`, `agent/history.rs`, `agent/tool_dispatch.rs`.
- PI : `/Users/hyrak/pi/pi/packages/coding-agent/src/core/tools/edit.ts`, `edit-diff.ts`, `file-mutation-queue.ts`.
- OpenCode : `/Users/hyrak/opencode/opencode/packages/opencode/src/tool/edit.ts`, `multiedit.ts`, `file/time.ts`, `tool/read.ts`.

## Résumé exécutif

- **Sinew** est le plus complet et le plus défensif : multi-fichiers, plusieurs edits par fichier, `replaceAll`, fingerprint SHA-256 après `read`, préservation BOM/CRLF, fallbacks de matching très nombreux. Son matching applique la règle exact-first : si `oldContent` apparaît exactement une fois, ce match est accepté avant les fallbacks fuzzy/permissifs. Les faux `oldContent not unique` restent surtout possibles quand aucun match exact n'existe et que les fallbacks trouvent plusieurs candidats équivalents. Il reste moins atomique que PI/OpenCode `edit` seul, car il peut appliquer partiellement des edits déjà planifiés si un edit suivant échoue.
- **PI** a le meilleur noyau pour le **multi-edit dans un seul fichier** : tous les `edits[]` sont matchés sur le contenu original, les overlaps sont rejetés, puis les remplacements sont appliqués en reverse. C'est propre et évite les effets de décalage. Mais PI n'a pas de `replaceAll`, n'impose pas un `read` préalable, n'a pas de protection forte contre les fichiers modifiés entre-temps, et son fuzzy matching réécrit le fichier dans un espace normalisé si un fuzzy match est utilisé.
- **OpenCode** est le plus pragmatique contre les faux “not unique” : il essaie d'abord le match exact simple et retourne tout de suite si ce match exact est unique, avant de tenter les fallbacks. Il a `replaceAll`, un lock par fichier, une vérification “read before edit” via mtime/size, une permission avec diff, puis diagnostics LSP. Mais son `multiedit` n'est pas réellement atomique malgré sa doc : il appelle `edit` en boucle et peut laisser les premiers edits appliqués si un edit suivant échoue. Son `edit` peut aussi créer un fichier avec `oldString: ""`, donc la séparation Edit/Create est moins nette.

**Verdict court :**

- Pour réduire les faux bugs `oldContent not unique`, **Sinew et OpenCode ont le meilleur comportement** sur le point critique : “exact unique wins before fuzzy”.
- Pour le design multi-edit, **PI garde la meilleure idée algorithmique** : matcher tous les edits contre l'original et rejeter les overlaps avant d'écrire.
- Le meilleur design cible serait donc : **Sinew + planning all-or-nothing PI + meilleurs diagnostics de duplicate**.

## Tableau comparatif

| Sujet | Sinew actuel | PI | OpenCode |
|---|---|---|---|
| Nom du tool | `edit_file` | `edit` | `edit`, plus `multiedit` séparé |
| Forme d'entrée | `files[]`, chaque item a `path` + `edits[]` | `path` + `edits[]` | `filePath`, `oldString`, `newString`, `replaceAll`; `multiedit` a `edits[]` |
| Multi-fichiers | Oui, dans un seul appel | Non | Non pour `edit`; `multiedit` reste mono-fichier |
| Multi-edit même fichier | Oui | Oui | Oui via `multiedit`, mais séquentiel |
| Ordre des edits | Séquentiel : edit N voit le résultat des edits précédents | Tous matchés contre le contenu original | `multiedit` séquentiel : chaque edit voit le résultat du précédent |
| Unicité requise | Oui, sauf `replaceAll` | Oui, pas de `replaceAll` | Oui, sauf `replaceAll` |
| `replaceAll` | Oui | Non | Oui |
| `read` préalable obligatoire | Oui, avec fingerprint SHA-256 + size + mtime | Non imposé par le tool | Oui pour fichiers existants via `FileTime`, mtime + size |
| Stale file protection | Forte : SHA-256 + size + modified_ms | Faible côté edit | Moyenne : mtime + size |
| Création via Edit | Non | Non | Oui si `oldString === ""` |
| Line endings/BOM | Normalise LF, restaure CRLF, préserve BOM | Normalise LF, restaure CRLF, préserve BOM | Préserve CRLF; pas de traitement BOM explicite |
| Fuzzy/permissive matching | Très large : exact-first, puis fuzzy ponctuation/espaces, whitespace, indentation, anchors, escapes, context | Exact + fuzzy ponctuation/espaces/trailing whitespace | Exact + nombreux fallbacks whitespace/indent/anchors/escapes/context, mais pas smart quotes/dashes |
| Exact unique vs fuzzy ambigu | Correct : l'exact unique gagne avant les fallbacks | Risque possible en fuzzy-normalized space selon le cas | Correct : l'exact unique gagne avant les fallbacks |
| Risque faux “non unique” | Faible pour les cas exacts; possible si aucun exact match n'existe et que fuzzy/permissif est ambigu | Possible aussi, car le comptage se fait en fuzzy-normalized space | Faible pour les cas exacts; possible si les fallbacks sont ambigus |
| Atomicité intra-fichier | Non : peut appliquer les edits précédents si un edit suivant échoue | Oui : si un edit échoue/overlap/no-op, rien n'est écrit | `edit` seul oui; `multiedit` non en pratique |
| Concurrency | Main turn séquentiel; lock workspace optionnel pour teams | Queue par realpath entre edit/write | Lock par fichier via `FileTime.withLock` |
| Post-edit | Retourne diff/file changes + met à jour fingerprint | Retourne diff + patch | Permission diff avant écriture, formatage, diff final, diagnostics LSP |

## Sinew actuel

### Interface

Sinew expose `edit_file` avec une forme assez riche :

```json
{
  "files": [
    {
      "path": "src/app.ts",
      "edits": [
        {
          "oldContent": "exact text",
          "newContent": "replacement",
          "replaceAll": false
        }
      ]
    }
  ]
}
```

Détails importants :

- `files` est obligatoire et chaque fichier a son tableau `edits`.
- Plusieurs fichiers peuvent être modifiés dans un seul appel.
- Plusieurs edits dans un même fichier sont appliqués **dans l'ordre**, et chaque edit voit le contenu modifié par les précédents.
- `replaceAll: true` remplace toutes les occurrences non chevauchantes.
- `oldText` / `newText` sont acceptés comme alias de `oldContent` / `newContent`, mais le schéma public pousse `oldContent` / `newContent`.
- Limites de sécurité : max `128` replacements et max `2 MiB` de contenu old+new dans un appel.

### Sécurité avant écriture

Sinew exige un `read` réussi avant `edit_file`. Le `read` produit un fingerprint : chemin relatif, taille, `modified_ms`, SHA-256. Avant l'edit, Sinew recalcule le fingerprint courant et refuse si le fichier a changé.

C'est le mécanisme le plus solide des trois agents, parce que le SHA-256 évite les cas où mtime/size ne suffisent pas.

### Matching

Sinew essaie :

1. match exact ;
2. match fuzzy avec mapping vers le texte original ;
3. plusieurs fallbacks permissifs : lignes trimées, block anchors, whitespace normalized, indentation flexible, escapes, trimmed boundary, context-aware.

Points forts :

- Préserve CRLF et BOM.
- Le fuzzy de Sinew mappe le match normalisé vers les offsets originaux, donc il évite mieux que PI de normaliser tout le fichier.
- `replaceAll` existe.
- Les erreurs sont explicites : not found, duplicate, no-op, stale read, partial failure.

Point clé : **Sinew évite les faux doublons exact-vs-fuzzy**.

`find_unique_replacement_match` applique l'ordre de décision suivant :

1. chercher les matchs exacts ;
2. si le match exact est unique, l'accepter immédiatement ;
3. si le match exact est dupliqué, refuser sauf `replaceAll` ;
4. seulement si aucun match exact n'existe, essayer le fuzzy puis les matchers permissifs.

Exemple conceptuel :

```txt
title: “Hello”
title: "Hello"
```

Si le modèle demande :

```json
{ "oldContent": "title: \"Hello\"", "newContent": "title: hi" }
```

Le match exact est unique : seulement la deuxième ligne. Même si le fuzzy normalise les smart quotes et voit deux lignes équivalentes, Sinew applique le match exact unique. Le fuzzy ne sert que de fallback quand l'exact ne trouve rien.

### Atomicité

Sinew n'est pas strictement all-or-nothing :

- Dans un même fichier, si `edits[0]` réussit puis `edits[1]` échoue, Sinew peut écrire le résultat de `edits[0]` et retourner une erreur partielle.
- Sur plusieurs fichiers, si un fichier plus tard échoue après des fichiers déjà planifiés, Sinew peut écrire les fichiers précédents puis signaler une erreur partielle.

C'est bien documenté dans les messages d'erreur, mais c'est moins sûr que “tout réussit ou rien n'est écrit”.

## PI

### Interface

PI expose `edit` avec :

```json
{
  "path": "src/app.ts",
  "edits": [
    { "oldText": "exact text", "newText": "replacement" }
  ]
}
```

Le schéma public ne contient plus `oldText` / `newText` au top-level, mais `prepareArguments` accepte encore l'ancien format et le transforme en `edits[]`. Il accepte aussi certains cas où `edits` est envoyé comme JSON string par le modèle.

### Algorithme multi-edit

C'est le meilleur point de PI :

- Tous les `edits[]` sont normalisés en LF.
- Tous les `oldText` sont cherchés sur le **même contenu original**.
- Chaque `oldText` doit être unique.
- Les overlaps sont rejetés explicitement.
- Les remplacements sont triés et appliqués en ordre inverse pour ne pas casser les offsets.
- Si un edit échoue, rien n'est écrit.

C'est une très bonne stratégie pour “changer 10 endroits distincts d'un fichier” sans que les premiers edits déplacent les suivants.

### Matching

PI fait :

1. exact match ;
2. fuzzy match si l'exact échoue.

Le fuzzy normalise :

- trailing whitespace ;
- smart quotes ;
- Unicode dashes ;
- espaces Unicode ;
- NFKC.

Limite importante : si un seul edit nécessite du fuzzy, PI bascule le `baseContent` en version fuzzy-normalized. Donc le remplacement est calculé sur un contenu normalisé, pas sur les offsets originaux. Cela peut corriger des détails utiles, mais aussi modifier indirectement de la ponctuation, des espaces spéciaux ou du trailing whitespace hors zone ciblée.

PI compte aussi les occurrences dans l'espace fuzzy-normalized. Donc un exact match unique peut devenir “duplicate” si plusieurs zones deviennent identiques après normalisation.

### Sécurité

PI a un bon lock par fichier : `withFileMutationQueue` sérialise les opérations `edit` et `write` ciblant le même realpath, tout en laissant les fichiers différents avancer en parallèle.

Mais PI n'impose pas un `read` préalable et ne vérifie pas un fingerprint stale avant édition. C'est un gros écart de sécurité par rapport à Sinew/OpenCode.

## OpenCode

### Interface

OpenCode expose un `edit` simple :

```json
{
  "filePath": "/absolute/path/to/file.ts",
  "oldString": "text to replace",
  "newString": "replacement",
  "replaceAll": false
}
```

Il expose aussi `multiedit`, qui appelle `edit` en boucle.

À noter : si `oldString` est vide, `edit` crée/écrase le fichier avec `newString`. Comme tu veux séparer Edit de Create, c'est un point négatif conceptuel : OpenCode mélange les responsabilités.

### Sécurité avant écriture

Pour un fichier existant, OpenCode impose un `read` préalable via `FileTime.assert`. Le `read` stocke `mtime` + `size`, puis `edit` refuse si l'un des deux a changé.

C'est moins robuste que le SHA-256 de Sinew, mais mieux que PI.

OpenCode prend aussi un lock par fichier avec `FileTime.withLock`, demande la permission avec un diff avant d'écrire, puis publie les événements de fichier.

### Matching

OpenCode essaie une chaîne de “replacers” :

1. exact simple ;
2. line-trimmed ;
3. block anchor ;
4. whitespace normalized ;
5. indentation flexible ;
6. escape normalized ;
7. trimmed boundary ;
8. context aware ;
9. multi-occurrence.

La différence cruciale avec Sinew : **OpenCode retourne immédiatement si le match exact simple est unique**. Il ne laisse pas un fallback fuzzy/permissif plus large transformer un exact unique en duplicate.

C'est probablement la raison pour laquelle OpenCode donne moins souvent l'impression que “old content n'est jamais unique”.

En revanche :

- OpenCode ne normalise pas les smart quotes / Unicode dashes comme PI/Sinew.
- Certains fallbacks par anchors peuvent choisir un “best candidate” si les première/dernière lignes matchent et que la similarité est suffisante. Ça peut être pratique, mais c'est moins déterministe qu'un exact-only strict.
- Le `replaceAll` existe et utilise `content.replaceAll(search, newString)` sur le candidat retenu.

### `multiedit` : doc vs réalité

La doc de `multiedit` dit que l'opération est atomique : “either all succeed or none are applied”. Mais l'implémentation appelle `EditTool.execute` dans une boucle. Chaque appel écrit le fichier immédiatement.

Conséquence : si edit 1 réussit et edit 2 échoue, edit 1 reste appliqué. Donc `multiedit` est **séquentiel et partiel**, pas atomique.

### Post-edit

OpenCode lance `Format.file(filePath)` après l'écriture. C'est agréable pour la qualité du code, mais ça peut modifier plus que `oldString/newString`, donc le diff final peut inclure des changements induits par le formatter. Il ajoute aussi les diagnostics LSP dans le résultat si des erreurs apparaissent.

## Pourquoi `oldContent` devient “jamais unique” ?

Il y a trois causes différentes :

### 1. Le modèle choisit un bloc trop court

Tous les agents rejettent un remplacement non-`replaceAll` si `oldContent` / `oldText` / `oldString` apparaît plusieurs fois. Exemple : remplacer juste `return null` dans un fichier avec 8 `return null` échoue partout.

Solution côté prompt/tool description : demander explicitement 2-5 lignes de contexte autour du changement, mais pas un énorme bloc.

### 2. Le tool rend l'unicité plus stricte que l'exact match

PI peut déclarer duplicate parce que plusieurs zones deviennent identiques après normalisation fuzzy. Sinew et OpenCode évitent ce faux positif dans le cas exact : si l'exact match est unique, il gagne avant les fallbacks.

Le risque restant existe seulement quand aucun match exact n'existe et que les fallbacks fuzzy/permissifs trouvent plusieurs candidats équivalents.

### 3. Les fallbacks permissifs augmentent les collisions

Les matchers whitespace/indent/context peuvent rendre plusieurs zones équivalentes. C'est utile quand le modèle oublie l'indentation, mais ça augmente le risque d'ambiguïté.

Le bon compromis selon moi :

- exact unique => accepter immédiatement ;
- exact duplicate => refuser sauf `replaceAll` ;
- fuzzy/permissive seulement si exact absent ;
- si fuzzy/permissive trouve plusieurs candidats, retourner les lignes candidates au modèle pour qu'il ajoute du contexte.

## Recommandations pour Sinew

Je ne copierais pas PI ou OpenCode entièrement. Sinew reprend déjà le point le plus important d'OpenCode côté matching. Les chantiers restants sont :

1. **Garder l'exact match prioritaire sur le fuzzy.**  
   Si `oldContent` apparaît exactement une fois, appliquer ce match. Ne pas rejeter parce qu'une normalisation fuzzy trouve plusieurs équivalents.

2. **Garder le fuzzy/permissive comme fallback seulement après échec de l'exact.**  
   Ça réduit fortement les faux “non unique”.

3. **Adopter l'algorithme PI pour les edits multiples d'un fichier.**  
   Matcher tous les edits non-`replaceAll` contre le contenu original, rejeter les overlaps, puis écrire une fois. Ça évite les surprises séquentielles.

4. **Rendre les edits per-file all-or-nothing.**  
   Si `edits[3]` échoue, ne pas écrire `edits[0..2]`. Les erreurs partielles sont dangereuses pour les agents : elles forcent le modèle à raisonner sur un état mi-appliqué.

5. **Garder le fingerprint SHA-256 de Sinew.**  
   C'est meilleur que le mtime/size d'OpenCode et meilleur que l'absence de garde de PI.

6. **Simplifier le schéma public si les modèles se trompent souvent.**  
   Le `files[]` top-level est puissant mais plus complexe. Pour les modèles, un tool mono-fichier `edit_file({ path, edits })` est plus simple. On peut garder un batch tool séparé pour multi-fichiers.

7. **En cas de duplicate, retourner des candidats exploitables.**  
   Exemple : “Found 3 matches at lines 18, 42, 119; include surrounding lines or use replaceAll.” Ça aiderait beaucoup le modèle à corriger son prochain appel.

8. **Ne pas auto-sélectionner un block-anchor ambigu.**  
   Les heuristiques “best candidate” sont pratiques mais risquées. Pour un coding agent, je préfère refuser et demander plus de contexte si plusieurs candidats existent.

## Classement final

Selon le critère :

1. **Moins de faux bugs `oldContent not unique` : Sinew et OpenCode.**  
   Les deux ont le comportement important : l'exact unique gagne avant le fuzzy/permissif.

2. **Meilleur multi-edit algorithmique : PI.**  
   Match sur original + overlap check + write unique est le design le plus propre.

3. **Meilleure sécurité globale : Sinew.**  
   Fingerprint SHA-256, read obligatoire, CRLF/BOM, limites de taille et `replaceAll` sont solides.

Donc mon avis : **pour ton problème précis, Sinew et OpenCode sont les meilleurs sur le faux `oldContent not unique` exact-vs-fuzzy**. Le meilleur design final reste hybride : garder la sécurité de Sinew, garder l'exact-first, prendre l'atomicité/multi-edit de PI et ajouter des diagnostics de duplicate avec lignes candidates.
