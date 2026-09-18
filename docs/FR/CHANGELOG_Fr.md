# Changelog — NeuraDex

Toutes les modifications notables apportées au projet **NeuraDex** sont documentées dans ce fichier.

Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et ce projet respecte les principes du [Semantic Versioning](https://semver.org/lang/fr/).

---

## [0.1.8] — Résolution Double Source (Ollama & Hugging Face) & Modernisation des Catégories du Hub

Cette version enrichit l'Omnibar du Hub de modèles avec une détection concurrente double source (Ollama et Hugging Face), introduit une fenêtre unifiée de sélection multi-sources et modernise les catégories de capacités.

### Ajouté
- **Résolution Concurrente & Sélecteur Multi-Sources (`window.rs`, `HfModelPickerDialog`)** :
  - Vérification simultanée et asynchrone via `tokio::join!` sur le registre Ollama et l'API Hugging Face lors de la saisie d'un identifiant `auteur/modèle`.
  - Si le modèle est disponible sur les deux plateformes, la fenêtre contextuelle affiche la section **🦙 Bibliothèque Ollama** tout en haut, au-dessus de la liste des quantifications Hugging Face, permettant à l'utilisateur de choisir librement sa source en un clic.
  - Bouton d'action direct "Ouvrir sur Ollama" renvoyant vers la page officielle sur `ollama.com`.
  - Notification par toast informative avec repli propre si le modèle n'est trouvé sur aucune des deux plateformes.
- **Bouton d'action "Rechercher" dans l'Omnibar** :
  - Remplacement du libellé "Télécharger" par "Rechercher" (`🔍 Rechercher` / `🔍 Search`) avec infobulle explicative reflétant la recherche multi-sources.

### Modifié
- **Modernisation des Catégories du Hub** :
  - Renommage de la catégorie `Spécialisé` en `Outils` ("Tools" / "Outils") dans les filtres, cartes et fichiers de localisation (`fr.json`, `en.json`).
  - Retrait du filtre obsolète `Général & Polyvalent` de la barre de filtres du Hub pour une interface plus claire et épurée.
  - Mise à jour du scraper et de la base de modèles (`ai-models-list`) pour catégoriser fidèlement les modèles avec appel d'outils (*tool calling*).

---

## [0.1.7] — Omnibar Intelligente Unifiée & Empaquetage Arch Linux

Cette version introduit une barre de recherche unifiée et ergonomique dans le Hub de modèles, ainsi que le support natif d'Arch Linux.

### Ajouté
- **Omnibar Intelligente Unifiée (`HubView`)** :
  - Remplacement des panneaux de saisie superposés par une unique barre de recherche compacte fusionnant recherche catalogue et téléchargement.
  - Détection automatique et dynamique des liens/dépôts Hugging Face (`hf.co/...`, `auteur/modèle`) avec affichage contextuel du bouton **"🤗 Explorer GGUFs"**.
  - Téléchargement direct de modèles Ollama non référencés via le bouton **"⬇️ Télécharger"** ou la touche Entrée.
  - Réinitialisation fluide de la vue après lancement d'un téléchargement.
- **Installeur Web Universel Multi-Distributions (`scripts/get.sh`)** :
  - Script d'installation one-liner (`curl -fsSL .../get.sh | bash`) compatible Debian, Ubuntu, Fedora et Arch Linux.
  - Détection d'architecture (`x86_64`) et résolution automatique de la dernière version via l'API GitHub.
- **Support & Empaquetage Arch Linux** :
  - Génération automatique du paquet Pacman `.pkg.tar.zst` dans le workflow CI (`release.yml`).
  - Ajout des recettes officielles `PKGBUILD` (avec fonction dynamique `pkgver()`) et `PKGBUILD.bin` (prête pour l'AUR).

---
## [0.1.6] — Correctif de Persistance Modelfile & Permissions Ollama

Ce correctif résout l'erreur HTTP 500 survenant lors de la sauvegarde des paramètres de modèle et de la compilation des Modelfiles dans Ollama.

### Corrigé
- **Gestion des permissions de stockage Ollama (`lifecycle.rs`)** :
  - Attribution prioritaire de la propriété des répertoires de modèles au compte système `ollama:ollama` (dans `migrate_models`, `apply_systemd_override` et `fix_directory_permissions`).
  - Conservation d'un accès intégral en lecture/écriture/exécution pour l'utilisateur de bureau grâce aux permissions de groupe `775` et aux listes de contrôle d'accès POSIX (`setfacl`).
  - Résolution de l'erreur système `chtimes: operation not permitted` (`EPERM` du noyau Linux sur `utimensat`) rencontrée par le démon Ollama lors de la réutilisation des couches de poids pour la compilation des variantes et la persistance des paramètres (`num_ctx`, `num_gpu`, etc.).
- **Diagnostic et retours d'erreurs d'API (`client.rs`)** :
  - Détection contextuelle de l'erreur `chtimes / operation not permitted` d'Ollama dans `create_model_stream`.
  - Affichage d'un message d'assistance clair indiquant la commande de correction des permissions de stockage au lieu d'un simple code générique HTTP 500.


## [0.1.5] — Système de Diagnostic & Préparation Première Publication

Cette version introduit un outil de diagnostic système et d'environnement.

### Ajouté
- **Outil de diagnostic et rapport d'environnement (`diagnostic.rs`)** :
  - Génération asynchrone d'un rapport au format Markdown.
  - Informations collectées :
    - **Application** : version de NeuraDex, langue active, mode de gestion du démon.
    - **Système d'exploitation & Bureau** : distribution Linux, version, noyau (kernel), architecture, environnement de bureau (GNOME, KDE...) et type de session (Wayland ou X11).
    - **Matériel & Mémoire** : marque et modèle du processeur (CPU), nombre de threads, mémoire vive totale et consommée.
    - **Accélération Graphique & VRAM** : détection des GPU (NVIDIA, AMD, Intel), VRAM allouée, utilisée et libre, températures et consommation électrique (avec prise en charge du mode dégradé CPU/RAM en l'absence de GPU).
    - **Service & Stockage Ollama** : hôte configuré, politique d'exposition réseau (CORS), répertoire des modèles, statut de la Flash Attention et politique de rétention en VRAM (*Keep Alive*).
    - **Connectivité & Modèles** : statut de connectivité et version du démon local, inventaire des modèles installés avec leurs tailles en GiB, et liste des modèles actuellement résidents en mémoire.
- **Intégration Graphique dans la Vue Paramètres (`SettingsView`)** :
  - Nouveau groupe dédié *Diagnostic & Rapport Système* avec icône de supervision système.
  - Bouton **« 📋 Copier le rapport »** avec retour visuel immédiat (pastille temporaire *Copié !*).
  - Bouton **« Inspecter »** ouvrant un dialogue modal Libadwaita (`adw::Dialog`) affichant le rapport complet dans un visualiseur textuel monospace avec bouton de copie rapide.
- **Raccourci Diagnostic dans la Boîte de Dialogue « À propos » (`Header`)** :
  - Ajout d'une action dans la boîte *À propos de NeuraDex* pour copier les informations système dans le presse-papiers.
- **Suite de Tests d'Intégration (`config_and_vault_test.rs`)** :
  - Test unitaire `test_generate_diagnostic_report` vérifiant la génération des différentes sections du rapport.

---

## [0.1.4] — Catalogue Hub et intégration Hugging Face

Cette version ajoute la page **Hub** pour explorer et télécharger des modèles, ainsi que la prise en charge des fichiers GGUF depuis **Hugging Face**.

### Ajouté
- **Page Hub (`HubView`)** :
  - Interface de navigation GTK4 / Libadwaita (`adw::Clamp`, `FlowBox`, `ListBox`, `ScrolledWindow`).
  - Catalogue de modèles officiels et communautaires avec métadonnées, tailles et quantifications disponibles.
- **Téléchargement Hugging Face (`hf_model_picker_dialog.rs`)** :
  - Prise en charge des URL Hugging Face (`https://huggingface.co/...`, `hf.co/...`) et des identifiants de dépôts (`auteur/nom_du_modele`).
  - Exploration de l'arborescence des dépôts distants pour lister les fichiers GGUF disponibles.
  - Sélecteur modal listant chaque variante de quantification (`IQ3`, `Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, `FP16`) avec sa taille en gigaoctets.
  - Vérification de compatibilité mémoire : avertissement si une quantification dépasse la VRAM ou la RAM disponible avant de lancer le téléchargement.
  - Déclenchement de l'opération de pull Ollama via le protocole `hf.co/...`.
- **Filtres et catégorisation thématique (`category_box`)** :
  - Boutons de filtrage thématique : *Raisonnement* (`reasoning`), *Vision*, *Code*, *Légers* (`lightweight`), *Généraliste*, *RAG*, *Cybersécurité* et *Spécialisés*.
  - Détection et affichage des badges de capacités par modèle : `think`, `vision`, `audio`, `tools`, `code`, `embeddings`.
- **Filtre de compatibilité matérielle (`btn_compat_filter`)** :
  - Option permettant de masquer les modèles qui dépassent la mémoire disponible (VRAM GPU + RAM système).
- **Recherche et tri multi-critères** :
  - Barre de recherche en temps réel filtrant sur le nom, l'auteur, les étiquettes et les descriptions de modèles.
  - Menu déroulant de tri : par popularité, ordre alphabétique, taille mémoire ou date d'ajout.
- **Cartes de modèles Hub (`HubModelCard`)** :
  - Affichage des logos des créateurs (Alibaba/Qwen, DeepSeek, Google Gemma, Meta Llama, Mistral, Microsoft Phi, NVIDIA, etc.) au format SVG.
  - Sélecteur déroulant de la variante de quantification directement dans la carte.
  - Suivi du téléchargement en direct : barre de progression, pourcentage, vitesse de transfert (Mo/s), volume téléchargé / total et libellés d'étape (blobs, vérification SHA256, finalisation).
  - Bouton d'annulation du téléchargement en cours.
- **Synchronisation distante (`btn_sync_online`)** :
  - Bouton de rafraîchissement manuel pour mettre à jour les métadonnées du catalogue depuis les dépôts distants sans redémarrer l'application.

---

## [0.1.3] — Page de discussion (Chat) pour le test de modèles

Cette version introduit une interface de discussion permettant de tester les modèles locaux installés et d'observer leurs performances d'inférence.

### Ajouté
- **Interface de discussion (`ChatView`)** :
  - Interface de conversation asynchrone conçue pour tester les modèles locaux ou distants.
  - Menu déroulant de sélection du modèle actif avec pastille d'état matériel (vert = VRAM complète, jaune = hybride RAM/VRAM).
- **Streaming de tokens en temps réel** :
  - Intégration du flux Server-Sent Events (SSE) d'Ollama (`/api/chat`).
  - Affichage progressif au fur et à mesure de l'émission des tokens sans blocage de l'interface graphique.
  - Défilement automatique avec bulles de messages dédiées pour l'utilisateur et l'assistant.
- **Métriques d'inférence (Badges d'information)** :
  - **Vitesse de génération** : calcul et affichage en temps réel du débit en **tokens/seconde (tok/s)**.
  - **Latence initiale (TTFT)** : mesure du *Time To First Token* en millisecondes.
  - **Compteur de tokens** : comptabilisation des tokens de prompt et des tokens générés.
  - **Empreinte VRAM** : suivi de la mémoire sollicitée lors de l'inférence.
- **Volet latéral de gestion multi-sessions (`ChatSidebar`, `ChatSession`)** :
  - Panneau latéral repliable pour ajuster l'espace d'affichage.
  - Gestion des sessions de discussion : création (`+`), bascule, renommage et suppression.
  - **Génération automatique du titre des sessions** : extraction du premier message utilisateur pour générer un titre court (nettoyage du markdown, des listes et du code, support français/anglais).
- **Panneau de réglages temporaires (`ChatSettingsPopover`)** :
  - Menu accessible depuis la barre d'outils du chat.
  - Ajustement de la température (`temperature`) pour moduler la créativité du modèle lors des tests.
  - Modification de la taille de fenêtre de contexte (`num_ctx`).
  - Surcharge temporaire des instructions système (`system prompt`) pour évaluer différents comportements ou rôles.
- **Contrôles d'inférence** :
  - Bouton d'arrêt (`Arrêter`) connecté à un drapeau atomique `AtomicBool` pour interrompre une génération en cours et libérer le GPU.
  - Raccourcis clavier : envoi des messages via `Ctrl+Entrée` ou `Entrée`.
  - Bouton de réinitialisation (`Effacer la discussion`) pour vider les messages de la session active.

---

## [0.1.2] — Paramètres d'Ollama et Journaux système

Cette version ajoute la configuration du service Ollama (cycle de vie, réseau, stockage, clés d'API) et la consultation des journaux système.

### Ajouté
- **Page des paramètres système (`SettingsView`)** :
  - Interface Libadwaita basée sur `adw::PreferencesPage` et `adw::PreferencesGroup`.
  - Barre de sauvegarde avec détection des modifications non appliquées (`is_dirty`).
- **Supervision du cycle de vie du démon Ollama (`lifecycle.rs`)** :
  - Détection automatique de l'état du service systemd (`ollama.service`).
  - Boutons de contrôle : Démarrer, Arrêter et Redémarrer le service local.
  - Prise en charge du mode service systemd ou démon utilisateur externe.
  - Vérification des mises à jour de NeuraDex via l'API GitHub Releases.
  - Sélecteur de langue (Français / Anglais) avec détection de la locale système.
- **Gestion de l'exposition réseau & Sécurité CORS** :
  - Configuration du mode d'exposition réseau :
    - 🔒 **Localhost Uniquement** (`127.0.0.1`) pour un usage strictement local.
    - 🏠 **Réseau Local (LAN)** (`0.0.0.0`) pour partager les modèles sur le réseau local.
    - 🌐 **Exposé / Internet** (`0.0.0.0` avec politique CORS `OLLAMA_ORIGINS=*`).
  - Configuration de l'hôte Ollama personnalisé (`OLLAMA_HOST`) avec prise en charge des ports non standards et gestion du mode hors-ligne si l'hôte est injoignable.
  - Option d'activation de la Flash Attention (`OLLAMA_FLASH_ATTENTION`).
  - Sélecteur de politique de rétention mémoire (`OLLAMA_KEEP_ALIVE`).
- **Gestion du stockage & Outil de migration** :
  - Sélecteur du répertoire de stockage des modèles (`OLLAMA_MODELS`) via explorateur de fichiers GTK4 (`FileDialog`).
  - Affichage de l'espace disque (espace libre, capacité totale et jauge d'occupation).
  - Outil de migration permettant de transférer les modèles vers un autre disque ou dossier avec barre de progression.
  - Validation des chemins (`validate_path_safe`) pour sécuriser les commandes exécutées avec `pkexec`.
- **Gestion des clés d'API et clés SSH (`vault.rs`, `secret_store.rs`)** :
  - Gestionnaire de profils de clés d'API (Ollama Cloud, endpoints compatibles OpenAI).
  - Intégration avec le **FreeDesktop Secret Service** (GNOME Keyring / KDE KWallet) via `zbus` (sans dépendances C).
  - Repli automatique sur un fichier local protégé (`chmod 0600`) si D-Bus est inaccessible, avec indicateurs visuels (`🔐 Keyring` vs `🔓 Local`).
  - Migration automatique au démarrage des tokens stockés en clair vers le trousseau du bureau Linux.
  - Application des clés dans la configuration drop-in systemd d'Ollama.
  - Gestion des clés de membre SSH Ollama : génération de paires de clés Ed25519 (`~/.ollama/id_ed25519_{member}`), copie dans le presse-papiers et lien vers la page de configuration web.
- **Page des Journaux Système (`LogsView`, `logs.rs`)** :
  - Affichage en continu des journaux d'exécution du service via `journalctl -u ollama.service`.
  - Coloration syntaxique par tags GTK (`TextTagTable`) : rouge pour les erreurs, jaune pour les avertissements, cyan pour les requêtes HTTP/GIN, vert pour les succès et estompé pour le débogage.
  - Filtre textuel en temps réel avec champ de recherche.
  - Filtre par niveau de log : *Tous*, *Requêtes GIN*, *Avertissements*, *Erreurs*.
  - Boutons d'action : mise en pause / reprise du défilement, vidage du tampon d'affichage et copie dans le presse-papiers.

---

## [0.1.1] — Réglages des modèles et persistance

Cette version permet de configurer les hyperparamètres d'inférence par modèle, d'ajuster le délestage GPU/CPU et de sauvegarder ces réglages.

### Ajouté
- **Dialogue de réglage des modèles (`ModelSettingsDialog`)** :
  - Interface modale Libadwaita (`adw::Dialog`, `ViewSwitcher`, `Clamp`) avec navigation par onglets.
- **Contrôles synchronisés des paramètres d'inférence (`bind_slider_and_spin`)** :
  - Contrôles synchronisés par curseur (`Scale`) et champ numérique (`SpinButton`) :
  - **Échantillonnage** : réglage de `temperature`, `top_p`, `top_k` et `min_p`.
  - **Contexte et prédiction** : taille de la fenêtre de contexte (`num_ctx` de 2K à 128K+) et limite de prédiction (`num_predict`).
  - **Pénalités** : pénalité de répétition (`repeat_penalty`), plage d'historique (`repeat_last_n`), pénalité de présence (`presence_penalty`) et de fréquence (`frequency_penalty`).
  - **Exécution et reproductibilité** : graine pseudo-aléatoire (`seed`) et nombre de threads processeur alloués (`num_thread`).
- **Délestage des couches GPU (`num_gpu`)** :
  - Détection automatique du nombre de couches du modèle (`total_layers` / `block_count`).
  - Sélection du nombre de couches à charger en VRAM GPU (`num_gpu`).
  - Possibilité d'assigner `num_gpu = 0` pour forcer une exécution sur le processeur (CPU).
  - Option de verrouillage mémoire physique `use_mlock` pour limiter le swap sur disque.
- **Estimation de l'empreinte mémoire et compatibilité matérielle** :
  - Panneau latéral recalculant le besoin mémoire estimé selon le `num_ctx` et le `num_gpu` choisis.
  - Indicateur visuel d'allocation :
    - 🟢 **100% VRAM GPU** : chargement intégral en mémoire vidéo.
    - 🟡 **Hybride CPU/GPU** : répartition entre mémoire vidéo et RAM système.
    - 🔴 **Mémoire insuffisante** : avertissement en cas de risque de dépassement de la mémoire disponible.
- **Éditeur de Modelfile & Création de variantes** :
  - Éditeur textuel pour définir des instructions système persistantes (`SYSTEM prompt`).
  - Éditeur de template de prompt (`TEMPLATE`).
  - Création de variantes de modèles personnalisées dans Ollama (`/api/create`) avec les réglages et le prompt définis.
- **Persistance de la configuration (`config.rs`)** :
  - Stockage des configurations personnalisées par modèle dans `AppConfig` (`CustomModelSettings`).
  - Normalisation des identifiants de modèles (`normalize_model_keys`) pour traiter de manière homogène les variantes avec ou sans tag `:latest`.
  - Sauvegarde atomique avec fichier temporaire puis renommage (`write-to-tmp` + `rename`) évitant la corruption en cas d'interruption abrupte.
  - Application systématique des permissions `chmod 0600` sous Unix/Linux.

---

## [0.1.0] — Version initiale : Page d'accueil et surveillance matérielle

Première version de NeuraDex, comprenant la surveillance matérielle en temps réel et la gestion des modèles Ollama installés.

### Ajouté
- **Vue d'accueil & Tableau de bord des instances (`InstancesView`)** :
  - Conception double colonne en GTK4 et Libadwaita.
- **Gestion des modèles installés (Colonne de gauche)** :
  - Détection et inventaire des modèles disponibles sur l'instance Ollama locale (`/api/tags`).
  - Cartes de modèles (`ModelCard`) affichant les détails techniques : taille sur disque, niveau de quantification (`Q4_K_M`, `FP16`, `Q8_0`), nombre de paramètres (ex: `7B`, `14B`), et famille architecturale.
  - Détection des capacités du modèle (`capabilities.rs`) avec badges dédiés (`think`, `vision`, `tools`, `audio`, `code`, `embeddings`).
  - Intégration de logos vectoriels représentant les familles et créateurs de modèles (Mistral, Meta Llama, Google, DeepSeek, Qwen, Microsoft, etc.).
  - Actions rapides par modèle :
    - Préchargement en mémoire vive / VRAM avec barre d'état (`ProgressBar`).
    - Raccourci vers la vue de discussion / test.
    - Accès direct au dialogue de configuration.
    - Suppression de modèle avec confirmation.
  - Compteur de modèles installés et gestion des états vides.
- **Surveillance matérielle et VRAM en temps réel (Colonne de droite)** :
  - **Jauge VRAM multi-GPU (`VramGauge`)** :
    - Surveillance des GPU **NVIDIA** via le wrapper NVML (`nvml-wrapper`).
    - Prise en charge des GPU **AMD Radeon** (lecture sysfs / ROCm `amdgpu`).
    - Prise en charge des GPU **Intel** (via `sysfs`/`i915`/`xe`).
    - Affichage par carte : VRAM occupée vs VRAM totale, pourcentage, température en degrés Celsius, charge de calcul (GPU Load %) et puissance consommée en Watts.
    - Affichage cumulé de la VRAM en environnement multi-GPU.
    - Prise en charge des configurations sans GPU : affichage « Mode CPU uniquement » et bascule sur la RAM système.
  - **Surveillance Système (RAM & CPU)** :
    - Barre de niveau GTK4 (`LevelBar`) indiquant l'utilisation de la RAM système (utilisée / totale en Go, pourcentage).
    - Mesure continue du taux d'occupation du processeur (CPU %).
- **Gestionnaire de modèles résidents en mémoire (`running_box`)** :
  - Suivi des modèles chargés en mémoire via `/api/ps`.
  - Affichage de la répartition mémoire entre VRAM et RAM système.
  - Compte à rebours avant déchargement automatique (expiration du keep-alive).
  - Bouton **`🧹 Décharger tout`** : déchargement de tous les modèles chargés en mémoire.
- **En-tête (`HeaderBar`) & Navigation** :
  - Indicateur d'état de connexion à Ollama avec pastilles d'état (`🟢 Connecté` / `🔴 Déconnecté`).
  - Sélecteur de vues principal Libadwaita (`ViewSwitcher`).

