id-about = Affiche les informations d'utilisateur et de groupe pour chaque UTILISATEUR spécifié,
  ou (si UTILISATEUR est omis) pour l'utilisateur actuel.
id-usage = id [OPTION]... [UTILISATEUR]...
id-after-help = L'utilitaire id affiche les noms d'utilisateur et de groupe ainsi que leurs ID numériques
  du processus appelant, vers la sortie standard. Si les ID réels et effectifs sont
  différents, les deux sont affichés, sinon seul l'ID réel est affiché.

  Si un utilisateur (nom de connexion ou ID utilisateur) est spécifié, les ID utilisateur et groupe
  de cet utilisateur sont affichés. Dans ce cas, les ID réels et effectifs sont
  supposés être identiques.

  NOTE : ces nombres sont des projections des SID du jeton. Ils sont ce que voient
  les programmes Linux et ce que stockent les systèmes de fichiers, mais rien n'est
  décidé à partir d'eux — le modèle d'accès utilise les SID dont ils proviennent.
  Le champ de contexte de la ligne par défaut porte cette identité ; `token` affiche
  le jeton entier.

  Les ID réels et effectifs ne diffèrent que lors d'une usurpation d'identité, ce
  qui est le sens de cette distinction ici. Un bit setuid ne modifie pas un jeton,
  donc -r rapporte rarement autre chose que les ID effectifs.

# Texte d'aide pour le contexte
id-context-help-enabled = affiche uniquement le contexte de sécurité du processus
id-context-help-peios = affiche uniquement le contexte de sécurité du processus — son
  identité de jeton KACS, sous la forme SID-UTILISATEUR:INTÉGRITÉ

# Messages d'erreur
id-error-names-real-ids-require-flags = l'affichage des noms uniquement ou des ID réels nécessite -u, -g, ou -G
id-error-zero-not-permitted-default = l'option --zero n'est pas autorisée dans le format par défaut
id-error-cannot-print-context-with-user = impossible d'afficher le contexte de sécurité quand un utilisateur est spécifié
id-error-cannot-get-context = impossible d'obtenir le contexte du processus
id-error-no-such-user = { $user } : utilisateur inexistant
id-error-cannot-find-group-name = impossible de trouver le nom pour l'ID de groupe { $gid }
id-error-cannot-find-user-name = impossible de trouver le nom pour l'ID utilisateur { $uid }
id-error-audit-retrieve = impossible de récupérer les informations

# Texte d'aide pour les arguments de ligne de commande
id-help-ignore = ignore, pour compatibilité avec d'autres versions
id-help-audit = Affiche l'ID utilisateur d'audit du processus et autres propriétés d'audit
  (une fonctionnalité BSD ; Peios audite via eventd et n'a pas de session BSM à rapporter).
id-help-user = Affiche uniquement l'ID utilisateur effectif sous forme de nombre.
id-help-group = Affiche uniquement l'ID de groupe effectif sous forme de nombre
id-help-groups = Affiche uniquement les différents ID de groupe sous forme de nombres séparés par des espaces,
  dans un ordre quelconque.
id-help-human-readable = Rend la sortie lisible par l'humain. Chaque affichage est sur une ligne séparée.
id-help-name = Affiche le nom de l'ID utilisateur ou groupe pour les options -G, -g et -u
  au lieu du nombre.
  Si certains ID numériques ne peuvent pas être convertis en
  noms, le nombre sera affiché comme d'habitude.
id-help-password = Affiche l'id comme une entrée de fichier de mots de passe.
id-help-real = Affiche l'ID réel pour les options -G, -g et -u au lieu de
  l'ID effectif.
id-help-zero = délimite les entrées avec des caractères NUL, pas des espaces ;
  non autorisé dans le format par défaut

# Étiquettes de sortie
id-output-uid = uid
id-output-groups = groupes
id-output-login = connexion
id-output-euid = euid
id-output-rgid = rgid
id-output-context = contexte
