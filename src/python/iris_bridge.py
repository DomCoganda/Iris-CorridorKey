import sys
import os
import re
import json
import time
import threading
import argparse
import importlib
import pathlib
import subprocess
import traceback

import torch
import cv2
import numpy as np
import imageio_ffmpeg

from PIL import Image
from torchvision import transforms
from safetensors.torch import load_file

cv2.setNumThreads(2)

DEBUG = True


def log(message):
    if DEBUG:
        print(f"[iris_bridge debug] {message}", file=sys.stderr, flush=True)


def emit(kind, **kwargs):
    payload = {"kind": kind, **kwargs}
    print(json.dumps(payload), flush=True)
    log(f"emit: {payload}")


# ---------------------------------------------------------------------------
# Device detection
# ---------------------------------------------------------------------------

def _detect_device():
    if torch.cuda.is_available():
        log(f"device: CUDA — {torch.cuda.get_device_name(0)}")
        return torch.device("cuda"), True

    try:
        import torch_directml
        dml = torch_directml.device(torch_directml.default_device())
        _ = torch.zeros(1).to(dml)
        log(f"device: DirectML — {torch_directml.device_name(torch_directml.default_device())}")
        return dml, False
    except Exception as e:
        log(f"DirectML not available: {e}")

    if torch.backends.mps.is_available():
        log("device: MPS")
        return torch.device("mps"), False

    log("device: CPU")
    return torch.device("cpu"), False


def _device_str(device) -> str:
    s = str(device)
    if s.startswith("privateuseone"):
        return "cpu"
    return s


# ---------------------------------------------------------------------------
# GPU info
# ---------------------------------------------------------------------------

_VRAM_PER_WORKER_GREEN_GB  = 4.0
_VRAM_PER_WORKER_YELLOW_GB = 3.0


def _get_gpu_info():
    if torch.cuda.is_available():
        props   = torch.cuda.get_device_properties(0)
        vram_gb = props.total_memory / (1024 ** 3)
        name    = props.name
    else:
        vram_gb = 0.0
        name    = "No CUDA GPU"

    green_max  = max(1, int(vram_gb / _VRAM_PER_WORKER_GREEN_GB))
    yellow_max = max(green_max + 1, int(vram_gb / _VRAM_PER_WORKER_YELLOW_GB))

    return {"vram_gb": round(vram_gb, 1), "name": name,
            "green_max": green_max, "yellow_max": yellow_max}


def run_gpu_info(_args):
    log("run_gpu_info() start")
    info = _get_gpu_info()
    emit("gpu_info", vram_gb=info["vram_gb"], name=info["name"],
         green_max=info["green_max"], yellow_max=info["yellow_max"])
    emit("done")


# ---------------------------------------------------------------------------
# Frame extraction
# ---------------------------------------------------------------------------

def _get_total_frames(video_path, ffmpeg_exe):
    try:
        result = subprocess.run([ffmpeg_exe, "-i", video_path],
                                capture_output=True, timeout=10)
        output = result.stderr.decode("utf-8", errors="replace")
        fps = None
        m = re.search(r"(\d+(?:\.\d+)?)\s+fps", output)
        if m:
            fps = float(m.group(1))
        if fps is None:
            m = re.search(r"(\d+(?:\.\d+)?)\s+tbr", output)
            if m:
                fps = float(m.group(1))
        m = re.search(r"Duration:\s*(\d+):(\d+):(\d+)\.(\d+)", output)
        if m and fps and fps > 0:
            h, mn, s, frac = int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4))
            total_seconds = h * 3600 + mn * 60 + s + frac / 100.0
            return int(total_seconds * fps)
    except Exception as e:
        log(f"frame count estimation failed: {e}")
    return 0


def _build_ffmpeg_cmd(ffmpeg_exe, video_path, out_pattern):
    return [ffmpeg_exe, "-y", "-i", video_path, "-threads", "0", out_pattern]


