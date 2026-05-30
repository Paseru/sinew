# Règles anti-boucle locales

- **Dossier de travail des commandes** : **ATTENTION EXCEPTION** : Contrairement aux outils `read`/`write` qui exigent des chemins absolus, l'outil terminal (bash) ne supporte **PAS** les chemins absolus ni `cwd: "."`. Vous devez obligatoirement utiliser `cwd: ""` pour rester à la racine sous peine d'erreur de snapshot `path escapes workspace`.
- **CHANGELOG avant modification** : juste avant toute modification de `CHANGELOG.md`, toujours relire `C:\dev\sinew\CHANGELOG.md` avec l'outil `read`, puis modifier `CHANGELOG.md` dans le même lot que les autres fichiers touchés.
- **Chemins de fichiers** : pour `read`, `edit_file` et `write_file`, utiliser les chemins absolus Windows du workspace, par exemple `C:\dev\sinew\...`.
- **Grep Limit** : le paramètre `limit` est strictement obligatoire pour les outils `grep` et `glob`. Ne jamais l'omettre.
- **Fichiers fantômes** : avant d'appeler `grep` ou `read` sur un chemin spécifique, s'assurer que le fichier existe réellement sur le disque (ex: via `bash` avec `Test-Path`).
# Code map
- L'agent doit garder Ã  jour cette carte simple des fichiers Ã  chaque crÃ©ation, suppression, renommage, dÃ©placement ou modification.

