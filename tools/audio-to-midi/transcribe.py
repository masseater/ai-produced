import argparse
import os
import shutil
import sys
import tempfile
from collections import Counter
from pathlib import Path

YOURMT3_SPACE = "mimbres/YourMT3"
YOURMT3_REVISION = "5e66c1ea173a8186e0d20432b841d3180cc015b5"
YOURMT3_CHECKPOINT = "mc13_256_g4_all_v7_mt3f_sqr_rms_moe_wf4_n8k2_silu_rope_rp_b36_nops"
YOURMT3_ARGS = [
    f"{YOURMT3_CHECKPOINT}@last.ckpt",
    "-p", "2024",
    "-tk", "mc13_full_plus_256",
    "-dec", "multi-t5",
    "-nl", "26",
    "-enc", "perceiver-tf",
    "-sqr", "1",
    "-ff", "moe",
    "-wf", "4",
    "-nmoe", "8",
    "-kmoe", "2",
    "-act", "silu",
    "-epe", "rope",
    "-rp", "1",
    "-ac", "spec",
    "-hop", "300",
    "-atc", "1",
    "-pr", "32",
]


def basic_pitch(inputs, out_dir, args):
    from basic_pitch import ICASSP_2022_MODEL_PATH
    from basic_pitch.inference import Model, predict

    model = Model(ICASSP_2022_MODEL_PATH)
    for wav in inputs:
        _, midi, _ = predict(
            wav,
            model,
            onset_threshold=args.onset_threshold,
            frame_threshold=args.frame_threshold,
            minimum_note_length=args.minimum_note_length,
        )
        out = out_dir / f"{wav.stem}.mid"
        midi.write(str(out))
        print(out)


def yourmt3_source(cache):
    from huggingface_hub import snapshot_download

    return Path(
        snapshot_download(
            YOURMT3_SPACE,
            repo_type="space",
            revision=YOURMT3_REVISION,
            local_dir=cache,
            allow_patterns=[
                "model_helper.py",
                "amt/src/**",
                f"amt/logs/2024/{YOURMT3_CHECKPOINT}/checkpoints/last.ckpt",
            ],
        )
    )


def yourmt3(inputs, out_dir, args):
    src = yourmt3_source(args.cache / "yourmt3")
    sys.path[:0] = [str(src / "amt/src"), str(src)]
    cwd = Path.cwd()
    os.chdir(src)
    import torch
    import torchaudio
    from model_helper import (
        load_model_checkpoint,
        merge_zipped_note_events_and_ties_to_notes,
        mix_notes,
        slice_padded_array,
        write_model_output_as_midi,
    )

    device = "cuda" if torch.cuda.is_available() else "cpu"
    model = load_model_checkpoint(args=YOURMT3_ARGS, device="cpu").to(device)
    os.chdir(cwd)
    frames = model.audio_cfg["input_frames"]
    rate = model.audio_cfg["sample_rate"]
    for wav in inputs:
        audio, sr = torchaudio.load(str(wav))
        audio = torchaudio.functional.resample(audio.mean(dim=0, keepdim=True), sr, rate)
        segments = torch.from_numpy(slice_padded_array(audio, frames, frames).astype("float32"))
        segments = segments.to(device).unsqueeze(1)
        with torch.no_grad():
            tokens, _ = model.inference_file(bsz=args.batch_size, audio_segments=segments)
        starts = [frames * i / rate for i in range(segments.shape[0])]
        channels = []
        errors = Counter()
        for ch in range(model.task_manager.num_decoding_channels):
            events, _, _ = model.task_manager.detokenize_list_batches(
                [t[:, ch, :] for t in tokens], starts, return_events=True
            )
            notes, err = merge_zipped_note_events_and_ties_to_notes(events)
            channels.append(notes)
            errors += err
        with tempfile.TemporaryDirectory() as tmp:
            write_model_output_as_midi(mix_notes(channels), tmp, wav.stem, model.midi_output_inverse_vocab)
            out = out_dir / f"{wav.stem}.mid"
            shutil.move(Path(tmp) / "model_output" / f"{wav.stem}.mid", out)
        print(out)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="+", type=Path)
    parser.add_argument("-o", "--out-dir", type=Path, default=Path.cwd())
    parser.add_argument("-e", "--engine", choices=["basic-pitch", "yourmt3"], default="basic-pitch")
    parser.add_argument("--onset-threshold", type=float, default=0.5)
    parser.add_argument("--frame-threshold", type=float, default=0.3)
    parser.add_argument("--minimum-note-length", type=float, default=58)
    parser.add_argument("--batch-size", type=int, default=1)
    parser.add_argument("--cache", type=Path, default=Path.home() / ".cache/audio-to-midi")
    args = parser.parse_args()
    inputs = [p.resolve() for p in args.inputs]
    args.out_dir = args.out_dir.resolve()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    {"basic-pitch": basic_pitch, "yourmt3": yourmt3}[args.engine](inputs, args.out_dir, args)


if __name__ == "__main__":
    main()
