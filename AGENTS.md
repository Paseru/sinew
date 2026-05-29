# Code map
- L'agent doit garder à jour cette carte simple des fichiers à chaque création, suppression, renommage, déplacement ou modification.

.
├── .gitignore
├── AGENTS.md
├── afaire.md
├── CHANGELOG.md
├── Cargo.lock
├── Cargo.toml
├── index.html
├── launch-sinew-dev.bat
├── LICENSE
├── package-lock.json
├── package.json
├── README.md
├── .sinew
│   └── skills
│       └── browser
│           └── SKILL.md
├── scripts
│   ├── compil.ps1
│   ├── export-agent-descriptor.mjs
│   ├── prepare-agent-bridge.mjs
│   ├── prepare-sidecars.mjs
│   ├── agent-bridge
│   │   ├── exec-handlers.mjs
│   │   ├── export-agent-fds-prost.mjs
│   │   ├── h2-bridge.mjs
│   │   ├── install-proto.mjs
│   │   ├── package-lock.json
│   │   ├── package.json
│   │   ├── run-stream.mjs
│   │   ├── test-live-rust.ps1
│   │   ├── test-live.ps1
│   │   └── vendor
│   │       └── agent_pb.ts
│   └── mitm
│       ├── check-mitm.ps1
│       ├── install-mitmproxy.ps1
│       ├── README.md
│       └── start-mitmweb.ps1
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts
├── .github
│   ├── assets
│   │   ├── architecture.png
│   │   ├── harness.png
│   │   ├── hero.png
│   │   ├── modes.png
│   │   ├── screenshot.png
│   │   └── swarm.png
│   └── workflows
│       ├── release.yml
│       └── security.yml
├── crates
│   ├── sinew-anthropic
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auth.rs
│   │       ├── client.rs
│   │       ├── lib.rs
│   │       ├── model_info.rs
│   │       ├── stream.rs
│   │       └── wire.rs
│   ├── sinew-app
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── agent.rs
│   │       ├── agent
│   │       │   ├── assistant_message.rs
│   │       │   ├── cancel.rs
│   │       │   ├── clean_context.rs
│   │       │   ├── compaction.rs
│   │       │   ├── context.rs
│   │       │   ├── events.rs
│   │       │   ├── history.rs
│   │       │   ├── mode.rs
│   │       │   ├── tests.rs
│   │       │   ├── tool_dispatch.rs
│   │       │   ├── tool_summary.rs
│   │       │   └── turn.rs
│   │       ├── bash.rs
│   │       ├── check_sota.rs
│   │       ├── codebase_search.rs
│   │       ├── compact.rs
│   │       ├── delete_file.rs
│   │       ├── edit.rs
│   │       ├── editor_diagnostics.rs
│   │       ├── glob.rs
│   │       ├── grep.rs
│   │       ├── image.rs
│   │       ├── lib.rs
│   │       ├── list_dir.rs
│   │       ├── mcp.rs
│   │       ├── question.rs
│   │       ├── read.rs
│   │       ├── read_lints.rs
│   │       ├── ripgrep.rs
│   │       ├── skill.rs
│   │       ├── store.rs
│   │       ├── subagent.rs
│   │       ├── team.rs
│   │       ├── team
│   │       │   ├── agent_turns.rs
│   │       │   ├── context.rs
│   │       │   ├── descriptors.rs
│   │       │   ├── launch.rs
│   │       │   ├── live.rs
│   │       │   ├── messaging.rs
│   │       │   ├── model.rs
│   │       │   ├── render.rs
│   │       │   ├── session.rs
│   │       │   ├── status_stop.rs
│   │       │   ├── task_board.rs
│   │       │   └── tests.rs
│   │       ├── text.rs
│   │       ├── todo.rs
│   │       ├── tool_names.rs
│   │       ├── tool_run.rs
│   │       ├── web.rs
│   │       ├── workspace.rs
│   │       └── write.rs
│   ├── sinew-core
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── message.rs
│   │       ├── model.rs
│   │       ├── provider.rs
│   │       ├── stream.rs
│   │       └── tool.rs
│   ├── sinew-deepseek
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auth.rs
│   │       ├── client.rs
│   │       ├── lib.rs
│   │       ├── model_info.rs
│   │       ├── stream.rs
│   │       └── wire.rs
│   ├── sinew-google
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auth.rs
│   │       ├── client.rs
│   │       ├── lib.rs
│   │       ├── model_info.rs
│   │       ├── stream.rs
│   │       └── wire.rs
│   ├── sinew-index
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── background.rs
│   │       ├── chunk.rs
│   │       ├── embeddings.rs
│   │       ├── indexer.rs
│   │       ├── lib.rs
│   │       ├── process.rs
│   │       ├── search.rs
│   │       └── store.rs
│   ├── sinew-kimi
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auth.rs
│   │       ├── client.rs
│   │       ├── lib.rs
│   │       ├── model_info.rs
│   │       ├── stream.rs
│   │       └── wire.rs
│   ├── sinew-openai
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── auth.rs
│   │       ├── client.rs
│   │       ├── lib.rs
│   │       ├── model_info.rs
│   │       ├── responses_stream.rs
│   │       ├── stream.rs
│   │       ├── websocket.rs
│   │       └── wire.rs
│   └── sinew-openrouter
│       ├── Cargo.toml
│       └── src
│           ├── auth.rs
│           ├── client.rs
│           ├── lib.rs
│           ├── model_info.rs
│           ├── stream.rs
│           └── wire.rs
│   └── sinew-cursor
│       ├── Cargo.toml
│       └── src
│           ├── agent
│           │   ├── bridge.rs
│           │   ├── client_proto.rs
│           │   ├── connect_proto.rs
│           │   ├── conversation_id.rs
│           │   ├── exec_handler.rs
│           │   ├── h2_client.rs
│           │   ├── mod.rs
│           │   ├── models.rs
│           │   ├── proto_dynamic.rs
│           │   ├── proto_pool.rs
│           │   ├── retry.rs
│           │   ├── run_h2.rs
│           │   ├── run_request.rs
│           │   ├── rust_bridge.rs
│           │   ├── server_decode.rs
│           │   ├── setup.rs
│           │   ├── state.rs
│           │   ├── tools.rs
│           │   ├── transcript.rs
│           │   └── transport.rs
│           ├── auth
│           │   ├── composer.rs
│           │   ├── mod.rs
│           │   └── oauth.rs
│           ├── proto
│           │   ├── agent.fds
│           │   ├── agent.pb
│           │   └── README.md
│           ├── client.rs
│           ├── connect.rs
│           ├── context_injection.rs
│           ├── conversation.rs
│           ├── encryption.rs
│           ├── identity.rs
│           ├── images.rs
│           ├── lib.rs
│           ├── model_info.rs
│           ├── sanitize.rs
│           ├── stream_state.rs
│           ├── tests.rs
│           ├── tools.rs
│           ├── usage.rs
│           └── workspace.rs
├── src-tauri
│   ├── Cargo.toml
│   ├── PROVIDERS.md
│   ├── binaries
│   │   └── .gitkeep
│   ├── build.rs
│   ├── tauri.sidecars.conf.json
│   ├── tauri.conf.json
│   ├── tauri.windows.conf.json
│   ├── capabilities
│   │   └── default.json
│   ├── gen
│   │   └── schemas
│   │       ├── acl-manifests.json
│   │       ├── capabilities.json
│   │       ├── desktop-schema.json
│   │       └── macOS-schema.json
│   ├── icons
│   │   ├── 128x128.png
│   │   ├── 128x128@2x.png
│   │   ├── 32x32.png
│   │   ├── 64x64.png
│   │   ├── Square107x107Logo.png
│   │   ├── Square142x142Logo.png
│   │   ├── Square150x150Logo.png
│   │   ├── Square284x284Logo.png
│   │   ├── Square30x30Logo.png
│   │   ├── Square310x310Logo.png
│   │   ├── Square44x44Logo.png
│   │   ├── Square71x71Logo.png
│   │   ├── Square89x89Logo.png
│   │   ├── StoreLogo.png
│   │   ├── icon.icns
│   │   ├── icon.ico
│   │   ├── icon.png
│   │   ├── nsis-sidebar.bmp
│   │   ├── source.svg
│   │   ├── android
│   │   │   ├── mipmap-anydpi-v26
│   │   │   │   └── ic_launcher.xml
│   │   │   ├── mipmap-hdpi
│   │   │   │   ├── ic_launcher.png
│   │   │   │   ├── ic_launcher_foreground.png
│   │   │   │   └── ic_launcher_round.png
│   │   │   ├── mipmap-mdpi
│   │   │   │   ├── ic_launcher.png
│   │   │   │   ├── ic_launcher_foreground.png
│   │   │   │   └── ic_launcher_round.png
│   │   │   ├── mipmap-xhdpi
│   │   │   │   ├── ic_launcher.png
│   │   │   │   ├── ic_launcher_foreground.png
│   │   │   │   └── ic_launcher_round.png
│   │   │   ├── mipmap-xxhdpi
│   │   │   │   ├── ic_launcher.png
│   │   │   │   ├── ic_launcher_foreground.png
│   │   │   │   └── ic_launcher_round.png
│   │   │   ├── mipmap-xxxhdpi
│   │   │   │   ├── ic_launcher.png
│   │   │   │   ├── ic_launcher_foreground.png
│   │   │   │   └── ic_launcher_round.png
│   │   │   └── values
│   │   │       └── ic_launcher_background.xml
│   │   └── ios
│   │       ├── AppIcon-20x20@1x.png
│   │       ├── AppIcon-20x20@2x-1.png
│   │       ├── AppIcon-20x20@2x.png
│   │       ├── AppIcon-20x20@3x.png
│   │       ├── AppIcon-29x29@1x.png
│   │       ├── AppIcon-29x29@2x-1.png
│   │       ├── AppIcon-29x29@2x.png
│   │       ├── AppIcon-29x29@3x.png
│   │       ├── AppIcon-40x40@1x.png
│   │       ├── AppIcon-40x40@2x-1.png
│   │       ├── AppIcon-40x40@2x.png
│   │       ├── AppIcon-40x40@3x.png
│   │       ├── AppIcon-512@2x.png
│   │       ├── AppIcon-60x60@2x.png
│   │       ├── AppIcon-60x60@3x.png
│   │       ├── AppIcon-76x76@1x.png
│   │       ├── AppIcon-76x76@2x.png
│   │       └── AppIcon-83.5x83.5@2x.png
│   └── src
│       ├── context.rs
│       ├── conversations.rs
│       ├── git.rs
│       ├── lib.rs
│       ├── main.rs
│       ├── models.rs
│       ├── platform.rs
│       ├── providers.rs
│       ├── state.rs
│       ├── swarm.rs
│       ├── terminal.rs
│       ├── tests.rs
│       ├── turns.rs
│       ├── updater.rs
│       ├── workflow.rs
│       └── workspace.rs
└── src
    ├── App.tsx
    ├── main.tsx
    ├── styles.css
    ├── types.ts
    ├── vite-env.d.ts
    ├── components
    │   ├── ConversationList.tsx
    │   ├── EditorPane.tsx
    │   ├── FileTree.tsx
    │   ├── GitPanel.tsx
    │   ├── ImageContextMenu.tsx
    │   ├── SearchPane.tsx
    │   ├── SettingsPane.tsx
    │   ├── SinewMark.tsx
    │   ├── Splitter.tsx
    │   ├── TerminalPanel.tsx
    │   ├── UpdateBadge.tsx
    │   ├── UpdaterLockScreen.tsx
    │   ├── Welcome.tsx
    │   ├── WindowControls.tsx
    │   ├── Workspace.tsx
    │   └── chat
    │       ├── AIThinkingBlock.tsx
    │       ├── ChatPane.tsx
    │       ├── DotmSquare2.tsx
    │       ├── DotmSquare5.tsx
    │       ├── FileChangeBlock.tsx
    │       ├── Markdown.tsx
    │       ├── MermaidDiagram.tsx
    │       ├── PlanningNextMoveBlock.tsx
    │       ├── Questionnaire.tsx
    │       ├── TodoStrip.tsx
    │       ├── ToolCard.tsx
    │       ├── dotmatrix-core.tsx
    │       ├── dotmatrix-hooks.ts
    │       └── stream.ts
    ├── lib
    │   ├── customIcons.ts
    │   ├── fileIcon.ts
    │   ├── frRuntime.ts
    │   ├── ipc.ts
    │   ├── language.ts
    │   ├── locale.ts
    │   ├── models.ts
    │   ├── quotas.ts
    │   ├── recents.ts
    │   └── tools.ts
└── sinew-chrome-bridge
    ├── add_to_sinew.py
    ├── background.js
    ├── com.sinew.chrome_bridge.json
    ├── e2e-local.mjs
    ├── e2e-structured.mjs
    ├── icon-128.png
    ├── icon-32.png
    ├── icon-64.png
    ├── icon.jpg
    ├── interact_chrome.js
    ├── launch_chrome_silent.bat
    ├── manifest.json
    ├── mcp_server.js
    ├── native-host-wrapper.exe
    ├── native_host.bat
    ├── package-lock.json
    ├── package.json
    ├── popup.html
    ├── popup.js
    ├── register.ps1
    ├── run_bridge.bat
    ├── run_sinew_bridge.bat
    ├── server.js
    ├── sinew_cursor.js
    └── native-host-wrapper
        ├── Cargo.toml
        └── src
            └── main.rs
