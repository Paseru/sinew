# R�gles anti-boucle locales

- **Dossier de travail des commandes** : **ATTENTION EXCEPTION** : Contrairement aux outils `read`/`write` qui exigent des chemins absolus, l'outil terminal (bash) ne supporte **PAS** les chemins absolus ni `cwd: "."`. Vous devez obligatoirement utiliser `cwd: ""` pour rester � la racine sous peine d'erreur de snapshot `path escapes workspace`.
- **CHANGELOG avant modification** : juste avant toute modification de `CHANGELOG.md`, toujours relire `C:\dev\sinew\CHANGELOG.md` avec l'outil `read`, puis modifier `CHANGELOG.md` dans le m�me lot que les autres fichiers touch�s.
- **Chemins de fichiers** : pour `read`, `edit_file` et `write_file`, utiliser les chemins absolus Windows du workspace, par exemple `C:\dev\sinew\...`.
- **Grep Limit** : le param�tre `limit` est strictement obligatoire pour les outils `grep` et `glob`. Ne jamais l'omettre.
- **Fichiers fant�mes** : avant d'appeler `grep` ou `read` sur un chemin sp�cifique, s'assurer que le fichier existe r�ellement sur le disque (ex: via `bash` avec `Test-Path`).
# Code map
- L'agent doit garder à jour cette carte simple des fichiers à chaque création, suppression, renommage, déplacement ou modification.

