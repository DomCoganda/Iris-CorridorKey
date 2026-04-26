# Iris — CorridorKey

A native desktop frontend for [CorridorKey](https://github.com/nikopueringer/CorridorKey) by Corridor Digital.

CorridorKey is a neural network that physically unmixes green screen footage — separating foreground colour and linear alpha at the pixel level, preserving motion blur, translucency, and fine hair detail. It's a powerful tool, but it runs from the terminal and requires manual Python setup. Iris wraps it in a clean, native desktop application so you can focus on the work instead of the environment.

No terminal. No Python knowledge required. Download, open, key.

---

## What Iris Does

Iris handles everything CorridorKey needs behind the scenes:

- Automatically sets up a Python virtual environment and installs all dependencies
- Downloads the CorridorKey model on first launch
- Lets you download optional alpha hint generators (BiRefNet, GMV Auto, VideoMaMa, MatAnyone2) from inside the app
- Queues clips, runs inference, and streams live progress back to the UI
- Displays your keyed output directly in the viewer without leaving the app

---

## Features

- **Zero setup** — Iris manages Python, dependencies, and models for you
- **Clip queue** — add multiple shots and process them in sequence
- **Alpha model selector** — switch between hint generators per clip
- **Live controls** — threshold, despill, despeckle, and refiner sliders with real-time preview
- **Output selector** — choose which passes to write: Matte, FG, Processed composite, PNG preview
- **Frame viewer** — scrub through your keyed output without leaving the app
- **Parallel processing control** — tune how many frames process simultaneously

---

## Hardware Requirements

Iris itself is lightweight. The GPU requirements come from CorridorKey's inference engine:

- **CorridorKey** — approximately **22.7 GB VRAM** at native 2048×2048. A 24 GB card (RTX 3090, 4090, 5090, etc.) is the minimum.
- **GVM / VideoMaMa / MatAnyone2** — optional alpha hint generators with higher VRAM demands. See the [CorridorKey repo](https://github.com/nikopueringer/CorridorKey) for details.

NVIDIA GPUs with CUDA 12.6+ are recommended on Windows. CUDA, MPS (Apple Silicon), and CPU fallback are all supported.

---

## Getting Started

1. Download the latest release for your platform
2. Run Iris — it will walk you through first-time setup automatically
3. Add your green screen clips to the queue
4. Hit **Run**

That's it.

---

## Built With

Iris is written in Rust and built on the [Kairos](https://github.com/DomCoganda/kairos-framework) UI framework stack — a custom component library targeting privacy-focused Linux desktop apps. The CorridorKey Python backend runs in a managed subprocess; Iris communicates with it over stdout/stderr.

---

## Licensing

Iris is subject to the same license terms as CorridorKey — [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc/4.0/). You may use it freely including for commercial projects. You may not repackage or sell it, and the CorridorKey name must be retained in any forks or releases.

If you want to integrate CorridorKey or Iris into a commercial software product or paid inference service, reach out to Corridor Digital at [contact@corridordigital.com](mailto:contact@corridordigital.com).

---

## Acknowledgements

All the hard work is CorridorKey's. Iris is just a window into it.

- [CorridorKey](https://github.com/nikopueringer/CorridorKey) by [@nikopueringer](https://github.com/nikopueringer) and Corridor Digital
- [GVM](https://github.com/aim-uofa/GVM) — Advanced Intelligent Machines, Zhejiang University
- [VideoMaMa](https://github.com/cvlab-kaist/VideoMaMa) — CVLAB, KAIST