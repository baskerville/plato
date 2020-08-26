Hooks are defined in `Settings.toml`.

Here's an example hook, that launches the default article fetcher included in
*Plato*'s release archive:
```toml
[[libraries.hooks]]
path = "Articles"
program = "bin/article_fetcher/article_fetcher"
sort-method = "added"
second-column = "progress"
```

The above chunk needs to be added after one of the `[[libraries]]` section.

The `path` key is the path of the directory that will trigger the hook. The
`sort-method` and `second-column` keys are optional.

The *Toogle Select* sub-menu of the library menu can be used to trigger a hook when the
corresponding directory doesn't exit yet. Otherwise, you can just tap
the directory label in the navigation bar. When the hook is triggered, the
associated `program` is spawned. It will receive the directory path, wifi and
online statuses (*true* or *false*) as arguments.

A fetcher can send events to *Plato* through its standard output.
Each event is a JSON object with a required `type` key:

```
// Display a notification message.
{"type": "notify", "message": STRING}
// Add a document to the current library. `info` is the camel cased JSON version
// of the `Info` structure defined in `src/metadata.rs`.
{"type": "addDocument", "info": OBJECT}
// Enable or disable the WiFi.
{"type": "setWifi", "enable": BOOL}
// Import new entries and update existing entries in the current library.
{"type": "import"}
// Remove entries with dangling paths from the current library.
{"type": "cleanUp"}
```

On *Plato*'s side, the events are read line by line, one event per line.

When the network becomes operational, *Plato* will send the `SIGUSR1` signal to
all the fetchers.

When the associated directory is deselected, *Plato* will send the `SIGTERM`
signal to the corresponding fetcher.