.
�S���� .gitignore
�S���� AGENTS.md
�S���� CHANGELOG.md
�S���� Cargo.lock
�S���� Cargo.toml
�S���� index.html
�S���� launch-sinew-dev.bat
�S���� LICENSE
�S���� package-lock.json
�S���� package.json
�S���� README.md
�S���� .sinew
�S���� scripts
�   �S���� check.ps1
�   �S���� compil.ps1
�   �S���� export-agent-descriptor.mjs
�   �S���� prepare-agent-bridge.mjs
�   �S���� prepare-sidecars.mjs
�   �S���� agent-bridge
�   �   �S���� exec-handlers.mjs
�   �   �S���� export-agent-fds-prost.mjs
�   �   �S���� h2-bridge.mjs
�   �   �S���� install-proto.mjs
�   �   �S���� package-lock.json
�   �   �S���� package.json
�   �   �S���� run-stream.mjs
�   �   �S���� test-live-rust.ps1
�   �   �S���� test-live.ps1
�   �   ����� vendor
�   �       ����� agent_pb.ts
�   ����� mitm
�       �S���� check-mitm.ps1
�       �S���� install-mitmproxy.ps1
�       �S���� README.md
�       ����� start-mitmweb.ps1
�S���� tsconfig.json
�S���� tsconfig.node.json
�S���� vite.config.ts
�S���� .github
�   �S���� assets
�   �   �S���� architecture.png
�   �   �S���� harness.png
�   �   �S���� hero.png
�   �   �S���� modes.png
�   �   �S���� screenshot.png
�   �   ����� swarm.png
�   ����� workflows
�       �S���� release.yml
�       ����� security.yml
�S���� crates
�   �S���� sinew-anthropic
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� auth.rs
�   �       �S���� client.rs
�   �       �S���� lib.rs
�   �       �S���� model_info.rs
�   �       �S���� stream.rs
�   �       ����� wire.rs
�   �S���� sinew-app
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� agent.rs
�   �       �S���� agent
�   �       �   �S���� assistant_message.rs
�   �       �   �S���� cancel.rs
�   �       �   �S���� clean_context.rs
�   �       �   �S���� compaction.rs
�   �       �   �S���� context.rs
�   �       �   �S���� events.rs
�   �       �   �S���� history.rs
�   �       �   �S���� mode.rs
�   �       �   �S���� tests.rs
�   �       �   �S���� tool_dispatch.rs
�   �       �   �S���� tool_summary.rs
�   �       �   ����� turn.rs
�   �       �S���� bash.rs
�   �       �S���� check_sota.rs
�   �       �S���� codebase_search.rs
�   �       �S���� compact.rs
�   �       �S���� computer_use.rs
�   �       �S���� delete_file.rs
�   �       �S���� edit.rs
�   �       �S���� editor_diagnostics.rs
�   �       �S���� glob.rs
�   �       �S���� grep.rs
�   �       �S���� image.rs
�   �       �S���� lib.rs
�   �       �S���� list_dir.rs
�   �       �S���� mcp.rs
�   �       �S���� question.rs
�   �       �S���� read.rs
�   �       �S���� read_lints.rs
�   �       �S���� ripgrep.rs
�   �       �S���� skill.rs
�   �       �S���� store.rs
�   �       �S���� subagent.rs
�   �       �S���� team.rs
�   �       �S���� team
�   �       �   �S���� agent_turns.rs
�   �       �   �S���� context.rs
�   �       �   �S���� descriptors.rs
�   �       �   �S���� launch.rs
�   �       �   �S���� live.rs
�   �       �   �S���� messaging.rs
�   �       �   �S���� model.rs
�   �       �   �S���� render.rs
�   �       �   �S���� session.rs
�   �       �   �S���� status_stop.rs
�   �       �   �S���� task_board.rs
�   �       �   ����� tests.rs
�   �       �S���� text.rs
�   �       �S���� todo.rs
�   �       �S���� tool_names.rs
�   �       �S���� tool_run.rs
�   �       �S���� web.rs
�   �       �S���� workspace.rs
�   �       ����� write.rs
�   �S���� sinew-core
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� error.rs
�   �       �S���� lib.rs
�   �       �S���� message.rs
�   �       �S���� model.rs
�   �       �S���� provider.rs
�   �       �S���� stream.rs
�   �       ����� tool.rs
�   �S���� sinew-deepseek
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� auth.rs
�   �       �S���� client.rs
�   �       �S���� lib.rs
�   �       �S���� model_info.rs
�   �       �S���� stream.rs
�   �       ����� wire.rs
�   �S���� sinew-google
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� auth.rs
�   �       �S���� client.rs
�   �       �S���� lib.rs
�   �       �S���� model_info.rs
�   �       �S���� stream.rs
�   �       ����� wire.rs
�   �S���� sinew-index
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� background.rs
�   �       �S���� chunk.rs
�   �       �S���� embeddings.rs
�   �       �S���� indexer.rs
�   �       �S���� lib.rs
�   �       �S���� process.rs
�   �       �S���� search.rs
�   �       ����� store.rs
�   �S���� sinew-kimi
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� auth.rs
�   �       �S���� client.rs
�   �       �S���� lib.rs
�   �       �S���� model_info.rs
�   �       �S���� stream.rs
�   �       ����� wire.rs
�   �S���� sinew-openai
�   �   �S���� Cargo.toml
�   �   ����� src
�   �       �S���� auth.rs
�   �       �S���� client.rs
�   �       �S���� lib.rs
�   �       �S���� model_info.rs
�   �       �S���� responses_stream.rs
�   �       �S���� stream.rs
�   �       �S���� websocket.rs
�   �       ����� wire.rs
�   ����� sinew-openrouter
�       �S���� Cargo.toml
�       ����� src
�           �S���� auth.rs
�           �S���� client.rs
�           �S���� lib.rs
�           �S���� model_info.rs
�           �S���� stream.rs
�           ����� wire.rs
�   ����� sinew-cursor
�   ����� sinew-ollama
�       �S���� Cargo.toml
�       ����� src
�           �S���� auth.rs
�           �S���� client.rs
�           �S���� lib.rs
�           �S���� model_info.rs
�           �S���� stream.rs
�           ����� wire.rs
�   ����� sinew-cursor
�       �S���� Cargo.toml
�       ����� src
�           �S���� agent
�           �   �S���� bridge.rs
�           �   �S���� client_proto.rs
�           �   �S���� connect_proto.rs
�           �   �S���� conversation_id.rs
�           �   �S���� exec_handler.rs
�           �   �S���� h2_client.rs
�           �   �S���� mod.rs
�           �   �S���� models.rs
�           �   �S���� proto_dynamic.rs
�           �   �S���� proto_pool.rs
�           �   �S���� retry.rs
�           �   �S���� run_h2.rs
�           �   �S���� run_request.rs
�           �   �S���� rust_bridge.rs
�           �   �S���� server_decode.rs
�           �   �S���� setup.rs
�           �   �S���� state.rs
�           �   �S���� tools.rs
�           �   �S���� transcript.rs
�           �   ����� transport.rs
�           �S���� auth
�           �   �S���� composer.rs
�           �   �S���� mod.rs
�           �   ����� oauth.rs
�           �S���� proto
�           �   �S���� agent.fds
�           �   �S���� agent.pb
�           �   ����� README.md
�           �S���� client.rs
�           �S���� connect.rs
�           �S���� context_injection.rs
�           �S���� conversation.rs
�           �S���� encryption.rs
�           �S���� identity.rs
�           �S���� images.rs
�           �S���� lib.rs
�           �S���� model_info.rs
�           �S���� sanitize.rs
�           �S���� stream_state.rs
�           �S���� tests.rs
�           �S���� tools.rs
�           �S���� usage.rs
�           ����� workspace.rs
�S���� src-tauri
�   �S���� Cargo.toml
�   �S���� PROVIDERS.md
�   �S���� binaries
�   �   ����� .gitkeep
�   �S���� build.rs
�   �S���� tauri.sidecars.conf.json
�   �S���� tauri.conf.json
�   �S���� tauri.windows.conf.json
�   �S���� capabilities
�   �   ����� default.json
�   �S���� gen
�   �   ����� schemas
�   �       �S���� acl-manifests.json
�   �       �S���� capabilities.json
�   �       �S���� desktop-schema.json
�   �       ����� macOS-schema.json
�   �S���� icons
�   �   �S���� 128x128.png
�   �   �S���� 128x128@2x.png
�   �   �S���� 32x32.png
�   �   �S���� 64x64.png
�   �   �S���� Square107x107Logo.png
�   �   �S���� Square142x142Logo.png
�   �   �S���� Square150x150Logo.png
�   �   �S���� Square284x284Logo.png
�   �   �S���� Square30x30Logo.png
�   �   �S���� Square310x310Logo.png
�   �   �S���� Square44x44Logo.png
�   �   �S���� Square71x71Logo.png
�   �   �S���� Square89x89Logo.png
�   �   �S���� StoreLogo.png
�   �   �S���� icon.icns
�   �   �S���� icon.ico
�   �   �S���� icon.png
�   �   �S���� nsis-sidebar.bmp
�   �   �S���� source.svg
�   �   �S���� android
�   �   �   �S���� mipmap-anydpi-v26
�   �   �   �   ����� ic_launcher.xml
�   �   �   �S���� mipmap-hdpi
�   �   �   �   �S���� ic_launcher.png
�   �   �   �   �S���� ic_launcher_foreground.png
�   �   �   �   ����� ic_launcher_round.png
�   �   �   �S���� mipmap-mdpi
�   �   �   �   �S���� ic_launcher.png
�   �   �   �   �S���� ic_launcher_foreground.png
�   �   �   �   ����� ic_launcher_round.png
�   �   �   �S���� mipmap-xhdpi
�   �   �   �   �S���� ic_launcher.png
�   �   �   �   �S���� ic_launcher_foreground.png
�   �   �   �   ����� ic_launcher_round.png
�   �   �   �S���� mipmap-xxhdpi
�   �   �   �   �S���� ic_launcher.png
�   �   �   �   �S���� ic_launcher_foreground.png
�   �   �   �   ����� ic_launcher_round.png
�   �   �   �S���� mipmap-xxxhdpi
�   �   �   �   �S���� ic_launcher.png
�   �   �   �   �S���� ic_launcher_foreground.png
�   �   �   �   ����� ic_launcher_round.png
�   �   �   ����� values
�   �   �       ����� ic_launcher_background.xml
�   �   ����� ios
�   �       �S���� AppIcon-20x20@1x.png
�   �       �S���� AppIcon-20x20@2x-1.png
�   �       �S���� AppIcon-20x20@2x.png
�   �       �S���� AppIcon-20x20@3x.png
�   �       �S���� AppIcon-29x29@1x.png
�   �       �S���� AppIcon-29x29@2x-1.png
�   �       �S���� AppIcon-29x29@2x.png
�   �       �S���� AppIcon-29x29@3x.png
�   �       �S���� AppIcon-40x40@1x.png
�   �       �S���� AppIcon-40x40@2x-1.png
�   �       �S���� AppIcon-40x40@2x.png
�   �       �S���� AppIcon-40x40@3x.png
�   �       �S���� AppIcon-512@2x.png
�   �       �S���� AppIcon-60x60@2x.png
�   �       �S���� AppIcon-60x60@3x.png
�   �       �S���� AppIcon-76x76@1x.png
�   �       �S���� AppIcon-76x76@2x.png
�   �       ����� AppIcon-83.5x83.5@2x.png
�   ����� src
�       �S���� context.rs
�       �S���� conversations.rs
�       �S���� git.rs
�       �S���� lib.rs
�       �S���� main.rs
�       �S���� models.rs
�       �S���� platform.rs
�       �S���� providers.rs
�       �S���� state.rs
�       �S���� swarm.rs
�       �S���� terminal.rs
�       �S���� tests.rs
�       �S���� turns.rs
�       �S���� updater.rs
�       �S���� workflow.rs
�       ����� workspace.rs
����� src
    �S���� App.tsx
    �S���� main.tsx
    �S���� styles.css
    �S���� types.ts
    �S���� vite-env.d.ts
    �S���� components
    �   �S���� ConversationList.tsx
    �   �S���� EditorPane.tsx
    �   �S���� FileTree.tsx
    �   �S���� GitPanel.tsx
    �   �S���� ImageContextMenu.tsx
    �   �S���� SearchPane.tsx
    �   �S���� SettingsPane.tsx
    �   �S���� SinewMark.tsx
    �   �S���� Splitter.tsx
    �   �S���� TerminalPanel.tsx
    �   �S���� UpdateBadge.tsx
    �   �S���� UpdaterLockScreen.tsx
    �   �S���� Welcome.tsx
    �   �S���� WindowControls.tsx
    �   �S���� Workspace.tsx
    �   ����� chat
    �       �S���� AIThinkingBlock.tsx
    �       �S���� ChatPane.tsx
    �       �S���� DotmSquare2.tsx
    �       �S���� DotmSquare5.tsx
    �       �S���� FileChangeBlock.tsx
    �       �S���� Markdown.tsx
    �       �S���� MermaidDiagram.tsx
    �       �S���� PlanningNextMoveBlock.tsx
    �       �S���� Questionnaire.tsx
    �       �S���� TodoStrip.tsx
    �       �S���� ToolCard.tsx
    �       �S���� dotmatrix-core.tsx
    �       �S���� dotmatrix-hooks.ts
    �       ����� stream.ts
    �S���� lib
    �   �S���� customIcons.ts
    �   �S���� fileIcon.ts
    �   �S���� frRuntime.ts
    �   �S���� ipc.ts
    �   �S���� language.ts
    �   �S���� locale.ts
    �   �S���� models.ts
    �   �S���� quotas.ts
    �   �S���� recents.ts
    �   ����� tools.ts
����� sinew-chrome-bridge
    �S���� add_to_sinew.py
    �S���� background.js
    �S���� com.sinew.chrome_bridge.json
    �S���� e2e-local.mjs
    �S���� e2e-structured.mjs
    �S���� icon-128.png
    �S���� icon-32.png
    �S���� icon-64.png
    �S���� icon.jpg
    �S���� interact_chrome.js
    �S���� launch_chrome_silent.bat
    �S���� manifest.json
    �S���� mcp_server.js
    �S���� native-host-wrapper.exe
    �S���� native_host.bat
    �S���� package-lock.json
    �S���� package.json
    �S���� popup.html
    �S���� popup.js
    �S���� register.ps1
    �S���� run_bridge.bat
    �S���� run_sinew_bridge.bat
    �S���� server.js
    �S���� sinew_cursor.js
    ����� native-host-wrapper
        �S���� Cargo.toml
        ����� src
            ����� main.rs

