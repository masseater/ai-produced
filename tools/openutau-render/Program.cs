using System.Collections.Concurrent;
using System.Globalization;
using System.Text;
using OpenUtau.Classic;
using OpenUtau.Core;
using OpenUtau.Core.Editing;
using OpenUtau.Core.Format;
using OpenUtau.Core.Ustx;
using Serilog;

namespace OpenUtauRender;

public static class Program {
    const string Usage = """
        usage:
          openutau-render singers
          openutau-render render <project.ustx> --out <dir> [--singer <id>] [--pitch] [--save <project.ustx>] [--phonemes <file.tsv>]

        singers   installed singers: id, name, type, voice colors (clr index order), default phonemizer
        render    phonemize and render every voice track to <dir>/<project>_<track>.wav, like File > Export Wav
          --singer    set this singer id on every voice track before phonemizing
          --pitch     run "Load rendered pitch" on every voice part before rendering
          --save      save the project after the steps above
          --phonemes  write the phonemes of every note as TSV
        """;

    public static int Main(string[] args) {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        Log.Logger = new LoggerConfiguration()
            .MinimumLevel.Error()
            .WriteTo.Console(standardErrorFromLevel: Serilog.Events.LogEventLevel.Verbose)
            .CreateLogger();
        if (args.Length == 0 || args[0] is "-h" or "--help") {
            Console.WriteLine(Usage);
            return args.Length == 0 ? 2 : 0;
        }
        using var loop = new MainLoop();
        var errors = new ErrorCollector();
        loop.Run(() => {
            Directory.CreateDirectory(PathManager.Inst.CachePath);
            ToolsManager.Inst.Initialize();
            SingerManager.Inst.Initialize();
            DocManager.Inst.Initialize(loop.Thread, loop);
            DocManager.Inst.PostOnUIThread = action => loop.Run(action);
            DocManager.Inst.AddSubscriber(errors);
        }).Wait();
        return args[0] switch {
            "singers" => ListSingers(),
            "render" => Render(loop, errors, Options.Parse(args[1..])),
            _ => Fail(Usage),
        };
    }

    static int Fail(string message) {
        Console.Error.WriteLine(message);
        return 2;
    }

    static int ListSingers() {
        foreach (var singer in SingerManager.Inst.Singers.Values.OrderBy(s => s.Id, StringComparer.Ordinal)) {
            singer.EnsureLoaded();
            var colors = singer.Subbanks.Select(s => s.Color).ToHashSet().OrderBy(c => c);
            Console.WriteLine(string.Join('\t',
                singer.Id, singer.Name, singer.SingerType, string.Join(',', colors), singer.DefaultPhonemizer ?? ""));
        }
        return 0;
    }

    static int Render(MainLoop loop, ErrorCollector errors, Options options) {
        var input = Path.GetFullPath(options.Project);
        var project = loop.Run(() => {
            var loaded = Formats.ReadProject([input]) ?? throw new InvalidOperationException($"cannot read {input}");
            DocManager.Inst.ExecuteCmd(new LoadProjectNotification(loaded));
            return loaded;
        }).Result;
        var voiceTracks = project.parts.OfType<UVoicePart>().Select(p => p.trackNo).Distinct().ToArray();

        if (options.Singer != null) {
            var singer = SingerManager.Inst.GetSinger(options.Singer);
            if (singer == null) {
                return Fail($"singer not found: {options.Singer} (see `openutau-render singers`)");
            }
            loop.Run(() => {
                DocManager.Inst.StartUndoGroup("cli");
                foreach (var trackNo in voiceTracks) {
                    DocManager.Inst.ExecuteCmd(new TrackChangeSingerCommand(project, project.tracks[trackNo], singer));
                }
                DocManager.Inst.EndUndoGroup();
            }).Wait();
        }
        foreach (var trackNo in voiceTracks) {
            var track = project.tracks[trackNo];
            if (track.Singer == null || !track.Singer.Found) {
                return Fail($"track {track.TrackName}: singer not found ({track.Singer?.Id ?? "none"}); pass --singer");
            }
        }
        WaitIdle(loop, project);

        if (options.Pitch) {
            foreach (var part in project.parts.OfType<UVoicePart>()) {
                new LoadRenderedPitch().Run(project, part, [], DocManager.Inst);
            }
            WaitIdle(loop, project);
        }
        if (options.Save != null) {
            var save = Path.GetFullPath(options.Save);
            loop.Run(() => Ustx.Save(save, project)).Wait();
            Console.WriteLine($"saved {save}");
        }
        if (options.Phonemes != null) {
            loop.Run(() => WritePhonemes(project, options.Phonemes)).Wait();
            Console.WriteLine($"wrote {options.Phonemes}");
        }
        if (options.Out != null) {
            var exportPath = Path.Combine(Path.GetFullPath(options.Out), Path.GetFileNameWithoutExtension(input) + ".wav");
            PlaybackManager.Inst.RenderToFiles(project, exportPath).Wait();
            foreach (var trackNo in voiceTracks) {
                Console.WriteLine($"rendered {PathManager.Inst.GetExportPath(exportPath, project.tracks[trackNo])}");
            }
        }
        if (errors.Messages.Count > 0) {
            foreach (var message in errors.Messages) {
                Console.Error.WriteLine(message);
            }
            return 1;
        }
        return 0;
    }

