# ink_stroke_export
Guide to converting/extracting ink stroke information from various windows-based note taking applications 

## Nebo

See the `interactive-ink-examples-uwp` subfolder/git repository. The format is proprietary and ink strokes are serialized into a binary file that's not easily readable. The workaround is to use the SDK and register a developper account to use the SDK to read `.nebo` files and export `jiix` files (see : [reference](https://developer.myscript.com/docs/interactive-ink/3.0/reference/jiix/)) that are easily readable.

Note that ink stroke information is not easily accessible when copying a selection to the clipboard.

To access nebo files, you can either select notebook or individual pages and export them one by one to a `.nebo` file.

All files are also accessible at this location
```
C:\Users\USERNAME\AppData\Local\Packages\VisionObjects.MyScriptNebo_1rjv6qr7skr92\LocalState\notes\.noUser\
```

## Inkodo

Database approach (sql files) + stroke data saved using the Ink Serialized Format

## Journal

Database approach (sql files) for each note file (`.journal` file saved in the `Documents` folder) where everything is readily readable EXCEPT ink stroke data (binary blobs).
Pdfs are converted to images as well.

One interesting property though is that stroke information is available when copying a selection to the clipboard (there is an inkml file format that's avaiable then). But that only applies to strokes, the rest is not copied.

## OneNote

Same remark as Journal's on inkml for strokes being available when copying a selection. This time images are also available and encoded in base64 for png under an html filetype in the clipboard.