.
â”œâ”€â”€ .gitignore
â”œâ”€â”€ AGENTS.md
â”œâ”€â”€ CHANGELOG.md
â”œâ”€â”€ Cargo.lock
â”œâ”€â”€ Cargo.toml
â”œâ”€â”€ index.html
â”œâ”€â”€ launch-sinew-dev.bat
â”œâ”€â”€ LICENSE
â”œâ”€â”€ package-lock.json
â”œâ”€â”€ package.json
â”œâ”€â”€ README.md
â”œâ”€â”€ .sinew
â”œâ”€â”€ scripts
â”‚   â”œâ”€â”€ check.ps1
â”‚   â”œâ”€â”€ compil.ps1
â”‚   â”œâ”€â”€ export-agent-descriptor.mjs
â”‚   â”œâ”€â”€ prepare-agent-bridge.mjs
â”‚   â”œâ”€â”€ prepare-sidecars.mjs
â”‚   â”œâ”€â”€ agent-bridge
â”‚   â”‚   â”œâ”€â”€ exec-handlers.mjs
â”‚   â”‚   â”œâ”€â”€ export-agent-fds-prost.mjs
â”‚   â”‚   â”œâ”€â”€ h2-bridge.mjs
â”‚   â”‚   â”œâ”€â”€ install-proto.mjs
â”‚   â”‚   â”œâ”€â”€ package-lock.json
â”‚   â”‚   â”œâ”€â”€ package.json
â”‚   â”‚   â”œâ”€â”€ run-stream.mjs
â”‚   â”‚   â”œâ”€â”€ test-live-rust.ps1
â”‚   â”‚   â”œâ”€â”€ test-live.ps1
â”‚   â”‚   â””â”€â”€ vendor
â”‚   â”‚       â””â”€â”€ agent_pb.ts
â”‚   â””â”€â”€ mitm
â”‚       â”œâ”€â”€ check-mitm.ps1
â”‚       â”œâ”€â”€ install-mitmproxy.ps1
â”‚       â”œâ”€â”€ README.md
â”‚       â””â”€â”€ start-mitmweb.ps1
â”œâ”€â”€ tsconfig.json
â”œâ”€â”€ tsconfig.node.json
â”œâ”€â”€ vite.config.ts
â”œâ”€â”€ .github
â”‚   â”œâ”€â”€ assets
â”‚   â”‚   â”œâ”€â”€ architecture.png
â”‚   â”‚   â”œâ”€â”€ harness.png
â”‚   â”‚   â”œâ”€â”€ hero.png
â”‚   â”‚   â”œâ”€â”€ modes.png
â”‚   â”‚   â”œâ”€â”€ screenshot.png
â”‚   â”‚   â””â”€â”€ swarm.png
â”‚   â””â”€â”€ workflows
â”‚       â”œâ”€â”€ release.yml
â”‚       â””â”€â”€ security.yml
â”œâ”€â”€ crates
â”‚   â”œâ”€â”€ sinew-anthropic
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ auth.rs
â”‚   â”‚       â”œâ”€â”€ client.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ model_info.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â””â”€â”€ wire.rs
â”‚   â”œâ”€â”€ sinew-app
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ agent.rs
â”‚   â”‚       â”œâ”€â”€ agent
â”‚   â”‚       â”‚   â”œâ”€â”€ assistant_message.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ cancel.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ clean_context.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ compaction.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ context.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ events.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ history.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ mode.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ tests.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ tool_dispatch.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ tool_summary.rs
â”‚   â”‚       â”‚   â””â”€â”€ turn.rs
â”‚   â”‚       â”œâ”€â”€ bash.rs
â”‚   â”‚       â”œâ”€â”€ check_sota.rs
â”‚   â”‚       â”œâ”€â”€ codebase_search.rs
â”‚   â”‚       â”œâ”€â”€ compact.rs
â”‚   â”‚       â”œâ”€â”€ computer_use.rs
â”‚   â”‚       â”œâ”€â”€ delete_file.rs
â”‚   â”‚       â”œâ”€â”€ edit.rs
â”‚   â”‚       â”œâ”€â”€ editor_diagnostics.rs
â”‚   â”‚       â”œâ”€â”€ glob.rs
â”‚   â”‚       â”œâ”€â”€ grep.rs
â”‚   â”‚       â”œâ”€â”€ image.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ list_dir.rs
â”‚   â”‚       â”œâ”€â”€ mcp.rs
â”‚   â”‚       â”œâ”€â”€ question.rs
â”‚   â”‚       â”œâ”€â”€ read.rs
â”‚   â”‚       â”œâ”€â”€ read_lints.rs
â”‚   â”‚       â”œâ”€â”€ ripgrep.rs
â”‚   â”‚       â”œâ”€â”€ skill.rs
â”‚   â”‚       â”œâ”€â”€ store.rs
â”‚   â”‚       â”œâ”€â”€ subagent.rs
â”‚   â”‚       â”œâ”€â”€ team.rs
â”‚   â”‚       â”œâ”€â”€ team
â”‚   â”‚       â”‚   â”œâ”€â”€ agent_turns.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ context.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ descriptors.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ launch.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ live.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ messaging.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ model.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ render.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ session.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ status_stop.rs
â”‚   â”‚       â”‚   â”œâ”€â”€ task_board.rs
â”‚   â”‚       â”‚   â””â”€â”€ tests.rs
â”‚   â”‚       â”œâ”€â”€ text.rs
â”‚   â”‚       â”œâ”€â”€ todo.rs
â”‚   â”‚       â”œâ”€â”€ tool_names.rs
â”‚   â”‚       â”œâ”€â”€ tool_run.rs
â”‚   â”‚       â”œâ”€â”€ web.rs
â”‚   â”‚       â”œâ”€â”€ workspace.rs
â”‚   â”‚       â””â”€â”€ write.rs
â”‚   â”œâ”€â”€ sinew-core
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ error.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ message.rs
â”‚   â”‚       â”œâ”€â”€ model.rs
â”‚   â”‚       â”œâ”€â”€ provider.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â””â”€â”€ tool.rs
â”‚   â”œâ”€â”€ sinew-deepseek
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ auth.rs
â”‚   â”‚       â”œâ”€â”€ client.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ model_info.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â””â”€â”€ wire.rs
â”‚   â”œâ”€â”€ sinew-google
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ auth.rs
â”‚   â”‚       â”œâ”€â”€ client.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ model_info.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â””â”€â”€ wire.rs
â”‚   â”œâ”€â”€ sinew-index
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ background.rs
â”‚   â”‚       â”œâ”€â”€ chunk.rs
â”‚   â”‚       â”œâ”€â”€ embeddings.rs
â”‚   â”‚       â”œâ”€â”€ indexer.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ process.rs
â”‚   â”‚       â”œâ”€â”€ search.rs
â”‚   â”‚       â””â”€â”€ store.rs
â”‚   â”œâ”€â”€ sinew-kimi
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ auth.rs
â”‚   â”‚       â”œâ”€â”€ client.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ model_info.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â””â”€â”€ wire.rs
â”‚   â”œâ”€â”€ sinew-openai
â”‚   â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”‚   â””â”€â”€ src
â”‚   â”‚       â”œâ”€â”€ auth.rs
â”‚   â”‚       â”œâ”€â”€ client.rs
â”‚   â”‚       â”œâ”€â”€ lib.rs
â”‚   â”‚       â”œâ”€â”€ model_info.rs
â”‚   â”‚       â”œâ”€â”€ responses_stream.rs
â”‚   â”‚       â”œâ”€â”€ stream.rs
â”‚   â”‚       â”œâ”€â”€ websocket.rs
â”‚   â”‚       â””â”€â”€ wire.rs
â”‚   â””â”€â”€ sinew-openrouter
â”‚       â”œâ”€â”€ Cargo.toml
â”‚       â””â”€â”€ src
â”‚           â”œâ”€â”€ auth.rs
â”‚           â”œâ”€â”€ client.rs
â”‚           â”œâ”€â”€ lib.rs
â”‚           â”œâ”€â”€ model_info.rs
â”‚           â”œâ”€â”€ stream.rs
â”‚           â””â”€â”€ wire.rs
â”‚   â””â”€â”€ sinew-cursor
â”‚       â”œâ”€â”€ Cargo.toml
â”‚       â””â”€â”€ src
â”‚           â”œâ”€â”€ agent
â”‚           â”‚   â”œâ”€â”€ bridge.rs
â”‚           â”‚   â”œâ”€â”€ client_proto.rs
â”‚           â”‚   â”œâ”€â”€ connect_proto.rs
â”‚           â”‚   â”œâ”€â”€ conversation_id.rs
â”‚           â”‚   â”œâ”€â”€ exec_handler.rs
â”‚           â”‚   â”œâ”€â”€ h2_client.rs
â”‚           â”‚   â”œâ”€â”€ mod.rs
â”‚           â”‚   â”œâ”€â”€ models.rs
â”‚           â”‚   â”œâ”€â”€ proto_dynamic.rs
â”‚           â”‚   â”œâ”€â”€ proto_pool.rs
â”‚           â”‚   â”œâ”€â”€ retry.rs
â”‚           â”‚   â”œâ”€â”€ run_h2.rs
â”‚           â”‚   â”œâ”€â”€ run_request.rs
â”‚           â”‚   â”œâ”€â”€ rust_bridge.rs
â”‚           â”‚   â”œâ”€â”€ server_decode.rs
â”‚           â”‚   â”œâ”€â”€ setup.rs
â”‚           â”‚   â”œâ”€â”€ state.rs
â”‚           â”‚   â”œâ”€â”€ tools.rs
â”‚           â”‚   â”œâ”€â”€ transcript.rs
â”‚           â”‚   â””â”€â”€ transport.rs
â”‚           â”œâ”€â”€ auth
â”‚           â”‚   â”œâ”€â”€ composer.rs
â”‚           â”‚   â”œâ”€â”€ mod.rs
â”‚           â”‚   â””â”€â”€ oauth.rs
â”‚           â”œâ”€â”€ proto
â”‚           â”‚   â”œâ”€â”€ agent.fds
â”‚           â”‚   â”œâ”€â”€ agent.pb
â”‚           â”‚   â””â”€â”€ README.md
â”‚           â”œâ”€â”€ client.rs
â”‚           â”œâ”€â”€ connect.rs
â”‚           â”œâ”€â”€ context_injection.rs
â”‚           â”œâ”€â”€ conversation.rs
â”‚           â”œâ”€â”€ encryption.rs
â”‚           â”œâ”€â”€ identity.rs
â”‚           â”œâ”€â”€ images.rs
â”‚           â”œâ”€â”€ lib.rs
â”‚           â”œâ”€â”€ model_info.rs
â”‚           â”œâ”€â”€ sanitize.rs
â”‚           â”œâ”€â”€ stream_state.rs
â”‚           â”œâ”€â”€ tests.rs
â”‚           â”œâ”€â”€ tools.rs
â”‚           â”œâ”€â”€ usage.rs
â”‚           â””â”€â”€ workspace.rs
â”œâ”€â”€ src-tauri
â”‚   â”œâ”€â”€ Cargo.toml
â”‚   â”œâ”€â”€ PROVIDERS.md
â”‚   â”œâ”€â”€ binaries
â”‚   â”‚   â””â”€â”€ .gitkeep
â”‚   â”œâ”€â”€ build.rs
â”‚   â”œâ”€â”€ tauri.sidecars.conf.json
â”‚   â”œâ”€â”€ tauri.conf.json
â”‚   â”œâ”€â”€ tauri.windows.conf.json
â”‚   â”œâ”€â”€ capabilities
â”‚   â”‚   â””â”€â”€ default.json
â”‚   â”œâ”€â”€ gen
â”‚   â”‚   â””â”€â”€ schemas
â”‚   â”‚       â”œâ”€â”€ acl-manifests.json
â”‚   â”‚       â”œâ”€â”€ capabilities.json
â”‚   â”‚       â”œâ”€â”€ desktop-schema.json
â”‚   â”‚       â””â”€â”€ macOS-schema.json
â”‚   â”œâ”€â”€ icons
â”‚   â”‚   â”œâ”€â”€ 128x128.png
â”‚   â”‚   â”œâ”€â”€ 128x128@2x.png
â”‚   â”‚   â”œâ”€â”€ 32x32.png
â”‚   â”‚   â”œâ”€â”€ 64x64.png
â”‚   â”‚   â”œâ”€â”€ Square107x107Logo.png
â”‚   â”‚   â”œâ”€â”€ Square142x142Logo.png
â”‚   â”‚   â”œâ”€â”€ Square150x150Logo.png
â”‚   â”‚   â”œâ”€â”€ Square284x284Logo.png
â”‚   â”‚   â”œâ”€â”€ Square30x30Logo.png
â”‚   â”‚   â”œâ”€â”€ Square310x310Logo.png
â”‚   â”‚   â”œâ”€â”€ Square44x44Logo.png
â”‚   â”‚   â”œâ”€â”€ Square71x71Logo.png
â”‚   â”‚   â”œâ”€â”€ Square89x89Logo.png
â”‚   â”‚   â”œâ”€â”€ StoreLogo.png
â”‚   â”‚   â”œâ”€â”€ icon.icns
â”‚   â”‚   â”œâ”€â”€ icon.ico
â”‚   â”‚   â”œâ”€â”€ icon.png
â”‚   â”‚   â”œâ”€â”€ nsis-sidebar.bmp
â”‚   â”‚   â”œâ”€â”€ source.svg
â”‚   â”‚   â”œâ”€â”€ android
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-anydpi-v26
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher.xml
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-hdpi
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher.png
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher_foreground.png
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher_round.png
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-mdpi
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher.png
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher_foreground.png
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher_round.png
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-xhdpi
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher.png
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher_foreground.png
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher_round.png
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-xxhdpi
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher.png
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher_foreground.png
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher_round.png
â”‚   â”‚   â”‚   â”œâ”€â”€ mipmap-xxxhdpi
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher.png
â”‚   â”‚   â”‚   â”‚   â”œâ”€â”€ ic_launcher_foreground.png
â”‚   â”‚   â”‚   â”‚   â””â”€â”€ ic_launcher_round.png
â”‚   â”‚   â”‚   â””â”€â”€ values
â”‚   â”‚   â”‚       â””â”€â”€ ic_launcher_background.xml
â”‚   â”‚   â””â”€â”€ ios
â”‚   â”‚       â”œâ”€â”€ AppIcon-20x20@1x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-20x20@2x-1.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-20x20@2x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-20x20@3x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-29x29@1x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-29x29@2x-1.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-29x29@2x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-29x29@3x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-40x40@1x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-40x40@2x-1.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-40x40@2x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-40x40@3x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-512@2x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-60x60@2x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-60x60@3x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-76x76@1x.png
â”‚   â”‚       â”œâ”€â”€ AppIcon-76x76@2x.png
â”‚   â”‚       â””â”€â”€ AppIcon-83.5x83.5@2x.png
â”‚   â””â”€â”€ src
â”‚       â”œâ”€â”€ context.rs
â”‚       â”œâ”€â”€ conversations.rs
â”‚       â”œâ”€â”€ git.rs
â”‚       â”œâ”€â”€ lib.rs
â”‚       â”œâ”€â”€ main.rs
â”‚       â”œâ”€â”€ models.rs
â”‚       â”œâ”€â”€ platform.rs
â”‚       â”œâ”€â”€ providers.rs
â”‚       â”œâ”€â”€ state.rs
â”‚       â”œâ”€â”€ swarm.rs
â”‚       â”œâ”€â”€ terminal.rs
â”‚       â”œâ”€â”€ tests.rs
â”‚       â”œâ”€â”€ turns.rs
â”‚       â”œâ”€â”€ updater.rs
â”‚       â”œâ”€â”€ workflow.rs
â”‚       â””â”€â”€ workspace.rs
â””â”€â”€ src
    â”œâ”€â”€ App.tsx
    â”œâ”€â”€ main.tsx
    â”œâ”€â”€ styles.css
    â”œâ”€â”€ types.ts
    â”œâ”€â”€ vite-env.d.ts
    â”œâ”€â”€ components
    â”‚   â”œâ”€â”€ ConversationList.tsx
    â”‚   â”œâ”€â”€ EditorPane.tsx
    â”‚   â”œâ”€â”€ FileTree.tsx
    â”‚   â”œâ”€â”€ GitPanel.tsx
    â”‚   â”œâ”€â”€ ImageContextMenu.tsx
    â”‚   â”œâ”€â”€ SearchPane.tsx
    â”‚   â”œâ”€â”€ SettingsPane.tsx
    â”‚   â”œâ”€â”€ SinewMark.tsx
    â”‚   â”œâ”€â”€ Splitter.tsx
    â”‚   â”œâ”€â”€ TerminalPanel.tsx
    â”‚   â”œâ”€â”€ UpdateBadge.tsx
    â”‚   â”œâ”€â”€ UpdaterLockScreen.tsx
    â”‚   â”œâ”€â”€ Welcome.tsx
    â”‚   â”œâ”€â”€ WindowControls.tsx
    â”‚   â”œâ”€â”€ Workspace.tsx
    â”‚   â””â”€â”€ chat
    â”‚       â”œâ”€â”€ AIThinkingBlock.tsx
    â”‚       â”œâ”€â”€ ChatPane.tsx
    â”‚       â”œâ”€â”€ DotmSquare2.tsx
    â”‚       â”œâ”€â”€ DotmSquare5.tsx
    â”‚       â”œâ”€â”€ FileChangeBlock.tsx
    â”‚       â”œâ”€â”€ Markdown.tsx
    â”‚       â”œâ”€â”€ MermaidDiagram.tsx
    â”‚       â”œâ”€â”€ PlanningNextMoveBlock.tsx
    â”‚       â”œâ”€â”€ Questionnaire.tsx
    â”‚       â”œâ”€â”€ TodoStrip.tsx
    â”‚       â”œâ”€â”€ ToolCard.tsx
    â”‚       â”œâ”€â”€ dotmatrix-core.tsx
    â”‚       â”œâ”€â”€ dotmatrix-hooks.ts
    â”‚       â””â”€â”€ stream.ts
    â”œâ”€â”€ lib
    â”‚   â”œâ”€â”€ customIcons.ts
    â”‚   â”œâ”€â”€ fileIcon.ts
    â”‚   â”œâ”€â”€ frRuntime.ts
    â”‚   â”œâ”€â”€ ipc.ts
    â”‚   â”œâ”€â”€ language.ts
    â”‚   â”œâ”€â”€ locale.ts
    â”‚   â”œâ”€â”€ models.ts
    â”‚   â”œâ”€â”€ quotas.ts
    â”‚   â”œâ”€â”€ recents.ts
    â”‚   â””â”€â”€ tools.ts
