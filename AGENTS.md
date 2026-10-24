# mdview

<!-- [unknown] one-line project description - replace me -->

- README.md

<!-- mdview:START -->
## Documentation Viewing (MDView)

After writing a long markdown doc (tables, Mermaid, multi-file sets), or when
asked to preview one, run `mdview open --json <absolute-path-to-file.md>` and
give the user the `url` from the JSON it prints (all of `urls` when several).
Show it as a markdown link on its own line, `[<relative_path>](<url>)`, never in
backticks or a code block, with no trailing punctuation. Use `url`, not `long_url`.
<!-- mdview:END -->
