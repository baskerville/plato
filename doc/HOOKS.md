Hooks are defined in `Settings.toml`.

Here's an example hook, that launches the default article fetcher included in
Plato's release archive:
```toml
[[home.hooks]]
name = "Articles"
program = "bin/article_fetcher/article_fetcher"
sort-method = "added"
second-column = "progress"
```

The `name` key is the name of the category that will trigger the hook. The
`sort-method` and `second-column` keys are optional.

The *Hooks* sub-menu of the matches menu can be used to trigger a hook when the
corresponding category isn't in the summary bar. Otherwise, you can just tap
the category name in the summary bar. When the hook is triggered, the
associated `program` is spawned. It will receive the category name and online
status (*true* or *false*) as arguments.

A fetcher can send events to *Plato* through its standard output.
Each event is a JSON object with a required `type` key:

```
// Display a notification message.
{"type": "notify", "message": STRING}
// Add a document to the DB. `info` is the camel cased JSON version
// of the `Info` structure defined in `src/metadata.rs`.
{"type": "addDocument", "info": OBJECT}
// Remove a document from the DB. `path` is relative to `library-path`.
{"type": "removeDocument", "path": STRING}
// Enable or disable the WiFi.
{"type": "setWifi", "enable": BOOL}
```

On *Plato*'s side, the events are read line by line, one event per line.

When the network becomes operational, *Plato* will send the `SIGUSR1` signal to
all the fetchers.

When the associated category is deselected, *Plato* will send the `SIGTERM`
signal to the corresponding fetcher.