â””â”€â”€ sinew-chrome-bridge
    â”œâ”€â”€ add_to_sinew.py
    â”œâ”€â”€ background.js
    â”œâ”€â”€ com.sinew.chrome_bridge.json
    â”œâ”€â”€ e2e-local.mjs
    â”œâ”€â”€ e2e-structured.mjs
    â”œâ”€â”€ icon-128.png
    â”œâ”€â”€ icon-32.png
    â”œâ”€â”€ icon-64.png
    â”œâ”€â”€ icon.jpg
    â”œâ”€â”€ interact_chrome.js
    â”œâ”€â”€ launch_chrome_silent.bat
    â”œâ”€â”€ manifest.json
    â”œâ”€â”€ mcp_server.js
    â”œâ”€â”€ native-host-wrapper.exe
    â”œâ”€â”€ native_host.bat
    â”œâ”€â”€ package-lock.json
    â”œâ”€â”€ package.json
    â”œâ”€â”€ popup.html
    â”œâ”€â”€ popup.js
    â”œâ”€â”€ register.ps1
    â”œâ”€â”€ run_bridge.bat
    â”œâ”€â”€ run_sinew_bridge.bat
    â”œâ”€â”€ server.js
    â”œâ”€â”€ sinew_cursor.js
    â””â”€â”€ native-host-wrapper
        â”œâ”€â”€ Cargo.toml
        â””â”€â”€ src
            â””â”€â”€ main.rs