def extract_frames(video_path, input_dir, emit_fn):
    log("extract_frames() start")
    os.makedirs(input_dir, exist_ok=True)
    ffmpeg_exe   = imageio_ffmpeg.get_ffmpeg_exe()
    total_frames = _get_total_frames(video_path, ffmpeg_exe)
    out_pattern  = os.path.join(input_dir, "frame_%06d.png")
    cmd          = _build_ffmpeg_cmd(ffmpeg_exe, video_path, out_pattern)
    process = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    last_current = -1
    last_growth_time = time.time()
    last_log_time = 0.0
    killed_for_stall = False
    while process.poll() is None:
        current = len([f for f in os.listdir(input_dir) if f.endswith(".png")])
        now = time.time()
        if current != last_current:
            last_growth_time = now
        if current != last_current or now - last_log_time > 2.0:
            log(f"ffmpeg running; frames_on_disk={current}; total={total_frames}")
            last_current = current
            last_log_time = now
        if current > 0 and now - last_growth_time > 30.0:
            log("ffmpeg stalled — terminating")
            killed_for_stall = True
            process.terminate()
            break
        if total_frames > 0:
            emit_fn("progress", current=current, total=total_frames,
                    message=f"Extracting frames... {current}/{total_frames}")
        else:
            emit_fn("progress", current=current, total=max(current + 30, 1),
                    message=f"Extracting frames... {current}")
        time.sleep(0.25)
    try:
        process.wait(timeout=10)
    except Exception:
        process.kill()
        process.wait()
    return_code = process.returncode
    frames = sorted(f for f in os.listdir(input_dir) if f.endswith(".png"))
    if return_code != 0 and not killed_for_stall and not frames:
        emit("error", message=f"FFmpeg failed (exit code {return_code})")
        return []
    if not frames:
        emit("error", message="No frames extracted.")
        return []
    emit("frame_count", count=len(frames))
    return frames


# ---------------------------------------------------------------------------
# BiRefNet
# ---------------------------------------------------------------------------

def load_birefnet(model_path):
    log("load_birefnet() start")
    model_dir = pathlib.Path(model_path)
    parent_dir = model_dir.parent
    init = model_dir / "__init__.py"
    if not init.exists():
        init.write_text("")
    parent_str = str(parent_dir)
    if parent_str not in sys.path:
        sys.path.insert(0, parent_str)
    mod = importlib.import_module("BiRefNet.birefnet")
    BiRefNet = getattr(mod, "BiRefNet")
    model = BiRefNet()
    weights = model_dir / "model.safetensors"
    state = load_file(str(weights), device="cpu")
    model.load_state_dict(state, strict=False)
    model.eval()
    device, is_cuda = _detect_device()
    model = model.to(device)
    if is_cuda:
        model = model.half()
    return model, device


