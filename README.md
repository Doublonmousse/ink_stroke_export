# ink_stroke_export
Guide to converting/extracting ink stroke information from various windows-based note taking applications 

## Nebo

See the `interactive-ink-examples-uwp` subfolder/git repository. The format is proprietary and ink strokes are serialized into a binary file that's not easily readable. The workaround is to use the SDK and register a developper account to use the SDK to read `.nebo` files and export `jiix` files (see : [reference](https://developer.myscript.com/docs/interactive-ink/3.0/reference/jiix/)) that are easily readable.

Note that ink stroke information is not easily accessible when copying a selection to the clipboard.

To access nebo files, you can either select notebook or individual pages and export them one by one to a `.nebo` file.




## Inkodo

Database approach (sql files) + stroke data saved using the Ink Serialization Format

## Journal

Database approach (sql files)
