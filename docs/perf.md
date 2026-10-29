# Latence des hooks

Claude Code lance `altium hook <event>` à chaque événement : démarrage de session, prompt, avant et après chaque outil, fin de réponse. L'objectif (spec claude-integration, « Hooks non bloquants ») est un p95 inférieur à 100 ms.

## Mesure

Le script lance 1 000 fois le binaire de release dans un projet temporaire, comme le fait Claude Code : un processus par appel, avec le payload JSON sur l'entrée standard. Le premier appel est un `SessionStart`, qui construit aussi le contexte injecté. Les 999 suivants sont des `PreToolUse`, tirés des fixtures capturées. Le temps mesuré va du lancement du processus à sa fin, création du processus comprise.

```python
for event in ["SessionStart"] + ["PreToolUse"] * 999:
    t = time.perf_counter()
    subprocess.run([exe, "hook", event], input=payload, cwd=project, env=env)
    times.append(time.perf_counter() - t)
```

## Résultats

| Système | Machine | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| Windows 11 Pro (26200), x86_64 | binaire release, disque local | 11,2 ms | 12,9 ms | 13,7 ms | 23,8 ms |
| Linux (Omarchy) | à mesurer | | | | |

Mesuré le 29 octobre 2026 avec Altium 0.1.0. Sous Windows, l'essentiel du temps est la création du processus. Le traitement lui-même (lecture du payload, recherche de `.altium/`, ajout d'une ligne au journal) prend moins d'une milliseconde.