def run_birefnet_frame(model, device, frame_path, hint_path):
    transform = transforms.Compose([
        transforms.Resize((1024, 1024)),
        transforms.ToTensor(),
        transforms.Normalize([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
    ])
    img = Image.open(frame_path).convert("RGB")
    original_size = img.size
    tensor = transform(img).unsqueeze(0).to(device)
    if str(device).startswith("cuda"):
        tensor = tensor.half()
    with torch.no_grad():
        preds = model(tensor)[-1].sigmoid()
    mask = preds[0].squeeze().cpu().numpy()
    mask = (mask * 255).clip(0, 255).astype("uint8")
    mask_img = Image.fromarray(mask).resize(original_size, Image.LANCZOS)
    os.makedirs(os.path.dirname(hint_path), exist_ok=True)
    mask_img.save(hint_path)


# ---------------------------------------------------------------------------
# CorridorKey shared helpers
# ---------------------------------------------------------------------------

def _setup_ck_module(args):
    """Import CorridorKeyEngine with the torch.load patch."""
    original_torch_load = torch.load
    def _patched_load(*a, **kwargs):
        kwargs["weights_only"] = False
        return original_torch_load(*a, **kwargs)
    torch.load = _patched_load

    for _key in list(sys.modules.keys()):
        if _key == "CorridorKeyModule" or _key.startswith("CorridorKeyModule."):
            del sys.modules[_key]

    _ck_pkg_dir   = os.path.join(args.ck_src, "CorridorKeyModule")
    _ck_core_dir  = os.path.join(_ck_pkg_dir, "core")
    _ck_init      = os.path.join(_ck_pkg_dir, "__init__.py")
    _ck_core_init = os.path.join(_ck_core_dir, "__init__.py")

    if not os.path.exists(_ck_init):
        with open(_ck_init, "w") as f:
            f.write("from .inference_engine import CorridorKeyEngine as CorridorKeyEngine\n")
    if os.path.isdir(_ck_core_dir) and not os.path.exists(_ck_core_init):
        open(_ck_core_init, "w").close()

    from CorridorKeyModule import CorridorKeyEngine

    torch.load = original_torch_load
    return CorridorKeyEngine


def _find_checkpoint(args):
    if args.ck_model and os.path.isfile(args.ck_model):
        return args.ck_model
    checkpoint_dir = os.path.join(args.ck_src, "CorridorKeyModule", "checkpoints")
    if os.path.isdir(checkpoint_dir):
        for ext in (".safetensors", ".pth"):
            for fname in os.listdir(checkpoint_dir):
                if fname.endswith(ext):
                    return os.path.join(checkpoint_dir, fname)
    raise FileNotFoundError(f"CorridorKey checkpoint not found. Tried: {args.ck_model}")


def _process_single_frame(engine, frame_name, input_dir, hint_gray_base,
                          despill, refiner, despeckle,
                          out_matte, out_fg, out_comp, out_proc):
    """Process one frame through CorridorKey and write all four outputs."""
    frame_path = os.path.join(input_dir, frame_name)
    img_bgr = cv2.imread(frame_path, cv2.IMREAD_UNCHANGED)
    if img_bgr is None:
        return False, f"Could not read frame: {frame_path}"

    img_rgb   = cv2.cvtColor(img_bgr, cv2.COLOR_BGR2RGB)
    hint_gray = hint_gray_base
    if hint_gray.shape[:2] != img_rgb.shape[:2]:
        hint_gray = cv2.resize(hint_gray, (img_rgb.shape[1], img_rgb.shape[0]),
                               interpolation=cv2.INTER_LINEAR)

    mask = hint_gray.astype(np.float32) / 255.0

    result = engine.process_frame(
        img_rgb, mask,
        input_is_linear=False,
        despill_strength=float(despill),
        auto_despeckle=True,
        despeckle_size=int(despeckle),
        refiner_scale=float(refiner),
    )

    stem = os.path.splitext(frame_name)[0]

    alpha_u8 = (result["alpha"].squeeze() * 255).clip(0, 255).astype(np.uint8)
    cv2.imwrite(os.path.join(out_matte, f"{stem}.png"), alpha_u8)

    fg_u8 = (result["fg"] * 255).clip(0, 255).astype(np.uint8)
    cv2.imwrite(os.path.join(out_fg, f"{stem}.png"), cv2.cvtColor(fg_u8, cv2.COLOR_RGB2BGR))

    comp_u8 = (result["comp"] * 255).clip(0, 255).astype(np.uint8)
    cv2.imwrite(os.path.join(out_comp, f"{stem}.png"), cv2.cvtColor(comp_u8, cv2.COLOR_RGB2BGR))

    proc_u8 = (result["processed"].clip(0, 1) * 255).astype(np.uint8)
    cv2.imwrite(os.path.join(out_proc, f"{stem}.png"), cv2.cvtColor(proc_u8, cv2.COLOR_RGBA2BGRA))

    return True, None


# ---------------------------------------------------------------------------
# CorridorKey — parallel workers with resume (full inference)
# ---------------------------------------------------------------------------

def _process_chunk(engine, chunk, input_dir, hint_gray_base, args,
                   out_matte, out_fg, out_comp, out_proc,
                   progress_lock, progress_counts, worker_idx, errors, error_lock):
    for fname in chunk:
        ok, err = _process_single_frame(
            engine, fname, input_dir, hint_gray_base,
            args.despill, args.refiner, args.despeckle,
            out_matte, out_fg, out_comp, out_proc,
        )
        if not ok:
            with error_lock:
                errors.append(err)
            return
        with progress_lock:
            progress_counts[worker_idx] += 1


def _run_corridorkey(args, frames, skip_resume=False):
    log("_run_corridorkey() start")

    CorridorKeyEngine = _setup_ck_module(args)

    raw_device, is_cuda = _detect_device()
    device    = _device_str(raw_device)
    precision = torch.float16 if is_cuda else torch.float32

    work      = args.work
    input_dir = os.path.join(work, "Input")
    hint_dir  = os.path.join(work, "AlphaHint")
    out_matte = os.path.join(work, "Output", "Matte")
    out_fg    = os.path.join(work, "Output", "FG")
    out_comp  = os.path.join(work, "Output", "Comp")
    out_proc  = os.path.join(work, "Output", "Processed")
    for d in [out_matte, out_fg, out_comp, out_proc]:
        os.makedirs(d, exist_ok=True)

    total_frames = len(frames)

    if skip_resume:
        remaining    = frames
        already_done = set()
    else:
        already_done = set()
        if os.path.isdir(out_matte):
            for f in os.listdir(out_matte):
                if f.endswith(".png"):
                    already_done.add(os.path.splitext(f)[0])
        remaining = [f for f in frames if os.path.splitext(f)[0] not in already_done]
        if not remaining:
            emit("status", message="All frames already processed.")
            emit("progress", current=total_frames, total=total_frames,
                 message=f"Inference {total_frames}/{total_frames}")
            emit("done")
            return
        if already_done:
            emit("status", message=f"Resuming — {len(already_done)}/{total_frames} already done...")

    frames = remaining
    checkpoint = _find_checkpoint(args)
    n_workers  = max(1, args.workers)
    info = _get_gpu_info()
    if info["vram_gb"] > 0:
        n_workers = min(n_workers, info["yellow_max"])

    chunk_size     = (len(frames) + n_workers - 1) // n_workers
    chunks         = [frames[i:i + chunk_size] for i in range(0, len(frames), chunk_size)]
    actual_workers = len(chunks)

    engines = []
    for i in range(actual_workers):
        emit("status", message=f"Loading CorridorKey worker {i + 1}/{actual_workers} on {device}...")
        engines.append(CorridorKeyEngine(
            checkpoint_path=checkpoint, device=device,
            img_size=2048, model_precision=precision,
        ))

    hint_candidates = [
        os.path.join(hint_dir, "alpha_hint.png"),
        os.path.join(hint_dir, frames[0]),
    ]
    hint_path = next((p for p in hint_candidates if os.path.isfile(p)), None)
    if hint_path is None:
        emit("error", message="Missing alpha hint.")
        return
    hint_gray_base = cv2.imread(hint_path, cv2.IMREAD_GRAYSCALE)
    if hint_gray_base is None:
        emit("error", message=f"Could not read alpha hint: {hint_path}")
        return

    emit("status", message=f"Running inference — {actual_workers} workers...")

    progress_counts = [0] * actual_workers
    progress_lock   = threading.Lock()
    errors          = []
    error_lock      = threading.Lock()

    threads = []
    for i, (chunk, engine) in enumerate(zip(chunks, engines)):
        t = threading.Thread(
            target=_process_chunk,
            args=(engine, chunk, input_dir, hint_gray_base, args,
                  out_matte, out_fg, out_comp, out_proc,
                  progress_lock, progress_counts, i, errors, error_lock),
            daemon=True,
        )
        threads.append(t)

    for t in threads:
        t.start()

    while any(t.is_alive() for t in threads):
        with progress_lock:
            new_done = sum(progress_counts)
        total_done = len(already_done) + new_done
        emit("progress", current=total_done, total=total_frames,
             message=f"Inference {total_done}/{total_frames}  ({actual_workers} workers)")
        time.sleep(0.5)

    for t in threads:
        t.join()

    if errors:
        emit("error", message=errors[0])
        return

    emit("status", message="Inference complete")
    emit("done")


# ---------------------------------------------------------------------------
# Actions
# ---------------------------------------------------------------------------

def run_extract(args):
    input_dir = os.path.join(args.work, "Input")
    os.makedirs(input_dir, exist_ok=True)
    emit("status", message="Extracting frames...")
    frames = extract_frames(args.input, input_dir, emit)
    if not frames:
        return
    emit("status", message=f"Extracted {len(frames)} frames")
    emit("done")


def run_alpha(args):
    input_dir = os.path.join(args.work, "Input")
    hint_dir  = os.path.join(args.work, "AlphaHint")
    os.makedirs(input_dir, exist_ok=True)
    os.makedirs(hint_dir, exist_ok=True)
    emit("status", message="Extracting frames...")
    frames = extract_frames(args.input, input_dir, emit)
    if not frames:
        return
    emit("status", message=f"Extracted {len(frames)} frames")
    emit("status", message="Loading alpha model...")
    model, device = load_birefnet(args.alpha_model)
    emit("status", message="Alpha model loaded")
    hint_path = os.path.join(hint_dir, "alpha_hint.png")
    emit("status", message=f"Generating alpha hint from {frames[0]}...")
    run_birefnet_frame(model, device, os.path.join(input_dir, frames[0]), hint_path)
    emit("progress", current=1, total=1, message="Alpha hint 1/1")
    del model
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
    emit("status", message="Alpha hint ready — starting inference...")
    _run_corridorkey(args, frames)


def run_hint(args):
    input_dir = os.path.join(args.work, "Input")
    hint_dir  = os.path.join(args.work, "AlphaHint")
    os.makedirs(hint_dir, exist_ok=True)
    frame_name = f"frame_{args.frame + 1:06d}.png"
    frame_path = os.path.join(input_dir, frame_name)
    if not os.path.exists(frame_path):
        emit("error", message=f"Frame not found: {frame_path}")
        return
    hint_path = os.path.join(hint_dir, "alpha_hint.png")
    emit("status", message="Loading alpha model...")
    model, device = load_birefnet(args.alpha_model)
    emit("status", message=f"Generating hint for {frame_name}...")
    run_birefnet_frame(model, device, frame_path, hint_path)
    emit("hint_ready", path=hint_path)


def run_infer(args):
    input_dir = os.path.join(args.work, "Input")
    frames = sorted(f for f in os.listdir(input_dir) if f.endswith(".png"))
    if not frames:
        emit("error", message="No frames found in Input/")
        return
    _run_corridorkey(args, frames, skip_resume=False)


def run_preview(args):
    """Single-frame preview — always re-processes the frame."""
    log("run_preview() start")
    input_dir  = os.path.join(args.work, "Input")
    frame_name = f"frame_{args.frame + 1:06d}.png"
    if not os.path.exists(os.path.join(input_dir, frame_name)):
        emit("error", message=f"Preview frame not found: {frame_name}")
        return
    emit("status", message=f"Generating preview for frame {args.frame + 1}...")
    args.workers = 1
    _run_corridorkey(args, [frame_name], skip_resume=True)


def run_preview_server(args):
    """
    Persistent preview server — loads CorridorKey ONCE then processes
    frame requests from stdin in a loop.

    Protocol (newline-delimited JSON on stdin):
      {"frame": N, "despill": X, "refiner": Y, "despeckle": Z}

    Responds with standard bridge events on stdout.
    Stays alive until stdin is closed (Rust side drops the handle).
    """
    log("run_preview_server() start")

    work      = args.work
    input_dir = os.path.join(work, "Input")
    hint_dir  = os.path.join(work, "AlphaHint")
    out_matte = os.path.join(work, "Output", "Matte")
    out_fg    = os.path.join(work, "Output", "FG")
    out_comp  = os.path.join(work, "Output", "Comp")
    out_proc  = os.path.join(work, "Output", "Processed")
    for d in [out_matte, out_fg, out_comp, out_proc]:
        os.makedirs(d, exist_ok=True)

    hint_path = os.path.join(hint_dir, "alpha_hint.png")
    if not os.path.isfile(hint_path):
        emit("error", message="No alpha hint found — run hint generation first")
        return

    hint_gray_base = cv2.imread(hint_path, cv2.IMREAD_GRAYSCALE)
    if hint_gray_base is None:
        emit("error", message=f"Could not read alpha hint: {hint_path}")
        return

    CorridorKeyEngine = _setup_ck_module(args)
    raw_device, is_cuda = _detect_device()
    device    = _device_str(raw_device)
    precision = torch.float16 if is_cuda else torch.float32
    checkpoint = _find_checkpoint(args)

    emit("status", message=f"Loading preview server on {device}...")
    engine = CorridorKeyEngine(
        checkpoint_path=checkpoint,
        device=device,
        img_size=2048,
        model_precision=precision,
    )
    log("preview server: engine loaded")
    emit("server_ready", message=f"Preview server ready on {device}")

    for raw_line in sys.stdin:
        raw_line = raw_line.strip()
        if not raw_line:
            continue

        try:
            cmd = json.loads(raw_line)
        except json.JSONDecodeError as e:
            log(f"preview server: bad command JSON: {e}")
            continue

        frame     = cmd.get("frame", 0)
        despill   = cmd.get("despill", 0.5)
        refiner   = cmd.get("refiner", 1.0)
        despeckle = cmd.get("despeckle", 400)

        frame_name = f"frame_{frame + 1:06d}.png"
        log(f"preview server: processing {frame_name}  despill={despill} refiner={refiner} despeckle={despeckle}")

        ok, err = _process_single_frame(
            engine, frame_name, input_dir, hint_gray_base,
            despill, refiner, despeckle,
            out_matte, out_fg, out_comp, out_proc,
        )

        if ok:
            emit("done")
        else:
            emit("error", message=err or "Unknown error")

    log("run_preview_server() stdin closed — exiting")


def run_export(args):
    """
    Convert completed PNG frame sequences into mp4 video files.
    Saves to {work}/Export/comp.mp4, matte.mp4, fg.mp4, processed.mp4.
    """
    log("run_export() start")

    work       = args.work
    ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
    fps        = args.fps if args.fps > 0 else 24.0

    export_dir = os.path.join(work, "Export")
    os.makedirs(export_dir, exist_ok=True)

    sequences = [
        ("Output/Comp",      "comp.mp4"),
        ("Output/Matte",     "matte.mp4"),
        ("Output/FG",        "fg.mp4"),
        ("Output/Processed", "processed.mp4"),
    ]

    to_export = [
        (os.path.join(work, subdir), name)
        for subdir, name in sequences
        if os.path.isdir(os.path.join(work, subdir))
           and any(f.endswith(".png") for f in os.listdir(os.path.join(work, subdir)))
    ]

    if not to_export:
        emit("error", message="No completed output frames found to export.")
        return

    total = len(to_export)
    emit("status", message=f"Exporting {total} sequence{'s' if total > 1 else ''}...")

    for i, (src_dir, out_name) in enumerate(to_export):
        out_path = os.path.join(export_dir, out_name)
        emit("status", message=f"Encoding {out_name}...")

        cmd = [
            ffmpeg_exe, "-y",
            "-framerate", str(fps),
            "-i", os.path.join(src_dir, "frame_%06d.png"),
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-crf", "18",
            "-preset", "fast",
            out_path,
        ]

        log(f"ffmpeg export cmd: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True)

        if result.returncode != 0:
            err = result.stderr.decode("utf-8", errors="replace")
            log(f"ffmpeg export error: {err}")
            emit("error", message=f"Export failed for {out_name}: {err[-300:]}")
            return

        emit("progress", current=i + 1, total=total,
             message=f"Exported {out_name} ({i + 1}/{total})")

    emit("status", message="Export complete — saved to Export/")
    emit("done")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main():
    log("main() start")

    parser = argparse.ArgumentParser()
    parser.add_argument("--action", required=True,
                        choices=["extract", "alpha", "hint", "infer",
                                 "preview", "preview_server", "export", "gpu_info"])
    parser.add_argument("--input",        default="")
    parser.add_argument("--work",         default="")
    parser.add_argument("--ck-src",       default="")
    parser.add_argument("--ck-model",     default="")
    parser.add_argument("--alpha-model",  default="")
    parser.add_argument("--frame",        type=int,   default=0)
    parser.add_argument("--despill",      type=float, default=0.5)
    parser.add_argument("--refiner",      type=float, default=1.0)
    parser.add_argument("--despeckle",    type=int,   default=400)
    parser.add_argument("--workers",      type=int,   default=2)
    parser.add_argument("--fps",          type=float, default=24.0)

    args = parser.parse_args()
    log(f"parsed args={args}")

    if args.ck_src:
        sys.path.insert(0, args.ck_src)

    try:
        if args.action == "extract":
            run_extract(args)
        elif args.action == "alpha":
            run_alpha(args)
        elif args.action == "hint":
            run_hint(args)
        elif args.action == "infer":
            run_infer(args)
        elif args.action == "preview":
            run_preview(args)
        elif args.action == "preview_server":
            run_preview_server(args)
        elif args.action == "export":
            run_export(args)
        elif args.action == "gpu_info":
            run_gpu_info(args)
        log("main() complete")
    except Exception:
        tb = traceback.format_exc()
        log(f"UNHANDLED EXCEPTION:\n{tb}")
        emit("error", message=tb)


if __name__ == "__main__":
    main()