    static void WaitIdle(MainLoop loop, UProject project) {
        var stable = 0;
        while (stable < 5) {
            Thread.Sleep(200);
            var ready = loop.Run(() => project.parts.OfType<UVoicePart>()
                .All(part => part.notes.Count == 0 || part.PhonemesUpToDate && part.renderPhrases.Count > 0)).Result;
            stable = ready ? stable + 1 : 0;
        }
    }

    static void WritePhonemes(UProject project, string path) {
        using var writer = new StreamWriter(path);
        writer.WriteLine("track\tnote_tick\tlyric\ttone\tphoneme\tphoneme_tick\tvoice_color");
        foreach (var part in project.parts.OfType<UVoicePart>()) {
            var track = project.tracks[part.trackNo];
            foreach (var phoneme in part.phonemes) {
                var note = phoneme.Parent;
                writer.WriteLine(string.Join('\t',
                    track.TrackName,
                    (part.position + note.position).ToString(CultureInfo.InvariantCulture),
                    note.lyric,
                    note.tone.ToString(CultureInfo.InvariantCulture),
                    phoneme.phoneme,
                    (part.position + phoneme.position).ToString(CultureInfo.InvariantCulture),
                    phoneme.GetVoiceColor(project, track) ?? ""));
            }
        }
    }
}

sealed record Options(string Project, string? Out, string? Singer, bool Pitch, string? Save, string? Phonemes) {
    public static Options Parse(string[] args) {
        string? project = null, output = null, singer = null, save = null, phonemes = null;
        var pitch = false;
        for (var i = 0; i < args.Length; i++) {
            switch (args[i]) {
                case "--out": output = args[++i]; break;
                case "--singer": singer = args[++i]; break;
                case "--save": save = args[++i]; break;
                case "--phonemes": phonemes = args[++i]; break;
                case "--pitch": pitch = true; break;
                default: project = args[i]; break;
            }
        }
        return new Options(project ?? throw new ArgumentException("project.ustx is required"), output, singer, pitch, save, phonemes);
    }
}

sealed class ErrorCollector : ICmdSubscriber {
    public ConcurrentQueue<string> Messages { get; } = new();

    public void OnNext(UCommand cmd, bool isUndo) {
        if (cmd is ErrorMessageNotification error) {
            Messages.Enqueue(error.e?.ToString() ?? error.message);
        }
    }
}

sealed class MainLoop : TaskScheduler, IDisposable {
    readonly BlockingCollection<Task> queue = [];

    public Thread Thread { get; }

    public MainLoop() {
        Thread = new Thread(() => {
            foreach (var task in queue.GetConsumingEnumerable()) {
                TryExecuteTask(task);
            }
        }) { IsBackground = true, Name = "main" };
        Thread.Start();
    }

    public Task Run(Action action) =>
        Task.Factory.StartNew(action, CancellationToken.None, TaskCreationOptions.None, this);

    public Task<T> Run<T>(Func<T> func) =>
        Task.Factory.StartNew(func, CancellationToken.None, TaskCreationOptions.None, this);

    protected override void QueueTask(Task task) => queue.Add(task);

    protected override bool TryExecuteTaskInline(Task task, bool taskWasPreviouslyQueued) =>
        Thread.CurrentThread == Thread && TryExecuteTask(task);

    protected override IEnumerable<Task> GetScheduledTasks() => queue.ToArray();

    public void Dispose() => queue.CompleteAdding();
}
